//! Opt-in sampled clip reduction. The original scene and clip remain unchanged.
use super::{
    animation::{Clip, Interpolation, Values},
    scene::{Scene, SceneMesh},
    skin::Skin,
};
use anyhow::{Result, ensure};
use monstertruck::modeling::{InnerSpace, Point3, Transform as _, Vector3};
use serde::Serialize;

pub struct Options {
    pub max_error_mm: f64,
    pub max_angle_degrees: f64,
    pub subdivisions: usize,
    pub max_trials: usize,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub original_keys: usize,
    pub reduced_keys: usize,
    pub trials: usize,
    pub trial_limit_reached: bool,
    pub sample_times_seconds: Vec<f64>,
    pub max_position_error_mm: f64,
    pub max_direction_error_degrees: f64,
}

pub(crate) struct Frame {
    points: Vec<Vector3>,
    directions: Vec<Vector3>,
    signs: Vec<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::animation::Track;
    fn options() -> Options {
        Options {
            max_error_mm: 0.01,
            max_angle_degrees: 0.01,
            subdivisions: 4,
            max_trials: 100,
        }
    }

    #[test]
    fn reduces_linear_motion_preserving_endpoints_steps_and_reference() {
        let (scene, _) = crate::asset::skin::tests::fixture();
        let clip = Clip {
            name: "slide".into(),
            tracks: vec![
                Track {
                    node: 1,
                    times: vec![0., 0.25, 0.5, 0.75, 1.],
                    interpolation: Interpolation::Linear,
                    values: Values::Translation(vec![
                        [0., 0., 0.],
                        [25., 0., 0.],
                        [50., 0., 0.],
                        [75., 0., 0.],
                        [100., 0., 0.],
                    ]),
                },
                Track {
                    node: 0,
                    times: vec![0., 0.5, 1.],
                    interpolation: Interpolation::Step,
                    values: Values::Scale(vec![[1.; 3], [-1., 1., 1.], [1.; 3]]),
                },
            ],
        };
        let (result, report) = reduce(&scene, &[], &clip, &options()).unwrap();
        assert_eq!(clip.tracks[0].times.len(), 5);
        assert_eq!(result.tracks[0].times, vec![0., 1.]);
        assert_eq!(result.tracks[1].times, clip.tracks[1].times);
        assert_eq!((report.original_keys, report.reduced_keys), (8, 5));
        assert!(
            report
                .sample_times_seconds
                .contains(&f64::from_bits(0.5_f64.to_bits() - 1))
        );
        let (_, limited) = reduce(
            &scene,
            &[],
            &clip,
            &Options {
                max_trials: 1,
                ..options()
            },
        )
        .unwrap();
        assert!(limited.trial_limit_reached);
        assert_eq!(limited.reduced_keys, 7);
    }

    #[test]
    fn skin_vertices_and_empty_node_orientations_prevent_unsafe_removal() {
        let (scene, weights) = crate::asset::skin::tests::fixture();
        let skin = Skin::bind(&scene, 0, vec![1, 2], weights).unwrap();
        let angle = 0.1_f64.to_radians() / 2.0;
        let clip = Clip {
            name: "bend".into(),
            tracks: vec![Track {
                node: 2,
                times: vec![0., 0.5, 1.],
                interpolation: Interpolation::Linear,
                values: Values::Rotation(vec![
                    [0., 0., 0., 1.],
                    [0., 0., angle.sin(), angle.cos()],
                    [0., 0., 0., 1.],
                ]),
            }],
        };
        let permissive_angle = Options {
            max_angle_degrees: 10.,
            ..options()
        };
        // Without the skin, this meshless node rotates about its unchanged origin.
        assert_eq!(
            reduce(&scene, &[], &clip, &permissive_angle)
                .unwrap()
                .1
                .reduced_keys,
            2
        );
        assert_eq!(
            reduce(&scene, &[skin], &clip, &permissive_angle)
                .unwrap()
                .1
                .reduced_keys,
            3
        );
        assert_eq!(
            reduce(&scene, &[], &clip, &options())
                .unwrap()
                .1
                .reduced_keys,
            3
        );
        assert!(
            reduce(
                &scene,
                &[],
                &clip,
                &Options {
                    max_error_mm: f64::NAN,
                    ..options()
                }
            )
            .is_err()
        );
    }
}

fn frame(scene: &Scene, skins: &[Skin], clip: &Clip, time: f64) -> Result<Frame> {
    let pose = clip.sample(scene, time, false)?;
    frame_pose(scene, skins, &pose)
}

pub(crate) fn frame_pose(
    scene: &Scene,
    skins: &[Skin],
    pose: &[super::scene::Transform],
) -> Result<Frame> {
    // Only the graph and mesh-index validity are needed for world transforms.
    let mut graph = Scene {
        nodes: scene.nodes.clone(),
        meshes: scene
            .meshes
            .iter()
            .map(|_| SceneMesh {
                name: String::new(),
                primitives: Vec::new(),
            })
            .collect(),
        ..Default::default()
    };
    for (node, transform) in graph.nodes.iter_mut().zip(pose) {
        node.transform = *transform;
    }
    let world = graph.world_transforms()?;
    let mut out = Frame {
        points: Vec::new(),
        directions: Vec::new(),
        signs: Vec::new(),
    };
    for (i, node) in scene.nodes.iter().enumerate() {
        let m = world[i];
        out.points.push(m.w.truncate());
        // Keep attachment-node orientations, including meshless nodes.
        for axis in [Vector3::unit_x(), Vector3::unit_y(), Vector3::unit_z()] {
            out.directions.push(m.transform_vector(axis).normalize());
        }
        use monstertruck::modeling::SquareMatrix;
        out.signs.push(m.determinant() < 0.0);
        if let Some(skin) = skins.iter().find(|s| s.mesh_node == i) {
            for primitive in skin.deform_positions(scene, &pose)? {
                out.points.extend(
                    primitive
                        .into_iter()
                        .map(|p| Vector3::new(p[0], p[1], p[2])),
                );
            }
            for basis in skin.deform_basis(scene, &pose)? {
                out.directions.extend(
                    basis
                        .normals
                        .into_iter()
                        .map(|p| Vector3::new(p[0], p[1], p[2])),
                );
                if let Some(tangents) = basis.tangents {
                    for t in tangents {
                        out.directions.push(Vector3::new(t[0], t[1], t[2]));
                        out.signs.push(t[3] < 0.0);
                    }
                }
            }
        } else if let Some(mesh) = node.mesh {
            use monstertruck::modeling::Matrix;
            let normal = m.invert().unwrap().transpose();
            for primitive in &scene.meshes[mesh].primitives {
                for p in &primitive.geometry.positions {
                    let p = m.transform_point(Point3::new(p[0], p[1], p[2]));
                    out.points.push(Vector3::new(p.x, p.y, p.z));
                }
                for p in &primitive.geometry.normals {
                    out.directions.push(
                        normal
                            .transform_vector(Vector3::new(p[0], p[1], p[2]))
                            .normalize(),
                    );
                }
                if let Some(tangents) = &primitive.geometry.tangents {
                    for t in tangents {
                        out.directions.push(
                            m.transform_vector(Vector3::new(t[0], t[1], t[2]))
                                .normalize(),
                        );
                    }
                }
            }
        }
    }
    ensure!(
        out.points
            .iter()
            .chain(&out.directions)
            .all(|p| p.x.is_finite() && p.y.is_finite() && p.z.is_finite()),
        "nonfinite reduction sample"
    );
    Ok(out)
}

pub(crate) fn error(a: &Frame, b: &Frame) -> (f64, f64) {
    if a.signs != b.signs {
        return (f64::INFINITY, f64::INFINITY);
    }
    let distance = a
        .points
        .iter()
        .zip(&b.points)
        .map(|(a, b)| (a - b).magnitude())
        .fold(0.0, f64::max);
    let angle = a
        .directions
        .iter()
        .zip(&b.directions)
        .map(|(a, b)| a.cross(*b).magnitude().atan2(a.dot(*b)).to_degrees())
        .fold(0.0, f64::max);
    (distance, angle)
}

pub fn reduce(
    scene: &Scene,
    skins: &[Skin],
    clip: &Clip,
    options: &Options,
) -> Result<(Clip, Report)> {
    scene.validate()?;
    clip.validate(scene.nodes.len())?;
    ensure!(
        options.max_error_mm.is_finite()
            && options.max_error_mm >= 0.0
            && options.max_angle_degrees.is_finite()
            && (0.0..=180.0).contains(&options.max_angle_degrees),
        "reduction tolerances must be finite and nonnegative (angle at most 180 degrees)"
    );
    ensure!(
        (1..=16).contains(&options.subdivisions) && (1..=10000).contains(&options.max_trials),
        "reduction requires 1..16 subdivisions and 1..10000 trials"
    );
    let mut bound = std::collections::HashSet::new();
    for skin in skins {
        skin.validate(scene)?;
        ensure!(bound.insert(skin.mesh_node), "duplicate skin mesh node");
    }
    let mut keys: Vec<_> = clip
        .tracks
        .iter()
        .flat_map(|t| t.times.iter().copied())
        .collect();
    keys.push(0.0);
    keys.sort_by(f64::total_cmp);
    keys.dedup();
    ensure!(
        keys.len().saturating_mul(options.subdivisions) <= 10001,
        "reduction sample count exceeds 10001"
    );
    let mut times = Vec::new();
    for pair in keys.windows(2) {
        for i in 0..options.subdivisions {
            times.push(pair[0] + (pair[1] - pair[0]) * (i as f64 / options.subdivisions as f64));
        }
    }
    times.push(*keys.last().unwrap());
    // Preserve discontinuity neighborhoods when other tracks are reduced.
    for t in &clip.tracks {
        if t.interpolation == Interpolation::Step {
            for &time in &t.times {
                if time > 0.0 {
                    times.push(f64::from_bits(time.to_bits() - 1));
                }
            }
        }
    }
    times.sort_by(f64::total_cmp);
    times.dedup();
    ensure!(times.len() <= 10001, "reduction sample count exceeds 10001");
    let mut references = Vec::new();
    let mut vectors = 0usize;
    for &time in &times {
        let f = frame(scene, skins, clip, time)?;
        vectors = vectors
            .checked_add(f.points.len() + f.directions.len())
            .ok_or_else(|| anyhow::anyhow!("reduction sample size overflow"))?;
        ensure!(
            vectors <= 10000000,
            "reduction reference samples exceed ten million vectors"
        );
        references.push(f);
    }
    let count = |c: &Clip| c.tracks.iter().map(|t| t.times.len()).sum();
    let mut result = clip.clone();
    let mut trials = 0;
    'tracks: for track in 0..result.tracks.len() {
        if result.tracks[track].interpolation == Interpolation::Step {
            continue;
        }
        let mut key = 1;
        while key + 1 < result.tracks[track].times.len() {
            if trials == options.max_trials {
                break 'tracks;
            }
            trials += 1;
            let mut candidate = result.clone();
            candidate.tracks[track].times.remove(key);
            match &mut candidate.tracks[track].values {
                Values::Translation(v) | Values::Scale(v) => {
                    v.remove(key);
                }
                Values::Rotation(v) => {
                    v.remove(key);
                }
            }
            let accepted = times.iter().zip(&references).all(|(&time, reference)| {
                frame(scene, skins, &candidate, time).is_ok_and(|f| {
                    let (p, a) = error(reference, &f);
                    p <= options.max_error_mm && a <= options.max_angle_degrees
                })
            });
            if accepted {
                result = candidate;
            } else {
                key += 1;
            }
        }
    }
    let mut position = 0.0_f64;
    let mut angle = 0.0_f64;
    for (&time, reference) in times.iter().zip(&references) {
        let (p, a) = error(reference, &frame(scene, skins, &result, time)?);
        position = position.max(p);
        angle = angle.max(a);
    }
    let report = Report {
        original_keys: count(clip),
        reduced_keys: count(&result),
        trials,
        trial_limit_reached: trials == options.max_trials,
        sample_times_seconds: times,
        max_position_error_mm: position,
        max_direction_error_degrees: angle,
    };
    Ok((result, report))
}
