//! Exact translation extraction from a top-level node. Rotational root motion
//! needs a separately specified decomposition and is not inferred here.
use super::animation::{Clip, Interpolation, Values};
use super::scene::Scene;
use anyhow::{Result, ensure};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Motion {
    pub schema: u32,
    pub source_clip: String,
    pub node: String,
    /// X,Y,Z selection in the scene's CAD millimetre/Z-up coordinate frame.
    pub axes: [bool; 3],
    pub duration_seconds: f64,
    pub interpolation: Interpolation,
    pub times: Vec<f64>,
    pub displacements_mm: Vec<[f64; 3]>,
}
pub struct Extracted {
    pub in_place: Clip,
    pub motion: Motion,
}
impl Motion {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.schema == 1, "unsupported root-motion schema");
        ensure!(
            !self.source_clip.trim().is_empty() && !self.node.trim().is_empty(),
            "root motion requires clip and node names"
        );
        ensure!(
            self.axes.iter().any(|v| *v),
            "root motion requires at least one translation axis"
        );
        ensure!(
            self.duration_seconds.is_finite() && self.duration_seconds > 0.,
            "root motion requires positive duration"
        );
        ensure!(
            !self.times.is_empty()
                && self.times.len() == self.displacements_mm.len()
                && self
                    .times
                    .iter()
                    .all(|v| v.is_finite() && *v >= 0. && *v <= self.duration_seconds)
                && self.times.windows(2).all(|t| t[1] > t[0]),
            "invalid root-motion key times"
        );
        ensure!(
            self.displacements_mm.iter().all(|p| p
                .iter()
                .enumerate()
                .all(|(k, v)| v.is_finite() && (self.axes[k] || *v == 0.))),
            "invalid root-motion displacement"
        );
        ensure!(
            self.displacements_mm[0] == [0.; 3],
            "root motion must start at zero displacement"
        );
        Ok(())
    }
    pub fn to_json(&self) -> Result<String> {
        self.validate()?;
        Ok(serde_json::to_string_pretty(self)?)
    }
    pub fn from_json(json: &str) -> Result<Self> {
        let motion: Self = serde_json::from_str(json)?;
        motion.validate()?;
        Ok(motion)
    }
    /// Absolute displacement, accumulated over whole cycles when looping. Add to
    /// the extracted node's in-place translation exactly once (not per frame).
    /// Negative time traverses cycles backwards. No engine unit conversion occurs.
    pub fn sample(&self, time: f64, looping: bool) -> Result<[f64; 3]> {
        self.validate()?;
        ensure!(time.is_finite(), "root-motion time must be finite");
        let cycles = if looping {
            (time / self.duration_seconds).floor()
        } else {
            0.
        };
        let phase = if looping {
            time.rem_euclid(self.duration_seconds)
        } else {
            time
        };
        let upper = self.times.partition_point(|t| *t <= phase);
        let a = upper.saturating_sub(1);
        let b = upper.min(self.times.len() - 1);
        let factor = if a == b || self.interpolation == Interpolation::Step {
            0.
        } else {
            ((phase - self.times[a]) / (self.times[b] - self.times[a])).clamp(0., 1.)
        };
        let last = self.displacements_mm.last().unwrap();
        let value = std::array::from_fn(|k| {
            self.displacements_mm[a][k] * (1. - factor)
                + self.displacements_mm[b][k] * factor
                + cycles * last[k]
        });
        ensure!(
            value.iter().all(|v| v.is_finite()),
            "root-motion displacement overflow"
        );
        Ok(value)
    }
}

/// Preserve the source clip; extract selected translations relative to its first
/// key. Other axes/properties/tracks remain unchanged. STEP and LINEAR extraction
/// are exact between keys, without resampling. The caller chooses the new name.
pub fn extract(
    scene: &Scene,
    clip: usize,
    node: usize,
    axes: [bool; 3],
    name: &str,
) -> Result<Extracted> {
    scene.validate()?;
    let source = scene
        .clips
        .get(clip)
        .ok_or_else(|| anyhow::anyhow!("root-motion clip not found"))?;
    let root = scene
        .nodes
        .get(node)
        .ok_or_else(|| anyhow::anyhow!("root-motion node not found"))?;
    ensure!(
        root.parent.is_none(),
        "root-motion extraction requires a top-level node"
    );
    ensure!(
        !name.trim().is_empty() && scene.clips.iter().all(|c| c.name != name),
        "in-place clip name is empty or already exists"
    );
    let index = source
        .tracks
        .iter()
        .position(|t| t.node == node && matches!(t.values, Values::Translation(_)))
        .ok_or_else(|| anyhow::anyhow!("root-motion node has no translation track in this clip"))?;
    let track = &source.tracks[index];
    let Values::Translation(values) = &track.values else {
        unreachable!()
    };
    let first = values[0];
    let motion = Motion {
        schema: 1,
        source_clip: source.name.clone(),
        node: root.name.clone(),
        axes,
        duration_seconds: source.duration(),
        interpolation: track.interpolation,
        times: track.times.clone(),
        displacements_mm: values
            .iter()
            .map(|v| std::array::from_fn(|k| if axes[k] { v[k] - first[k] } else { 0. }))
            .collect(),
    };
    motion.validate()?;
    let mut in_place = source.clone();
    in_place.name = name.into();
    in_place.tracks[index].values = Values::Translation(
        values
            .iter()
            .map(|v| std::array::from_fn(|k| if axes[k] { first[k] } else { v[k] }))
            .collect(),
    );
    in_place.validate(scene.nodes.len())?;
    Ok(Extracted { in_place, motion })
}

#[cfg(test)]
mod tests {
    use super::super::{
        animation::Track,
        scene::{Node, Transform},
    };
    use super::*;
    fn scene(interpolation: Interpolation) -> Scene {
        Scene {
            consumer:None, collision: Vec::new(),
            nodes: vec![
                Node {
                    name: "root".into(),
                    transform: Transform {
                        translation: [3., 4., 5.],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Node {
                    name: "child".into(),
                    parent: Some(0),
                    mesh: Some(0),
                    transform: Transform {
                        translation: [2., 3., 4.],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Node {
                    name: "unrelated".into(),
                    ..Default::default()
                },
            ],
            clips: vec![Clip {
                name: "walk".into(),
                tracks: vec![
                    Track {
                        node: 0,
                        times: vec![0.5, 1., 2.],
                        interpolation,
                        values: Values::Translation(vec![
                            [10., 20., 30.],
                            [14., 25., 35.],
                            [20., 30., 40.],
                        ]),
                    },
                    Track {
                        node: 0,
                        times: vec![0., 3.],
                        interpolation: Interpolation::Linear,
                        values: Values::Rotation(vec![
                            [0., 0., 0., 1.],
                            [0., 0., 0.6_f64.sin(), 0.6_f64.cos()],
                        ]),
                    },
                ],
            }],
            meshes: vec![super::super::scene::SceneMesh {
                name: "triangle".into(),
                primitives: vec![super::super::scene::Primitive {
                    geometry: super::super::tests::triangle(),
                    material: 0,
                }],
            }],
            materials: vec![Default::default()],
        }
    }
    #[test]
    fn extracted_translation_reconstructs_all_samples_without_mutating_source() {
        for interpolation in [Interpolation::Step, Interpolation::Linear] {
            let scene = scene(interpolation);
            let skin = super::super::skin::Skin::bind(
                &scene,
                1,
                vec![0],
                vec![vec![
                    super::super::skin::Weights {
                        joints: [0; 4],
                        weights: [1., 0., 0., 0.]
                    };
                    3
                ]],
            )
            .unwrap();
            let before = format!("{:?}", scene);
            let result = extract(&scene, 0, 0, [true, true, false], "walk-in-place").unwrap();
            for time in [-1., 0., 0.5, 0.75, 1., 1.5, 2., 2.9, 3., 4.] {
                let original = scene.clips[0].sample(&scene, time, false).unwrap();
                let mut composed = result.in_place.sample(&scene, time, false).unwrap();
                let motion = result.motion.sample(time, false).unwrap();
                assert_eq!(composed[0].translation[0], 10.);
                assert_eq!(composed[0].translation[1], 20.);
                for k in 0..3 {
                    composed[0].translation[k] += motion[k];
                }
                assert_eq!(composed, original);
                let reference = skin.deform_positions(&scene, &original).unwrap();
                let restored = skin.deform_positions(&scene, &composed).unwrap();
                assert_eq!(restored, reference);
            }
            assert_eq!(format!("{:?}", scene), before);
            let (bytes, exported) = super::super::glb::encode_root_motion(
                &scene,
                &[skin],
                0,
                0,
                [true, true, false],
                "walk-in-place",
            )
            .unwrap();
            let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
            let document: serde_json::Value =
                serde_json::from_slice(&bytes[20..20 + length]).unwrap();
            assert_eq!(document["animations"].as_array().unwrap().len(), 2);
            assert_eq!(document["skins"].as_array().unwrap().len(), 1);
            let metadata = &document["extras"]["gcad_root_motion"];
            assert_eq!(
                metadata["translation_axes"],
                serde_json::json!([true, false, true])
            );
            assert_eq!(
                metadata["loop_displacement"],
                serde_json::json!([0.01, 0., -0.01])
            );
            assert_eq!(metadata["in_place_animation"], 1);
            assert_eq!(metadata["node"], 0);
            assert_eq!(metadata["length_unit"], "metre");
            assert_eq!(
                exported.to_json().unwrap(),
                result.motion.to_json().unwrap()
            );
            assert_eq!(format!("{:?}", scene), before);
            let restored = Motion::from_json(&result.motion.to_json().unwrap()).unwrap();
            assert_eq!(
                restored.sample(7.5, true).unwrap(),
                result.motion.sample(7.5, true).unwrap()
            );
        }
    }
    #[test]
    fn loops_accumulate_endpoint_displacement_and_support_reverse_time() {
        let scene = scene(Interpolation::Linear);
        let motion = extract(&scene, 0, 0, [true, false, false], "in-place")
            .unwrap()
            .motion;
        assert_eq!(motion.sample(3., true).unwrap(), [10., 0., 0.]);
        assert_eq!(motion.sample(6., true).unwrap(), [20., 0., 0.]);
        assert_eq!(motion.sample(-3., true).unwrap(), [-10., 0., 0.]);
        assert_eq!(motion.sample(-0.5, true).unwrap(), [0., 0., 0.]);
        assert_eq!(motion.sample(3.75, true).unwrap(), [12., 0., 0.]);
        assert!(motion.sample(f64::NAN, true).is_err());
    }
    #[test]
    fn invalid_extraction_and_motion_files_fail_explicitly() {
        let scene = scene(Interpolation::Linear);
        assert!(extract(&scene, 0, 1, [true; 3], "in-place").is_err());
        assert!(extract(&scene, 0, 0, [false; 3], "in-place").is_err());
        assert!(extract(&scene, 0, 0, [true; 3], "walk").is_err());
        let json = extract(&scene, 0, 0, [true; 3], "in-place")
            .unwrap()
            .motion
            .to_json()
            .unwrap();
        assert!(Motion::from_json(&json.replace("\"schema\": 1", "\"schema\": 2")).is_err());
        assert!(Motion::from_json(&json.replacen('{', "{\"unknown\":1,", 1)).is_err());
    }
}
