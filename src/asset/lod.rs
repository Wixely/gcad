//! Conservative endpoint-collapse LODs with explicit whole-tuple remapping.
//! Pose samples must contain the original vertex order, in millimetres.
use super::{Mesh, cross, dot, sub};
use anyhow::{Result, ensure};
use std::collections::{BTreeMap, BTreeSet};

pub struct Options {
    pub target_triangles: usize,
    pub max_error_mm: f64,
    pub max_uv_error: f64,
    pub max_angle_degrees: f64,
    pub max_trials: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct Report {
    pub original_triangles: usize,
    pub triangles: usize,
    pub trials: usize,
    /// Candidates skipped because the original vertices proving rejection have
    /// exactly the same representatives as when they were last tested.
    pub cached_rejections: usize,
    /// A rejecting gate per tested candidate; excludes cached skips.
    pub rejection_counts: BTreeMap<&'static str, usize>,
    pub target_reached: bool,
    /// Upper bound from the original-to-collapsed barycentric correspondence,
    /// across rest and supplied poses. Not a closest-surface percentile.
    pub max_correspondence_error_mm: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct SceneReport {
    pub node: String,
    pub primitive: usize,
    pub sampled_poses: usize,
    pub original_vertices: usize,
    pub vertices: usize,
    pub max_shading_angle_degrees: f64,
    pub reduction: Report,
    pub surface: Vec<SurfaceSample>,
    pub acceptance: Option<Acceptance>,
    /// Rest-pose normal transfer; geometric/pose acceptance remains separate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normal_bake: Option<super::projection::Report>,
}

#[derive(Debug, serde::Serialize)]
pub struct SurfaceSample {
    pub clip: Option<String>,
    pub time_seconds: Option<f64>,
    pub error: super::surface::Comparison,
    pub silhouettes: Vec<super::silhouette::View>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Acceptance {
    pub max_surface_mm: Option<f64>,
    pub max_surface_p95_mm: Option<f64>,
    pub max_silhouette_difference: Option<f64>,
}
impl Acceptance {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            [self.max_surface_mm, self.max_surface_p95_mm]
                .iter()
                .all(|x| x.is_none_or(|v| v.is_finite() && v >= 0.)),
            "surface limits must be finite nonnegative millimetres"
        );
        ensure!(
            self.max_silhouette_difference
                .is_none_or(|v| v.is_finite() && (0.0..=1.0).contains(&v)),
            "silhouette difference limit must be between 0 and 1"
        );
        Ok(())
    }
}
impl SceneReport {
    /// Enforce optional sampled acceptance limits before creating an export file.
    pub fn accept(&mut self, limits: Acceptance) -> Result<()> {
        limits.validate()?;
        for pose in &self.surface {
            let label = pose.clip.as_deref().unwrap_or("rest");
            let time = pose.time_seconds.unwrap_or(0.);
            for (direction, distribution) in [
                ("source-to-LOD", &pose.error.source_to_lod),
                ("LOD-to-source", &pose.error.lod_to_source),
            ] {
                if let Some(limit) = limits.max_surface_mm {
                    ensure!(
                        distribution.max_mm <= limit,
                        "LOD `{}` {label} at {time} s {direction} sampled maximum {} mm exceeds {limit} mm",
                        self.node,
                        distribution.max_mm
                    );
                }
                if let Some(limit) = limits.max_surface_p95_mm {
                    ensure!(
                        distribution.p95_mm <= limit,
                        "LOD `{}` {label} at {time} s {direction} sampled p95 {} mm exceeds {limit} mm",
                        self.node,
                        distribution.p95_mm
                    );
                }
            }
            if let Some(limit) = limits.max_silhouette_difference {
                ensure!(
                    pose.silhouettes
                        .iter()
                        .any(|v| v.difference_fraction.is_some()),
                    "LOD silhouette is unresolved in every view at {label} {time} s"
                );
                for view in &pose.silhouettes {
                    if let Some(error) = view.difference_fraction {
                        ensure!(
                            error <= limit,
                            "LOD `{}` {label} at {time} s axis {} silhouette difference {error} exceeds {limit}",
                            self.node,
                            view.axis
                        );
                    }
                }
            }
        }
        self.acceptance = Some(limits);
        Ok(())
    }
}

/// Derive one node/primitive, isolating it from shared mesh instances. Rest and
/// all clip keys/regular samples/STEP left limits are checked. Bones stay intact.
pub fn derive_node(
    scene: &super::scene::Scene,
    skins: &[super::skin::Skin],
    node: usize,
    primitive: usize,
    locked: &[usize],
    samples: usize,
    options: &Options,
) -> Result<(super::scene::Scene, Vec<super::skin::Skin>, SceneReport)> {
    use super::scene::{Scene, SceneMesh};
    use monstertruck::modeling::{
        InnerSpace, Matrix, Point3, SquareMatrix, Transform as _, Vector3,
    };
    scene.validate()?;
    ensure!(
        (2..=10001).contains(&samples),
        "LOD samples must be between 2 and 10001"
    );
    let mesh_index = scene
        .nodes
        .get(node)
        .and_then(|n| n.mesh)
        .ok_or_else(|| anyhow::anyhow!("LOD target requires a mesh node"))?;
    let mesh = scene.meshes[mesh_index]
        .primitives
        .get(primitive)
        .ok_or_else(|| anyhow::anyhow!("LOD primitive missing"))?;
    let mut seen = BTreeSet::new();
    for skin in skins {
        skin.validate(scene)?;
        ensure!(seen.insert(skin.mesh_node), "duplicate skin node");
    }
    let skin = skins.iter().find(|s| s.mesh_node == node);
    let mut poses = vec![scene.nodes.iter().map(|n| n.transform).collect::<Vec<_>>()];
    let mut pose_labels = vec![(None, None)];
    for clip in &scene.clips {
        for time in super::report::sample_times(clip, samples) {
            ensure!(
                poses.len() < 10001
                    && poses.len().saturating_mul(mesh.geometry.positions.len()) < 10000000,
                "LOD pose budget exceeded"
            );
            poses.push(clip.sample(scene, time, false)?);
            pose_labels.push((Some(clip.name.clone()), Some(time)));
        }
    }
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut tangents = Vec::new();
    for pose in &poses {
        if let Some(skin) = skin {
            positions.push(skin.deform_positions(scene, pose)?.remove(primitive));
            let basis = skin.deform_basis(scene, pose)?.remove(primitive);
            normals.push(basis.normals);
            tangents.push(basis.tangents);
        } else {
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
            for (n, t) in graph.nodes.iter_mut().zip(pose) {
                n.transform = *t;
            }
            let world = graph.world_transforms()?[node];
            let normal = world.invert().unwrap().transpose();
            positions.push(
                mesh.geometry
                    .positions
                    .iter()
                    .map(|p| {
                        let p = world.transform_point(Point3::new(p[0], p[1], p[2]));
                        [p.x, p.y, p.z]
                    })
                    .collect(),
            );
            normals.push(
                mesh.geometry
                    .normals
                    .iter()
                    .map(|p| {
                        let p = normal
                            .transform_vector(Vector3::new(p[0], p[1], p[2]))
                            .normalize();
                        [p.x, p.y, p.z]
                    })
                    .collect(),
            );
            tangents.push(mesh.geometry.tangents.as_ref().map(|ts| {
                ts.iter()
                    .map(|t| {
                        let v = world
                            .transform_vector(Vector3::new(t[0], t[1], t[2]))
                            .normalize();
                        [v.x, v.y, v.z, t[3] * world.determinant().signum()]
                    })
                    .collect()
            }));
        }
    }
    let lod = simplify_with_basis(&mesh.geometry, &positions, locked, options, true,
        Some((&normals, &tangents)))?;
    let mut angle = 0.0_f64;
    for (ns, ts) in normals.iter().zip(&tangents) {
        for (i, &j) in lod.original_to_lod.iter().enumerate() {
            let source = lod.source_vertices[j as usize];
            let measure =
                |a: [f64; 3], b: [f64; 3]| length(cross(a, b)).atan2(dot(a, b)).to_degrees();
            angle = angle.max(measure(ns[i], ns[source]));
            if let Some(ts) = ts {
                ensure!(
                    ts[i][3] == ts[source][3],
                    "LOD changes sampled tangent handedness"
                );
                angle = angle.max(measure(
                    [ts[i][0], ts[i][1], ts[i][2]],
                    [ts[source][0], ts[source][1], ts[source][2]],
                ));
            }
        }
    }
    ensure!(
        angle <= options.max_angle_degrees + 1e-9,
        "LOD sampled shading angle {angle} exceeds {} degrees; reduce position/UV budgets or increase the angle limit",
        options.max_angle_degrees
    );
    let mut derived = scene.clone();
    let mut derived_mesh = scene.meshes[mesh_index].clone();
    derived_mesh.name = format!("{}.lod", derived_mesh.name);
    derived_mesh.primitives[primitive].geometry = lod.mesh.clone();
    if scene
        .nodes
        .iter()
        .filter(|n| n.mesh == Some(mesh_index))
        .count()
        == 1
    {
        derived.meshes[mesh_index] = derived_mesh;
    } else {
        derived.nodes[node].mesh = Some(derived.meshes.len());
        derived.meshes.push(derived_mesh);
    }
    let mut derived_skins = Vec::new();
    for source in skins {
        if source.mesh_node == node {
            let mut weights = source.weights.clone();
            weights[primitive] = lod.remap(&weights[primitive])?;
            derived_skins.push(super::skin::Skin::bind(
                &derived,
                node,
                source.joints.clone(),
                weights,
            )?);
        } else {
            source.validate(&derived)?;
            derived_skins.push(source.clone());
        }
    }
    derived.validate()?;
    let mut surface = Vec::new();
    for (pose, (clip, time_seconds)) in positions.iter().zip(pose_labels) {
        let target = lod.remap(pose)?;
        let error =
            super::surface::compare(pose, &mesh.geometry.triangles, &target, &lod.mesh.triangles)?;
        let silhouettes = super::silhouette::compare(
            pose,
            &mesh.geometry.triangles,
            &target,
            &lod.mesh.triangles,
            256,
        )?;
        surface.push(SurfaceSample {
            clip,
            time_seconds,
            error,
            silhouettes,
        });
    }
    let report = SceneReport {
        node: scene.nodes[node].name.clone(),
        primitive,
        sampled_poses: poses.len(),
        original_vertices: mesh.geometry.positions.len(),
        vertices: lod.mesh.positions.len(),
        max_shading_angle_degrees: angle,
        reduction: lod.report,
        surface,
        acceptance: None,
        normal_bake: None,
    };
    Ok((derived, derived_skins, report))
}

pub struct Lod {
    /// Original triangle index for each surviving output triangle, in output order.
    pub source_triangles: Vec<usize>,
    pub mesh: Mesh,
    /// Each output tuple (including its skin weights) comes from this original index.
    pub source_vertices: Vec<usize>,
    /// Map every original vertex to its retained representative in the output.
    pub original_to_lod: Vec<u32>,
    pub report: Report,
}

impl Lod {
    /// Remap weights or another per-original-vertex payload with exactly the same
    /// retained tuple indices as the mesh. Never average or renormalise implicitly.
    pub fn remap<T: Clone>(&self, values: &[T]) -> Result<Vec<T>> {
        ensure!(
            values.len() == self.original_to_lod.len(),
            "LOD payload must match original vertex count"
        );
        Ok(self
            .source_vertices
            .iter()
            .map(|&i| values[i].clone())
            .collect())
    }
}

fn length(v: [f64; 3]) -> f64 {
    dot(v, v).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::{
        scene::{Node, Primitive, Scene, SceneMesh, Transform},
        skin::{Skin, Weights},
    };
    fn grid() -> Mesh {
        let mut mesh = Mesh {
            positions: Vec::new(),
            normals: vec![[0., 0., 1.]; 49],
            texcoords: Some(Vec::new()),
            tangents: Some(vec![[1., 0., 0., 1.]; 49]),
            triangles: Vec::new(),
        };
        for y in 0..7 {
            for x in 0..7 {
                mesh.positions.push([x as f64, y as f64, 0.]);
                mesh.texcoords
                    .as_mut()
                    .unwrap()
                    .push([x as f64 / 6., y as f64 / 6.]);
            }
        }
        for y in 0..6 {
            for x in 0..6 {
                let a = y * 7 + x;
                mesh.triangles
                    .extend([[a, a + 1, a + 8], [a, a + 8, a + 7]]);
            }
        }
        mesh
    }
    fn options() -> Options {
        Options {
            target_triangles: 60,
            max_error_mm: 1.01,
            max_uv_error: 0.17,
            max_angle_degrees: 20.,
            max_trials: 1000,
        }
    }

    #[test]
    fn sampled_acceptance_rejects_each_limit_before_recording_success() {
        let make = || SceneReport {
            node: "test".into(),
            primitive: 0,
            sampled_poses: 1,
            original_vertices: 3,
            vertices: 3,
            max_shading_angle_degrees: 0.,
            reduction: Report {
                original_triangles: 1,
                triangles: 1,
                trials: 0,
                cached_rejections: 0,
                rejection_counts: BTreeMap::new(),
                target_reached: true,
                max_correspondence_error_mm: 0.25,
            },
            acceptance: None,
            normal_bake: None,
            surface: vec![SurfaceSample {
                clip: Some("bend".into()),
                time_seconds: Some(0.5),
                error: super::super::surface::Comparison {
                    source_to_lod: super::super::surface::Distribution {
                        samples: 100,
                        max_mm: 0.25,
                        p95_mm: 0.1,
                        rms_mm: 0.05,
                    },
                    lod_to_source: super::super::surface::Distribution {
                        samples: 100,
                        max_mm: 0.2,
                        p95_mm: 0.1,
                        rms_mm: 0.05,
                    },
                },
                silhouettes: vec![super::super::silhouette::View {
                    axis: 2,
                    resolution: 256,
                    mm_per_pixel: 1.,
                    source_pixels: 100,
                    lod_pixels: 90,
                    differing_pixels: 10,
                    union_pixels: 100,
                    difference_fraction: Some(0.1),
                }],
            }],
        };
        let good = Acceptance {
            max_surface_mm: Some(0.3),
            max_surface_p95_mm: Some(0.2),
            max_silhouette_difference: Some(0.2),
        };
        let mut passed = make();
        passed.accept(good.clone()).unwrap();
        assert!(passed.acceptance.is_some());
        for limits in [
            Acceptance {
                max_surface_mm: Some(0.2),
                ..good.clone()
            },
            Acceptance {
                max_surface_p95_mm: Some(0.05),
                ..good.clone()
            },
            Acceptance {
                max_silhouette_difference: Some(0.05),
                ..good.clone()
            },
        ] {
            let mut report = make();
            let error = report.accept(limits).unwrap_err().to_string();
            assert!(error.contains("bend") && error.contains("exceeds"));
            assert!(report.acceptance.is_none());
        }
        assert!(
            Acceptance {
                max_surface_mm: Some(f64::NAN),
                ..good.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            Acceptance {
                max_silhouette_difference: Some(1.1),
                ..good
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn preserves_boundaries_attachments_attributes_and_cumulative_pose_budget() {
        let mesh = grid();
        let before = mesh.clone();
        let lod = simplify(&mesh, &[], &[24], &options()).unwrap();
        assert!(lod.mesh.triangles.len() < mesh.triangles.len());
        assert!(lod.report.max_correspondence_error_mm <= 1.01);
        for i in 0..49 {
            if i % 7 == 0 || i % 7 == 6 || i / 7 == 0 || i / 7 == 6 || i == 24 {
                assert_eq!(lod.source_vertices[lod.original_to_lod[i] as usize], i);
            }
        }
        assert_eq!(
            lod.mesh.texcoords,
            Some(lod.remap(mesh.texcoords.as_ref().unwrap()).unwrap())
        );
        assert_eq!(
            lod.mesh.tangents,
            Some(lod.remap(mesh.tangents.as_ref().unwrap()).unwrap())
        );
        assert_eq!(mesh, before);
        let stretched: Vec<_> = mesh.positions.iter().map(|p| p.map(|x| x * 10.)).collect();
        let constrained = simplify(&mesh, &[stretched], &[], &options()).unwrap();
        assert_eq!(constrained.mesh.triangles.len(), mesh.triangles.len());
        assert!(!constrained.report.target_reached);
        let no_uv = simplify(
            &mesh,
            &[],
            &[],
            &Options {
                max_uv_error: 0.,
                ..options()
            },
        )
        .unwrap();
        assert_eq!(no_uv.mesh.triangles.len(), mesh.triangles.len());
        let mut invalid = mesh.clone();
        invalid.triangles.push(invalid.triangles[0]);
        assert!(simplify(&invalid, &[], &[], &options()).is_err());
        assert!(simplify(&mesh, &[vec![[0.; 3]; 1]], &[], &options()).is_err());
        assert!(lod.remap(&[1, 2]).is_err());
        assert!(topology(&[[0, 1, 2], [0, 3, 4]]).is_err());
    }

    #[test]
    fn posed_basis_rejects_collapses_before_publication() {
        let mesh=grid();
        let options=Options {target_triangles:40,max_angle_degrees:0.01,..options()};
        let poses=vec![mesh.positions.clone()];
        assert!(simplify(&mesh,&poses,&[],&options).unwrap().report.triangles<mesh.triangles.len());
        for tangent in [false,true] {
            let normals=vec![mesh.positions.iter().enumerate().map(|(i,_)| {
                let a=i as f64*0.01;
                if tangent {[0.,0.,1.]} else {[a.sin(),0.,a.cos()]}
            }).collect::<Vec<_>>()];
            let tangents=vec![tangent.then(||mesh.positions.iter().enumerate().map(|(i,_)| {
                let a=i as f64*0.01;[a.cos(),a.sin(),0.,1.]
            }).collect::<Vec<_>>())];
            let a=simplify_with_basis(&mesh,&poses,&[],&options,true,Some((&normals,&tangents))).unwrap();
            let b=simplify_with_basis(&mesh,&poses,&[],&options,false,Some((&normals,&tangents))).unwrap();
            assert_eq!(a.mesh,b.mesh);
            assert_eq!(a.report.triangles,mesh.triangles.len());
            assert!(a.report.rejection_counts[if tangent {"posed_tangent"} else {"posed_normal"}]>0);
        }
    }

    #[test]
    fn rejection_witness_cache_matches_uncached_search() {
        let mesh = grid();
        let pose: Vec<_> = mesh
            .positions
            .iter()
            .map(|p| [p[0], p[1], 0.04 * p[0] * p[1]])
            .collect();
        let mut skips = 0;
        for budget in [0.9, 1.01, 1.5, 2.1] {
            let options = Options {
                target_triangles: 40,
                max_error_mm: budget,
                max_uv_error: 0.4,
                max_trials: 100000,
                ..options()
            };
            let cached = simplify_impl(&mesh, &[pose.clone()], &[24], &options, true).unwrap();
            let reference = simplify_impl(&mesh, &[pose.clone()], &[24], &options, false).unwrap();
            assert_eq!(cached.mesh, reference.mesh);
            assert_eq!(cached.source_vertices, reference.source_vertices);
            assert_eq!(cached.original_to_lod, reference.original_to_lod);
            assert_eq!(
                cached.report.max_correspondence_error_mm,
                reference.report.max_correspondence_error_mm
            );
            assert!(cached.report.trials <= reference.report.trials);
            for result in [&cached, &reference] {
                let accepted=(result.report.original_triangles-result.report.triangles)/2;
                assert_eq!(result.report.rejection_counts.values().sum::<usize>()+accepted,result.report.trials);
            }
            skips += cached.report.cached_rejections;
        }
        assert!(skips > 0, "fixture must exercise cached failures");
    }

    #[test]
    fn remapped_skin_preserves_retained_deformation_and_rejects_stale_binding() {
        let mesh = grid();
        let weights: Vec<_> = mesh
            .positions
            .iter()
            .map(|p| {
                let w = p[1] / 6.;
                Weights {
                    joints: [0, 1, 0, 0],
                    weights: [1. - w, w, 0., 0.],
                }
            })
            .collect();
        let mut scene = Scene {
            nodes: vec![
                Node {
                    name: "mesh".into(),
                    mesh: Some(0),
                    ..Default::default()
                },
                Node {
                    name: "base".into(),
                    ..Default::default()
                },
                Node {
                    name: "tip".into(),
                    parent: Some(1),
                    transform: Transform {
                        translation: [0., 3., 0.],
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            meshes: vec![SceneMesh {
                name: "grid".into(),
                primitives: vec![Primitive {
                    geometry: mesh.clone(),
                    material: 0,
                }],
            }],
            materials: vec![Default::default()],
            ..Default::default()
        };
        scene.nodes.push(Node {
            name: "copy".into(),
            mesh: Some(0),
            transform: Transform {
                translation: [10., 0., 0.],
                ..Default::default()
            },
            ..Default::default()
        });
        scene.clips.push(crate::asset::animation::Clip {
            name: "bend".into(),
            tracks: vec![crate::asset::animation::Track {
                node: 2,
                times: vec![0., 1.],
                interpolation: crate::asset::animation::Interpolation::Linear,
                values: crate::asset::animation::Values::Rotation(vec![
                    [0., 0., 0., 1.],
                    [0.3_f64.sin(), 0., 0., 0.3_f64.cos()],
                ]),
            }],
        });
        let skin = Skin::bind(&scene, 0, vec![1, 2], vec![weights.clone()]).unwrap();
        let (mut derived, bound, mut report) =
            derive_node(&scene, &[skin.clone()], 0, 0, &[24], 5, &options()).unwrap();
        assert_eq!(report.sampled_poses, 6);
        assert!(report.reduction.triangles < 72);
        assert_ne!(derived.nodes[0].mesh, derived.nodes[3].mesh);
        assert_eq!(
            derived.meshes[derived.nodes[3].mesh.unwrap()].primitives[0].geometry,
            mesh
        );
        assert_eq!(bound[0].joints, skin.joints);
        assert_eq!(derived.nodes.len(), scene.nodes.len());
        assert_eq!(
            derived.clips[0].tracks[0].times,
            scene.clips[0].tracks[0].times
        );
        assert!(crate::asset::glb::encode_skinned_scene(&derived, &bound).is_ok());
        report.normal_bake = Some(
            crate::asset::projection::apply_to_scene(
                &mut derived,
                0,
                0,
                &scene,
                0,
                0,
                crate::asset::projection::Options {
                    width: 32,
                    height: 32,
                    max_distance_mm: 0.1,
                    min_normal_dot: 0.9,
                    max_missed_fraction: 0.,
                },
                2,
            )
            .unwrap(),
        );
        assert_eq!(report.normal_bake.as_ref().unwrap().hit_pixels, 1024);
        assert_eq!(report.normal_bake.as_ref().unwrap().missed_pixels, 0);
        bound[0].validate(&derived).unwrap();
        assert!(derived.materials[0].normal_map.is_none());
        assert!(derived.materials.last().unwrap().normal_map.is_some());
        assert!(crate::asset::glb::encode_skinned_scene(&derived, &bound).is_ok());
        assert_eq!(
            serde_json::to_value(&report).unwrap()["normal_bake"]["missed_pixels"],
            0
        );
        let mut poses = Vec::new();
        let mut samples = Vec::new();
        for angle in [0.0_f64, 0.3, 0.6] {
            let mut pose: Vec<_> = scene.nodes.iter().map(|n| n.transform).collect();
            pose[2].rotation = [(angle / 2.).sin(), 0., 0., (angle / 2.).cos()];
            samples.push(skin.deform_positions(&scene, &pose).unwrap().remove(0));
            poses.push(pose);
        }
        let lod = simplify(&mesh, &samples, &[], &options()).unwrap();
        assert!(lod.mesh.triangles.len() < mesh.triangles.len());
        scene.meshes[0].primitives[0].geometry = lod.mesh.clone();
        assert!(skin.validate(&scene).is_err());
        let rebound =
            Skin::bind(&scene, 0, vec![1, 2], vec![lod.remap(&weights).unwrap()]).unwrap();
        for (pose, reference) in poses.iter().zip(&samples) {
            let points = rebound.deform_positions(&scene, pose).unwrap().remove(0);
            for (point, &source) in points.iter().zip(&lod.source_vertices) {
                assert!(length(sub(*point, reference[source])) < 1e-12);
            }
            for (i, &j) in lod.original_to_lod.iter().enumerate() {
                assert!(length(sub(reference[i], points[j as usize])) <= options().max_error_mm);
            }
        }
    }
}
fn cosine(a: [f64; 3], b: [f64; 3]) -> f64 {
    dot(a, b) / (length(a) * length(b))
}
fn edge(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

fn topology(
    triangles: &[[u32; 3]],
) -> Result<(BTreeMap<(u32, u32), Vec<u32>>, Vec<BTreeSet<u32>>)> {
    let n = triangles
        .iter()
        .flatten()
        .max()
        .map_or(0, |i| *i as usize + 1);
    let mut edges = BTreeMap::<_, Vec<_>>::new();
    let mut neighbours = vec![BTreeSet::new(); n];
    let mut links = vec![BTreeMap::<u32, BTreeSet<u32>>::new(); n];
    let mut faces = BTreeSet::new();
    let mut directed = BTreeSet::new();
    for &[a, b, c] in triangles {
        let mut face = [a, b, c];
        face.sort();
        ensure!(faces.insert(face), "LOD requires unique triangles");
        for (a, b, c) in [(a, b, c), (b, c, a), (c, a, b)] {
            ensure!(
                directed.insert((a, b)),
                "LOD requires consistently oriented manifold edges"
            );
            edges.entry(edge(a, b)).or_default().push(c);
            neighbours[a as usize].insert(b);
            neighbours[b as usize].insert(a);
            links[a as usize].entry(b).or_default().insert(c);
            links[a as usize].entry(c).or_default().insert(b);
        }
    }
    ensure!(
        edges.values().all(|v| v.len() <= 2),
        "LOD requires manifold edges"
    );
    for link in links.iter().filter(|l| !l.is_empty()) {
        let ends = link.values().filter(|v| v.len() == 1).count();
        ensure!(
            (ends == 0 || ends == 2) && link.values().all(|v| v.len() <= 2),
            "LOD requires manifold vertex fans"
        );
        let mut pending = vec![*link.keys().next().unwrap()];
        let mut reached = BTreeSet::new();
        while let Some(i) = pending.pop() {
            if reached.insert(i) {
                pending.extend(link[&i].iter().copied());
            }
        }
        ensure!(
            reached.len() == link.len(),
            "LOD requires connected vertex fans"
        );
    }
    Ok((edges, neighbours))
}

pub fn simplify(
    mesh: &Mesh,
    poses: &[Vec<[f64; 3]>],
    locked_vertices: &[usize],
    options: &Options,
) -> Result<Lod> {
    simplify_impl(mesh, poses, locked_vertices, options, true)
}

fn simplify_impl(
    mesh: &Mesh,
    poses: &[Vec<[f64; 3]>],
    locked_vertices: &[usize],
    options: &Options,
    cache_enabled: bool,
) -> Result<Lod> {
    simplify_with_basis(mesh, poses, locked_vertices, options, cache_enabled, None)
}

type SampledBasis<'a> = (&'a [Vec<[f64; 3]>], &'a [Option<Vec<[f64; 4]>>]);

fn simplify_with_basis(
    mesh: &Mesh,
    poses: &[Vec<[f64; 3]>],
    locked_vertices: &[usize],
    options: &Options,
    cache_enabled: bool,
    basis: Option<SampledBasis<'_>>,
) -> Result<Lod> {
    mesh.validate()?;
    if let Some((normals, tangents)) = basis {
        ensure!(normals.len()==poses.len() && tangents.len()==poses.len()
            && normals.iter().all(|n| n.len()==mesh.positions.len())
            && tangents.iter().all(|t| t.as_ref().is_none_or(|t| t.len()==mesh.positions.len())),
            "LOD sampled basis must preserve pose and vertex order");
    }
    ensure!(
        options.target_triangles > 0 && options.target_triangles <= mesh.triangles.len(),
        "LOD target must be positive and no larger than the source"
    );
    ensure!(
        options.max_error_mm.is_finite()
            && options.max_error_mm >= 0.0
            && options.max_uv_error.is_finite()
            && options.max_uv_error >= 0.0
            && options.max_angle_degrees.is_finite()
            && (0.0..90.0).contains(&options.max_angle_degrees)
            && (1..=100000).contains(&options.max_trials),
        "invalid LOD error or work budgets"
    );
    ensure!(
        poses
            .iter()
            .all(|p| p.len() == mesh.positions.len() && p.iter().flatten().all(|v| v.is_finite())),
        "LOD poses must preserve original vertex order and finite millimetre coordinates"
    );
    ensure!(
        locked_vertices.iter().all(|&i| i < mesh.positions.len()),
        "LOD attachment vertex is out of range"
    );
    let samples: Vec<_> = std::iter::once(&mesh.positions)
        .chain(poses.iter())
        .collect();
    let (initial_edges, _) = topology(&mesh.triangles)?;
    let used: BTreeSet<_> = mesh.triangles.iter().flatten().copied().collect();
    ensure!(
        used.len() == mesh.positions.len(),
        "LOD requires every source vertex to be referenced"
    );
    let mut locked: BTreeSet<usize> = locked_vertices.iter().copied().collect();
    // Attribute discontinuities represented by split tuples are chart boundaries.
    for (&(a, b), faces) in &initial_edges {
        if faces.len() == 1 {
            locked.insert(a as usize);
            locked.insert(b as usize);
        }
        if faces.len() == 2 {
            let p = &mesh.positions;
            let n1 = cross(
                sub(p[b as usize], p[a as usize]),
                sub(p[faces[0] as usize], p[a as usize]),
            );
            let n2 = cross(
                sub(p[faces[1] as usize], p[a as usize]),
                sub(p[b as usize], p[a as usize]),
            );
            if cosine(n1, n2) < options.max_angle_degrees.to_radians().cos() - 1e-12 {
                locked.insert(a as usize);
                locked.insert(b as usize);
            }
        }
    }
    for pose in &samples {
        for &[a, b, c] in &mesh.triangles {
            let area = length(cross(
                sub(pose[b as usize], pose[a as usize]),
                sub(pose[c as usize], pose[a as usize]),
            ));
            ensure!(
                area.is_finite() && area > 1e-15,
                "LOD source pose has a degenerate or nonfinite triangle"
            );
        }
    }
    let threshold = options.max_angle_degrees.to_radians().cos();
    let mut representatives: Vec<_> = (0..mesh.positions.len() as u32).collect();
    let mut triangles = mesh.triangles.clone();
    let mut trials = 0;
    let mut cached_rejections = 0;
    let mut rejection_counts = BTreeMap::new();
    // Only an accepted mapping proves the unchanged vertex/triangle checks.
    // Keep the first full pass: source UV validity need not imply this gate.
    let mut mapping_validated = false;
    // A witness is an original vertex or triangle whose mapped attributes/pose
    // failed. The failure remains valid only while all its representatives match.
    // Topology failures are deliberately not cached: their dependency is wider.
    let mut rejected = BTreeMap::<(u32, u32), Vec<(usize, u32)>>::new();
    while triangles.len() > options.target_triangles && trials < options.max_trials {
        let (edges, neighbours) = topology(&triangles)?;
        let mut candidates: Vec<_> = edges
            .iter()
            .filter(|((a, b), faces)| {
                faces.len() == 2
                    && !locked.contains(&(*a as usize))
                    && !locked.contains(&(*b as usize))
            })
            .map(|(&(a, b), _)| {
                (
                    length(sub(mesh.positions[a as usize], mesh.positions[b as usize])),
                    a,
                    b,
                )
            })
            .collect();
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let mut accepted = false;
        for (_, keep, remove) in candidates {
            if cache_enabled
                && rejected
                    .get(&(keep, remove))
                    .is_some_and(|w| w.iter().all(|&(i, r)| representatives[i] == r))
            {
                cached_rejections += 1;
                continue;
            }
            if trials == options.max_trials {
                break;
            }
            trials += 1;
            if triangles.len() - 2 < options.target_triangles {
                *rejection_counts.entry("target_floor").or_insert(0) += 1;
                continue;
            }
            let common: BTreeSet<_> = neighbours[keep as usize]
                .intersection(&neighbours[remove as usize])
                .copied()
                .collect();
            if common != edges[&edge(keep, remove)].iter().copied().collect() {
                *rejection_counts.entry("topology_link").or_insert(0) += 1;
                continue;
            }
            let mut map = representatives.clone();
            for representative in &mut map {
                if *representative == remove {
                    *representative = keep;
                }
            }
            // Global error checks always use the original, not the previous LOD.
            let mut failed_vertex = 0;
            let mut reason = "position";
            let valid = map.iter().enumerate().all(|(i, &j)| {
                if cache_enabled && mapping_validated && j == representatives[i] {
                    return true;
                }
                failed_vertex = i;
                let j = j as usize;
                if samples
                    .iter()
                    .any(|p| length(sub(p[i], p[j])) > options.max_error_mm)
                {
                    return false;
                }
                if cosine(mesh.normals[i], mesh.normals[j]) < threshold - 1e-12 {
                    reason = "normal";
                    return false;
                }
                if let Some(uv) = &mesh.texcoords {
                    let d = [uv[i][0] - uv[j][0], uv[i][1] - uv[j][1]];
                    if d[0].hypot(d[1]) > options.max_uv_error {
                        reason = "uv";
                        return false;
                    }
                }
                if let Some(t) = &mesh.tangents {
                    if t[i][3] != t[j][3]
                        || cosine([t[i][0], t[i][1], t[i][2]], [t[j][0], t[j][1], t[j][2]])
                            < threshold - 1e-12
                    {
                        reason = "tangent";
                        return false;
                    }
                }
                if let Some((normals, tangents)) = basis {
                    let angle=|a,b| length(cross(a,b)).atan2(dot(a,b)).to_degrees();
                    for (ns,ts) in normals.iter().zip(tangents) {
                        if angle(ns[i],ns[j])>options.max_angle_degrees+1e-9 {
                            reason="posed_normal";return false;
                        }
                        if let Some(ts)=ts {
                            if ts[i][3]!=ts[j][3] || angle([ts[i][0],ts[i][1],ts[i][2]],[ts[j][0],ts[j][1],ts[j][2]])>options.max_angle_degrees+1e-9 {
                                reason="posed_tangent";return false;
                            }
                        }
                    }
                }
                true
            });
            if !valid {
                *rejection_counts.entry(reason).or_insert(0) += 1;
                if cache_enabled {
                    rejected.insert(
                        (keep, remove),
                        vec![(failed_vertex, representatives[failed_vertex])],
                    );
                }
                continue;
            }
            let mut next = Vec::new();
            let mut valid = true;
            for &[a, b, c] in &mesh.triangles {
                let [x, y, z] = [map[a as usize], map[b as usize], map[c as usize]];
                if x == y || y == z || z == x {
                    continue;
                }
                if cache_enabled && mapping_validated
                    && [x,y,z] == [representatives[a as usize],representatives[b as usize],representatives[c as usize]]
                {
                    next.push([x,y,z]);
                    continue;
                }
                for p in &samples {
                    let before = cross(
                        sub(p[b as usize], p[a as usize]),
                        sub(p[c as usize], p[a as usize]),
                    );
                    let after = cross(
                        sub(p[y as usize], p[x as usize]),
                        sub(p[z as usize], p[x as usize]),
                    );
                    if !length(after).is_finite()
                        || length(after) <= 1e-15
                        || cosine(before, after) < threshold - 1e-12
                    {
                        valid = false;
                        reason = "posed_triangle";
                        break;
                    }
                }
                // UV triangle orientation is independent of the 3D pose.
                if let Some(uv) = &mesh.texcoords {
                    let area = |a: u32, b: u32, c: u32| {
                        let [a, b, c] = [uv[a as usize], uv[b as usize], uv[c as usize]];
                        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
                    };
                    if area(a, b, c) * area(x, y, z) <= 0.0 {
                        valid = false;
                        reason = "uv_orientation";
                    }
                }
                if !valid {
                    if cache_enabled {
                        rejected.insert(
                            (keep, remove),
                            [a, b, c]
                                .map(|i| (i as usize, representatives[i as usize]))
                                .to_vec(),
                        );
                    }
                    break;
                }
                next.push([x, y, z]);
            }
            if !valid {
                *rejection_counts.entry(reason).or_insert(0) += 1;
                continue;
            }
            if topology(&next).is_err() {
                *rejection_counts.entry("topology_result").or_insert(0) += 1;
                continue;
            }
            representatives = map;
            mapping_validated = true;
            triangles = next;
            accepted = true;
            break;
        }
        if !accepted {
            break;
        }
    }
    let used: BTreeSet<_> = triangles.iter().flatten().copied().collect();
    // A valid edge collapse must not strand a mapped point off the remaining surface.
    ensure!(
        representatives.iter().all(|i| used.contains(i)),
        "LOD collapse stranded a vertex representative"
    );
    let source_vertices: Vec<_> = used.into_iter().map(|i| i as usize).collect();
    let mut packed = vec![0u32; mesh.positions.len()];
    for (i, &source) in source_vertices.iter().enumerate() {
        packed[source] = u32::try_from(i)?;
    }
    let output = Mesh {
        positions: source_vertices.iter().map(|&i| mesh.positions[i]).collect(),
        normals: source_vertices.iter().map(|&i| mesh.normals[i]).collect(),
        texcoords: mesh
            .texcoords
            .as_ref()
            .map(|a| source_vertices.iter().map(|&i| a[i]).collect()),
        tangents: mesh
            .tangents
            .as_ref()
            .map(|a| source_vertices.iter().map(|&i| a[i]).collect()),
        triangles: triangles
            .iter()
            .map(|t| t.map(|i| packed[i as usize]))
            .collect(),
    };
    output.validate()?;
    let error = samples
        .iter()
        .flat_map(|p| {
            representatives
                .iter()
                .enumerate()
                .map(|(i, &j)| length(sub(p[i], p[j as usize])))
        })
        .fold(0.0, f64::max);
    let report = Report {
        original_triangles: mesh.triangles.len(),
        triangles: output.triangles.len(),
        trials,
        cached_rejections,
        rejection_counts,
        target_reached: output.triangles.len() <= options.target_triangles,
        max_correspondence_error_mm: error,
    };
    let source_triangles: Vec<_> = mesh
        .triangles
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            let [a, b, c] = t.map(|v| representatives[v as usize]);
            (a != b && b != c && c != a).then_some(i)
        })
        .collect();
    ensure!(
        source_triangles.len() == output.triangles.len(),
        "LOD triangle lineage count changed"
    );
    ensure!(
        source_triangles
            .iter()
            .zip(&output.triangles)
            .all(|(&i, t)| {
                mesh.triangles[i].map(|v| packed[representatives[v as usize] as usize]) == *t
            }),
        "LOD triangle lineage order changed"
    );
    Ok(Lod {
        source_triangles,
        mesh: output,
        source_vertices,
        original_to_lod: representatives
            .iter()
            .map(|&i| packed[i as usize])
            .collect(),
        report,
    })
}
