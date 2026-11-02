//! CPU high-to-low tangent-space normal baking in one shared millimetre frame.
//! Pixel-centre samples, no supersampling, cage or material-map transfer.
use super::{Mesh, cross, dot, sub};
use crate::model::texture::Map;
use anyhow::{Result, ensure};
type P = [f64; 3];

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub width: u32,
    pub height: u32,
    /// Search along both signs of the low surface's interpolated normal.
    pub max_distance_mm: f64,
    /// Reject the nearest surface if its interpolated normal faces too far away.
    pub min_normal_dot: f64,
    pub max_missed_fraction: f64,
}
#[derive(Debug, serde::Serialize)]
pub struct Report {
    pub covered_pixels: usize,
    pub hit_pixels: usize,
    pub missed_pixels: usize,
    pub max_hit_distance_mm: f64,
    pub padded_pixels: usize,
}
#[derive(Debug)]
pub struct Bake {
    pub normal_map: Map,
    /// 0 outside charts, 1 missed/rejected, 2 hit, 3 padded outside a chart.
    pub coverage: Vec<u8>,
    pub report: Report,
}
impl Bake {
    /// Deterministic four-neighbour dilation into empty UV space only. Missed
    /// projections stay visible in the mask and are never repaired by padding.
    pub fn padded(mut self, radius: u8) -> Result<Self> {
        ensure!(radius <= 32, "normal padding must be 0..32 pixels");
        let (w, h) = (self.normal_map.width, self.normal_map.height);
        ensure!(
            w > 0
                && h > 0
                && w.checked_mul(h) == Some(self.coverage.len())
                && self.normal_map.rgba.len() == self.coverage.len() * 4,
            "invalid bake image/mask"
        );
        ensure!(
            self.coverage.iter().all(|&v| v <= 2),
            "normal bake has already been padded or has invalid coverage"
        );
        if radius == 0 {
            return Ok(self);
        }
        let neighbours = |i: usize| {
            [
                i.checked_sub(w),
                if i / w + 1 < h { Some(i + w) } else { None },
                if i % w > 0 { Some(i - 1) } else { None },
                if i % w + 1 < w { Some(i + 1) } else { None },
            ]
        };
        let mut queue = std::collections::VecDeque::new();
        for i in 0..self.coverage.len() {
            if self.coverage[i] == 2
                && neighbours(i)
                    .into_iter()
                    .flatten()
                    .any(|j| self.coverage[j] == 0)
            {
                queue.push_back((i, 0_u8));
            }
        }
        while let Some((i, distance)) = queue.pop_front() {
            if distance == radius {
                continue;
            }
            for j in neighbours(i).into_iter().flatten() {
                if self.coverage[j] != 0 {
                    continue;
                }
                let pixel: [u8; 4] = self.normal_map.rgba[i * 4..i * 4 + 4].try_into().unwrap();
                self.normal_map.rgba[j * 4..j * 4 + 4].copy_from_slice(&pixel);
                self.coverage[j] = 3;
                self.report.padded_pixels += 1;
                queue.push_back((j, distance + 1));
            }
        }
        Ok(self)
    }
}

/// Project a source primitive into one target primitive's rest-local frame.
/// Isolates shared target meshes/materials; geometry and skin bindings stay intact.
pub fn apply_to_scene(
    target: &mut super::scene::Scene,
    target_node: usize,
    target_primitive: usize,
    source: &super::scene::Scene,
    source_node: usize,
    source_primitive: usize,
    options: Options,
    padding: u8,
) -> Result<Report> {
    use super::material::{NormalConvention, Wrap};
    use monstertruck::modeling::{Matrix as _, Point3, SquareMatrix, Transform as _, Vector3};
    target.validate()?;
    source.validate()?;
    let target_mesh = target
        .nodes
        .get(target_node)
        .and_then(|n| n.mesh)
        .ok_or_else(|| anyhow::anyhow!("normal bake target node has no mesh"))?;
    let source_mesh = source
        .nodes
        .get(source_node)
        .and_then(|n| n.mesh)
        .ok_or_else(|| anyhow::anyhow!("normal bake source node has no mesh"))?;
    let low = target.meshes[target_mesh]
        .primitives
        .get(target_primitive)
        .ok_or_else(|| anyhow::anyhow!("normal bake target primitive missing"))?;
    let high = source.meshes[source_mesh]
        .primitives
        .get(source_primitive)
        .ok_or_else(|| anyhow::anyhow!("normal bake source primitive missing"))?;
    let mut material = target.materials[low.material].clone();
    let other_maps = [
        &material.colour_map,
        &material.roughness_map,
        &material.metallic_map,
        &material.occlusion_map,
        &material.emissive_map,
    ];
    ensure!(
        !other_maps.iter().any(|m| m.is_some())
            || (material.sampler.wrap_u == Wrap::Clamp && material.sampler.wrap_v == Wrap::Clamp),
        "normal bake requires existing colour/scalar maps to use clamp sampling; atlas them first"
    );
    let target_world = target.world_transforms()?[target_node];
    let relative = target_world
        .invert()
        .ok_or_else(|| anyhow::anyhow!("normal bake target transform is singular"))?
        * source.world_transforms()?[source_node];
    let normal_matrix = relative
        .invert()
        .ok_or_else(|| anyhow::anyhow!("normal bake source transform is singular"))?
        .transpose();
    let mut high = high.geometry.clone();
    high.tangents = None;
    for p in &mut high.positions {
        let v = relative.transform_point(Point3::from(*p));
        *p = [v.x, v.y, v.z];
    }
    for n in &mut high.normals {
        let v = normal_matrix.transform_vector(Vector3::from(*n));
        *n = unit([v.x, v.y, v.z])?;
    }
    let bake = normals(&low.geometry, &high, options)?.padded(padding)?;
    material.normal_map = Some(bake.normal_map);
    material.normal_convention = NormalConvention::PositiveY;
    material.normal_scale = 1.;
    material.sampler.wrap_u = Wrap::Clamp;
    material.sampler.wrap_v = Wrap::Clamp;
    material.sampler.mipmap = None;
    material.validate()?;
    let mut mesh = target.meshes[target_mesh].clone();
    mesh.primitives[target_primitive].material = target.materials.len();
    target.materials.push(material);
    if target
        .nodes
        .iter()
        .filter(|n| n.mesh == Some(target_mesh))
        .count()
        > 1
    {
        target.nodes[target_node].mesh = Some(target.meshes.len());
        target.meshes.push(mesh);
    } else {
        target.meshes[target_mesh] = mesh;
    }
    Ok(bake.report)
}
fn unit(p: P) -> Result<P> {
    let length = dot(p, p).sqrt();
    ensure!(
        length.is_finite() && length > 1e-15,
        "projection has a degenerate normal or tangent"
    );
    Ok(p.map(|v| v / length))
}
fn interpolate(values: [P; 3], weights: P) -> P {
    std::array::from_fn(|k| (0..3).map(|i| values[i][k] * weights[i]).sum())
}
struct Tree {
    low: P,
    high: P,
    kind: Kind,
}
enum Kind {
    Leaf(Vec<usize>),
    Branch(Box<Tree>, Box<Tree>),
}
#[derive(Clone, Copy, Debug)]
struct Hit {
    distance: f64,
    triangle: usize,
    weights: P,
}
fn triangle_hit(origin: P, direction: P, [a, b, c]: [P; 3], limit: f64) -> Option<(f64, P)> {
    let e1 = sub(b, a);
    let e2 = sub(c, a);
    let h = cross(direction, e2);
    let det = dot(e1, h);
    if det.abs() <= 32. * f64::EPSILON * (dot(e1, e1) * dot(e2, e2)).sqrt() {
        return None;
    }
    let s = sub(origin, a);
    let u = dot(s, h) / det;
    let q = cross(s, e1);
    let v = dot(direction, q) / det;
    let t = dot(e2, q) / det;
    if u < -1e-12 || v < -1e-12 || u + v > 1. + 1e-12 || t < 0. || t > limit {
        return None;
    }
    Some((t, [1. - u - v, u, v]))
}
impl Tree {
    fn aligned_hit(
        &self,
        triangles: &[[P; 3]],
        high: &Mesh,
        origin: P,
        direction: P,
        low_normal: P,
        limit: f64,
        best: &mut Option<(f64, Hit)>,
    ) -> Result<()> {
        if !self.intersects(origin, direction, limit) {
            return Ok(());
        }
        match &self.kind {
            Kind::Leaf(ids) => {
                for &i in ids {
                    if let Some((distance, weights)) =
                        triangle_hit(origin, direction, triangles[i], limit)
                    {
                        let normal = unit(interpolate(
                            high.triangles[i].map(|v| high.normals[v as usize]),
                            weights,
                        ))?;
                        let alignment = dot(low_normal, normal);
                        if best.is_none_or(|(score, h)| {
                            alignment > score || (alignment == score && i < h.triangle)
                        }) {
                            *best = Some((
                                alignment,
                                Hit {
                                    distance,
                                    weights,
                                    triangle: i,
                                },
                            ));
                        }
                    }
                }
            }
            Kind::Branch(a, b) => {
                a.aligned_hit(triangles, high, origin, direction, low_normal, limit, best)?;
                b.aligned_hit(triangles, high, origin, direction, low_normal, limit, best)?;
            }
        }
        Ok(())
    }
    fn build(triangles: &[[P; 3]], mut ids: Vec<usize>) -> Self {
        let mut low = [f64::INFINITY; 3];
        let mut high = [f64::NEG_INFINITY; 3];
        for &i in &ids {
            for p in triangles[i] {
                for k in 0..3 {
                    low[k] = low[k].min(p[k]);
                    high[k] = high[k].max(p[k]);
                }
            }
        }
        let kind = if ids.len() <= 8 {
            Kind::Leaf(ids)
        } else {
            let axis = (0..3)
                .max_by(|&a, &b| (high[a] - low[a]).total_cmp(&(high[b] - low[b])))
                .unwrap();
            let centre = |i: usize| triangles[i].iter().map(|p| p[axis] / 3.).sum::<f64>();
            ids.sort_by(|&a, &b| centre(a).total_cmp(&centre(b)).then(a.cmp(&b)));
            let right = ids.split_off(ids.len() / 2);
            Kind::Branch(
                Box::new(Self::build(triangles, ids)),
                Box::new(Self::build(triangles, right)),
            )
        };
        Self { low, high, kind }
    }
    fn intersects(&self, origin: P, direction: P, limit: f64) -> bool {
        let (mut near, mut far) = (0.0_f64, limit);
        for k in 0..3 {
            // Interpolated surface points can round just outside a tight box.
            // Expand traversal bounds only; triangle intersection stays exact.
            let margin = 64. * f64::EPSILON * self.low[k].abs().max(self.high[k].abs()).max(1.);
            let low = self.low[k] - margin;
            let high = self.high[k] + margin;
            if direction[k] == 0. {
                if origin[k] < low || origin[k] > high {
                    return false;
                }
            } else {
                let a = (low - origin[k]) / direction[k];
                let b = (high - origin[k]) / direction[k];
                near = near.max(a.min(b));
                far = far.min(a.max(b));
                if near > far {
                    return false;
                }
            }
        }
        true
    }
    fn cast(
        &self,
        triangles: &[[P; 3]],
        origin: P,
        direction: P,
        limit: f64,
        best: &mut Option<Hit>,
    ) {
        let limit = best.map_or(limit, |h| h.distance.min(limit));
        if !self.intersects(origin, direction, limit) {
            return;
        }
        match &self.kind {
            Kind::Leaf(ids) => {
                for &i in ids {
                    if let Some((distance, weights)) =
                        triangle_hit(origin, direction, triangles[i], limit)
                    {
                        if best.is_none_or(|h| {
                            distance < h.distance || (distance == h.distance && i < h.triangle)
                        }) {
                            *best = Some(Hit {
                                distance,
                                triangle: i,
                                weights,
                            });
                        }
                    }
                }
            }
            Kind::Branch(a, b) => {
                a.cast(triangles, origin, direction, limit, best);
                b.cast(triangles, origin, direction, limit, best);
            }
        }
    }
}

/// Bake the nearest geometric hit along either normal direction. A rejected
/// nearest normal is a miss, not permission to project through another surface.
/// Low UVs must lie in [0,1] with no interior overlaps, including separate charts.
/// Input geometry, normals and tangents are borrowed and never changed.
pub fn normals(low: &Mesh, high: &Mesh, options: Options) -> Result<Bake> {
    low.validate()?;
    high.validate()?;
    ensure!(
        (1..=4096).contains(&options.width) && (1..=4096).contains(&options.height),
        "projection dimensions must be 1..4096"
    );
    ensure!(
        options.max_distance_mm.is_finite() && options.max_distance_mm > 0.,
        "projection distance must be positive and finite"
    );
    ensure!(
        options.min_normal_dot.is_finite() && (0.0..=1.0).contains(&options.min_normal_dot),
        "projection normal threshold must be 0..1"
    );
    ensure!(
        options.max_missed_fraction.is_finite()
            && (0.0..=1.0).contains(&options.max_missed_fraction),
        "projection missed fraction must be 0..1"
    );
    let uv = low
        .texcoords
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("projection requires low UVs"))?;
    let tangents = low
        .tangents
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("projection requires low tangents"))?;
    ensure!(
        uv.iter().flatten().all(|v| (0.0..=1.0).contains(v)),
        "projection requires packed 0..1 UVs"
    );
    let mut bounds = Vec::new();
    for (index, triangle) in low.triangles.iter().enumerate() {
        let t = triangle.map(|i| uv[i as usize]);
        let area =
            (t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[1][1] - t[0][1]) * (t[2][0] - t[0][0]);
        ensure!(area != 0., "projection has a degenerate UV triangle");
        ensure!(
            triangle
                .iter()
                .all(|&i| tangents[i as usize][3] == tangents[triangle[0] as usize][3]),
            "projection requires split mirrored tangent charts"
        );
        let min = std::array::from_fn::<_, 2, _>(|k| {
            t.iter().map(|v| v[k]).fold(f64::INFINITY, f64::min)
        });
        let max = std::array::from_fn::<_, 2, _>(|k| {
            t.iter().map(|v| v[k]).fold(f64::NEG_INFINITY, f64::max)
        });
        bounds.push((min, max, t, index));
    }
    bounds.sort_by(|a, b| a.0[0].total_cmp(&b.0[0]).then(a.3.cmp(&b.3)));
    let mut pairs = 0;
    for i in 0..bounds.len() {
        for j in i + 1..bounds.len() {
            if bounds[j].0[0] >= bounds[i].1[0] {
                break;
            }
            if bounds[j].0[1] >= bounds[i].1[1] || bounds[i].0[1] >= bounds[j].1[1] {
                continue;
            }
            pairs += 1;
            ensure!(
                pairs <= 2_000_000,
                "projection UV validation exceeds two million pairs"
            );
            ensure!(
                !super::atlas::overlap(bounds[i].2, bounds[j].2),
                "projection has overlapping low UV triangles"
            );
        }
    }
    let triangles: Vec<_> = high
        .triangles
        .iter()
        .map(|t| t.map(|i| high.positions[i as usize]))
        .collect();
    let tree = Tree::build(&triangles, (0..triangles.len()).collect());
    let width = options.width as usize;
    let height = options.height as usize;
    let candidates: u64 = bounds
        .iter()
        .map(|(min, max, _, _)| {
            let w = (max[0] * width as f64).floor().min((width - 1) as f64) as u64
                - (min[0] * width as f64).floor().min((width - 1) as f64) as u64
                + 1;
            let h = (max[1] * height as f64).floor().min((height - 1) as f64) as u64
                - (min[1] * height as f64).floor().min((height - 1) as f64) as u64
                + 1;
            w * h
        })
        .sum();
    ensure!(
        candidates <= 100_000_000,
        "projection raster exceeds one hundred million candidate samples"
    );
    let mut rgba = [128, 128, 255, 255].repeat(width * height);
    let mut coverage = vec![0; width * height];
    let mut report = Report {
        covered_pixels: 0,
        hit_pixels: 0,
        missed_pixels: 0,
        max_hit_distance_mm: 0.,
        padded_pixels: 0,
    };
    for (min, max, t, index) in bounds {
        let indices = low.triangles[index].map(|i| i as usize);
        let denominator =
            (t[1][1] - t[2][1]) * (t[0][0] - t[2][0]) + (t[2][0] - t[1][0]) * (t[0][1] - t[2][1]);
        for y in (min[1] * height as f64).floor() as usize
            ..=((max[1] * height as f64).floor() as usize).min(height - 1)
        {
            for x in (min[0] * width as f64).floor() as usize
                ..=((max[0] * width as f64).floor() as usize).min(width - 1)
            {
                let p = [
                    (x as f64 + 0.5) / width as f64,
                    (y as f64 + 0.5) / height as f64,
                ];
                let a = ((t[1][1] - t[2][1]) * (p[0] - t[2][0])
                    + (t[2][0] - t[1][0]) * (p[1] - t[2][1]))
                    / denominator;
                let b = ((t[2][1] - t[0][1]) * (p[0] - t[2][0])
                    + (t[0][0] - t[2][0]) * (p[1] - t[2][1]))
                    / denominator;
                let mut weights = [a, b, 1. - a - b];
                if weights.iter().any(|v| *v < -1e-12) {
                    continue;
                }
                // The inclusion tolerance must not extrapolate beyond the face.
                weights = weights.map(|v| v.max(0.));
                let sum: f64 = weights.iter().sum();
                weights = weights.map(|v| v / sum);
                let pixel = y * width + x;
                // Shared UV edges are owned by the first triangle in stable order.
                if coverage[pixel] != 0 {
                    continue;
                }
                coverage[pixel] = 1;
                report.covered_pixels += 1;
                let position = interpolate(indices.map(|i| low.positions[i]), weights);
                let normal = unit(interpolate(indices.map(|i| low.normals[i]), weights))?;
                let tangent = interpolate(
                    indices.map(|i| [tangents[i][0], tangents[i][1], tangents[i][2]]),
                    weights,
                );
                let along = dot(normal, tangent);
                let tangent = unit(std::array::from_fn(|k| tangent[k] - normal[k] * along))?;
                let bitangent = cross(normal, tangent).map(|v| v * tangents[indices[0]][3]);
                let mut hit = None;
                tree.cast(
                    &triangles,
                    position,
                    normal,
                    options.max_distance_mm,
                    &mut hit,
                );
                tree.cast(
                    &triangles,
                    position,
                    normal.map(|v| -v),
                    options.max_distance_mm,
                    &mut hit,
                );
                let Some(hit) = hit else {
                    continue;
                };
                // Coincident hard-edge faces can yield different shading normals.
                // Search only the fixed nearest-distance roundoff band, avoiding
                // traversal-order dependence or tolerance accumulation.
                let scale = tree
                    .low
                    .iter()
                    .chain(&tree.high)
                    .chain(&position)
                    .map(|v| v.abs())
                    .fold(1., f64::max);
                let limit =
                    (hit.distance + 64. * f64::EPSILON * scale).min(options.max_distance_mm);
                let mut aligned = None;
                tree.aligned_hit(
                    &triangles,
                    high,
                    position,
                    normal,
                    normal,
                    limit,
                    &mut aligned,
                )?;
                tree.aligned_hit(
                    &triangles,
                    high,
                    position,
                    normal.map(|v| -v),
                    normal,
                    limit,
                    &mut aligned,
                )?;
                let hit = aligned.map_or(hit, |(_, hit)| hit);
                let projected = unit(interpolate(
                    high.triangles[hit.triangle].map(|i| high.normals[i as usize]),
                    hit.weights,
                ))?;
                if dot(projected, normal) < options.min_normal_dot {
                    continue;
                }
                for (k, value) in [
                    dot(projected, tangent),
                    dot(projected, bitangent),
                    dot(projected, normal),
                ]
                .into_iter()
                .enumerate()
                {
                    rgba[4 * pixel + k] = ((value.clamp(-1., 1.) * 0.5 + 0.5) * 255.).round() as u8;
                }
                coverage[pixel] = 2;
                report.hit_pixels += 1;
                report.max_hit_distance_mm = report.max_hit_distance_mm.max(hit.distance);
            }
        }
    }
    report.missed_pixels = report.covered_pixels - report.hit_pixels;
    ensure!(
        report.covered_pixels > 0 && report.hit_pixels > 0,
        "projection has no covered pixels or successful hits"
    );
    ensure!(
        report.missed_pixels as f64 / report.covered_pixels as f64 <= options.max_missed_fraction,
        "projection missed {} of {} covered pixels, exceeding allowed fraction {}",
        report.missed_pixels,
        report.covered_pixels,
        options.max_missed_fraction
    );
    Ok(Bake {
        normal_map: Map {
            width,
            height,
            rgba,
        },
        coverage,
        report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coincident_surfaces_choose_the_aligned_normal_without_skipping_nearer_faces() {
        let low = plane();
        let mut high = plane();
        high.normals.fill(unit([1., 0., 1.]).unwrap());
        high.tangents = None;
        high.positions.extend(low.positions.clone());
        high.normals.extend(low.normals.clone());
        high.texcoords = None;
        high.triangles.extend([[4, 5, 6], [5, 7, 6]]);
        let result = normals(&low, &high, options()).unwrap();
        for pixel in result.normal_map.rgba.chunks_exact(4) {
            assert_eq!(pixel, [128, 128, 255, 255]);
        }
    }
    #[test]
    fn cad_plate_identity_projection_has_no_misses() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let low =
            super::super::author::load(&root.join("examples/game-assets/plate-atlas.gasset"), &[])
                .unwrap();
        let high = super::super::author::load(&root.join("examples/game-assets/plate.gasset"), &[])
            .unwrap();
        let result = normals(
            &low.scene.meshes[0].primitives[0].geometry,
            &high.scene.meshes[0].primitives[0].geometry,
            Options {
                width: 256,
                height: 256,
                max_distance_mm: 0.1,
                ..options()
            },
        )
        .unwrap();
        assert_eq!(result.report.missed_pixels, 0);
        assert!(result.report.max_hit_distance_mm < 1e-10);
        for (pixel, &coverage) in result.normal_map.rgba.chunks_exact(4).zip(&result.coverage) {
            if coverage == 2 {
                assert!(
                    pixel[0].abs_diff(128) <= 1 && pixel[1].abs_diff(128) <= 1 && pixel[2] >= 254,
                    "identity projection changed the CAD shading normal: {pixel:?}"
                );
            }
        }
    }
    #[test]
    fn padding_is_bounded_and_never_fills_missed_projection_pixels() {
        let high = plane();
        let mut low = plane();
        for uv in low.texcoords.as_mut().unwrap() {
            for v in uv {
                *v = 0.25 + 0.5 * *v;
            }
        }
        let mut high = high;
        for p in &mut high.positions {
            p[0] *= 0.5;
        }
        let result = normals(
            &low,
            &high,
            Options {
                max_missed_fraction: 0.5,
                ..options()
            },
        )
        .unwrap()
        .padded(1)
        .unwrap();
        assert_eq!(result.report.hit_pixels, 8);
        assert_eq!(result.report.missed_pixels, 8);
        assert_eq!(result.report.padded_pixels, 8);
        for y in 2..6 {
            for x in 4..6 {
                assert_eq!(result.coverage[y * 8 + x], 1);
            }
        }
        assert_eq!(result.coverage[0], 0);
        assert_eq!(result.coverage[2 * 8 + 1], 3);
        assert!(result.padded(1).is_err());
    }
    #[test]
    fn scene_bake_respects_relative_placement_and_preserves_shared_instances_and_skin() {
        use super::super::scene::{Node, Primitive, Scene, SceneMesh, Transform};
        use super::super::skin::{Skin, Weights};
        let mut target = Scene {
            meshes: vec![SceneMesh {
                name: "plane".into(),
                primitives: vec![Primitive {
                    geometry: plane(),
                    material: 0,
                }],
            }],
            materials: vec![Default::default()],
            nodes: vec![
                Node {
                    name: "target".into(),
                    mesh: Some(0),
                    transform: Transform {
                        translation: [0., 0., 0.1],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Node {
                    name: "copy".into(),
                    mesh: Some(0),
                    ..Default::default()
                },
                Node {
                    name: "bone".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let skin = Skin::bind(
            &target,
            0,
            vec![2],
            vec![vec![
                Weights {
                    joints: [0; 4],
                    weights: [1., 0., 0., 0.]
                };
                4
            ]],
        )
        .unwrap();
        let mut source = target.clone();
        source.nodes[0].transform.translation[2] = 0.3;
        let original = target.meshes[0].primitives[0].geometry.clone();
        assert!(
            apply_to_scene(
                &mut target,
                0,
                0,
                &source,
                0,
                0,
                Options {
                    max_distance_mm: 0.01,
                    ..options()
                },
                1
            )
            .is_err()
        );
        assert_eq!(target.materials.len(), 1);
        assert_eq!(target.meshes.len(), 1);
        let report = apply_to_scene(&mut target, 0, 0, &source, 0, 0, options(), 1).unwrap();
        assert!((report.max_hit_distance_mm - 0.2).abs() < 1e-12);
        assert_eq!(target.nodes[1].mesh, Some(0));
        assert_eq!(target.nodes[0].mesh, Some(1));
        assert_eq!(target.meshes[0].primitives[0].material, 0);
        assert_eq!(target.meshes[1].primitives[0].geometry, original);
        assert!(target.materials[0].normal_map.is_none());
        assert!(target.materials[1].normal_map.is_some());
        assert_eq!(target.materials[1].sampler.mipmap, None);
        skin.validate(&target).unwrap();
        super::super::glb::encode_skinned_scene(&target, &[skin]).unwrap();
    }
    fn plane() -> Mesh {
        Mesh {
            positions: vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [1., 1., 0.]],
            normals: vec![[0., 0., 1.]; 4],
            texcoords: Some(vec![[0., 0.], [1., 0.], [0., 1.], [1., 1.]]),
            tangents: Some(vec![[1., 0., 0., 1.]; 4]),
            triangles: vec![[0, 1, 2], [1, 3, 2]],
        }
    }
    fn options() -> Options {
        Options {
            width: 8,
            height: 8,
            max_distance_mm: 1.,
            min_normal_dot: 0.,
            max_missed_fraction: 0.,
        }
    }
    #[test]
    fn slope_projects_to_tangent_space_and_preserves_sources() {
        let low = plane();
        let mut high = plane();
        for p in &mut high.positions {
            p[2] = 0.1 + 0.2 * p[0];
        }
        let n = unit([-0.2, 0., 1.]).unwrap();
        high.normals.fill(n);
        high.tangents = None;
        let before = (low.clone(), high.clone());
        let baked = normals(&low, &high, options()).unwrap();
        assert_eq!(baked.report.hit_pixels, 64);
        assert_eq!(baked.report.missed_pixels, 0);
        assert!((baked.report.max_hit_distance_mm - 0.2875).abs() < 1e-12);
        for rgba in baked.normal_map.rgba.chunks_exact(4) {
            let decoded = unit(std::array::from_fn(|k| rgba[k] as f64 / 255. * 2. - 1.)).unwrap();
            assert!(dot(decoded, n) > 0.99998);
        }
        assert_eq!((low.clone(), high.clone()), before);
        let mut mirrored = low;
        for uv in mirrored.texcoords.as_mut().unwrap() {
            uv[0] = 1. - uv[0];
        }
        mirrored.tangents.as_mut().unwrap().fill([-1., 0., 0., -1.]);
        let result = normals(&mirrored, &high, options()).unwrap();
        for rgba in result.normal_map.rgba.chunks_exact(4) {
            assert!(rgba[0] > 128 && rgba[1] == 128 && rgba[2] > 250);
        }
        assert!(
            normals(
                &mirrored,
                &high,
                Options {
                    max_distance_mm: 0.05,
                    ..options()
                }
            )
            .is_err()
        );
        assert!(
            normals(
                &mirrored,
                &high,
                Options {
                    min_normal_dot: 0.99,
                    ..options()
                }
            )
            .is_err()
        );
    }
    #[test]
    fn misses_are_explicit_and_overlapping_charts_are_rejected() {
        let low = plane();
        let mut high = plane();
        for p in &mut high.positions {
            p[0] *= 0.5;
            p[2] = -0.2;
        }
        assert!(normals(&low, &high, options()).is_err());
        let baked = normals(
            &low,
            &high,
            Options {
                max_missed_fraction: 0.5,
                ..options()
            },
        )
        .unwrap();
        assert_eq!(baked.report.hit_pixels, 32);
        assert_eq!(baked.report.missed_pixels, 32);
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(baked.coverage[y * 8 + x], if x < 4 { 2 } else { 1 });
            }
        }
        let mut overlapping = low.clone();
        overlapping
            .positions
            .extend(low.positions.iter().map(|p| [p[0], p[1], p[2] + 2.]));
        overlapping.normals.extend(low.normals.clone());
        overlapping
            .texcoords
            .as_mut()
            .unwrap()
            .extend(low.texcoords.clone().unwrap());
        overlapping
            .tangents
            .as_mut()
            .unwrap()
            .extend(low.tangents.clone().unwrap());
        overlapping.triangles.extend([[4, 5, 6], [5, 7, 6]]);
        assert!(
            normals(&overlapping, &high, options())
                .unwrap_err()
                .to_string()
                .contains("overlapping")
        );
    }
    #[test]
    fn chart_coverage_is_separate_from_misses_and_invalid_options_fail() {
        let high = plane();
        let mut low = plane();
        for uv in low.texcoords.as_mut().unwrap() {
            for v in uv {
                *v = 0.25 + 0.5 * *v;
            }
        }
        let result = normals(&low, &high, options()).unwrap();
        assert_eq!(result.report.covered_pixels, 16);
        assert_eq!(result.report.hit_pixels, 16);
        assert_eq!(result.report.missed_pixels, 0);
        assert_eq!(result.report.max_hit_distance_mm, 0.);
        assert_eq!(result.coverage.iter().filter(|&&v| v == 0).count(), 48);
        for invalid in [
            Options {
                width: 0,
                ..options()
            },
            Options {
                max_distance_mm: f64::NAN,
                ..options()
            },
            Options {
                min_normal_dot: -0.1,
                ..options()
            },
            Options {
                max_missed_fraction: 1.1,
                ..options()
            },
        ] {
            assert!(normals(&low, &high, invalid).is_err());
        }
    }
    #[test]
    fn nearest_surface_is_not_skipped_when_normal_is_rejected() {
        let low = plane();
        let mut high = plane();
        for p in &mut high.positions {
            p[2] = 0.1;
        }
        high.normals.fill([0., 0., -1.]);
        high.positions
            .extend(low.positions.iter().map(|p| [p[0], p[1], 0.2]));
        high.normals.extend(low.normals.clone());
        high.texcoords = None;
        high.tangents = None;
        high.triangles.extend([[4, 5, 6], [5, 7, 6]]);
        assert!(normals(&low, &high, options()).is_err());
    }
    #[test]
    fn tree_matches_exhaustive_intersections_on_layered_surfaces() {
        let triangles: Vec<_> = (0..37)
            .map(|i| {
                let z = (i as f64 - 18.) * 0.07;
                [[-2., -2., z], [2., -2., z], [0., 2., z]]
            })
            .collect();
        let tree = Tree::build(&triangles, (0..triangles.len()).collect());
        for i in 0..100 {
            let origin = [(i % 7) as f64 * 0.1, 0., i as f64 * 0.03 - 1.5];
            let direction = unit([0.1, 0.05, if i % 2 == 0 { 1. } else { -1. }]).unwrap();
            let mut hit = None;
            tree.cast(&triangles, origin, direction, 2., &mut hit);
            let expected = triangles
                .iter()
                .enumerate()
                .filter_map(|(id, &t)| triangle_hit(origin, direction, t, 2.).map(|(d, _)| (d, id)))
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            assert_eq!(hit.map(|h| (h.distance, h.triangle)), expected);
        }
    }
}
