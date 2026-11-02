//! Translation-only root motion, using the exporter's exact extraction contract.
use crate::asset::{
    animation::{Interpolation, Values},
    author::Asset,
    root_motion,
};

struct Trajectory {
    motion: root_motion::Motion,
    origin: [f64; 3],
}

fn trajectory(asset: &Asset, clip: usize, node: usize) -> anyhow::Result<Trajectory> {
    let mut name = "__preview_in_place".to_owned();
    while asset.scene.clips.iter().any(|clip| clip.name == name) {
        name.push('_');
    }
    let extracted = root_motion::extract(&asset.scene, clip, node, [true; 3], &name)?;
    let origin = asset.scene.clips[clip].sample(&asset.scene, 0., false)?[node].translation;
    Ok(Trajectory {
        motion: extracted.motion,
        origin,
    })
}

#[derive(Default)]
pub(super) struct MotionPreview {
    selected: Option<String>,
    key: Option<(usize, String)>,
    cached: Option<Result<Trajectory, String>>,
}

impl MotionPreview {
    pub fn invalidate(&mut self) {
        self.key = None;
        self.cached = None;
    }

    pub fn controls(&mut self, ui: &mut egui::Ui, asset: &Asset, clip: Option<usize>, time: f64) {
        let Some(clip) = clip else {
            self.invalidate();
            return;
        };
        let eligible: Vec<_> = asset.scene.clips[clip]
            .tracks
            .iter()
            .filter(|track| {
                asset.scene.nodes[track.node].parent.is_none()
                    && matches!(track.values, Values::Translation(_))
            })
            .map(|track| track.node)
            .collect();
        if self
            .selected
            .as_ref()
            .is_some_and(|name| !eligible.iter().any(|&i| asset.scene.nodes[i].name == *name))
        {
            self.selected = None;
        }
        if eligible.is_empty() {
            self.invalidate();
            return;
        }
        egui::ComboBox::from_id_salt("root-motion-preview")
            .selected_text(
                self.selected
                    .as_deref()
                    .map_or("Root motion: off".to_owned(), |name| {
                        format!("Root motion: {name}")
                    }),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.selected, None, "Root motion: off");
                for &node in &eligible {
                    let name = &asset.scene.nodes[node].name;
                    ui.selectable_value(&mut self.selected, Some(name.clone()), name);
                }
            });
        let key = self.selected.clone().map(|name| (clip, name));
        if self.key != key {
            self.cached = key.as_ref().map(|(_, name)| {
                let node = eligible
                    .iter()
                    .copied()
                    .find(|&i| asset.scene.nodes[i].name == *name)
                    .unwrap();
                trajectory(asset, clip, node).map_err(|error| format!("{error:#}"))
            });
            self.key = key;
        }
        match &self.cached {
            Some(Ok(path)) => {
                ui.label("Root translation · XYZ millimetres · X-ray · one clip cycle; rotation is not extracted.");
                if path.motion.interpolation == Interpolation::Step {
                    ui.label("STEP motion: markers are jumps, not a continuous path.");
                }
                if let Ok(displacement) = path.motion.sample(time, false) {
                    ui.label(format!(
                        "Displacement from first key (mm): {:.3?}",
                        displacement
                    ));
                }
            }
            Some(Err(error)) => {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
            None => {}
        }
    }

    pub fn paint(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        projector: &egui_bench::viewer3d::Projector,
        time: f64,
    ) {
        let Some(Ok(path)) = &self.cached else {
            return;
        };
        let project = |delta: [f64; 3]| {
            let point = std::array::from_fn(|k| path.origin[k] + delta[k]);
            super::v3(point)
                .ok()
                .and_then(|p| projector.project(p))
                .filter(|p| p.x.is_finite() && p.y.is_finite())
        };
        let painter = ui.painter_at(rect);
        let points: Vec<_> = path
            .motion
            .displacements_mm
            .iter()
            .copied()
            .map(project)
            .collect();
        if path.motion.interpolation == Interpolation::Linear {
            for pair in points.windows(2) {
                if let [Some(a), Some(b)] = pair {
                    painter
                        .line_segment([*a, *b], egui::Stroke::new(2., egui::Color32::LIGHT_GREEN));
                }
            }
        }
        for point in points.iter().flatten() {
            painter.circle_filled(*point, 3., egui::Color32::LIGHT_GREEN);
        }
        if let Ok(delta) = path.motion.sample(time, false) {
            if let Some(point) = project(delta) {
                painter.circle_stroke(point, 7., egui::Stroke::new(2., egui::Color32::WHITE));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trajectory_matches_source_root_and_preserves_step_discontinuities() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut asset =
            crate::asset::author::load(&root.join("examples/game-assets/plate.gasset"), &[])
                .unwrap();
        asset.scene.clips.push(crate::asset::animation::Clip {
            name: "travel".into(),
            tracks: vec![crate::asset::animation::Track {
                node: 0,
                times: vec![0., 1., 2.],
                interpolation: Interpolation::Linear,
                values: Values::Translation(vec![
                    [10., 20., 30.],
                    [20., 40., 50.],
                    [30., 50., 60.],
                ]),
            }],
        });
        let path = trajectory(&asset, 0, 0).unwrap();
        assert_eq!(path.origin, [10., 20., 30.]);
        assert_eq!(path.motion.sample(0.5, false).unwrap(), [5., 10., 10.]);
        let delta = path.motion.sample(1.5, false).unwrap();
        let source = asset.scene.clips[0]
            .sample(&asset.scene, 1.5, false)
            .unwrap()[0]
            .translation;
        assert_eq!(
            std::array::from_fn::<_, 3, _>(|k| path.origin[k] + delta[k]),
            source
        );
        asset.scene.clips[0].tracks[0].interpolation = Interpolation::Step;
        let path = trajectory(&asset, 0, 0).unwrap();
        assert_eq!(path.motion.interpolation, Interpolation::Step);
        assert_eq!(path.motion.sample(0.999, false).unwrap(), [0.; 3]);
        assert_eq!(path.motion.sample(1., false).unwrap(), [10., 20., 20.]);
    }
}
