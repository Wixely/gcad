//! Sampled diagnostics, not automatic quality acceptance or surface-error bounds.
use super::{
    animation::{Clip, Interpolation},
    cross, dot,
    scene::Scene,
    skin::Skin,
    sub,
};
use anyhow::{Result, ensure};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Report {
    pub version: u32,
    pub meshes: usize,
    pub nodes: usize,
    pub stored_vertices: usize,
    pub stored_triangles: usize,
    pub instanced_triangles: usize,
    pub primitive_instances: usize,
    pub materials: usize,
    /// Sum of loaded RGBA map payloads, counting each material slot separately.
    /// Excludes GPU compression, mipmaps and export channel packing.
    pub source_texture_rgba_bytes: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mask_coverage: Vec<MaterialMaskCoverage>,
    pub animation_keys: usize,
    pub skins: Vec<SkinReport>,
}

#[derive(Debug, Serialize)]
pub struct MaterialMaskCoverage {
    pub material: usize,
    pub levels: Vec<super::atlas::MaskCoverage>,
}

#[derive(Debug, Serialize)]
pub struct SkinReport {
    pub node: String,
    pub joints: usize,
    /// Number of vertices with 0, 1, 2, 3 and 4 nonzero influences.
    pub influence_histogram: [usize; 5],
    pub clips: Vec<Deformation>,
}

#[derive(Debug, Serialize)]
pub struct Deformation {
    pub clip: String,
    pub sample_times_seconds: Vec<f64>,
    pub min_area_ratio: f64,
    pub max_area_ratio: f64,
    pub min_edge_ratio: f64,
    pub max_edge_ratio: f64,
    pub max_displacement_mm: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_normal_tangent_dot: Option<f64>,
}

pub fn inspect(scene: &Scene, skins: &[Skin], samples: usize) -> Result<Report> {
    inspect_with_shading(scene, skins, samples, false)
}

/// Optional strict shading diagnostics reject singular blended transforms.
pub fn inspect_with_shading(
    scene: &Scene,
    skins: &[Skin],
    samples: usize,
    shading: bool,
) -> Result<Report> {
    ensure!(
        (2..=10001).contains(&samples),
        "samples must be between 2 and 10001"
    );
    scene.validate()?;
    let mut report = Report {
        version: 1,
        meshes: scene.meshes.len(),
        nodes: scene.nodes.len(),
        stored_vertices: 0,
        stored_triangles: 0,
        instanced_triangles: 0,
        primitive_instances: 0,
        materials: scene.materials.len(),
        source_texture_rgba_bytes: 0,
        mask_coverage: Vec::new(),
        animation_keys: 0,
        skins: Vec::new(),
    };
    for mesh in &scene.meshes {
        for p in &mesh.primitives {
            report.stored_vertices += p.geometry.positions.len();
            report.stored_triangles += p.geometry.triangles.len();
        }
    }
    for node in &scene.nodes {
        if let Some(index) = node.mesh {
            report.primitive_instances += scene.meshes[index].primitives.len();
            report.instanced_triangles += scene.meshes[index]
                .primitives
                .iter()
                .map(|p| p.geometry.triangles.len())
                .sum::<usize>();
        }
    }
    for (material, m) in scene.materials.iter().enumerate() {
        if let Some(chain) = &m.mip_chain {
            if !chain.mask_coverage().is_empty() {
                report.mask_coverage.push(MaterialMaskCoverage {
                    material,
                    levels: chain.mask_coverage().to_vec(),
                });
            }
        }
        for map in [
            &m.colour_map,
            &m.normal_map,
            &m.metallic_map,
            &m.roughness_map,
            &m.occlusion_map,
            &m.emissive_map,
        ]
        .into_iter()
        .flatten()
        {
            report.source_texture_rgba_bytes += map.rgba.len();
        }
    }
    report.animation_keys = scene
        .clips
        .iter()
        .flat_map(|c| &c.tracks)
        .map(|t| t.times.len())
        .sum();
    let rest_pose: Vec<_> = scene.nodes.iter().map(|n| n.transform).collect();
    let mut seen = std::collections::HashSet::new();
    for skin in skins {
        skin.validate(scene)?;
        ensure!(seen.insert(skin.mesh_node), "duplicate skin node");
        let mut row = SkinReport {
            node: scene.nodes[skin.mesh_node].name.clone(),
            joints: skin.joints.len(),
            influence_histogram: [0; 5],
            clips: Vec::new(),
        };
        for weights in skin.weights.iter().flatten() {
            row.influence_histogram[weights.weights.iter().filter(|&&w| w > 0.0).count()] += 1;
        }
        let rest = skin.deform_positions(scene, &rest_pose)?;
        let mesh = &scene.meshes[scene.nodes[skin.mesh_node].mesh.unwrap()];
        for clip in &scene.clips {
            let times = sample_times(clip, samples);
            let mut metrics = Deformation {
                clip: clip.name.clone(),
                sample_times_seconds: times.clone(),
                min_area_ratio: f64::INFINITY,
                max_area_ratio: 0.,
                min_edge_ratio: f64::INFINITY,
                max_edge_ratio: 0.,
                max_displacement_mm: 0.,
                max_normal_tangent_dot: None,
            };
            for time in times {
                let pose = clip.sample(scene, time, false)?;
                let positions = skin.deform_positions(scene, &pose)?;
                if shading {
                    for basis in skin.deform_basis(scene, &pose)? {
                        if let Some(tangents) = basis.tangents {
                            for (n, t) in basis.normals.iter().zip(tangents) {
                                let value = dot(*n, [t[0], t[1], t[2]]).abs();
                                metrics.max_normal_tangent_dot =
                                    Some(metrics.max_normal_tangent_dot.unwrap_or(0.).max(value));
                            }
                        }
                    }
                }
                for ((primitive, rest), posed) in mesh.primitives.iter().zip(&rest).zip(&positions)
                {
                    for (&a, &b) in rest.iter().zip(posed) {
                        metrics.max_displacement_mm =
                            metrics.max_displacement_mm.max(length(sub(b, a))?);
                    }
                    for triangle in &primitive.geometry.triangles {
                        let r = triangle.map(|i| rest[i as usize]);
                        let p = triangle.map(|i| posed[i as usize]);
                        let area = length(cross(sub(p[1], p[0]), sub(p[2], p[0])))?;
                        let original = length(cross(sub(r[1], r[0]), sub(r[2], r[0])))?;
                        let ratio = ratio(area, original)?;
                        metrics.min_area_ratio = metrics.min_area_ratio.min(ratio);
                        metrics.max_area_ratio = metrics.max_area_ratio.max(ratio);
                        for k in 0..3 {
                            let ratio = ratio_edge(p[k], p[(k + 1) % 3], r[k], r[(k + 1) % 3])?;
                            metrics.min_edge_ratio = metrics.min_edge_ratio.min(ratio);
                            metrics.max_edge_ratio = metrics.max_edge_ratio.max(ratio);
                        }
                    }
                }
            }
            row.clips.push(metrics);
        }
        report.skins.push(row);
    }
    Ok(report)
}

fn length(v: [f64; 3]) -> Result<f64> {
    let value = dot(v, v).sqrt();
    ensure!(value.is_finite(), "report geometry exceeds numeric range");
    Ok(value)
}
fn ratio(value: f64, original: f64) -> Result<f64> {
    ensure!(original > 0., "report requires nondegenerate rest geometry");
    let result = value / original;
    ensure!(result.is_finite(), "report ratio exceeds numeric range");
    Ok(result)
}
fn ratio_edge(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) -> Result<f64> {
    ratio(length(sub(a, b))?, length(sub(c, d))?)
}

pub(crate) fn sample_times(clip: &Clip, samples: usize) -> Vec<f64> {
    let duration = clip.duration();
    let mut times: Vec<_> = (0..samples)
        .map(|i| duration * (i as f64 / (samples - 1) as f64))
        .collect();
    for track in &clip.tracks {
        for &time in &track.times {
            times.push(time);
            // Sample both sides of each STEP jump, even off the regular grid.
            if track.interpolation == Interpolation::Step && time > 0. {
                times.push(f64::from_bits(time.to_bits() - 1));
            }
        }
    }
    times.sort_by(f64::total_cmp);
    times.dedup_by(|a, b| *a == *b);
    times
}

#[cfg(test)]
mod tests {
    use super::super::{
        animation::{Track, Values},
        skin::Assignment,
    };
    use super::*;
    #[test]
    fn counts_instances_and_measures_rigid_scale_with_off_grid_step() {
        let (mut scene, _) = super::super::skin::tests::fixture();
        scene.nodes.push(super::super::scene::Node {
            name: "copy".into(),
            mesh: Some(0),
            ..Default::default()
        });
        scene.clips.push(Clip {
            name: "scale".into(),
            tracks: vec![Track {
                node: 1,
                times: vec![0., 0.37, 1.],
                interpolation: Interpolation::Step,
                values: Values::Scale(vec![[1.; 3], [2.; 3], [1.; 3]]),
            }],
        });
        let skin = Skin::assign(&scene, 0, vec![1], Assignment::Rigid).unwrap();
        let report = inspect_with_shading(&scene, &[skin], 3, true).unwrap();
        assert_eq!(report.stored_triangles, 1);
        assert_eq!(report.instanced_triangles, 2);
        assert_eq!(report.skins[0].influence_histogram, [0, 3, 0, 0, 0]);
        let clip = &report.skins[0].clips[0];
        assert_eq!(clip.min_area_ratio, 1.);
        assert_eq!(clip.max_area_ratio, 4.);
        assert_eq!(clip.max_edge_ratio, 2.);
        assert_eq!(clip.max_normal_tangent_dot, Some(0.));
        assert!(clip.sample_times_seconds.contains(&0.37));
        assert!(
            clip.sample_times_seconds
                .contains(&f64::from_bits(0.37f64.to_bits() - 1))
        );
        assert!(
            serde_json::to_string(&report)
                .unwrap()
                .contains("sample_times_seconds")
        );
        assert!(inspect(&scene, &[], 1).is_err());
    }
}
