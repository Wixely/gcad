//! Visibility controls for explicitly authored consumer roles in the asset UI.
use super::Frame;
use crate::asset::author::Asset;
use std::{collections::HashSet, sync::Arc};

#[derive(Clone, Copy, PartialEq)]
pub(super) struct ConsumerView {
    pub level: Option<usize>,
    pub collision: bool,
    pub boxes: bool,
}
impl Default for ConsumerView {
    fn default() -> Self {
        Self {
            level: Some(0),
            collision: false,
            boxes: false,
        }
    }
}
impl ConsumerView {
    pub fn controls(&mut self, ui: &mut egui::Ui, asset: &Asset) {
        let Some(profile) = &asset.scene.consumer else {
            return;
        };
        ui.collapsing("Consumer preview", |ui| {
            ui.label(format!("{} profile v{}", profile.engine, profile.version));
            if !profile.colliders.is_empty() {
                ui.checkbox(&mut self.collision, "Show collision source meshes");
                if profile.colliders.iter().any(|c| c.shape == crate::asset::consumer::Shape::Box) {
                    ui.checkbox(&mut self.boxes, "Box volumes (X-ray)");
                }
                if self.boxes {
                    ui.label("Local box bounds follow the pose; engine shear handling may differ.");
                }
                if profile.colliders.iter().any(|c| c.shape == crate::asset::consumer::Shape::Convex) {
                    ui.label("Convex source meshes are not cooked hulls. Validate the hull in the engine.");
                }
            }
            let count = profile
                .lods
                .iter()
                .map(|g| g.levels.len())
                .max()
                .unwrap_or(0);
            if count > 0 {
                egui::ComboBox::from_label("Preview LOD")
                    .selected_text(self.level.map_or("All LODs".into(), |i| format!("LOD {i}")))
                    .show_ui(ui, |ui| {
                        for i in 0..count {
                            ui.selectable_value(&mut self.level, Some(i), format!("LOD {i}"));
                        }
                        ui.selectable_value(&mut self.level, None, "All LODs");
                    });
                ui.label("Manual level selection; engine distance switching is separate.");
            }
            for socket in &profile.sockets {
                ui.label(format!("Socket {} → {}", socket.name, socket.node));
            }
        });
    }
    pub fn apply(self, asset: &Asset, mut frame: Frame) -> Frame {
        let Some(profile) = &asset.scene.consumer else {
            return frame;
        };
        let mut hidden = HashSet::new();
        if !self.collision {
            hidden.extend(profile.colliders.iter().map(|c| c.node.as_str()));
        }
        if let Some(level) = self.level {
            for group in &profile.lods {
                let selected = level.min(group.levels.len() - 1);
                for (i, entry) in group.levels.iter().enumerate() {
                    if i != selected {
                        hidden.extend(entry.nodes.iter().map(String::as_str));
                    }
                }
            }
        }
        let mut visible = Vec::new();
        for node in &asset.scene.nodes {
            if let Some(mesh) = node.mesh {
                for _ in &asset.scene.meshes[mesh].primitives {
                    visible.push(!hidden.contains(node.name.as_str()));
                }
            }
        }
        let scene = Arc::get_mut(&mut frame.scene)
            .expect("consumer visibility applies to a newly built frame");
        let mut index = 0;
        scene.surfaces.retain(|_| {
            let keep = visible[index];
            index += 1;
            keep
        });
        let mut low = [f32::INFINITY; 3];
        let mut high = [f32::NEG_INFINITY; 3];
        for surface in &scene.surfaces {
            for point in &surface.positions {
                for k in 0..3 {
                    low[k] = low[k].min(point[k]);
                    high[k] = high[k].max(point[k]);
                }
            }
        }
        if self.boxes {
            for volume in &frame.collision_boxes {
                for point in &volume.corners {
                    for k in 0..3 { low[k] = low[k].min(point[k]); high[k] = high[k].max(point[k]); }
                }
            }
        }
        if scene.surfaces.is_empty() && (!self.boxes || frame.collision_boxes.is_empty()) {
            scene.centre = [0.; 3];
            scene.radius = 1.;
        } else {
            scene.centre = std::array::from_fn(|k| low[k] * 0.5 + high[k] * 0.5);
            scene.radius = ((high[0] - low[0])
                .hypot(high[1] - low[1])
                .hypot(high[2] - low[2])
                * 0.5)
                .max(1e-6);
        }
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn consumer_visibility_selects_lods_and_collision_without_editing_asset() {
        let asset = crate::asset::author::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples/game-assets/plate-consumer-profile.gasset"),
            &[],
        )
        .unwrap();
        let source = asset.encode_glb().unwrap();
        let frame = super::super::frame(&asset, None, 0.).unwrap();
        assert_eq!(frame.scene.surfaces.len(), 3);
        let high = ConsumerView::default().apply(&asset, frame);
        assert_eq!(high.scene.surfaces.len(), 1);
        let low = ConsumerView {
            level: Some(1),
            collision: false,
            boxes: false,
        }
        .apply(&asset, super::super::frame(&asset, None, 0.).unwrap());
        assert_eq!(low.scene.surfaces.len(), 1);
        assert!(low.scene.surfaces[0].positions.len() < high.scene.surfaces[0].positions.len());
        let all = ConsumerView {
            level: None,
            collision: true,
            boxes: false,
        }
        .apply(&asset, super::super::frame(&asset, None, 0.).unwrap());
        assert_eq!(all.scene.surfaces.len(), 3);
        assert_eq!(asset.encode_glb().unwrap(), source);
    }
}
