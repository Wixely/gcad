//! Background texture variants; source pixels never change.
use super::*;
use crate::asset::texel_density::{Report, Settings};
use std::sync::mpsc;

#[derive(Default)]
pub(super) struct Preview {
    requested: Option<Settings>,
    result: Option<Asset>,
    pending: Option<mpsc::Receiver<Result<(Asset, Report)>>>,
    status: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_matches_export_transform_and_reverts_to_source() {
        let source = crate::asset::author::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/game-assets/plate-density.gasset"), &[]).unwrap();
        let settings=Settings {pixels_per_metre:64.,max_dimension:64};
        let expected=crate::asset::texel_density::normalize(&source.scene,settings).unwrap().0;
        let mut harness=egui_kittest::Harness::builder().build_ui_state(
            |ui,state: &mut (Preview,Option<Settings>)| {state.0.update(ui,&source,state.1);},
            (Preview::default(),Some(settings)));
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(30);
        while harness.state().0.result.is_none() {
            assert!(std::time::Instant::now()<deadline,"{}",harness.state().0.status);
            std::thread::sleep(std::time::Duration::from_millis(10)); harness.step();
        }
        assert_eq!(harness.state().0.displayed(&source).scene.materials,expected.materials);
        harness.state_mut().1=None; harness.step();
        assert!(harness.state().0.result.is_none());
        assert!(std::ptr::eq(harness.state().0.displayed(&source),&source));
    }
}
impl Preview {
    pub fn invalidate(&mut self) {
        self.requested = None;
        self.result = None;
        self.pending = None;
        self.status.clear();
    }
    pub fn displayed<'a>(&'a self, source: &'a Asset) -> &'a Asset {
        self.result.as_ref().unwrap_or(source)
    }
    pub fn update(
        &mut self,
        ui: &mut egui::Ui,
        source: &Asset,
        settings: Option<Settings>,
    ) -> bool {
        let mut changed = false;
        if self.requested != settings && self.pending.is_none() {
            self.invalidate();
            self.requested = settings;
            changed = true;
            if let Some(settings) = settings {
                let scene = source.scene.clone();
                let skins = source.skins.clone();
                let (tx, rx) = mpsc::channel();
                self.pending = Some(rx);
                self.status = "Preparing normalized textures; showing source textures…".into();
                let ctx = ui.ctx().clone();
                std::thread::spawn(move || {
                    let result = crate::asset::texel_density::normalize(&scene, settings).and_then(
                        |(scene, report)| Ok((Asset::preview_scene(scene, skins)?, report)),
                    );
                    let _ = tx.send(result);
                    ctx.request_repaint();
                });
            }
        }
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok(Ok((asset, report))) => {
                    if self.requested != settings {
                        self.pending = None;
                        self.result = None;
                        ui.ctx().request_repaint();
                        return true;
                    }
                    let target = self.requested.unwrap().pixels_per_metre;
                    let below = report
                        .maps
                        .iter()
                        .filter(|m| m.achieved_pixels_per_metre < target * 0.95)
                        .count();
                    self.status = format!(
                        "Normalized preview: {} map uses; {below} below target due to source resolution/dimension limits. Area-average density; UVs unchanged.",
                        report.maps.len()
                    );
                    self.result = Some(asset);
                    self.pending = None;
                    changed = true;
                }
                Ok(Err(e)) => {
                    self.status =
                        format!("Normalized preview unavailable: {e:#}. Showing source textures.");
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.status = "Texture worker stopped; showing source textures.".into();
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => ui
                    .ctx()
                    .request_repaint_after(std::time::Duration::from_millis(100)),
            }
        }
        if settings.is_some() {
            ui.label(&self.status);
        }
        changed
    }
}
