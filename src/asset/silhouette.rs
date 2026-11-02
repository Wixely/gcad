//! Opaque, double-sided orthographic coverage comparisons at a declared resolution.
//! Pixel-centre rasterisation is a sampled metric, not a continuous contour bound.
use anyhow::{Result, ensure};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct View {
    pub axis: usize,
    pub resolution: usize,
    pub mm_per_pixel: f64,
    pub source_pixels: usize,
    pub lod_pixels: usize,
    pub differing_pixels: usize,
    pub union_pixels: usize,
    /// Symmetric-difference area / union area, or null when both masks are empty.
    pub difference_fraction: Option<f64>,
}

fn raster(points: &[[f64; 2]], faces: &[[u32; 3]], resolution: usize) -> Vec<bool> {
    let mut mask = vec![false; resolution * resolution];
    let edge = |a: [f64; 2], b: [f64; 2], p: [f64; 2]| {
        (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
    };
    for f in faces {
        let [a, b, c] = f.map(|i| points[i as usize]);
        let area = edge(a, b, c);
        if area == 0. {
            continue;
        }
        let low = std::array::from_fn::<_, 2, _>(|k| {
            a[k].min(b[k])
                .min(c[k])
                .floor()
                .clamp(0., (resolution - 1) as f64) as usize
        });
        let high = std::array::from_fn::<_, 2, _>(|k| {
            a[k].max(b[k])
                .max(c[k])
                .ceil()
                .clamp(0., (resolution - 1) as f64) as usize
        });
        for y in low[1]..=high[1] {
            for x in low[0]..=high[0] {
                let p = [x as f64 + 0.5, y as f64 + 0.5];
                if [(a, b), (b, c), (c, a)]
                    .iter()
                    .all(|&(a, b)| edge(a, b, p) * area >= 0.)
                {
                    mask[y * resolution + x] = true;
                }
            }
        }
    }
    mask
}

pub fn compare(
    source: &[[f64; 3]],
    source_faces: &[[u32; 3]],
    lod: &[[f64; 3]],
    lod_faces: &[[u32; 3]],
    resolution: usize,
) -> Result<Vec<View>> {
    ensure!(
        (16..=2048).contains(&resolution),
        "silhouette resolution must be 16..2048"
    );
    for (points, faces) in [(source, source_faces), (lod, lod_faces)] {
        ensure!(
            !faces.is_empty()
                && points.iter().flatten().all(|x| x.is_finite())
                && faces.iter().flatten().all(|&i| (i as usize) < points.len()),
            "invalid silhouette mesh"
        );
    }
    let mut result = Vec::new();
    for axis in 0..3 {
        let projected: Vec<_> = (0..3).filter(|&i| i != axis).collect();
        let mut low = [f64::INFINITY; 2];
        let mut high = [f64::NEG_INFINITY; 2];
        // Use referenced vertices only; unused payload must not change the camera.
        for (points, faces) in [(source, source_faces), (lod, lod_faces)] {
            for &i in faces.iter().flatten() {
                for k in 0..2 {
                    let p = points[i as usize][projected[k]];
                    low[k] = low[k].min(p);
                    high[k] = high[k].max(p);
                }
            }
        }
        let span = (high[0] - low[0]).max(high[1] - low[1]);
        ensure!(span.is_finite(), "silhouette extent exceeds numeric range");
        let scale = if span > 0. {
            span / (resolution - 4) as f64
        } else {
            1.
        };
        ensure!(
            scale > 0. && scale.is_finite(),
            "silhouette pixel scale exceeds numeric range"
        );
        let project = |points: &[[f64; 3]]| {
            points
                .iter()
                .map(|p| {
                    std::array::from_fn(|k| {
                        (p[projected[k]] - (low[k] * 0.5 + high[k] * 0.5)) / scale
                            + resolution as f64 / 2.
                    })
                })
                .collect::<Vec<_>>()
        };
        let a = raster(&project(source), source_faces, resolution);
        let b = raster(&project(lod), lod_faces, resolution);
        let count = |mask: &[bool]| mask.iter().filter(|&&v| v).count();
        let union = a.iter().zip(&b).filter(|(a, b)| **a || **b).count();
        let difference = a.iter().zip(&b).filter(|(a, b)| a != b).count();
        result.push(View {
            axis,
            resolution,
            mm_per_pixel: scale,
            source_pixels: count(&a),
            lod_pixels: count(&b),
            differing_pixels: difference,
            union_pixels: union,
            difference_fraction: (union > 0).then_some(difference as f64 / union as f64),
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identical_retriangulated_and_missing_coverage() {
        let p = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [1., 1., 0.]];
        let f = [[0, 1, 2], [1, 3, 2]];
        let same = compare(&p, &f, &p, &[[0, 1, 3], [0, 3, 2]], 64).unwrap();
        assert_eq!(same[2].difference_fraction, Some(0.));
        assert_eq!(same[0].difference_fraction, None);
        let half = compare(&p, &f, &p, &f[..1], 64).unwrap();
        assert!((half[2].difference_fraction.unwrap() - 0.5).abs() < 0.02);
        let moved: Vec<_> = p.iter().map(|p| [p[0] + 2., p[1], p[2]]).collect();
        assert_eq!(
            compare(&p, &f, &moved, &f, 64).unwrap()[2].difference_fraction,
            Some(1.)
        );
        assert!(compare(&p, &f, &p, &f, 8).is_err());
    }
}
