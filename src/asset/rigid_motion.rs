//! Full rigid root-motion decomposition. Translation and rotation tracks retain
//! their own interpolation/times; a matrix trajectory is evaluated, not rebaked.
use super::{
    animation::{Clip, Track, Values},
    scene::{Node, Scene, Transform},
};
use anyhow::{Result, ensure};
use monstertruck::modeling::{Matrix4, SquareMatrix};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Motion {
    pub schema: u32,
    pub source_clip: String,
    pub node: String,
    pub duration_seconds: f64,
    pub rest_translation_mm: [f64; 3],
    pub rest_rotation_xyzw: [f64; 4],
    /// Native millimetre/Z-up channels. Node indices are local to this record (0).
    pub tracks: Vec<Track>,
}
pub struct Extracted {
    pub in_place: Clip,
    pub motion: Motion,
}
impl Motion {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.schema == 1, "unsupported rigid root-motion schema");
        ensure!(
            !self.source_clip.trim().is_empty() && !self.node.trim().is_empty(),
            "rigid root motion requires names"
        );
        ensure!(
            self.duration_seconds.is_finite() && self.duration_seconds > 0.,
            "rigid root motion requires positive duration"
        );
        self.rest().validate()?;
        ensure!(
            self.tracks
                .iter()
                .any(|t| matches!(t.values, Values::Rotation(_))),
            "rigid root motion requires a rotation track"
        );
        ensure!(
            self.tracks
                .iter()
                .all(|t| !matches!(t.values, Values::Scale(_))
                    && t.times.iter().all(|&v| v <= self.duration_seconds)),
            "rigid root motion cannot contain scale or keys beyond its duration"
        );
        self.clip().validate(1)?;
        Ok(())
    }
    fn rest(&self) -> Transform {
        Transform {
            translation: self.rest_translation_mm,
            rotation: self.rest_rotation_xyzw,
            ..Default::default()
        }
    }
    fn clip(&self) -> Clip {
        Clip {
            name: self.source_clip.clone(),
            tracks: self.tracks.clone(),
        }
    }
    fn pose(&self, time: f64) -> Result<Transform> {
        let scene = Scene {
            nodes: vec![Node {
                transform: self.rest(),
                ..Default::default()
            }],
            ..Default::default()
        };
        Ok(self.clip().sample(&scene, time, false)?[0])
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
    /// Initial sampled rigid pose, excluding scale. In-place clips retain it.
    pub fn anchor(&self) -> Result<Transform> {
        self.validate()?;
        self.pose(0.)
    }
    /// D(t) = rigid_pose(t) * inverse(rigid_pose(0)). Apply on the left of the
    /// named in-place root's local matrix. Looping uses D(end)^cycles * D(phase),
    /// so turning rotates subsequent travel. Negative cycles use the inverse.
    pub fn sample(&self, time: f64, looping: bool) -> Result<Matrix4> {
        self.validate()?;
        ensure!(time.is_finite(), "rigid root-motion time must be finite");
        let anchor_inverse = self
            .pose(0.)?
            .matrix()?
            .invert()
            .ok_or_else(|| anyhow::anyhow!("rigid root anchor is singular"))?;
        let cycles = if looping {
            (time / self.duration_seconds).floor()
        } else {
            0.
        };
        ensure!(
            cycles.abs() <= 1_000_000.,
            "rigid root motion exceeds one million accumulated cycles"
        );
        let phase = if looping {
            time.rem_euclid(self.duration_seconds)
        } else {
            time
        };
        let local = self.pose(phase)?.matrix()? * anchor_inverse;
        let end = self.pose(self.duration_seconds)?.matrix()? * anchor_inverse;
        let accumulated = power(end, cycles as i64)?;
        let result = accumulated * local;
        let raw: &[[f64; 4]; 4] = result.as_ref();
        ensure!(
            raw.iter().flatten().all(|v| v.is_finite()),
            "rigid root-motion matrix overflow"
        );
        Ok(result)
    }
}
fn power(mut matrix: Matrix4, cycles: i64) -> Result<Matrix4> {
    if cycles < 0 {
        matrix = matrix
            .invert()
            .ok_or_else(|| anyhow::anyhow!("rigid loop transform is singular"))?;
    }
    let mut exponent = cycles.unsigned_abs();
    let mut result = Matrix4::identity();
    while exponent > 0 {
        if exponent & 1 != 0 {
            result = result * matrix;
        }
        exponent >>= 1;
        if exponent > 0 {
            matrix = matrix * matrix;
        }
    }
    Ok(result)
}

/// Extract all translation and full quaternion rotation from a top-level node.
/// Scale and all other node channels remain in place. This is not yaw-only or
/// partial-axis extraction: those need an explicit residual-motion policy.
pub fn extract(scene: &Scene, clip: usize, node: usize, name: &str) -> Result<Extracted> {
    scene.validate()?;
    let source = scene
        .clips
        .get(clip)
        .ok_or_else(|| anyhow::anyhow!("rigid root-motion clip not found"))?;
    let root = scene
        .nodes
        .get(node)
        .ok_or_else(|| anyhow::anyhow!("rigid root-motion node not found"))?;
    ensure!(
        root.parent.is_none(),
        "rigid root motion requires a top-level node"
    );
    ensure!(
        !name.trim().is_empty() && scene.clips.iter().all(|c| c.name != name),
        "in-place clip name is empty or already exists"
    );
    let tracks = source
        .tracks
        .iter()
        .filter(|t| t.node == node && !matches!(t.values, Values::Scale(_)))
        .map(|t| {
            let mut t = t.clone();
            t.node = 0;
            t
        })
        .collect();
    let motion = Motion {
        schema: 1,
        source_clip: source.name.clone(),
        node: root.name.clone(),
        duration_seconds: source.duration(),
        rest_translation_mm: root.transform.translation,
        rest_rotation_xyzw: root.transform.rotation,
        tracks,
    };
    let anchor = motion.anchor()?;
    let mut in_place = source.clone();
    in_place.name = name.into();
    for t in &mut in_place.tracks {
        if t.node == node {
            match &mut t.values {
                Values::Translation(v) => v.fill(anchor.translation),
                Values::Rotation(v) => v.fill(anchor.rotation),
                Values::Scale(_) => {}
            }
        }
    }
    in_place.validate(scene.nodes.len())?;
    Ok(Extracted { in_place, motion })
}

#[cfg(test)]
mod tests {
    use super::super::animation::Interpolation;
    use super::*;
    use monstertruck::modeling::{Point3, Transform as _};
    fn fixture() -> Scene {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        super::super::author::load(
            &root.join("examples/game-assets/plate-root-motion.gasset"),
            &[],
        )
        .unwrap()
        .scene
    }
    fn close(a: Matrix4, b: Matrix4) {
        for p in [Point3::new(0., 0., 0.), Point3::new(10., -3., 7.)] {
            let (a, b) = (a.transform_point(p), b.transform_point(p));
            assert!(
                (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9 && (a.z - b.z).abs() < 1e-9,
                "{a:?} != {b:?}"
            );
        }
    }
    #[test]
    fn rigid_recomposition_preserves_nonzero_anchor_rotation_scale_and_source() {
        let mut scene = fixture();
        scene.clips[0].tracks.push(Track {
            node: 0,
            times: vec![0., 2.],
            interpolation: Interpolation::Linear,
            values: Values::Scale(vec![[1., 2., 3.], [2., 3., 4.]]),
        });
        for t in &mut scene.clips[0].tracks {
            if let Values::Rotation(v) = &mut t.values {
                *v = vec![
                    [0., 0., 0.3_f64.sin(), 0.3_f64.cos()],
                    [0., 0., 1.1_f64.sin(), 1.1_f64.cos()],
                ];
            }
        }
        let before = format!("{scene:?}");
        for mode in [Interpolation::Linear, Interpolation::Step] {
            let mut scene = scene.clone();
            for track in &mut scene.clips[0].tracks {
                if matches!(track.values, Values::Rotation(_)) {
                    track.interpolation = mode;
                }
            }
            let extracted = extract(&scene, 0, 0, "in-place").unwrap();
            for time in [-0.5, 0., 0.25, 0.999, 1., 1.75, 2., 3.] {
                let original = scene.clips[0].sample(&scene, time, false).unwrap();
                let pose = extracted.in_place.sample(&scene, time, false).unwrap();
                close(
                    extracted.motion.sample(time, false).unwrap() * pose[0].matrix().unwrap(),
                    original[0].matrix().unwrap(),
                );
                assert_eq!(&pose[1..], &original[1..]);
            }
            let json = extracted.motion.to_json().unwrap();
            close(
                Motion::from_json(&json)
                    .unwrap()
                    .sample(1.25, false)
                    .unwrap(),
                extracted.motion.sample(1.25, false).unwrap(),
            );
        }
        assert_eq!(format!("{scene:?}"), before);
    }
    #[test]
    fn turning_loops_compose_travel_and_reverse_cycles() {
        let scene = fixture();
        let motion = extract(&scene, 0, 0, "in-place").unwrap().motion;
        let anchor = motion.anchor().unwrap().matrix().unwrap();
        let end = motion.sample(2., false).unwrap();
        close(motion.sample(4., true).unwrap(), end * end);
        close(motion.sample(-2., true).unwrap(), end.invert().unwrap());
        close(
            motion.sample(4.5, true).unwrap(),
            end * end * motion.sample(0.5, false).unwrap(),
        );
        // First travel is (100,50,10); the next is rotated +90 degrees about Z.
        let p =
            (motion.sample(4., true).unwrap() * anchor).transform_point(Point3::new(0., 0., 0.));
        assert!((p.x - 60.).abs() < 1e-9 && (p.y - 170.).abs() < 1e-9 && (p.z - 50.).abs() < 1e-9);
        assert!(motion.sample(f64::INFINITY, true).is_err());
        assert!(motion.sample(2_000_002., true).is_err());
    }
    #[test]
    fn invalid_rigid_records_and_child_nodes_fail() {
        let scene = fixture();
        assert!(extract(&scene, 0, 1, "in-place").is_err());
        let mut motion = extract(&scene, 0, 0, "in-place").unwrap().motion;
        motion
            .tracks
            .retain(|t| !matches!(t.values, Values::Rotation(_)));
        assert!(motion.validate().is_err());
        let mut motion = extract(&scene, 0, 0, "in-place").unwrap().motion;
        motion.tracks[0].node = 1;
        assert!(motion.validate().is_err());
    }
    #[test]
    fn rigid_glb_exports_anchor_channels_loop_transform_and_skin() {
        let scene = fixture();
        let before = format!("{scene:?}");
        let count = scene.meshes[0].primitives[0].geometry.positions.len();
        let skin = super::super::skin::Skin::bind(
            &scene,
            1,
            vec![0],
            vec![vec![
                super::super::skin::Weights {
                    joints: [0; 4],
                    weights: [1., 0., 0., 0.]
                };
                count
            ]],
        )
        .unwrap();
        let (bytes, motion) =
            super::super::glb::encode_rigid_root_motion(&scene, &[skin], 0, 0, "rigid-in-place")
                .unwrap();
        let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let doc: serde_json::Value = serde_json::from_slice(&bytes[20..20 + length]).unwrap();
        let metadata = &doc["extras"]["gcad_root_motion"];
        assert_eq!(doc["animations"].as_array().unwrap().len(), 2);
        assert_eq!(doc["skins"].as_array().unwrap().len(), 1);
        assert_eq!(metadata["schema"], 2);
        assert_eq!(metadata["mode"], "rigid");
        assert_eq!(
            metadata["anchor"]["translation"],
            serde_json::json!([0.01, 0.03, -0.02])
        );
        assert_eq!(
            metadata["anchor"]["rotation"],
            serde_json::json!([0., 0., 0., 1.])
        );
        assert_eq!(metadata["composition"], "left-multiply-in-place-root");
        assert_eq!(metadata["loop_composition"], "endpoint-power-times-phase");
        assert_eq!(metadata["channels"].as_array().unwrap().len(), 2);
        for channel in metadata["channels"].as_array().unwrap() {
            if channel["path"] == "translation" {
                assert_eq!(channel["values"][2], serde_json::json!([0.11, 0.04, -0.07]));
            } else {
                let q: Vec<f64> = serde_json::from_value(channel["values"][1].clone()).unwrap();
                assert!(
                    q[0].abs() < 1e-12
                        && (q[1] - 0.5_f64.sqrt()).abs() < 1e-12
                        && q[2].abs() < 1e-12
                );
            }
        }
        let endpoint: Vec<f64> =
            serde_json::from_value(metadata["loop_transform"]["translation"].clone()).unwrap();
        for (a, b) in endpoint.iter().zip([0.13, 0.01, -0.06]) {
            assert!((a - b).abs() < 1e-12);
        }
        close(
            motion.sample(2., false).unwrap(),
            extract(&scene, 0, 0, "rigid-in-place")
                .unwrap()
                .motion
                .sample(2., false)
                .unwrap(),
        );
        assert_eq!(format!("{scene:?}"), before);
    }
    #[test]
    fn rotation_without_translation_tracks_preserves_the_root_pivot() {
        let mut scene = fixture();
        scene.clips[0]
            .tracks
            .retain(|t| !matches!(t.values, Values::Translation(_)));
        let extracted = extract(&scene, 0, 0, "in-place").unwrap();
        for time in [0., 0.5, 1.5, 2.] {
            let original = scene.clips[0].sample(&scene, time, false).unwrap()[0];
            let in_place = extracted.in_place.sample(&scene, time, false).unwrap()[0];
            let recomposed =
                extracted.motion.sample(time, false).unwrap() * in_place.matrix().unwrap();
            close(recomposed, original.matrix().unwrap());
            let p = recomposed.transform_point(Point3::new(0., 0., 0.));
            assert!(
                (p.x - 10.).abs() < 1e-9 && (p.y - 20.).abs() < 1e-9 && (p.z - 30.).abs() < 1e-9
            );
        }
        let json = extracted.motion.to_json().unwrap();
        assert!(Motion::from_json(&json.replace("\"schema\": 1", "\"schema\": 2")).is_err());
        assert!(
            Motion::from_json(&json.replacen("\"path\":", "\"unknown\": 1, \"path\":", 1)).is_err()
        );
    }
}
