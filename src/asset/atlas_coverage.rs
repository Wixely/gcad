//! Opt-in, sampled UV-area coverage correction. This cannot recreate lost spatial
//! detail or guarantee a projected silhouette, trilinear coverage or exact area.
use super::{Atlas, Chart, Material, MipChain};
use crate::{asset::material::AlphaMode, model::texture::Map};
use anyhow::{Context, Result, ensure};

const SAMPLES: usize = 1024;
const ITERATIONS: usize = 32;
const EVALUATIONS: usize = ITERATIONS + 4;

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct MaskCoverage {
    pub chart: usize,
    pub level: u32,
    pub samples: usize,
    pub target_pass: usize,
    pub before_pass: usize,
    pub after_pass: usize,
    pub alpha_scale: f64,
}

fn preflight(charts: usize, triangles: usize, levels: u32) -> Result<()> {
    ensure!(
        (1..=13).contains(&levels),
        "mask coverage requires 1..13 mip levels"
    );
    ensure!(
        triangles <= 1024 * 1024,
        "mask coverage exceeds 1 Mi triangle budget"
    );
    let work = charts
        .checked_mul(SAMPLES)
        .and_then(|v| v.checked_mul(levels as usize * (EVALUATIONS + 1)))
        .context("mask coverage work overflow")?;
    ensure!(
        work <= 16 * 1024 * 1024,
        "mask coverage exceeds 16 Mi sample-evaluation budget"
    );
    Ok(())
}

fn radical_inverse(mut index: usize, base: usize) -> f64 {
    let mut value = 0.;
    let mut place = 1. / base as f64;
    while index != 0 {
        value += (index % base) as f64 * place;
        index /= base;
        place /= base as f64;
    }
    value
}

fn chart_samples(atlas: &Atlas, chart: &Chart) -> Result<Vec<[f64; 2]>> {
    let uv = atlas
        .mesh
        .texcoords
        .as_ref()
        .context("mask coverage requires UVs")?;
    let mut cumulative = Vec::with_capacity(chart.triangles.len());
    let mut total = 0.;
    for &index in &chart.triangles {
        let t = atlas
            .mesh
            .triangles
            .get(index)
            .context("invalid coverage triangle")?;
        let [a, b, c] = t.map(|i| uv[i as usize]);
        let area = ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs();
        ensure!(
            area.is_finite() && area > 0.,
            "degenerate coverage UV triangle"
        );
        total += area;
        cumulative.push(total);
    }
    ensure!(total.is_finite() && total > 0., "empty coverage chart");
    let mut triangle = 0;
    Ok((0..SAMPLES)
        .map(|i| {
            let area = (i as f64 + 0.5) / SAMPLES as f64 * total;
            while triangle + 1 < cumulative.len() && cumulative[triangle] < area {
                triangle += 1;
            }
            let t = atlas.mesh.triangles[chart.triangles[triangle]];
            let [a, b, c] = t.map(|i| uv[i as usize]);
            let u = radical_inverse(i + 1, 2).sqrt();
            let v = radical_inverse(i + 1, 3);
            std::array::from_fn(|k| a[k] * (1. - u) + b[k] * u * (1. - v) + c[k] * u * v)
        })
        .collect())
}

fn scaled(alpha: u8, scale: f64) -> u8 {
    (alpha as f64 * scale).round().clamp(0., 255.) as u8
}

fn count(map: &Map, points: &[[f64; 2]], scale: f64, factor: f64, cutoff: f64) -> usize {
    points
        .iter()
        .filter(|uv| {
            let x = uv[0] * map.width as f64 - 0.5;
            let y = uv[1] * map.height as f64 - 0.5;
            let (ix, iy) = (x.floor() as i64, y.floor() as i64);
            let (fx, fy) = (x - x.floor(), y - y.floor());
            let pixel = |x: i64, y: i64| {
                let x = x.clamp(0, map.width as i64 - 1) as usize;
                let y = y.clamp(0, map.height as i64 - 1) as usize;
                scaled(map.rgba[(y * map.width + x) * 4 + 3], scale) as f64 / 255.
            };
            let alpha = pixel(ix, iy) * (1. - fx) * (1. - fy)
                + pixel(ix + 1, iy) * fx * (1. - fy)
                + pixel(ix, iy + 1) * (1. - fx) * fy
                + pixel(ix + 1, iy + 1) * fx * fy;
            alpha * factor >= cutoff
        })
        .count()
}

fn choose_scale(
    map: &Map,
    points: &[[f64; 2]],
    target: usize,
    factor: f64,
    cutoff: f64,
) -> (f64, usize, usize) {
    let before = count(map, points, 1., factor, cutoff);
    let mut best = (1., before);
    let mut consider = |scale: f64, passed: usize| {
        if passed.abs_diff(target) < best.1.abs_diff(target)
            || (passed.abs_diff(target) == best.1.abs_diff(target)
                && (scale - 1.).abs() < (best.0 - 1f64).abs())
        {
            best = (scale, passed);
        }
    };
    if before != target {
        let (mut low, mut high) = (0., 255.);
        consider(low, count(map, points, low, factor, cutoff));
        consider(high, count(map, points, high, factor, cutoff));
        for _ in 0..ITERATIONS {
            let middle = (low + high) * 0.5;
            let passed = count(map, points, middle, factor, cutoff);
            consider(middle, passed);
            if passed < target {
                low = middle;
            } else {
                high = middle;
            }
        }
    }
    (best.0, before, best.1)
}

impl Atlas {
    /// Preserve sampled level-zero mask coverage as closely as a uniform chart
    /// alpha multiplier permits. RGB, other maps, factors and base level remain
    /// unchanged. Reports retain residuals; this is not an exact-coverage promise.
    pub fn rebake_mask_chain(&self, material: &Material, max_level: u32) -> Result<MipChain> {
        material.validate()?;
        let AlphaMode::Mask { cutoff } = material.alpha else {
            anyhow::bail!("mask coverage requires alpha=mask");
        };
        ensure!(
            material.colour_map.is_some(),
            "mask coverage requires a base-colour alpha map"
        );
        let triangles = self.charts.iter().try_fold(0usize, |a, c| {
            a.checked_add(c.triangles.len())
                .context("coverage triangle count overflow")
        })?;
        preflight(self.charts.len(), triangles, max_level)?;
        self.mesh.validate()?;
        let mut chain = self.rebake_chain(material, max_level)?;
        for level in 1..=max_level {
            let layout = self.mip_atlas(level)?;
            for (chart_index, chart) in self.charts.iter().enumerate() {
                let points = chart_samples(self, chart)?;
                let target = count(
                    chain.levels[0].colour_map.as_ref().unwrap(),
                    &points,
                    1.,
                    material.base_colour[3],
                    cutoff,
                );
                let map = chain.levels[level as usize].colour_map.as_mut().unwrap();
                let (scale, before, expected) =
                    choose_scale(map, &points, target, material.base_colour[3], cutoff);
                let [x, y, w, h] = layout.charts[chart_index].rectangle;
                for py in y..y + h {
                    for px in x..x + w {
                        let alpha = &mut map.rgba[(py as usize * map.width + px as usize) * 4 + 3];
                        *alpha = scaled(*alpha, scale);
                    }
                }
                let after = count(map, &points, 1., material.base_colour[3], cutoff);
                ensure!(
                    after == expected,
                    "coverage sampling escaped chart ownership"
                );
                chain.mask_coverage.push(MaskCoverage {
                    chart: chart_index,
                    level,
                    samples: SAMPLES,
                    target_pass: target,
                    before_pass: before,
                    after_pass: after,
                    alpha_scale: scale,
                });
            }
        }
        Ok(chain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn correction_handles_quantization_factors_and_unreachable_coverage() {
        let map = Map {
            width: 4,
            height: 1,
            rgba: [64, 96, 128, 255]
                .into_iter()
                .flat_map(|a| [12, 34, 56, a])
                .collect(),
        };
        let points: Vec<_> = (0..4).map(|i| [(i as f64 + 0.5) / 4., 0.5]).collect();
        let (scale, before, after) = choose_scale(&map, &points, 3, 0.5, 0.25);
        assert_eq!((before, after), (2, 3));
        assert!(scale > 1.);
        let flat = Map {
            width: 1,
            height: 1,
            rgba: vec![12, 34, 56, 64],
        };
        let (scale, before, after) = choose_scale(&flat, &points, 1, 1., 0.5);
        assert_eq!((scale, before, after), (1., 0, 0)); // 0/4 is closer than 4/4 to 1/4.
        assert_eq!(choose_scale(&map, &points, 4, 0., 0.), (1., 4, 4));
        assert_eq!(choose_scale(&map, &points, 0, 0., 0.5), (1., 0, 0));
        assert_eq!(scaled(0, 255.), 0);
        assert!(preflight(usize::MAX, 0, 1).is_err());
        assert!(preflight(1000, 1000, 13).is_err());
        assert!(preflight(1, 1024 * 1024 + 1, 1).is_err());
    }

    #[test]
    fn mask_chain_preserves_base_rgb_other_maps_and_reports_residuals() {
        let mut mesh = crate::asset::tests::triangle();
        mesh.texcoords = Some(vec![[0., 0.], [1., 0.], [0., 1.]]);
        let atlas = super::super::pack(&mesh, 64, 64, 8, 24.).unwrap();
        let map = Map {
            width: 16,
            height: 16,
            rgba: (0..256)
                .flat_map(|i| [40, 80, 120, ((i % 16) * (i % 16)) as u8])
                .collect(),
        };
        let material = Material {
            alpha: AlphaMode::Mask { cutoff: 0.35 },
            base_colour: [1., 1., 1., 0.8],
            colour_map: Some(map.clone()),
            normal_map: Some(map.clone()),
            emissive_map: Some(map),
            ..Default::default()
        };
        let original = material.clone();
        let raw = atlas.rebake_chain(&material, 2).unwrap();
        let corrected = atlas.rebake_mask_chain(&material, 2).unwrap();
        assert_eq!(material, original);
        assert_eq!(corrected.levels[0], raw.levels[0]);
        assert_eq!(corrected.geometry, raw.geometry);
        assert_eq!(corrected.mask_coverage.len(), 2);
        for (a, b) in corrected.levels.iter().zip(&raw.levels) {
            assert_eq!(a.normal_map, b.normal_map);
            assert_eq!(a.emissive_map, b.emissive_map);
            assert_eq!(a.alpha, b.alpha);
            assert_eq!(a.base_colour, b.base_colour);
            for (a, b) in a
                .colour_map
                .as_ref()
                .unwrap()
                .rgba
                .chunks_exact(4)
                .zip(b.colour_map.as_ref().unwrap().rgba.chunks_exact(4))
            {
                assert_eq!(a[..3], b[..3]);
                if b[3] == 0 {
                    assert_eq!(a[3], 0);
                }
            }
        }
        for record in corrected.mask_coverage() {
            assert_eq!(record.samples, 1024);
            assert!(
                record.after_pass.abs_diff(record.target_pass)
                    <= record.before_pass.abs_diff(record.target_pass)
            );
            println!(
                "Mask coverage level {}: target {}, before {}, after {}, scale {}",
                record.level,
                record.target_pass,
                record.before_pass,
                record.after_pass,
                record.alpha_scale
            );
        }
        assert!(
            corrected
                .mask_coverage()
                .iter()
                .any(|r| r.after_pass.abs_diff(r.target_pass)
                    < r.before_pass.abs_diff(r.target_pass))
        );
        let mut invalid = material.clone();
        invalid.alpha = AlphaMode::Blend;
        assert!(atlas.rebake_mask_chain(&invalid, 2).is_err());
        invalid = material.clone();
        invalid.colour_map = None;
        assert!(atlas.rebake_mask_chain(&invalid, 2).is_err());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn authored_mask_coverage_is_opt_in_shared_and_reported() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/game-assets/atlas-mask-coverage.gasset");
        let asset = crate::asset::author::load(&path, &[]).unwrap();
        let chain = asset.scene.materials[0].mip_chain.as_ref().unwrap();
        assert_eq!(asset.scene.materials.len(), 1);
        assert_eq!(
            asset
                .scene
                .nodes
                .iter()
                .filter(|n| n.mesh.is_some())
                .count(),
            2
        );
        assert_eq!(chain.mask_coverage().len(), 12);
        let report = crate::asset::report::inspect(&asset.scene, &asset.skins, 2).unwrap();
        assert_eq!(report.mask_coverage.len(), 1);
        assert_eq!(report.mask_coverage[0].levels, chain.mask_coverage());
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(
            json["mask_coverage"][0]["levels"].as_array().unwrap().len(),
            12
        );
        let text = std::fs::read_to_string(path).unwrap();
        for bad in [
            text.replace("coverage=mask", "coverage=blend"),
            text.replace(" mips=2", ""),
            text.replace("filter=linear", "filter=nearest"),
        ] {
            assert!(crate::asset::author::Document::parse(&bad).is_err());
        }
        assert!(crate::asset::author::Document::parse(&text.replace(" coverage=mask", "")).is_ok());
    }
}
