//! Rigid local-TRS clips; times are seconds, translation is CAD millimetres.
use super::scene::{Scene, Transform};
use anyhow::{Result, ensure};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Interpolation {
    Step,
    Linear,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "path",
    content = "keys",
    rename_all = "lowercase",
    deny_unknown_fields
)]
pub enum Values {
    Translation(Vec<[f64; 3]>),
    Rotation(Vec<[f64; 4]>),
    Scale(Vec<[f64; 3]>),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    pub node: usize,
    pub times: Vec<f64>,
    pub interpolation: Interpolation,
    pub values: Values,
}

#[derive(Clone, Debug)]
pub struct Clip {
    pub name: String,
    pub tracks: Vec<Track>,
}

impl Values {
    pub fn path(&self) -> &'static str {
        match self {
            Self::Translation(_) => "translation",
            Self::Rotation(_) => "rotation",
            Self::Scale(_) => "scale",
        }
    }
}

impl Clip {
    pub fn validate(&self, node_count: usize) -> Result<()> {
        ensure!(
            !self.name.trim().is_empty() && !self.tracks.is_empty(),
            "clip requires a name and tracks"
        );
        let mut targets = HashSet::new();
        for track in &self.tracks {
            ensure!(
                track.node < node_count,
                "animation node reference is invalid"
            );
            ensure!(
                targets.insert((track.node, track.values.path())),
                "duplicate animation target property"
            );
            ensure!(
                !track.times.is_empty()
                    && track.times.iter().all(|t| t.is_finite() && *t >= 0.0)
                    && track.times.windows(2).all(|t| t[1] > t[0]),
                "animation times must be finite, nonnegative and strictly increasing"
            );
            let count = match &track.values {
                Values::Translation(values) => {
                    ensure!(
                        values.iter().flatten().all(|v| v.is_finite()),
                        "invalid translation keys"
                    );
                    values.len()
                }
                Values::Rotation(values) => {
                    for &rotation in values {
                        Transform {
                            rotation,
                            ..Default::default()
                        }
                        .validate()?;
                    }
                    values.len()
                }
                Values::Scale(values) => {
                    for &scale in values {
                        Transform {
                            scale,
                            ..Default::default()
                        }
                        .validate()?;
                    }
                    if track.interpolation == Interpolation::Linear {
                        ensure!(
                            values
                                .windows(2)
                                .all(|v| (0..3).all(|k| v[0][k].signum() == v[1][k].signum())),
                            "linear scale track crosses a singular transform"
                        );
                    }
                    values.len()
                }
            };
            ensure!(
                count == track.times.len(),
                "animation key count differs from times"
            );
        }
        Ok(())
    }

    pub fn duration(&self) -> f64 {
        self.tracks
            .iter()
            .filter_map(|t| t.times.last())
            .copied()
            .fold(0.0, f64::max)
    }

    /// Untracked properties retain the rest transform; no accumulated mutation.
    /// Looping wraps [0,duration); non-looping clamps each track to its key range.
    pub fn sample(&self, scene: &Scene, time: f64, looping: bool) -> Result<Vec<Transform>> {
        self.validate(scene.nodes.len())?;
        ensure!(time.is_finite(), "playback time must be finite");
        let duration = self.duration();
        let time = if looping && duration > 0.0 {
            time.rem_euclid(duration)
        } else {
            time
        };
        let mut result: Vec<_> = scene.nodes.iter().map(|n| n.transform).collect();
        for track in &self.tracks {
            let upper = track.times.partition_point(|t| *t <= time);
            let a = upper.saturating_sub(1);
            let b = upper.min(track.times.len() - 1);
            let t = if a == b || track.interpolation == Interpolation::Step {
                0.0
            } else {
                ((time - track.times[a]) / (track.times[b] - track.times[a])).clamp(0.0, 1.0)
            };
            let target = &mut result[track.node];
            match &track.values {
                Values::Translation(values) => target.translation = lerp(values[a], values[b], t),
                Values::Scale(values) => target.scale = lerp(values[a], values[b], t),
                Values::Rotation(values) => target.rotation = slerp(values[a], values[b], t),
            }
        }
        for transform in &result {
            transform.validate()?;
        }
        Ok(result)
    }
}

fn lerp(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    std::array::from_fn(|k| a[k] * (1.0 - t) + b[k] * t)
}

pub(super) fn normalized(q: [f64; 4]) -> [f64; 4] {
    let norm = q.iter().map(|v| v * v).sum::<f64>().sqrt();
    q.map(|v| v / norm)
}

fn slerp(a: [f64; 4], b: [f64; 4], t: f64) -> [f64; 4] {
    let a = normalized(a);
    let mut b = normalized(b);
    let mut dot: f64 = a.iter().zip(b).map(|(a, b)| a * b).sum();
    if dot < 0.0 {
        b = b.map(|v| -v);
        dot = -dot;
    }
    if dot > 0.9995 {
        return normalized(std::array::from_fn(|k| a[k] * (1.0 - t) + b[k] * t));
    }
    let angle = dot.clamp(-1.0, 1.0).acos();
    let (x, y) = (
        ((1.0 - t) * angle).sin() / angle.sin(),
        (t * angle).sin() / angle.sin(),
    );
    normalized(std::array::from_fn(|k| a[k] * x + b[k] * y))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn samples_shortest_rotation_linear_translation_steps_and_loop_bounds() {
        let scene = crate::asset::scene::tests::fixture();
        let q = std::f64::consts::FRAC_1_SQRT_2;
        let clip = Clip {
            name: "move".into(),
            tracks: vec![
                Track {
                    node: 0,
                    times: vec![0.0, 2.0],
                    interpolation: Interpolation::Linear,
                    values: Values::Rotation(vec![[0.0, 0.0, 0.0, 1.0], [0.0, 0.0, -q, -q]]),
                },
                Track {
                    node: 0,
                    times: vec![0.0, 2.0],
                    interpolation: Interpolation::Linear,
                    values: Values::Translation(vec![[0.0; 3], [2000.0, 0.0, 0.0]]),
                },
                Track {
                    node: 1,
                    times: vec![0.0, 2.0],
                    interpolation: Interpolation::Step,
                    values: Values::Scale(vec![[1.0; 3], [-1.0, 2.0, 3.0]]),
                },
            ],
        };
        let mid = clip.sample(&scene, 1.0, false).unwrap();
        assert_eq!(mid[0].translation, [1000.0, 0.0, 0.0]);
        assert!((mid[0].rotation[2] - (std::f64::consts::PI / 8.0).sin()).abs() < 1e-12);
        assert_eq!(mid[0].scale, scene.nodes[0].transform.scale);
        assert_eq!(mid[1].scale, [1.0; 3]);
        assert_eq!(
            clip.sample(&scene, 2.0, false).unwrap()[1].scale,
            [-1.0, 2.0, 3.0]
        );
        assert_eq!(
            clip.sample(&scene, 2.0, true).unwrap()[0].translation,
            [0.0; 3]
        );
        assert_eq!(
            clip.sample(&scene, -1.0, false).unwrap()[0].translation,
            [0.0; 3]
        );
        assert_eq!(clip.sample(&scene, 3.0, true).unwrap(), mid);
        assert_eq!(scene.nodes[0].transform.translation, [1000.0, 0.0, 0.0]);
    }
    #[test]
    fn rejects_duplicate_targets_bad_keys_and_singular_interpolation() {
        let base = Clip {
            name: "scale".into(),
            tracks: vec![Track {
                node: 0,
                times: vec![0.0, 1.0],
                interpolation: Interpolation::Linear,
                values: Values::Scale(vec![[1.0; 3], [2.0; 3]]),
            }],
        };
        base.validate(1).unwrap();
        for change in [
            |c: &mut Clip| c.tracks[0].node = 1,
            |c: &mut Clip| c.tracks.push(c.tracks[0].clone()),
            |c: &mut Clip| c.tracks[0].times[1] = 0.0,
            |c: &mut Clip| c.tracks[0].times[1] = f64::NAN,
            |c: &mut Clip| c.tracks[0].values = Values::Scale(vec![[1.0; 3], [-1.0; 3]]),
            |c: &mut Clip| c.tracks[0].values = Values::Rotation(vec![[0.0; 4]; 2]),
            |c: &mut Clip| c.tracks[0].times.pop().map(|_| ()).unwrap(),
        ] {
            let mut clip = base.clone();
            change(&mut clip);
            assert!(clip.validate(1).is_err());
        }
    }
}
