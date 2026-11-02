//! Bidirectional, sampled closest-triangle distances. These are sample statistics,
//! not continuous Hausdorff bounds or area-weighted surface percentiles.
use super::{cross, dot, sub};
use anyhow::{Result, ensure};
use serde::Serialize;
use std::collections::BTreeSet;
type P = [f64; 3];

#[derive(Debug, Serialize)]
pub struct Distribution {
    pub samples: usize,
    pub max_mm: f64,
    pub p95_mm: f64,
    pub rms_mm: f64,
}
#[derive(Debug, Serialize)]
pub struct Comparison {
    pub source_to_lod: Distribution,
    pub lod_to_source: Distribution,
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
fn mix(a: P, b: P, t: f64) -> P {
    std::array::from_fn(|i| a[i] * (1. - t) + b[i] * t)
}
fn distance2(a: P, b: P) -> f64 {
    let d = sub(a, b);
    dot(d, d)
}
fn segment2(p: P, a: P, b: P) -> f64 {
    let d = sub(b, a);
    let n = dot(d, d);
    distance2(
        p,
        mix(
            a,
            b,
            if n > 0. {
                (dot(sub(p, a), d) / n).clamp(0., 1.)
            } else {
                0.
            },
        ),
    )
}
fn triangle2(p: P, [a, b, c]: [P; 3]) -> f64 {
    let n = cross(sub(b, a), sub(c, a));
    let n2 = dot(n, n);
    if n2 > 0. {
        let t = dot(sub(p, a), n) / n2;
        let q = std::array::from_fn(|i| p[i] - t * n[i]);
        if [(a, b), (b, c), (c, a)]
            .iter()
            .all(|&(a, b)| dot(cross(sub(b, a), sub(q, a)), n) >= 0.)
        {
            return distance2(p, q);
        }
    }
    segment2(p, a, b)
        .min(segment2(p, b, c))
        .min(segment2(p, c, a))
}
impl Tree {
    fn build(triangles: &[[P; 3]], mut indices: Vec<usize>) -> Self {
        let mut low = [f64::INFINITY; 3];
        let mut high = [f64::NEG_INFINITY; 3];
        for &i in &indices {
            for p in triangles[i] {
                for k in 0..3 {
                    low[k] = low[k].min(p[k]);
                    high[k] = high[k].max(p[k]);
                }
            }
        }
        let kind = if indices.len() <= 8 {
            Kind::Leaf(indices)
        } else {
            let axis = (0..3)
                .max_by(|&a, &b| (high[a] - low[a]).total_cmp(&(high[b] - low[b])))
                .unwrap();
            let centre = |i: usize| triangles[i].iter().map(|p| p[axis] / 3.).sum::<f64>();
            indices.sort_by(|&a, &b| centre(a).total_cmp(&centre(b)).then(a.cmp(&b)));
            let right = indices.split_off(indices.len() / 2);
            Kind::Branch(
                Box::new(Self::build(triangles, indices)),
                Box::new(Self::build(triangles, right)),
            )
        };
        Self { low, high, kind }
    }
    fn bound(&self, p: P) -> f64 {
        (0..3)
            .map(|i| (p[i] - p[i].clamp(self.low[i], self.high[i])).powi(2))
            .sum()
    }
    fn nearest(&self, triangles: &[[P; 3]], p: P, best: &mut f64) {
        if self.bound(p) > *best {
            return;
        }
        match &self.kind {
            Kind::Leaf(ids) => {
                for &i in ids {
                    *best = best.min(triangle2(p, triangles[i]));
                }
            }
            Kind::Branch(a, b) => {
                let (a, b) = if a.bound(p) <= b.bound(p) {
                    (a, b)
                } else {
                    (b, a)
                };
                a.nearest(triangles, p, best);
                b.nearest(triangles, p, best);
            }
        }
    }
}
fn triangles(points: &[P], faces: &[[u32; 3]]) -> Result<Vec<[P; 3]>> {
    ensure!(
        !faces.is_empty() && points.iter().flatten().all(|x| x.is_finite()),
        "surface requires finite positions and triangles"
    );
    ensure!(
        faces.iter().flatten().all(|&i| (i as usize) < points.len()),
        "surface triangle index is out of range"
    );
    let result: Vec<_> = faces
        .iter()
        .map(|f| f.map(|i| points[i as usize]))
        .collect();
    ensure!(
        result.iter().all(|[a, b, c]| dot(
            cross(sub(*b, *a), sub(*c, *a)),
            cross(sub(*b, *a), sub(*c, *a))
        )
        .is_finite()),
        "surface triangle arithmetic exceeds numeric range"
    );
    Ok(result)
}
fn distribution(points: &[P], faces: &[[u32; 3]], target: &[[P; 3]]) -> Result<Distribution> {
    let tree = Tree::build(target, (0..target.len()).collect());
    let vertices: BTreeSet<_> = faces.iter().flatten().copied().collect();
    let edges: BTreeSet<_> = faces
        .iter()
        .flat_map(|&[a, b, c]| {
            [
                (a.min(b), a.max(b)),
                (b.min(c), b.max(c)),
                (c.min(a), c.max(a)),
            ]
        })
        .collect();
    let mut distances = Vec::with_capacity(vertices.len() + edges.len() + faces.len());
    let mut measure = |p: P| {
        let mut d = f64::INFINITY;
        tree.nearest(target, p, &mut d);
        distances.push(d.sqrt());
    };
    for i in vertices {
        measure(points[i as usize]);
    }
    for (a, b) in edges {
        measure(mix(points[a as usize], points[b as usize], 0.5));
    }
    for &[a, b, c] in faces {
        measure(std::array::from_fn(|k| {
            points[a as usize][k] / 3. + points[b as usize][k] / 3. + points[c as usize][k] / 3.
        }));
    }
    ensure!(
        distances.iter().all(|x| x.is_finite()),
        "surface distance exceeds numeric range"
    );
    distances.sort_by(f64::total_cmp);
    let n = distances.len();
    let rms = (distances.iter().map(|d| d * d / n as f64).sum::<f64>()).sqrt();
    ensure!(rms.is_finite(), "surface RMS exceeds numeric range");
    Ok(Distribution {
        samples: n,
        max_mm: distances[n - 1],
        p95_mm: distances[(95 * n).div_ceil(100) - 1],
        rms_mm: rms,
    })
}

pub fn compare(
    source: &[P],
    source_faces: &[[u32; 3]],
    lod: &[P],
    lod_faces: &[[u32; 3]],
) -> Result<Comparison> {
    let source_triangles = triangles(source, source_faces)?;
    let lod_triangles = triangles(lod, lod_faces)?;
    Ok(Comparison {
        source_to_lod: distribution(source, source_faces, &lod_triangles)?,
        lod_to_source: distribution(lod, lod_faces, &source_triangles)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn analytic_planes_missing_surface_and_retriangulation() {
        let p = vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [1., 1., 0.]];
        let f = [[0, 1, 2], [1, 3, 2]];
        let q: Vec<_> = p.iter().map(|p| [p[0], p[1], 0.25]).collect();
        let d = compare(&p, &f, &q, &f).unwrap();
        assert_eq!(d.source_to_lod.max_mm, 0.25);
        assert_eq!(d.lod_to_source.p95_mm, 0.25);
        let missing = compare(&p, &f, &p, &f[..1]).unwrap();
        assert!((missing.source_to_lod.max_mm - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-12);
        assert!(missing.lod_to_source.max_mm < 1e-12);
        assert!(
            compare(&p, &f, &p, &[[0, 1, 3], [0, 3, 2]])
                .unwrap()
                .source_to_lod
                .max_mm
                < 1e-12
        );
        assert!(compare(&p, &f, &p, &[[0, 1, 9]]).is_err());
    }
    #[test]
    fn hierarchy_matches_exhaustive_triangle_distances() {
        let triangles: Vec<_> = (0..80)
            .map(|i| {
                let x = i as f64;
                [[x, 0., 0.], [x + 0.8, 0., 0.], [x, 1., 0.]]
            })
            .collect();
        let tree = Tree::build(&triangles, (0..triangles.len()).collect());
        for i in 0..100 {
            let p = [i as f64 * 0.83 - 4., (i as f64).sin(), 0.3];
            let mut d = f64::INFINITY;
            tree.nearest(&triangles, p, &mut d);
            let expected = triangles
                .iter()
                .map(|t| triangle2(p, *t))
                .fold(f64::INFINITY, f64::min);
            assert!((d - expected).abs() < 1e-12);
        }
    }
}
