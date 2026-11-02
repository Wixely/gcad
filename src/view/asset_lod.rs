//! In-memory LOD comparison. Work is bounded by the same reducer options as CLI.
use crate::asset::{author::Asset, lod, scene::Scene, skin::Skin};
use std::sync::mpsc::{self, Receiver, TryRecvError};

#[derive(Clone, PartialEq)]
struct Settings {
    node: String,
    primitive: usize,
    target: usize,
    error: f64,
    uv_error: f64,
    angle: f64,
    trials: usize,
    samples: usize,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            node: String::new(),
            primitive: 0,
            target: 500,
            error: 1.,
            uv_error: 0.01,
            angle: 10.,
            trials: 5000,
            samples: 5,
        }
    }
}

fn generate(
    scene: Scene,
    skins: Vec<Skin>,
    settings: Settings,
) -> anyhow::Result<(Asset, lod::SceneReport)> {
    let node = scene
        .nodes
        .iter()
        .position(|n| n.name == settings.node)
        .ok_or_else(|| anyhow::anyhow!("LOD node no longer exists"))?;
    let (scene, skins, report) = lod::derive_node(
        &scene,
        &skins,
        node,
        settings.primitive,
        &[],
        settings.samples,
        &lod::Options {
            target_triangles: settings.target,
            max_error_mm: settings.error,
            max_uv_error: settings.uv_error,
            max_angle_degrees: settings.angle,
            max_trials: settings.trials,
        },
    )?;
    Ok((Asset::preview_scene(scene, skins)?, report))
}

#[derive(Default)]
pub(super) struct Comparison {
    settings: Settings,
    result: Option<(Asset, lod::SceneReport)>,
    job: Option<Receiver<Result<(Asset, lod::SceneReport), String>>>,
    show_lod: bool,
    error: Option<String>,
}

impl Comparison {
    pub fn invalidate(&mut self) {
        // Dropping the receiver prevents a result from an old document revision
        // from being adopted. The bounded reducer finishes off the UI thread.
        self.job = None;
        self.result = None;
        self.show_lod = false;
        self.error = None;
    }

    pub fn displayed<'a>(&'a self, source: &'a Asset) -> &'a Asset {
        if self.show_lod {
            self.result.as_ref().map_or(source, |(asset, _)| asset)
        } else {
            source
        }
    }

    /// Returns true when the rendered asset must be rebuilt at the same pose.
    pub fn controls(&mut self, ui: &mut egui::Ui, source: &Asset) -> bool {
        let mut changed = false;
        if let Some(job) = &self.job {
            match job.try_recv() {
                Ok(result) => {
                    self.job = None;
                    match result {
                        Ok(result) => {
                            self.result = Some(result);
                            self.show_lod = true;
                            changed = true;
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    self.job = None;
                    self.error = Some("LOD worker stopped without a result".into());
                }
                Err(TryRecvError::Empty) => {
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(100));
                }
            }
        }
        ui.collapsing("LOD comparison", |ui| {
            let old = self.settings.clone();
            let old_show = self.show_lod;
            ui.add_enabled_ui(self.job.is_none(), |ui| {
                if !source.scene.nodes.iter().any(|n| n.mesh.is_some() && n.name == self.settings.node) {
                    self.settings.node = source.scene.nodes.iter().find(|n| n.mesh.is_some()).map_or(String::new(), |n| n.name.clone());
                }
                egui::ComboBox::from_id_salt("lod-node").selected_text(&self.settings.node).show_ui(ui, |ui| {
                    for node in source.scene.nodes.iter().filter(|n| n.mesh.is_some()) {
                        ui.selectable_value(&mut self.settings.node, node.name.clone(), &node.name);
                    }
                });
                let count = source.scene.nodes.iter().find(|n| n.name == self.settings.node).and_then(|n| n.mesh).map_or(0, |mesh| source.scene.meshes[mesh].primitives.len());
                self.settings.primitive = self.settings.primitive.min(count.saturating_sub(1));
                ui.add(egui::Slider::new(&mut self.settings.primitive, 0..=count.saturating_sub(1)).text("Primitive"));
                ui.horizontal(|ui| { ui.label("Target triangles"); ui.add(egui::DragValue::new(&mut self.settings.target).range(1..=usize::MAX)); });
                ui.horizontal(|ui| { ui.label("Position error (mm)"); ui.add(egui::DragValue::new(&mut self.settings.error).range(0.0..=f64::MAX).speed(0.1)); });
                ui.horizontal(|ui| { ui.label("UV error"); ui.add(egui::DragValue::new(&mut self.settings.uv_error).range(0.0..=f64::MAX).speed(0.001)); });
                ui.horizontal(|ui| { ui.label("Angle limit (degrees)"); ui.add(egui::DragValue::new(&mut self.settings.angle).range(0.0..=180.)); });
                ui.horizontal(|ui| { ui.label("Pose samples per clip"); ui.add(egui::DragValue::new(&mut self.settings.samples).range(2..=10001)); });
                ui.horizontal(|ui| { ui.label("Maximum trials"); ui.add(egui::DragValue::new(&mut self.settings.trials).range(1..=usize::MAX)); });
            });
            if old != self.settings {
                self.invalidate();
                changed |= old_show;
            }
            if ui.add_enabled(self.job.is_none(), egui::Button::new("Generate comparison LOD")).clicked() {
                let (send, receive) = mpsc::channel();
                let scene = source.scene.clone();
                let skins = source.skins.clone();
                let settings = self.settings.clone();
                let ctx = ui.ctx().clone();
                self.job = Some(receive);
                self.error = None;
                std::thread::spawn(move || {
                    let result = generate(scene, skins, settings).map_err(|e| format!("{e:#}"));
                    let _ = send.send(result);
                    ctx.request_repaint();
                });
            }
            if self.job.is_some() { ui.label("Generating LOD in background..."); }
            if let Some((_, report)) = &self.result {
                ui.horizontal(|ui| {
                    changed |= ui.selectable_value(&mut self.show_lod, false, "Source").changed();
                    changed |= ui.selectable_value(&mut self.show_lod, true, "Generated LOD").changed();
                });
                ui.label(format!("Triangles: {} → {} · target reached: {}", report.reduction.original_triangles, report.reduction.triangles, report.reduction.target_reached));
                ui.label(format!("{} sampled poses · correspondence error {:.4} mm", report.sampled_poses, report.reduction.max_correspondence_error_mm));
            }
            if let Some(error) = &self.error { ui.colored_label(egui::Color32::LIGHT_RED, error); }
            ui.label("Comparison stays in memory. Reload or parameter changes discard it. No extra vertex locks or surface/silhouette acceptance limits are requested here; use asset-lod for export and those controls.");
        });
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lod_comparison_preserves_source_and_discards_stale_results() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let asset =
            crate::asset::author::load(&root.join("examples/game-assets/plate.gasset"), &[])
                .unwrap();
        let before = asset.encode_glb().unwrap();
        let settings = Settings {
            node: asset.scene.nodes[0].name.clone(),
            trials: 8,
            ..Default::default()
        };
        let (derived, report) =
            generate(asset.scene.clone(), asset.skins.clone(), settings).unwrap();
        assert!(report.reduction.triangles <= report.reduction.original_triangles);
        assert_eq!(before, asset.encode_glb().unwrap());
        super::super::frame(&derived, None, 0.).unwrap();
        let mut comparison = Comparison {
            result: Some((derived, report)),
            show_lod: true,
            ..Default::default()
        };
        assert!(!std::ptr::eq(comparison.displayed(&asset), &asset));
        let (send, receive) = mpsc::channel();
        comparison.job = Some(receive);
        comparison.invalidate();
        assert!(send.send(Err("old document".to_owned())).is_err());
        assert!(std::ptr::eq(comparison.displayed(&asset), &asset));
    }
}
