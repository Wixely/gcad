//! Desktop export controls, isolated from the CAD document UI.
use anyhow::{Result, ensure};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

pub(super) struct Export {
    source: PathBuf,
    vars: Vec<(String, f64)>,
    output: String,
    helper: String,
    fbx: bool,
    pending: Option<mpsc::Receiver<Result<PathBuf>>>,
    cancelled: Arc<AtomicBool>,
    status: String,
}

impl Export {
    pub fn new(source: PathBuf, vars: Vec<(String, f64)>) -> Self {
        Self {
            output: source.with_extension("glb").display().to_string(),
            source,
            vars,
            helper: std::env::var("GCAD_FBX_HELPER").unwrap_or_default(),
            fbx: false,
            pending: None,
            cancelled: Arc::new(AtomicBool::new(false)),
            status: String::new(),
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, ready: bool) {
        if let Some(receiver) = &self.pending {
            match receiver.try_recv() {
                Ok(result) => {
                    self.status = match result {
                        Ok(path) => format!("Exported {}", path.display()),
                        Err(error) => format!("Export failed: {error:#}"),
                    };
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.status = "Export worker stopped unexpectedly.".into();
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ui.ctx().request_repaint_after(Duration::from_millis(100));
                }
            }
        }
        ui.collapsing("Export game asset", |ui| {
            ui.label("Exports saved source, including materials and clips. Preview pose and visibility do not change the export.");
            ui.add_enabled_ui(self.pending.is_none(), |ui| {
                ui.horizontal(|ui| {
                    let before = self.fbx;
                    ui.selectable_value(&mut self.fbx, false, "GLB");
                    ui.selectable_value(&mut self.fbx, true, "FBX + PNG (experimental)");
                    if before != self.fbx { self.output = self.source.with_extension(if self.fbx { "fbx-package" } else { "glb" }).display().to_string(); }
                });
                ui.label(if self.fbx { "New package directory" } else { "New .glb file" });
                ui.text_edit_singleline(&mut self.output);
                if self.fbx {
                    ui.label("Native FBX helper (absolute executable path)");
                    ui.text_edit_singleline(&mut self.helper);
                }
                if ui.add_enabled(ready, egui::Button::new("Export saved asset")).clicked() {
                    let source = self.source.clone();
                    let vars = self.vars.clone();
                    let output = PathBuf::from(self.output.trim());
                    let helper = self.fbx.then(|| PathBuf::from(self.helper.trim()));
                    self.cancelled = Arc::new(AtomicBool::new(false));
                    let cancelled = self.cancelled.clone();
                    let (send, receive) = mpsc::channel();
                    let ctx = ui.ctx().clone();
                    self.pending = Some(receive);
                    self.status = "Exporting saved asset…".into();
                    std::thread::spawn(move || {
                        let result = run(&source, &vars, &output, helper.as_deref(), &cancelled);
                        let _ = send.send(result);
                        ctx.request_repaint();
                    });
                }
            });
            if !self.status.is_empty() { ui.label(&self.status); }
        });
    }
}

impl Drop for Export {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

fn write_new(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(error.into());
    }
    Ok(())
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(
    source: &std::path::Path,
    vars: &[(String, f64)],
    output: &std::path::Path,
    helper: Option<&std::path::Path>,
    cancelled: &AtomicBool,
) -> Result<PathBuf> {
    ensure!(!output.as_os_str().is_empty(), "Choose an output path");
    ensure!(
        !output.exists(),
        "Preserving existing output: {}",
        output.display()
    );
    if let Some(helper) = helper {
        ensure!(
            helper.is_absolute() && helper.is_file(),
            "Choose an existing native helper by absolute path"
        );
    } else {
        ensure!(
            output
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("glb")),
            "GLB output must end in .glb"
        );
    }
    let (asset, reads) = crate::host::recording(|| crate::asset::author::load(source, vars));
    let bytes = asset?.encode_glb()?;
    ensure!(
        reads.unchanged(),
        "Source changed during export; try again after saving"
    );
    ensure!(!cancelled.load(Ordering::Relaxed), "Export cancelled");
    if let Some(helper) = helper {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("gcad-view-export-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&path)?;
        let scratch = Scratch(path);
        let input = scratch.0.join("asset.glb");
        write_new(&input, &bytes)?;
        crate::asset::fbx_helper::convert_experimental(
            helper,
            &input,
            output,
            Duration::from_secs(120),
            cancelled,
        )
    } else {
        write_new(output, &bytes)?;
        Ok(output.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_panel_runs_background_export() {
        use egui_kittest::kittest::Queryable as _;
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/game-assets/plate.gasset");
        let output = std::env::temp_dir().join(format!("gcad-panel-{}.glb", std::process::id()));
        let mut export = Export::new(source, vec![]);
        export.output = output.display().to_string();
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(900., 400.))
            .build_ui_state(|ui, state: &mut Export| state.ui(ui, true), export);
        harness.get_by_label("Export game asset").click();
        harness.run_steps(4);
        harness.get_by_label("Export saved asset").click();
        harness.run_steps(2);
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while harness.state().pending.is_some() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(25));
            harness.step();
        }
        assert!(
            harness.state().status.starts_with("Exported "),
            "{}",
            harness.state().status
        );
        assert_eq!(&std::fs::read(&output).unwrap()[..4], b"glTF");
        std::fs::remove_file(output).unwrap();
    }
    #[test]
    #[ignore = "requires GCAD_FBX_HELPER pointing to the reviewed native helper"]
    fn viewer_fbx_export_creates_package() {
        let helper = PathBuf::from(std::env::var("GCAD_FBX_HELPER").unwrap());
        let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("viewer-mvp-fbx-{}", std::process::id()));
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/game-assets/plate.gasset");
        run(
            &source,
            &[],
            &output,
            Some(&helper),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(std::fs::read_dir(&output).unwrap().any(|entry| {
            entry
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "fbx")
        }));
        assert!(
            run(
                &source,
                &[],
                &output,
                Some(&helper),
                &AtomicBool::new(false)
            )
            .is_err()
        );
    }
    #[test]
    fn viewer_export_preserves_existing_output_and_exports_saved_source() {
        let path =
            std::env::temp_dir().join(format!("gcad-view-export-test-{}.glb", std::process::id()));
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/game-assets/plate.gasset");
        let cancelled = AtomicBool::new(false);
        run(&source, &[], &path, None, &cancelled).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..4], b"glTF");
        assert!(run(&source, &[], &path, None, &cancelled).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_file(path).unwrap();
    }
}
