//! Sample mechanical constraints into local TRS tracks without modifying CAD.
use super::{
    animation::{Clip, Interpolation, Track, Values},
    scene::{Scene, Transform},
};
use anyhow::{Context, Result, ensure};
use monstertruck::modeling::{InnerSpace, Matrix4, Point3, SquareMatrix, Transform as _};

#[derive(Clone, Debug)]
pub struct Motion {
    pub name: String,
    pub instance: String,
    pub joint: String,
    /// Degrees for a turn joint, millimetres for a slide joint.
    pub from: f64,
    pub to: f64,
    pub duration: f64,
    pub rate: f64,
    /// Optional world-space vertex error budget checked at keys and quarter intervals.
    pub max_error_mm: Option<f64>,
    /// Optional ceiling for adaptive uniform rate increases after a sampling failure.
    pub max_rate: Option<f64>,
}

#[derive(Debug)]
struct SamplingError(String);
impl std::fmt::Display for SamplingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for SamplingError {}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn constrained_linkage_bake_matches_cad_pose_and_preserves_other_instance() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let asset = super::super::author::load(
            &root.join("examples/game-assets/linkage-motion.gasset"),
            &[],
        )
        .unwrap();
        let clip = &asset.scene.clips[0];
        assert_eq!(clip.tracks.len(), 12);
        assert!(
            clip.tracks
                .iter()
                .all(|t| t.times.len() == 25 && t.times[24] == 1.0)
        );
        let (run, _) =
            crate::model::run_path_recorded(&root.join("examples/assemblies/linkage.gasm"), &[])
                .unwrap();
        let before = run.model.placements.clone();
        let mut posed = run.model.clone();
        let mut max_between = 0.0_f64;
        for sample in 0..=48 {
            let time = sample as f64 / 48.0;
            let line = crate::parse::parse_line(1, &format!("pose drive {}", time * 30.0)).unwrap();
            posed.op_pose(&line).unwrap();
            let transforms = clip.sample(&asset.scene, time, false).unwrap();
            for (i, node) in asset.scene.nodes.iter().enumerate() {
                if let Some(body) = node.name.strip_prefix("first/") {
                    let actual = transforms[i].matrix().unwrap();
                    let expected = posed
                        .placements
                        .get(body)
                        .copied()
                        .unwrap_or_else(Matrix4::identity);
                    let mesh = &asset.scene.meshes[node.mesh.unwrap()].primitives[0].geometry;
                    for p in &mesh.positions {
                        let p = Point3::new(p[0], p[1], p[2]);
                        let a = actual.transform_point(p);
                        let b = expected.transform_point(p);
                        let error =
                            ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2))
                                .sqrt();
                        if sample % 2 == 0 {
                            assert!(error < 1e-5, "key pose differs {error} mm");
                        } else {
                            max_between = max_between.max(error);
                        }
                    }
                } else {
                    assert_eq!(transforms[i], node.transform);
                }
            }
        }
        assert_eq!(run.model.placements, before);
        assert!(
            max_between < 0.01,
            "midpoint interpolation error {max_between} mm"
        );
        eprintln!("linkage maximum sampled between-key vertex error: {max_between} mm");
        let base = Motion {
            name: "test".into(),
            instance: "first".into(),
            joint: "drive".into(),
            from: 0.,
            to: 30.,
            duration: 1.,
            rate: 24.,
            max_error_mm: Some(0.01),
            max_rate: None,
        };
        for bad in [
            Motion {
                max_error_mm: Some(0.0001),
                ..base.clone()
            },
            Motion {
                max_error_mm: Some(-1.0),
                ..base.clone()
            },
            Motion {
                rate: 0.,
                ..base.clone()
            },
            Motion {
                duration: f64::NAN,
                ..base.clone()
            },
            Motion {
                rate: 10001.,
                ..base.clone()
            },
            Motion {
                to: 360.,
                rate: 1.,
                ..base.clone()
            },
            Motion {
                joint: "missing".into(),
                ..base.clone()
            },
            Motion {
                from: 10001.,
                ..base.clone()
            },
        ] {
            assert!(bad.bake(&asset.scene, &run.model).is_err());
        }
        assert!(base.bake(&asset.scene, &run.model).is_ok());
        let adaptive = Motion {
            rate: 2.0,
            max_rate: Some(96.0),
            max_error_mm: Some(0.001),
            ..base.clone()
        };
        let adapted = adaptive.bake(&asset.scene, &run.model).unwrap();
        let count = adapted.tracks[0].times.len() - 1;
        assert!(count > 2 && count <= 96);
        // Every retry starts from the source rig; accepted output equals an explicit bake.
        let explicit = Motion {
            rate: count as f64,
            max_rate: None,
            ..adaptive.clone()
        }
        .bake(&asset.scene, &run.model)
        .unwrap();
        for time in [0.0, 0.123, 0.5, 0.999, 1.0] {
            assert_eq!(
                adapted.sample(&asset.scene, time, false).unwrap(),
                explicit.sample(&asset.scene, time, false).unwrap()
            );
        }
        let exhausted = Motion {
            max_rate: Some(2.0),
            ..adaptive.clone()
        }
        .bake(&asset.scene, &run.model)
        .unwrap_err();
        assert!(exhausted.downcast_ref::<SamplingError>().is_some());
        assert!(
            Motion {
                max_error_mm: None,
                ..adaptive.clone()
            }
            .bake(&asset.scene, &run.model)
            .is_err()
        );
        eprintln!("adaptive linkage accepted {count} intervals from initial 2");
        let mut scaled = asset.scene.clone();
        let owner = scaled.nodes.iter().position(|n| n.name == "first").unwrap();
        scaled.nodes[owner].transform.scale = [100.0; 3];
        let error = base.bake(&scaled, &run.model).unwrap_err().to_string();
        assert!(
            error.contains("world-space vertex error") && error.contains("increase rate"),
            "{error}"
        );
        let mut limited = run.model.clone();
        limited
            .joints
            .iter_mut()
            .find(|j| j.name == "drive")
            .unwrap()
            .range = (0.0, 10.0);
        assert!(
            base.bake(&asset.scene, &limited)
                .unwrap_err()
                .to_string()
                .contains("sample")
        );
        // Exercise the same scene with synthetic slider joints and an explicit
        // 2:1 coupling. This isolates solver/bake semantics from CAD construction.
        let mut sliders = run.model.clone();
        sliders.mates.clear();
        for joint in &mut sliders.joints {
            joint.value = 0.0;
            joint.range = (-100.0, 100.0);
            joint.wraps = false;
            joint.kind = match joint.name.as_str() {
                "drive" => crate::model::JointKind::Slide {
                    along: monstertruck::modeling::Vector3::unit_x(),
                },
                "rock" => crate::model::JointKind::Slide {
                    along: monstertruck::modeling::Vector3::unit_y(),
                },
                _ => crate::model::JointKind::Fixed,
            };
        }
        sliders.couples = vec![crate::model::Couple {
            driver: "drive".into(),
            driven: "rock".into(),
            ratio: 2.0,
            ..Default::default()
        }];
        let slide = Motion {
            from: 10.0,
            to: -5.0,
            rate: 1.0,
            max_error_mm: Some(1e-8),
            ..base.clone()
        };
        let clip = slide.bake(&asset.scene, &sliders).unwrap();
        for time in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let poses = clip.sample(&asset.scene, time, false).unwrap();
            let travel = 10.0 - 15.0 * time;
            for (i, node) in asset.scene.nodes.iter().enumerate() {
                let mut expected = node.transform.translation;
                match node.name.as_str() {
                    "first/crank" | "first/coupler" => expected[0] += travel,
                    "first/rocker" => expected[1] += 2.0 * travel,
                    _ => {}
                }
                for axis in 0..3 {
                    assert!((poses[i].translation[axis] - expected[axis]).abs() < 1e-8);
                }
            }
        }
        assert!(
            Motion {
                joint: "rock".into(),
                ..slide.clone()
            }
            .bake(&asset.scene, &sliders)
            .is_err()
        );
        assert!(
            Motion {
                joint: "bend".into(),
                ..slide
            }
            .bake(&asset.scene, &sliders)
            .is_err()
        );
        let text = std::fs::read_to_string(root.join("examples/game-assets/linkage-motion.gasset"))
            .unwrap();
        for bad in [
            format!("{text}\nmotion drive second joint=drive from=0 to=1 duration=1 rate=24"),
            text.replace("motion drive first", "motion drive absent"),
            format!("{text}\nclip drive interpolation=linear"),
        ] {
            assert!(super::super::author::Document::parse(&bad).is_err());
        }
    }
}

impl Motion {
    pub fn intervals(&self) -> Result<usize> {
        if let Some(rate) = self.max_rate {
            ensure!(
                self.max_error_mm.is_some()
                    && rate.is_finite()
                    && rate >= self.rate
                    && (1.0..=10000.0).contains(&(rate * self.duration).ceil()),
                "adaptive max_rate requires max_error, a finite rate at least as large as rate, and at most 10000 intervals"
            );
        }
        ensure!(
            !self.name.trim().is_empty()
                && !self.instance.trim().is_empty()
                && !self.joint.trim().is_empty(),
            "motion requires clip, instance and joint names"
        );
        ensure!(
            [self.from, self.to, self.duration, self.rate]
                .iter()
                .all(|v| v.is_finite())
                && self.duration > 0.0
                && self.rate > 0.0,
            "motion values must be finite; duration and rate must be positive"
        );
        let count = (self.duration * self.rate).ceil();
        ensure!(
            self.max_error_mm.is_none_or(|e| e.is_finite() && e > 0.0),
            "motion max_error must be finite and positive millimetres"
        );
        ensure!(
            (1.0..=10000.0).contains(&count),
            "motion requires 1..10000 sampling intervals"
        );
        Ok(count as usize)
    }

    pub fn bake(&self, scene: &Scene, model: &crate::model::Model) -> Result<Clip> {
        let mut count = self.intervals()?;
        let ceiling = if let Some(rate) = self.max_rate {
            ensure!(
                self.max_error_mm.is_some() && rate.is_finite() && rate >= self.rate,
                "adaptive max_rate requires max_error and a finite rate at least as large as rate"
            );
            Motion {
                rate,
                max_rate: None,
                ..self.clone()
            }
            .intervals()?
        } else {
            count
        };
        loop {
            match self.bake_at_count(scene, model, count) {
                Ok(clip) => return Ok(clip),
                Err(error)
                    if error.downcast_ref::<SamplingError>().is_some() && count < ceiling =>
                {
                    count = (count * 2).min(ceiling);
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn bake_at_count(
        &self,
        scene: &Scene,
        model: &crate::model::Model,
        count: usize,
    ) -> Result<Clip> {
        ensure!(model.assembly, "motion requires an assembly source");
        let owner = scene
            .nodes
            .iter()
            .position(|n| n.name == self.instance)
            .context("motion instance missing")?;
        let rig = model.rig();
        let driver = rig
            .joints
            .iter()
            .position(|j| j.name == self.joint)
            .context("motion joint missing")?;
        ensure!(
            rig.joints[driver].movable() && rig.driver_of(&self.joint).is_none(),
            "motion must drive an independent movable joint"
        );
        // Bound the solver's internal continuation work, including reaching the first pose.
        ensure!(
            (self.from - rig.joints[driver].value).abs() <= 10000.0
                && (self.to - self.from).abs() <= 10000.0,
            "motion joint travel exceeds the bake work limit"
        );
        let relative = |mut node: usize| -> Result<Option<Matrix4>> {
            let mut matrix = Matrix4::identity();
            for _ in 0..=scene.nodes.len() {
                if node == owner {
                    return Ok(Some(matrix));
                }
                matrix = scene.nodes[node].transform.matrix()? * matrix;
                let Some(parent) = scene.nodes[node].parent else {
                    return Ok(None);
                };
                node = parent;
            }
            anyhow::bail!("cycle in motion hierarchy")
        };
        scene.world_transforms()?;
        let children: Vec<_> = scene
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.mesh.is_some())
            .filter_map(|(i, n)| {
                let rest = match relative(i) {
                    Ok(Some(rest)) => rest,
                    Ok(None) => return None,
                    Err(e) => return Some(Err(e)),
                };
                Some((|| {
                    Ok((
                        i,
                        n.name
                            .strip_prefix(&format!("{}/", self.instance))
                            .context("motion child name does not identify a CAD body")?
                            .to_string(),
                        rest,
                        relative(n.parent.context("motion body has no parent")?)?
                            .context("motion parent outside instance")?,
                    ))
                })())
            })
            .collect::<Result<_>>()?;
        ensure!(
            !children.is_empty(),
            "motion instance has no evaluated bodies"
        );
        // Explicitly guard direct and coupled turn speeds; quaternion endpoints alone
        // cannot detect a whole revolution hidden between two samples.
        let mut changes = std::collections::HashMap::new();
        changes.insert(self.joint.clone(), (self.to - self.from) / count as f64);
        for _ in 0..rig.couples.len() {
            for couple in &rig.couples {
                if let Some(delta) = changes.get(&couple.driver).copied() {
                    changes.insert(couple.driven.clone(), delta * couple.ratio);
                }
            }
        }
        for joint in &rig.joints {
            if matches!(joint.kind, crate::model::JointKind::Turn { .. }) {
                if !changes.get(&joint.name).is_none_or(|d| d.abs() < 90.0) {
                    return Err(SamplingError(format!("motion `{}` rotation sampling is too coarse at {count} intervals; increase rate or max_rate",self.name)).into());
                }
            }
        }
        let mut samples = vec![Vec::<Transform>::with_capacity(count + 1); children.len()];
        let mut current = rig.values();
        for step in 0..=count {
            let fraction = step as f64 / count as f64;
            let value = self.from * (1.0 - fraction) + self.to * fraction;
            let (values, moves) = rig.drive(&current, driver, value).map_err(|why| {
                anyhow::anyhow!("motion `{}` sample {step}/{count}: {why}", self.name)
            })?;
            current = values;
            for (output, (_, body, rest, parent_frame)) in samples.iter_mut().zip(&children) {
                let mut pose = Transform::from_matrix(
                    parent_frame.invert().context("singular motion parent")?
                        * moves.get(body).copied().unwrap_or_else(Matrix4::identity)
                        * rest,
                )?;
                if let Some(previous) = output.last() {
                    let dot: f64 = previous
                        .rotation
                        .iter()
                        .zip(pose.rotation)
                        .map(|(a, b)| a * b)
                        .sum();
                    if dot.abs() <= std::f64::consts::FRAC_1_SQRT_2 {
                        return Err(SamplingError(format!("motion `{}` part `{body}` rotates at least 90 degrees between samples at {count} intervals; increase rate or max_rate",self.name)).into());
                    }
                    if dot < 0.0 {
                        pose.rotation.iter_mut().for_each(|v| *v = -*v);
                    }
                }
                output.push(pose);
            }
        }
        let times: Vec<_> = (0..=count)
            .map(|i| self.duration * (i as f64 / count as f64))
            .collect();
        let mut clip = Clip {
            name: self.name.clone(),
            tracks: Vec::new(),
        };
        for ((node, _, _, _), samples) in children.iter().zip(samples) {
            for values in [
                Values::Translation(samples.iter().map(|p| p.translation).collect()),
                Values::Rotation(samples.iter().map(|p| p.rotation).collect()),
                Values::Scale(samples.iter().map(|p| p.scale).collect()),
            ] {
                clip.tracks.push(Track {
                    node: *node,
                    times: times.clone(),
                    interpolation: Interpolation::Linear,
                    values,
                });
            }
        }
        clip.validate(scene.nodes.len())?;
        if let Some(budget) = self.max_error_mm {
            let parent = scene.world_transforms()?[owner];
            let mut current = rig.values();
            for sample in 0..=count * 4 {
                let fraction = sample as f64 / (count * 4) as f64;
                let time = self.duration * fraction;
                let value = self.from * (1.0 - fraction) + self.to * fraction;
                let (values, moves) = rig.drive(&current, driver, value).map_err(|why| {
                    anyhow::anyhow!("motion `{}` error-check sample {sample}: {why}", self.name)
                })?;
                current = values;
                let interpolated = clip.sample(scene, time, false)?;
                for (node, body, rest, parent_frame) in &children {
                    let expected =
                        parent * moves.get(body).copied().unwrap_or_else(Matrix4::identity) * rest;
                    let actual = parent * parent_frame * interpolated[*node].matrix()?;
                    let mesh = &scene.meshes[scene.nodes[*node].mesh.unwrap()];
                    for primitive in &mesh.primitives {
                        for p in &primitive.geometry.positions {
                            let p = Point3::new(p[0], p[1], p[2]);
                            let error = (actual.transform_point(p) - expected.transform_point(p))
                                .magnitude();
                            ensure!(error.is_finite(), "motion vertex error is nonfinite");
                            if error > budget {
                                return Err(SamplingError(format!("motion `{}` part `{body}` at {time:.9} s: world-space vertex error {error:.9} mm exceeds {budget} mm at {count} intervals; increase rate or max_rate",self.name)).into());
                            }
                        }
                    }
                }
            }
        }
        Ok(clip)
    }
}
