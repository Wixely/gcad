//! Deterministic packing of existing, non-overlapping UV charts. This does not
//! unwrap geometry or merge materials; texture rebaking must accompany the remap.
use super::Mesh;
use super::material::{Filter, Material, Sampler, Wrap};
use crate::model::texture::Map;
use anyhow::{Result, ensure};
use std::collections::BTreeMap;
#[path = "atlas_coverage.rs"]
mod coverage;
pub use coverage::MaskCoverage;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Chart {
    pub vertices: Vec<usize>,
    pub triangles: Vec<usize>,
    pub source_min: [f64; 2],
    pub source_max: [f64; 2],
    /// Allocated x,y,width,height in pixels, including padding.
    pub rectangle: [u32; 4],
    pub uv_scale: [f64; 2],
    pub uv_offset: [f64; 2],
}
#[derive(Debug)]
pub struct Atlas {
    pub mesh: Mesh,
    pub charts: Vec<Chart>,
    pub width: u32,
    pub height: u32,
    pub padding: u32,
    pub pixels_per_uv: f64,
}

/// Explicit, contiguous atlas levels. Level zero is the full-sized bake.
/// Consumers must upload exactly these levels, clamp sampling to `max_lod`, and
/// disable anisotropy; ordinary automatic mip generation is not equivalent.
#[derive(Debug, Clone, PartialEq)]
pub struct MipChain {
    levels: Vec<Material>,
    geometry: Mesh,
    mask_coverage: Vec<MaskCoverage>,
}

impl MipChain {
    pub fn mask_coverage(&self) -> &[MaskCoverage] {
        &self.mask_coverage
    }
    pub fn geometry(&self) -> &Mesh {
        &self.geometry
    }

    /// Retain the chain with its base material, sharing immutable data on clones.
    pub fn into_material(self) -> Material {
        let mut material = self.levels[0].clone();
        material.mip_chain = Some(std::sync::Arc::new(self));
        material
    }
    pub fn levels(&self) -> &[Material] {
        &self.levels
    }

    pub fn max_lod(&self) -> u32 {
        self.levels.len() as u32 - 1
    }
}

fn address(i: i64, size: usize, wrap: Wrap) -> usize {
    let n = size as i64;
    match wrap {
        Wrap::Clamp => i.clamp(0, n - 1) as usize,
        Wrap::Repeat => i.rem_euclid(n) as usize,
        Wrap::Mirror => {
            let p = i.rem_euclid(2 * n);
            (if p < n { p } else { 2 * n - 1 - p }) as usize
        }
    }
}

fn sample(map: &Map, uv: [f64; 2], sampler: Sampler, filter: Filter, srgb: bool) -> [u8; 4] {
    // Reduce coordinates before integer conversion, including very large tiled UVs.
    let coordinate = |u: f64, wrap| match wrap {
        Wrap::Repeat => u.rem_euclid(1.),
        Wrap::Mirror => u.rem_euclid(2.),
        Wrap::Clamp => u.clamp(0., 1.),
    };
    let x = coordinate(uv[0], sampler.wrap_u) * map.width as f64 - 0.5;
    let y = coordinate(uv[1], sampler.wrap_v) * map.height as f64 - 0.5;
    let pixel = |x, y| {
        let i = 4
            * (address(y, map.height, sampler.wrap_v) * map.width
                + address(x, map.width, sampler.wrap_u));
        <[u8; 4]>::try_from(&map.rgba[i..i + 4]).unwrap()
    };
    if filter == Filter::Nearest {
        return pixel((x + 0.5).floor() as i64, (y + 0.5).floor() as i64);
    }
    let ix = x.floor() as i64;
    let iy = y.floor() as i64;
    let fx = x - x.floor();
    let fy = y - y.floor();
    let taps = [
        pixel(ix, iy),
        pixel(ix + 1, iy),
        pixel(ix, iy + 1),
        pixel(ix + 1, iy + 1),
    ];
    let weights = [
        (1. - fx) * (1. - fy),
        fx * (1. - fy),
        (1. - fx) * fy,
        fx * fy,
    ];
    std::array::from_fn(|c| {
        let linear = srgb && c < 3;
        let value: f64 = taps
            .iter()
            .zip(weights)
            .map(|(p, w)| {
                let v = p[c] as f64 / 255.;
                w * if linear {
                    if v <= 0.04045 {
                        v / 12.92
                    } else {
                        ((v + 0.055) / 1.055).powf(2.4)
                    }
                } else {
                    v
                }
            })
            .sum();
        let encoded = if linear {
            if value <= 0.0031308 {
                value * 12.92
            } else {
                1.055 * value.powf(1. / 2.4) - 0.055
            }
        } else {
            value
        };
        (encoded.clamp(0., 1.) * 255.).round() as u8
    })
}

fn axis_taps(uv: f64, span: f64, size: usize, wrap: Wrap) -> Vec<(usize, f64)> {
    if wrap == Wrap::Clamp {
        if uv <= -span / (2. * size as f64) {
            return vec![(0, 1.)];
        }
        if uv >= 1. + span / (2. * size as f64) {
            return vec![(size - 1, 1.)];
        }
    }
    let center = match wrap {
        Wrap::Repeat => uv.rem_euclid(1.),
        Wrap::Mirror => uv.rem_euclid(2.),
        Wrap::Clamp => uv,
    } * size as f64;
    if span <= 1. {
        let x = center - 0.5;
        let i = x.floor() as i64;
        let f = x - x.floor();
        return vec![
            (address(i, size, wrap), 1. - f),
            (address(i + 1, size, wrap), f),
        ];
    }
    let low = center - span / 2.;
    let high = center + span / 2.;
    (low.floor() as i64..high.ceil() as i64)
        .map(|i| {
            (
                address(i, size, wrap),
                (high.min(i as f64 + 1.) - low.max(i as f64)).max(0.) / span,
            )
        })
        .collect()
}

fn footprint_sample(
    map: &Map,
    uv: [f64; 2],
    footprint: [f64; 2],
    sampler: Sampler,
    filter: Filter,
    srgb: bool,
) -> [u8; 4] {
    if filter == Filter::Nearest || footprint.iter().all(|s| *s <= 1.) {
        return sample(map, uv, sampler, filter, srgb);
    }
    let xs = axis_taps(uv[0], footprint[0], map.width, sampler.wrap_u);
    let ys = axis_taps(uv[1], footprint[1], map.height, sampler.wrap_v);
    let mut values = [0.; 4];
    for (y, wy) in ys {
        for &(x, wx) in &xs {
            let i = 4 * (y * map.width + x);
            for (c, value) in values.iter_mut().enumerate() {
                let v = map.rgba[i + c] as f64 / 255.;
                let decoded = if srgb && c < 3 {
                    if v <= 0.04045 {
                        v / 12.92
                    } else {
                        ((v + 0.055) / 1.055).powf(2.4)
                    }
                } else {
                    v
                };
                *value += decoded * wx * wy;
            }
        }
    }
    std::array::from_fn(|c| {
        let value = values[c].clamp(0., 1.);
        let encoded = if srgb && c < 3 {
            if value <= 0.0031308 {
                value * 12.92
            } else {
                1.055 * value.powf(1. / 2.4) - 0.055
            }
        } else {
            value
        };
        (encoded.clamp(0., 1.) * 255.).round() as u8
    })
}

impl Atlas {
    /// Build exactly levels zero through `max_level`, without silently truncating
    /// unsafe requests. All layouts and the aggregate 256 MiB map budget are
    /// checked before allocating any baked maps. The original material is intact.
    pub fn rebake_chain(&self, material: &Material, max_level: u32) -> Result<MipChain> {
        material.validate()?;
        ensure!(max_level <= 13, "atlas mip level must be 0..13");
        ensure!(
            (16..=8192).contains(&self.width) && (16..=8192).contains(&self.height),
            "invalid atlas dimensions"
        );
        let count = [
            &material.colour_map,
            &material.roughness_map,
            &material.metallic_map,
            &material.normal_map,
            &material.occlusion_map,
            &material.emissive_map,
        ]
        .into_iter()
        .filter(|map| map.is_some())
        .count() as u64;
        let bytes: u64 = (0..=max_level)
            .map(|level| {
                (self.width >> level).max(1) as u64
                    * (self.height >> level).max(1) as u64
                    * 4
                    * count
            })
            .sum();
        ensure!(
            bytes <= 256 * 1024 * 1024,
            "atlas mip chain exceeds 256 MiB bake budget"
        );
        // Do not retain copies of the mesh/layout for every level. Preflight is
        // repeated during baking, but only one temporary layout is live at once.
        for level in 1..=max_level {
            self.mip_atlas(level)?;
        }
        let mut levels = vec![self.rebake(material, Filter::Linear)?];
        for level in 1..=max_level {
            levels.push(self.rebake_mip(material, level)?);
        }
        Ok(MipChain {
            levels,
            geometry: self.mesh.clone(),
            mask_coverage: Vec::new(),
        })
    }

    /// Bake one explicitly requested mip level from the original material, not
    /// from a downsampled atlas. UVs stay unchanged. Each chart owns every texel
    /// its bilinear footprint can reach; overlapping ownership is refused.
    /// This returns a standalone image level, not an exportable mip chain.
    pub fn rebake_mip(&self, material: &Material, level: u32) -> Result<Material> {
        self.mip_atlas(level)?
            .rebake_level(material, Filter::Linear)
    }

    fn mip_atlas(&self, level: u32) -> Result<Self> {
        ensure!(level > 0 && level <= 13, "atlas mip level must be 1..13");
        ensure!(
            (16..=8192).contains(&self.width) && (16..=8192).contains(&self.height),
            "invalid atlas dimensions"
        );
        let width = (self.width >> level).max(1);
        let height = (self.height >> level).max(1);
        ensure!(
            level <= self.width.max(self.height).ilog2(),
            "atlas mip level exceeds image chain"
        );
        let mut charts = self.charts.clone();
        // At most 16 MiB for level one of an 8192-square atlas. The owner map
        // avoids quadratic chart-pair comparisons and is allocated before maps.
        let mut occupied = vec![false; width as usize * height as usize];
        for chart in &mut charts {
            for (k, size) in [width, height].into_iter().enumerate() {
                ensure!(
                    chart.source_min[k].is_finite()
                        && chart.source_max[k].is_finite()
                        && chart.source_max[k] > chart.source_min[k]
                        && chart.uv_scale[k].is_finite()
                        && chart.uv_scale[k] > 0.
                        && chart.uv_offset[k].is_finite(),
                    "invalid atlas mip chart transform"
                );
                let low = chart.source_min[k] * chart.uv_scale[k] + chart.uv_offset[k];
                let high = chart.source_max[k] * chart.uv_scale[k] + chart.uv_offset[k];
                ensure!(
                    low.is_finite() && high.is_finite() && low >= 0. && high >= low && high <= 1.,
                    "atlas mip chart UVs exceed image"
                );
                // Include both bilinear taps even at exact texel centres. Clamp
                // at the image edge matches the derived material's sampler.
                let first = (low * size as f64 - 0.5).floor().max(0.) as u32;
                let last = ((high * size as f64 - 0.5).floor() + 1.).min(size as f64 - 1.) as u32;
                chart.rectangle[k] = first;
                chart.rectangle[k + 2] = last - first + 1;
            }
            let [x, y, w, h] = chart.rectangle;
            for py in y..y + h {
                for px in x..x + w {
                    let pixel = &mut occupied[(py * width + px) as usize];
                    ensure!(
                        !*pixel,
                        "atlas mip level {level} has overlapping chart footprints"
                    );
                    *pixel = true;
                }
            }
        }
        Ok(Self {
            mesh: self.mesh.clone(),
            charts,
            width,
            height,
            padding: 0,
            pixels_per_uv: self.pixels_per_uv / (1u32 << level) as f64,
        })
    }

    /// Resample each source map through the chart transform, including padding.
    /// Linear filtering averages texel coverage on minified axes and uses bilinear
    /// interpolation on other axes. Output mipmaps remain disabled to avoid bleed.
    /// Normal channels remain numeric; conventions and material factors persist.
    pub fn rebake(&self, material: &Material, filter: Filter) -> Result<Material> {
        ensure!(
            (16..=8192).contains(&self.width) && (16..=8192).contains(&self.height),
            "invalid atlas dimensions"
        );
        self.rebake_level(material, filter)
    }

    fn rebake_level(&self, material: &Material, filter: Filter) -> Result<Material> {
        material.validate()?;
        ensure!(
            material.mip_chain.is_none(),
            "atlas rebake requires original material without an attached mip chain"
        );
        for chart in &self.charts {
            let [x, y, w, h] = chart.rectangle;
            ensure!(
                w > 0
                    && h > 0
                    && x.checked_add(w).is_some_and(|v| v <= self.width)
                    && y.checked_add(h).is_some_and(|v| v <= self.height),
                "invalid atlas chart rectangle"
            );
            ensure!(
                chart.uv_scale.iter().all(|v| v.is_finite() && *v > 0.)
                    && chart.uv_offset.iter().all(|v| v.is_finite()),
                "invalid atlas UV transform"
            );
        }
        let sources = [
            &material.colour_map,
            &material.roughness_map,
            &material.metallic_map,
            &material.normal_map,
            &material.occlusion_map,
            &material.emissive_map,
        ];
        let count = sources.iter().filter(|m| m.is_some()).count();
        let bytes = self.width as usize * self.height as usize * 4;
        ensure!(
            bytes * count <= 256 * 1024 * 1024,
            "atlas maps exceed 256 MiB bake budget"
        );
        let footprint = |source: &Map, chart: &Chart| {
            [
                source.width as f64 / (self.width as f64 * chart.uv_scale[0]),
                source.height as f64 / (self.height as f64 * chart.uv_scale[1]),
            ]
        };
        // Refuse excessive exact filtering work before allocating destination maps;
        // never silently undersample a large footprint to satisfy a tap limit.
        let mut work = 0u64;
        for source in sources.iter().filter_map(|s| s.as_ref()) {
            for chart in &self.charts {
                let spans = footprint(source, chart);
                let mut taps = [1u64; 2];
                if filter == Filter::Linear {
                    for k in 0..2 {
                        ensure!(
                            spans[k].is_finite() && spans[k] <= 65534.,
                            "atlas filter footprint exceeds 65536 axis taps"
                        );
                        taps[k] = if spans[k] > 1. {
                            spans[k].ceil() as u64 + 2
                        } else {
                            2
                        };
                    }
                }
                let pixels = chart.rectangle[2] as u64 * chart.rectangle[3] as u64;
                work = work.saturating_add(pixels.saturating_mul(taps[0]).saturating_mul(taps[1]));
                ensure!(
                    work <= 256 * 1024 * 1024,
                    "atlas filtering exceeds 256 Mi tap budget"
                );
            }
        }
        let bake = |source: &Option<Map>, srgb| -> Result<Option<Map>> {
            let Some(source) = source else {
                return Ok(None);
            };
            let mut rgba = vec![0; bytes];
            for chart in &self.charts {
                let [x, y, w, h] = chart.rectangle;
                for py in y..y + h {
                    for px in x..x + w {
                        let uv = [
                            ((px as f64 + 0.5) / self.width as f64 - chart.uv_offset[0])
                                / chart.uv_scale[0],
                            ((py as f64 + 0.5) / self.height as f64 - chart.uv_offset[1])
                                / chart.uv_scale[1],
                        ];
                        ensure!(
                            uv.iter().all(|v| v.is_finite()),
                            "atlas inverse UV transform overflow"
                        );
                        let i = 4 * (py as usize * self.width as usize + px as usize);
                        rgba[i..i + 4].copy_from_slice(&footprint_sample(
                            source,
                            uv,
                            footprint(source, chart),
                            material.sampler,
                            filter,
                            srgb,
                        ));
                    }
                }
            }
            Ok(Some(Map {
                width: self.width as usize,
                height: self.height as usize,
                rgba,
            }))
        };
        let mut result = material.clone();
        result.colour_map = bake(&material.colour_map, true)?;
        result.roughness_map = bake(&material.roughness_map, false)?;
        result.metallic_map = bake(&material.metallic_map, false)?;
        result.normal_map = bake(&material.normal_map, false)?;
        result.occlusion_map = bake(&material.occlusion_map, false)?;
        result.emissive_map = bake(&material.emissive_map, true)?;
        result.sampler = Sampler {
            wrap_u: Wrap::Clamp,
            wrap_v: Wrap::Clamp,
            magnify: filter,
            minify: filter,
            mipmap: None,
        };
        result.validate()?;
        Ok(result)
    }
}
fn root(parent: &mut [usize], mut i: usize) -> usize {
    let mut representative = i;
    while parent[representative] != representative {
        representative = parent[representative];
    }
    while parent[i] != i {
        let next = parent[i];
        parent[i] = representative;
        i = next;
    }
    representative
}
pub(super) fn overlap(a: [[f64; 2]; 3], b: [[f64; 2]; 3]) -> bool {
    for t in [a, b] {
        for i in 0..3 {
            let d = [t[(i + 1) % 3][0] - t[i][0], t[(i + 1) % 3][1] - t[i][1]];
            let axis = [-d[1], d[0]];
            let origin = t[i];
            let project = |t: [[f64; 2]; 3]| {
                // Translate to the tested edge: its endpoints project to exactly
                // zero, avoiding a false interior overlap at shared CAD edges.
                let v = t.map(|p| (p[0] - origin[0]) * axis[0] + (p[1] - origin[1]) * axis[1]);
                (
                    v.iter().copied().fold(f64::INFINITY, f64::min),
                    v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                )
            };
            let (amin, amax) = project(a);
            let (bmin, bmax) = project(b);
            // Boundary contact is allowed; positive-area interior overlap is not.
            if amax <= bmin || bmax <= amin {
                return false;
            }
        }
    }
    true
}

pub fn pack(
    mesh: &Mesh,
    width: u32,
    height: u32,
    padding: u32,
    pixels_per_uv: f64,
) -> Result<Atlas> {
    mesh.validate()?;
    ensure!(
        (16..=8192).contains(&width) && (16..=8192).contains(&height),
        "atlas dimensions must be 16..8192 pixels"
    );
    ensure!(
        padding >= 1 && padding <= width.min(height) / 4,
        "atlas padding must be positive and at most one quarter of the smaller dimension"
    );
    ensure!(
        pixels_per_uv.is_finite() && pixels_per_uv > 0.,
        "atlas pixel density must be finite and positive"
    );
    let uv = mesh
        .texcoords
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("atlas requires existing UV charts"))?;
    let mut parent: Vec<_> = (0..mesh.positions.len()).collect();
    let mut used = vec![false; parent.len()];
    for triangle in &mesh.triangles {
        let [a, b, c] = triangle.map(|i| i as usize);
        for i in [a, b, c] {
            used[i] = true;
            let ra = root(&mut parent, a);
            let ri = root(&mut parent, i);
            parent[ri] = ra;
        }
        let [a, b, c] = [uv[a], uv[b], uv[c]];
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        ensure!(
            area.is_finite() && area != 0.,
            "atlas source contains a degenerate UV triangle"
        );
    }
    ensure!(
        used.iter().all(|u| *u),
        "atlas requires all vertices to be referenced"
    );
    let mut groups = BTreeMap::<usize, Chart>::new();
    for i in 0..parent.len() {
        let r = root(&mut parent, i);
        let chart = groups.entry(r).or_insert_with(|| Chart {
            vertices: Vec::new(),
            triangles: Vec::new(),
            source_min: [f64::INFINITY; 2],
            source_max: [f64::NEG_INFINITY; 2],
            rectangle: [0; 4],
            uv_scale: [0.; 2],
            uv_offset: [0.; 2],
        });
        chart.vertices.push(i);
        for k in 0..2 {
            chart.source_min[k] = chart.source_min[k].min(uv[i][k]);
            chart.source_max[k] = chart.source_max[k].max(uv[i][k]);
        }
    }
    for (i, t) in mesh.triangles.iter().enumerate() {
        groups
            .get_mut(&root(&mut parent, t[0] as usize))
            .unwrap()
            .triangles
            .push(i);
    }
    let mut charts: Vec<_> = groups.into_values().collect();
    let mut comparisons = 0;
    for chart in &mut charts {
        let mut boxes: Vec<_> = chart
            .triangles
            .iter()
            .map(|&i| {
                let t = mesh.triangles[i].map(|v| uv[v as usize]);
                let low = std::array::from_fn::<_, 2, _>(|k| {
                    t.iter().map(|p| p[k]).fold(f64::INFINITY, f64::min)
                });
                let high = std::array::from_fn::<_, 2, _>(|k| {
                    t.iter().map(|p| p[k]).fold(f64::NEG_INFINITY, f64::max)
                });
                (low, high, t)
            })
            .collect();
        boxes.sort_by(|a, b| a.0[0].total_cmp(&b.0[0]));
        for i in 0..boxes.len() {
            for j in i + 1..boxes.len() {
                if boxes[j].0[0] >= boxes[i].1[0] {
                    break;
                }
                if boxes[j].0[1] >= boxes[i].1[1] || boxes[i].0[1] >= boxes[j].1[1] {
                    continue;
                }
                comparisons += 1;
                ensure!(
                    comparisons <= 2000000,
                    "atlas UV-overlap validation exceeds two million candidate pairs"
                );
                ensure!(
                    !overlap(boxes[i].2, boxes[j].2),
                    "atlas chart contains overlapping UV triangles {:?} and {:?}; split or unwrap the source chart",
                    boxes[i].2,
                    boxes[j].2
                );
            }
        }
        for k in 0..2 {
            let pixels = ((chart.source_max[k] - chart.source_min[k]) * pixels_per_uv).ceil();
            ensure!(
                pixels.is_finite() && pixels >= 1. && pixels <= 8192.,
                "atlas chart extent exceeds requested density or numeric range"
            );
            chart.rectangle[k + 2] = pixels as u32 + 1 + 2 * padding;
        }
    }
    // Stable, height-first shelf packing. Never silently lower density or rotate charts.
    charts.sort_by(|a, b| {
        b.rectangle[3]
            .cmp(&a.rectangle[3])
            .then(b.rectangle[2].cmp(&a.rectangle[2]))
            .then(a.vertices[0].cmp(&b.vertices[0]))
    });
    let (mut x, mut y, mut row_height) = (0, 0, 0);
    for chart in &mut charts {
        let [_, _, w, h] = chart.rectangle;
        ensure!(
            w <= width && h <= height,
            "atlas chart does not fit at requested density and padding"
        );
        if x + w > width {
            x = 0;
            y += row_height;
            row_height = 0;
        }
        ensure!(
            y + h <= height,
            "atlas packing exceeds dimensions; increase atlas size or reduce density"
        );
        chart.rectangle = [x, y, w, h];
        chart.uv_scale = [pixels_per_uv / width as f64, pixels_per_uv / height as f64];
        chart.uv_offset = std::array::from_fn(|k| {
            (chart.rectangle[k] as f64 + padding as f64 + 0.5)
                / (if k == 0 { width } else { height }) as f64
                - chart.source_min[k] * chart.uv_scale[k]
        });
        x += w;
        row_height = row_height.max(h);
    }
    let mut output = mesh.clone();
    let mapped = output.texcoords.as_mut().unwrap();
    for chart in &charts {
        for &i in &chart.vertices {
            mapped[i] = std::array::from_fn(|k| uv[i][k] * chart.uv_scale[k] + chart.uv_offset[k]);
        }
    }
    output.tangents = None;
    output = output.generate_tangents()?;
    ensure!(
        output
            .texcoords
            .as_ref()
            .unwrap()
            .iter()
            .flatten()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
        "atlas UV coordinates exceed range"
    );
    Ok(Atlas {
        mesh: output,
        charts,
        width,
        height,
        padding,
        pixels_per_uv,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mip_levels_preserve_chart_sampling_and_refuse_shared_texels() {
        let mut mesh = crate::asset::tests::triangle();
        mesh.texcoords = Some(vec![[0., 0.], [1., 0.], [0., 1.]]);
        let mut atlas = pack(&mesh, 32, 32, 2, 4.).unwrap();
        let mut left = atlas.charts[0].clone();
        left.source_min = [0.1, 0.1];
        left.source_max = [0.21, 0.21];
        left.uv_scale = [2.5, 2.5];
        left.uv_offset = [-0.125, -0.125];
        let mut right = left.clone();
        right.source_min[0] = 0.79;
        right.source_max[0] = 0.9;
        right.uv_offset[0] = -1.375;
        atlas.charts = vec![left, right];
        let source = Material {
            colour_map: Some(Map {
                width: 32,
                height: 1,
                rgba: (0..32)
                    .flat_map(|x| {
                        if x < 16 {
                            [255, 0, 0, 255]
                        } else {
                            [0, 0, 255, 255]
                        }
                    })
                    .collect(),
            }),
            sampler: Sampler {
                wrap_u: Wrap::Clamp,
                wrap_v: Wrap::Clamp,
                ..Sampler::default()
            },
            ..Material::default()
        };
        let original = source.clone();
        for level in [1, 2] {
            let baked = atlas.rebake_mip(&source, level).unwrap();
            let map = baked.colour_map.as_ref().unwrap();
            assert_eq!((map.width, map.height), (32 >> level, 32 >> level));
            assert_eq!(baked.sampler.mipmap, None);
            // Sample the complete chart bounds including all corners and edges.
            for (chart, expected) in atlas
                .charts
                .iter()
                .zip([[255, 0, 0, 255], [0, 0, 255, 255]])
            {
                for y in 0..=100 {
                    for x in 0..=100 {
                        let uv = std::array::from_fn(|k| {
                            (chart.source_min[k]
                                + [x, y][k] as f64 / 100.
                                    * (chart.source_max[k] - chart.source_min[k]))
                                * chart.uv_scale[k]
                                + chart.uv_offset[k]
                        });
                        assert_eq!(
                            sample(map, uv, baked.sampler, Filter::Linear, true),
                            expected
                        );
                    }
                }
            }
        }
        assert!(
            atlas
                .rebake_mip(&source, 3)
                .unwrap_err()
                .to_string()
                .contains("overlapping chart footprints")
        );
        // Bypassing ownership makes the second chart overwrite texels needed
        // by the first: a sample inside the red chart becomes blue.
        let mut unsafe_charts = atlas.charts.clone();
        unsafe_charts[0].rectangle = [0, 0, 3, 3];
        unsafe_charts[1].rectangle = [1, 0, 3, 3];
        let unsafe_atlas = Atlas {
            mesh: atlas.mesh.clone(),
            charts: unsafe_charts,
            width: 4,
            height: 4,
            padding: 0,
            pixels_per_uv: 0.5,
        };
        let contaminated = unsafe_atlas.rebake_level(&source, Filter::Linear).unwrap();
        assert_eq!(
            sample(
                contaminated.colour_map.as_ref().unwrap(),
                [0.4, 0.25],
                contaminated.sampler,
                Filter::Linear,
                true
            ),
            [0, 0, 255, 255]
        );
        assert!(atlas.rebake_mip(&source, 5).is_err());
        assert!(atlas.rebake_mip(&source, 0).is_err());
        assert!(atlas.rebake_mip(&source, 6).is_err());
        assert!(atlas.rebake_mip(&source, u32::MAX).is_err());
        assert!(
            atlas
                .rebake_chain(&source, 3)
                .unwrap_err()
                .to_string()
                .contains("overlapping chart footprints")
        );
        assert_eq!(source, original);
    }

    #[test]
    fn mip_levels_filter_original_pixels_in_linear_light() {
        let mut mesh = crate::asset::tests::triangle();
        mesh.texcoords = Some(vec![[0., 0.], [1., 0.], [0., 1.]]);
        let atlas = pack(&mesh, 32, 16, 2, 4.).unwrap();
        let map = Map {
            width: 16,
            height: 1,
            rgba: (0..16)
                .flat_map(|x| if x % 4 == 1 { [255; 4] } else { [0; 4] })
                .collect(),
        };
        let source = Material {
            colour_map: Some(map.clone()),
            roughness_map: Some(map),
            ..Material::default()
        };
        for level in 1..=5 {
            let mip = atlas.rebake_mip(&source, level).unwrap();
            for (map, expected) in [
                (mip.colour_map.unwrap(), [137, 137, 137, 64]),
                (mip.roughness_map.unwrap(), [64; 4]),
            ] {
                assert_eq!(
                    (map.width, map.height),
                    ((32usize >> level).max(1), (16usize >> level).max(1))
                );
                assert!(
                    map.rgba
                        .chunks_exact(4)
                        .all(|p| p == [0; 4] || p == expected)
                );
                assert!(map.rgba.chunks_exact(4).any(|p| p == expected));
            }
        }
        let chain = atlas.rebake_chain(&source, 5).unwrap();
        assert_eq!(chain.max_lod(), 5);
        assert_eq!(chain.levels().len(), 6);
        assert_eq!(
            chain.levels()[0],
            atlas.rebake(&source, Filter::Linear).unwrap()
        );
        for level in 1..=5 {
            assert_eq!(
                chain.levels()[level],
                atlas.rebake_mip(&source, level as u32).unwrap()
            );
        }
    }

    #[test]
    fn mip_chain_bounds_total_storage_and_preserves_non_power_of_two_sizes() {
        let mut mesh = crate::asset::tests::triangle();
        mesh.texcoords = Some(vec![[0., 0.], [1., 0.], [0., 1.]]);
        let mut atlas = pack(&mesh, 31, 17, 2, 4.).unwrap();
        let source = Material {
            colour_map: Some(Map {
                width: 1,
                height: 1,
                rgba: vec![255; 4],
            }),
            ..Material::default()
        };
        let chain = atlas.rebake_chain(&source, 4).unwrap();
        assert_eq!(chain.max_lod(), 4);
        let sizes: Vec<_> = chain
            .levels()
            .iter()
            .map(|level| {
                let map = level.colour_map.as_ref().unwrap();
                (map.width, map.height)
            })
            .collect();
        assert_eq!(sizes, [(31, 17), (15, 8), (7, 4), (3, 2), (1, 1)]);
        assert_eq!(atlas.rebake_chain(&source, 0).unwrap().max_lod(), 0);
        assert!(atlas.rebake_chain(&source, 5).is_err());
        assert!(atlas.rebake_chain(&source, u32::MAX).is_err());
        // One base map alone would fit, but its first mip takes the aggregate
        // over budget. Refuse before allocating either large destination map.
        atlas.width = 8192;
        atlas.height = 8192;
        assert!(
            atlas
                .rebake_chain(&source, 1)
                .unwrap_err()
                .to_string()
                .contains("mip chain exceeds 256 MiB")
        );
    }

    #[test]
    fn linear_rebake_preserves_minified_stripe_energy() {
        let mut mesh = crate::asset::tests::triangle();
        mesh.texcoords = Some(vec![[0., 0.], [1., 0.], [0., 1.]]);
        let atlas = pack(&mesh, 16, 16, 2, 4.).unwrap();
        let rgba = (0..16)
            .flat_map(|x| if x % 4 == 1 { [255; 4] } else { [0; 4] })
            .collect();
        let map = Map {
            width: 16,
            height: 1,
            rgba,
        };
        let source = Material {
            colour_map: Some(map.clone()),
            roughness_map: Some(map),
            ..Material::default()
        };
        let before = source.clone();
        let baked = atlas.rebake(&source, Filter::Linear).unwrap();
        let [x, y, w, h] = atlas.charts[0].rectangle;
        for py in y..y + h {
            for px in x..x + w {
                let i = 4 * (py as usize * 16 + px as usize);
                assert_eq!(
                    &baked.colour_map.as_ref().unwrap().rgba[i..i + 4],
                    &[137, 137, 137, 64]
                );
                assert_eq!(
                    &baked.roughness_map.as_ref().unwrap().rgba[i..i + 4],
                    &[64; 4]
                );
            }
        }
        assert_eq!(source, before);
        assert_eq!(baked.sampler.mipmap, None);
        let nearest = atlas.rebake(&source, Filter::Nearest).unwrap();
        let i = 4 * (y as usize * 16 + x as usize);
        assert_eq!(&nearest.colour_map.unwrap().rgba[i..i + 4], &[0; 4]);
    }

    #[test]
    fn minified_footprints_integrate_partial_texels_and_wrap_boundaries() {
        let values = [0, 100, 200, 255];
        let map = Map {
            width: 4,
            height: 1,
            rgba: values.into_iter().flat_map(|v| [v, v, v, 255]).collect(),
        };
        for (wrap, expected) in [(Wrap::Clamp, 10), (Wrap::Repeat, 132), (Wrap::Mirror, 20)] {
            let sampler = Sampler {
                wrap_u: wrap,
                ..Sampler::default()
            };
            assert_eq!(
                footprint_sample(&map, [0., 0.5], [2.5, 1.], sampler, Filter::Linear, false),
                [expected, expected, expected, 255]
            );
            // Swap axes to exercise vertical minification with horizontal magnification.
            let vertical = Map {
                width: 1,
                height: 4,
                rgba: map.rgba.clone(),
            };
            let sampler = Sampler {
                wrap_v: wrap,
                ..Sampler::default()
            };
            assert_eq!(
                footprint_sample(
                    &vertical,
                    [0.5, 0.],
                    [1., 2.5],
                    sampler,
                    Filter::Linear,
                    false
                ),
                [expected, expected, expected, 255]
            );
        }
        let clamp = Sampler {
            wrap_u: Wrap::Clamp,
            ..Sampler::default()
        };
        for (u, v) in [(-1e200, 0), (1e200, 255)] {
            assert_eq!(
                footprint_sample(&map, [u, 0.5], [2.5, 1.], clamp, Filter::Linear, false),
                [v, v, v, 255]
            );
        }
        let repeat = Sampler::default();
        assert_eq!(
            footprint_sample(&map, [-1., 0.5], [2.5, 1.], repeat, Filter::Linear, false),
            [132, 132, 132, 255]
        );
        // Full 2D coverage: one white texel out of four, with separate linear alpha.
        let square = Map {
            width: 2,
            height: 2,
            rgba: vec![255, 255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        };
        assert_eq!(
            footprint_sample(&square, [0.5, 0.5], [2., 2.], repeat, Filter::Linear, true),
            [137, 137, 137, 64]
        );
    }

    #[test]
    fn excessive_minification_is_refused_without_changing_source() {
        let mut mesh = crate::asset::tests::triangle();
        mesh.texcoords = Some(vec![[0., 0.], [1., 0.], [0., 1.]]);
        let mut atlas = pack(&mesh, 16, 16, 2, 4.).unwrap();
        let source = Material {
            colour_map: Some(Map {
                width: 1,
                height: 1,
                rgba: vec![255; 4],
            }),
            ..Material::default()
        };
        let before = source.clone();
        atlas.charts[0].uv_scale = [1e-20; 2];
        assert!(
            atlas
                .rebake(&source, Filter::Linear)
                .unwrap_err()
                .to_string()
                .contains("axis taps")
        );
        // Finite per-axis work, but the combined two-dimensional budget is excessive.
        atlas.charts[0].uv_scale = [1. / (16. * 4096.); 2];
        assert!(
            atlas
                .rebake(&source, Filter::Linear)
                .unwrap_err()
                .to_string()
                .contains("tap budget")
        );
        assert_eq!(source, before);
        assert!(atlas.rebake(&source, Filter::Nearest).is_ok());
    }
    #[test]
    fn sampling_respects_wrap_filter_and_colour_space() {
        let map = Map {
            width: 2,
            height: 1,
            rgba: vec![0, 0, 0, 0, 255, 255, 255, 255],
        };
        let sampler = Sampler::default();
        assert_eq!(
            sample(&map, [0.5, 0.5], sampler, Filter::Linear, true),
            [188, 188, 188, 128]
        );
        assert_eq!(
            sample(&map, [0.5, 0.5], sampler, Filter::Linear, false),
            [128; 4]
        );
        assert_eq!(
            sample(&map, [-0.25, 0.5], sampler, Filter::Nearest, false),
            [255; 4]
        );
        let mirror = Sampler {
            wrap_u: Wrap::Mirror,
            ..sampler
        };
        assert_eq!(
            sample(&map, [-0.25, 0.5], mirror, Filter::Nearest, false),
            [0; 4]
        );
        assert_eq!(
            sample(&map, [1.25, 0.5], mirror, Filter::Nearest, false),
            [255; 4]
        );
        let clamp = Sampler {
            wrap_u: Wrap::Clamp,
            ..sampler
        };
        assert_eq!(
            sample(&map, [-100., 0.5], clamp, Filter::Linear, false),
            [0; 4]
        );
        assert_eq!(
            sample(&map, [100., 0.5], clamp, Filter::Linear, false),
            [255; 4]
        );
    }

    #[test]
    fn rebakes_all_maps_and_padding_without_modifying_factors_or_source() {
        let mut mesh = crate::asset::tests::triangle();
        mesh.texcoords = Some(vec![[0., 0.], [1., 0.], [0., 1.]]);
        let atlas = pack(&mesh, 16, 16, 2, 4.).unwrap();
        let map = Map {
            width: 2,
            height: 1,
            rgba: vec![0, 0, 0, 0, 255, 255, 255, 255],
        };
        let source = Material {
            base_colour: [0.2, 0.3, 0.4, 0.5],
            metallic: 0.3,
            roughness: 0.6,
            colour_map: Some(map.clone()),
            emissive_map: Some(map.clone()),
            roughness_map: Some(map.clone()),
            metallic_map: Some(map.clone()),
            normal_map: Some(map.clone()),
            occlusion_map: Some(map),
            normal_convention: super::super::material::NormalConvention::NegativeY,
            normal_scale: 0.7,
            ..Material::default()
        };
        let before = source.clone();
        let baked = atlas.rebake(&source, Filter::Linear).unwrap();
        assert_eq!(before, source);
        let mut factors = baked.clone();
        factors.colour_map = source.colour_map.clone();
        factors.emissive_map = source.emissive_map.clone();
        factors.roughness_map = source.roughness_map.clone();
        factors.metallic_map = source.metallic_map.clone();
        factors.normal_map = source.normal_map.clone();
        factors.occlusion_map = source.occlusion_map.clone();
        factors.sampler = source.sampler;
        assert_eq!(factors, source);
        assert_eq!(baked.sampler.mipmap, None);
        assert_eq!(baked.sampler.wrap_u, Wrap::Clamp);
        let pixel = |map: &Option<Map>, x: usize, y: usize| {
            let i = 4 * (y * 16 + x);
            map.as_ref().unwrap().rgba[i..i + 4].to_vec()
        };
        // x=4 maps to old u=0.5. x=0 is padding at old u=-0.5,
        // which repeats to the same colour, rather than an empty gutter.
        for x in [0, 4] {
            for map in [&baked.colour_map, &baked.emissive_map] {
                assert_eq!(pixel(map, x, 2), vec![188, 188, 188, 128]);
            }
            for map in [
                &baked.roughness_map,
                &baked.metallic_map,
                &baked.normal_map,
                &baked.occlusion_map,
            ] {
                assert_eq!(pixel(map, x, 2), vec![128; 4]);
            }
        }
        assert_eq!(pixel(&baked.colour_map, 3, 2), vec![0; 4]);
        assert_eq!(pixel(&baked.colour_map, 5, 2), vec![255; 4]);
        let bytes = super::super::glb::encode(&atlas.mesh, &baked).unwrap();
        let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let doc: serde_json::Value = serde_json::from_slice(&bytes[20..20 + json_len]).unwrap();
        let texture = doc["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"]["index"]
            .as_u64()
            .unwrap() as usize;
        let image = doc["textures"][texture]["source"].as_u64().unwrap() as usize;
        let view =
            &doc["bufferViews"][doc["images"][image]["bufferView"].as_u64().unwrap() as usize];
        let start = 28 + json_len + view["byteOffset"].as_u64().unwrap() as usize;
        let end = start + view["byteLength"].as_u64().unwrap() as usize;
        let decoded = image::load_from_memory(&bytes[start..end])
            .unwrap()
            .to_rgba8();
        assert_eq!(decoded.dimensions(), (16, 16));
        assert_eq!(decoded.as_raw(), &baked.colour_map.as_ref().unwrap().rgba);
        let sampler =
            &doc["samplers"][doc["textures"][texture]["sampler"].as_u64().unwrap() as usize];
        assert_eq!(sampler["minFilter"], 9729);
        assert_eq!(sampler["wrapS"], 33071);
        assert_eq!(sampler["wrapT"], 33071);
    }

    #[test]
    fn packs_separate_overlapping_charts_preserving_geometry_and_mapping() {
        let mut mesh = crate::asset::tests::triangle();
        mesh.positions
            .extend([[2., 0., 0.], [3., 0., 0.], [2., 1., 0.]]);
        mesh.normals.extend([[0., 0., 1.]; 3]);
        mesh.triangles.push([3, 4, 5]);
        mesh.texcoords = Some(vec![
            [0., 0.],
            [1., 0.],
            [0., 1.],
            [0., 0.],
            [1., 0.],
            [0., 1.],
        ]);
        mesh.tangents = None;
        let before = mesh.clone();
        let atlas = pack(&mesh, 64, 32, 2, 8.).unwrap();
        assert_eq!(atlas.charts.len(), 2);
        assert_eq!(atlas.mesh.positions, mesh.positions);
        assert_eq!(atlas.mesh.triangles, mesh.triangles);
        assert_eq!(mesh, before);
        for chart in &atlas.charts {
            for &i in &chart.vertices {
                for k in 0..2 {
                    let restored = (atlas.mesh.texcoords.as_ref().unwrap()[i][k]
                        - chart.uv_offset[k])
                        / chart.uv_scale[k];
                    assert!((restored - mesh.texcoords.as_ref().unwrap()[i][k]).abs() < 1e-12);
                }
            }
        }
        let a = atlas.charts[0].rectangle;
        let b = atlas.charts[1].rectangle;
        assert!(
            a[0] + a[2] <= b[0]
                || b[0] + b[2] <= a[0]
                || a[1] + a[3] <= b[1]
                || b[1] + b[3] <= a[1]
        );
        assert!(pack(&mesh, 16, 16, 2, 32.).is_err());
        assert!(pack(&mesh, 64, 32, 0, 8.).is_err());
    }
    #[test]
    fn rejects_overlapping_triangles_in_one_chart_but_allows_shared_edges() {
        assert!(!overlap(
            [
                [-0.6587354801919514, -0.42541935023993976],
                [-0.8, -0.44],
                [-0.7887661281699888, -0.49889515352989966]
            ],
            [
                [-0.7887661281699888, -0.49889515352989966],
                [-0.645254833995939, -0.44525483399593907],
                [-0.6587354801919514, -0.42541935023993976]
            ],
        ));
        let mesh = Mesh {
            positions: vec![
                [0., 0., 0.],
                [1., 0., 0.],
                [0., 1., 0.],
                [2., 0., 0.],
                [0., 2., 0.],
            ],
            normals: vec![[0., 0., 1.]; 5],
            texcoords: Some(vec![[0., 0.], [1., 0.], [0., 1.], [0.5, 0.], [0., 0.5]]),
            tangents: None,
            triangles: vec![[0, 1, 2], [0, 3, 4]],
        };
        assert!(
            pack(&mesh, 64, 64, 2, 8.)
                .unwrap_err()
                .to_string()
                .contains("overlapping")
        );
        let mut square = mesh;
        square.positions.truncate(4);
        square.normals.truncate(4);
        square.positions[3] = [1., 1., 0.];
        square.texcoords = Some(vec![[0., 0.], [1., 0.], [0., 1.], [1., 1.]]);
        square.triangles = vec![[0, 1, 2], [1, 3, 2]];
        assert_eq!(pack(&square, 64, 64, 2, 8.).unwrap().charts.len(), 1);
    }
}
