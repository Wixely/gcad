use super::{Mesh, P3, add, dot, length, normalized, scale, sub};
use anyhow::{Result, bail};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Combine {
    Add,
    Cut,
    Intersect,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Ellipsoid {
        centre: P3,
        radii: P3,
        rotation: [[f64; 3]; 3],
    },
    Limb {
        joints: Vec<P3>,
        radii: Vec<f64>,
    },
    Box {
        centre: P3,
        half: P3,
        round: f64,
        rotation: [[f64; 3]; 3],
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Primitive {
    pub shape: Shape,
    pub blend: f64,
    pub combine: Combine,
    pub part: String,
}

fn transposed_apply(m: &[[f64; 3]; 3], v: P3) -> P3 {
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
    ]
}

fn ellipsoid(p: P3, r: P3) -> f64 {
    let k0 = length([p[0] / r[0], p[1] / r[1], p[2] / r[2]]);
    let k1 = length([
        p[0] / (r[0] * r[0]),
        p[1] / (r[1] * r[1]),
        p[2] / (r[2] * r[2]),
    ]);
    if k1 < 1.0e-12 {
        return -r[0].min(r[1]).min(r[2]);
    }
    k0 * (k0 - 1.0) / k1
}

fn round_cone(p: P3, a: P3, b: P3, ra: f64, rb: f64) -> f64 {
    let ba = sub(b, a);
    let l2 = dot(ba, ba);
    if l2 < 1.0e-18 {
        return length(sub(p, a)) - ra.max(rb);
    }
    let rr = ra - rb;
    let a2 = l2 - rr * rr;
    let il2 = 1.0 / l2;
    let pa = sub(p, a);
    let y = dot(pa, ba);
    let z = y - l2;
    let xv = sub(scale(pa, l2), scale(ba, y));
    let x2 = dot(xv, xv);
    let y2 = y * y * l2;
    let z2 = z * z * l2;
    let k = rr.signum() * rr * rr * x2;
    if z.signum() * a2 * z2 > k {
        return (x2 + z2).sqrt() * il2 - rb;
    }
    if y.signum() * a2 * y2 < k {
        return (x2 + y2).sqrt() * il2 - ra;
    }
    ((x2 * a2 * il2).sqrt() + y * rr) * il2 - ra
}

fn rounded_box(p: P3, half: P3, round: f64) -> f64 {
    let q: P3 = std::array::from_fn(|k| p[k].abs() - (half[k] - round));
    let outside = length(q.map(|v| v.max(0.0)));
    outside + q[0].max(q[1]).max(q[2]).min(0.0) - round
}

impl Shape {
    pub fn distance(&self, p: P3) -> f64 {
        match self {
            Shape::Ellipsoid {
                centre,
                radii,
                rotation,
            } => ellipsoid(transposed_apply(rotation, sub(p, *centre)), *radii),
            Shape::Limb { joints, radii } => joints
                .windows(2)
                .zip(radii.windows(2))
                .map(|(j, r)| round_cone(p, j[0], j[1], r[0], r[1]))
                .fold(f64::INFINITY, f64::min),
            Shape::Box {
                centre,
                half,
                round,
                rotation,
            } => rounded_box(transposed_apply(rotation, sub(p, *centre)), *half, *round),
        }
    }

    pub fn bounds(&self) -> (P3, P3) {
        match self {
            Shape::Ellipsoid { centre, radii, .. } => {
                let r = radii[0].max(radii[1]).max(radii[2]);
                (sub(*centre, [r; 3]), add(*centre, [r; 3]))
            }
            Shape::Limb { joints, radii } => {
                let r = radii.iter().cloned().fold(0.0, f64::max);
                joints.iter().fold(
                    ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]),
                    |(lo, hi), j| {
                        (
                            std::array::from_fn(|k| lo[k].min(j[k] - r)),
                            std::array::from_fn(|k| hi[k].max(j[k] + r)),
                        )
                    },
                )
            }
            Shape::Box { centre, half, .. } => {
                let r = length(*half);
                (sub(*centre, [r; 3]), add(*centre, [r; 3]))
            }
        }
    }

    pub fn mirrored(&self, axis: usize, at: f64) -> Shape {
        let flip = |p: P3| {
            let mut q = p;
            q[axis] = 2.0 * at - q[axis];
            q
        };
        let flip_rotation = |m: &[[f64; 3]; 3]| -> [[f64; 3]; 3] {
            let s = |i: usize| if i == axis { -1.0 } else { 1.0 };
            std::array::from_fn(|i| std::array::from_fn(|j| m[i][j] * s(i) * s(j)))
        };
        match self {
            Shape::Ellipsoid {
                centre,
                radii,
                rotation,
            } => Shape::Ellipsoid {
                centre: flip(*centre),
                radii: *radii,
                rotation: flip_rotation(rotation),
            },
            Shape::Limb { joints, radii } => Shape::Limb {
                joints: joints.iter().map(|&j| flip(j)).collect(),
                radii: radii.clone(),
            },
            Shape::Box {
                centre,
                half,
                round,
                rotation,
            } => Shape::Box {
                centre: flip(*centre),
                half: *half,
                round: *round,
                rotation: flip_rotation(rotation),
            },
        }
    }
}

fn smooth_union(a: f64, b: f64, k: f64) -> f64 {
    if k <= 0.0 || !b.is_finite() {
        return a.min(b);
    }
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b + (a - b) * h - k * h * (1.0 - h)
}

fn smooth_cut(a: f64, b: f64, k: f64) -> f64 {
    -smooth_union(-a, b, k)
}

fn smooth_intersect(a: f64, b: f64, k: f64) -> f64 {
    -smooth_union(-a, -b, k)
}

pub fn field(primitives: &[Primitive], p: P3) -> f64 {
    primitives.iter().fold(f64::INFINITY, |d, prim| {
        let e = prim.shape.distance(p);
        match prim.combine {
            Combine::Add => smooth_union(e, d, prim.blend),
            Combine::Cut => smooth_cut(d, e, prim.blend),
            Combine::Intersect => smooth_intersect(d, e, prim.blend),
        }
    })
}

fn owner(primitives: &[Primitive], p: P3) -> usize {
    primitives
        .iter()
        .enumerate()
        .filter(|(_, prim)| prim.combine == Combine::Add)
        .map(|(i, prim)| (i, prim.shape.distance(p)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(0, |(i, _)| i)
}

fn gradient(primitives: &[Primitive], p: P3, h: f64) -> P3 {
    std::array::from_fn(|k| {
        let mut a = p;
        let mut b = p;
        a[k] += h;
        b[k] -= h;
        (field(primitives, a) - field(primitives, b)) / (2.0 * h)
    })
}

pub const MAX_CELLS: usize = 400;

pub fn remesh(primitives: &[Primitive], cell: f64) -> Result<Mesh> {
    if primitives.iter().all(|p| p.combine != Combine::Add) {
        bail!("there is nothing to mesh: add a `blob` or `limb` first");
    }
    if cell <= 0.0 {
        bail!("the cell size must be positive, got {cell}");
    }
    let (lo, hi) = primitives
        .iter()
        .filter(|p| p.combine == Combine::Add)
        .map(|p| {
            let (lo, hi) = p.shape.bounds();
            (sub(lo, [p.blend; 3]), add(hi, [p.blend; 3]))
        })
        .fold(
            ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]),
            |(a, b), (lo, hi)| {
                (
                    std::array::from_fn(|k| a[k].min(lo[k])),
                    std::array::from_fn(|k| b[k].max(hi[k])),
                )
            },
        );
    let lo = sub(lo, [2.0 * cell; 3]);
    let hi = add(hi, [2.0 * cell; 3]);
    let n: [usize; 3] = std::array::from_fn(|k| ((hi[k] - lo[k]) / cell).ceil() as usize + 1);
    if n.iter().any(|&c| c > MAX_CELLS) {
        bail!(
            "a {cell} cell makes a {} x {} x {} grid; use a cell of at least {:.3}",
            n[0],
            n[1],
            n[2],
            (0..3)
                .map(|k| (hi[k] - lo[k]) / (MAX_CELLS - 1) as f64)
                .fold(0.0, f64::max)
        );
    }
    let at = |i: usize, j: usize, k: usize| -> P3 {
        [
            lo[0] + i as f64 * cell,
            lo[1] + j as f64 * cell,
            lo[2] + k as f64 * cell,
        ]
    };
    let index = |i: usize, j: usize, k: usize| (k * n[1] + j) * n[0] + i;
    let values: Vec<f64> = {
        use rayon::prelude::*;
        (0..n[0] * n[1] * n[2])
            .into_par_iter()
            .map(|id| {
                let i = id % n[0];
                let j = (id / n[0]) % n[1];
                let k = id / (n[0] * n[1]);
                field(primitives, at(i, j, k))
            })
            .collect()
    };
    let mut vertex_of: HashMap<(usize, usize, usize), [u32; 12]> = HashMap::new();
    let mut positions: Vec<P3> = Vec::new();
    for k in 0..n[2] - 1 {
        for j in 0..n[1] - 1 {
            for i in 0..n[0] - 1 {
                let corners: [f64; 8] = std::array::from_fn(|c| {
                    values[index(i + (c & 1), j + ((c >> 1) & 1), k + ((c >> 2) & 1))]
                });
                let inside = corners.iter().filter(|v| **v < 0.0).count();
                if inside == 0 || inside == 8 {
                    continue;
                }
                let groups = edge_groups(&corners);
                let mut ids = [u32::MAX; 12];
                let mut sums: Vec<(P3, f64)> = Vec::new();
                let mut slot: HashMap<usize, usize> = HashMap::new();
                for (e, &(a, b)) in EDGES.iter().enumerate() {
                    let (va, vb) = (corners[a], corners[b]);
                    if (va < 0.0) == (vb < 0.0) {
                        continue;
                    }
                    let t = va / (va - vb);
                    let pa = corner_offset(a);
                    let pb = corner_offset(b);
                    let p: P3 = std::array::from_fn(|d| pa[d] + (pb[d] - pa[d]) * t);
                    let g = groups[e];
                    let at_slot = *slot.entry(g).or_insert_with(|| {
                        sums.push(([0.0; 3], 0.0));
                        sums.len() - 1
                    });
                    sums[at_slot].0 = add(sums[at_slot].0, p);
                    sums[at_slot].1 += 1.0;
                }
                let base = positions.len() as u32;
                for (sum, count) in &sums {
                    positions.push(add(at(i, j, k), scale(*sum, cell / count)));
                }
                for (e, id) in ids.iter_mut().enumerate() {
                    if let Some(s) = slot.get(&groups[e]) {
                        let (a, b) = EDGES[e];
                        if (corners[a] < 0.0) != (corners[b] < 0.0) {
                            *id = base + *s as u32;
                        }
                    }
                }
                vertex_of.insert((i, j, k), ids);
            }
        }
    }
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    for k in 1..n[2] - 1 {
        for j in 1..n[1] - 1 {
            for i in 1..n[0] - 1 {
                let here = values[index(i, j, k)] < 0.0;
                for axis in 0..3 {
                    let (ni, nj, nk) = match axis {
                        0 => (i + 1, j, k),
                        1 => (i, j + 1, k),
                        _ => (i, j, k + 1),
                    };
                    if ni >= n[0] || nj >= n[1] || nk >= n[2] {
                        continue;
                    }
                    let there = values[index(ni, nj, nk)] < 0.0;
                    if here == there {
                        continue;
                    }
                    let cells = match axis {
                        0 => [(i, j - 1, k - 1), (i, j, k - 1), (i, j, k), (i, j - 1, k)],
                        1 => [(i - 1, j, k - 1), (i - 1, j, k), (i, j, k), (i, j, k - 1)],
                        _ => [(i - 1, j - 1, k), (i, j - 1, k), (i, j, k), (i - 1, j, k)],
                    };
                    let bit = 1usize << axis;
                    let Some(q) = cells
                        .iter()
                        .map(|&(ci, cj, ck)| {
                            let start = (i - ci) | ((j - cj) << 1) | ((k - ck) << 2);
                            let edge = EDGES
                                .iter()
                                .position(|&(a, b)| a == start && b == start | bit)?;
                            let id = vertex_of.get(&(ci, cj, ck))?[edge];
                            (id != u32::MAX).then_some(id)
                        })
                        .collect::<Option<Vec<u32>>>()
                    else {
                        continue;
                    };
                    let quad = if here {
                        [q[0], q[1], q[2], q[3]]
                    } else {
                        [q[0], q[3], q[2], q[1]]
                    };
                    triangles.push([quad[0], quad[1], quad[2]]);
                    triangles.push([quad[0], quad[2], quad[3]]);
                }
            }
        }
    }
    let positions: Vec<P3> = {
        use rayon::prelude::*;
        positions
            .par_iter()
            .map(|&p| {
                let mut q = p;
                for _ in 0..3 {
                    let d = field(primitives, q);
                    let g = gradient(primitives, q, cell * 0.05);
                    let gg = dot(g, g);
                    if gg < 1.0e-18 {
                        break;
                    }
                    q = sub(q, scale(g, d / gg));
                }
                if length(sub(q, p)) > cell {
                    add(p, scale(normalized(sub(q, p)), cell))
                } else {
                    q
                }
            })
            .collect()
    };
    let mut parts: Vec<String> = Vec::new();
    let part_of: Vec<u16> = primitives
        .iter()
        .map(|p| match parts.iter().position(|q| *q == p.part) {
            Some(i) => i as u16,
            None => {
                parts.push(p.part.clone());
                (parts.len() - 1) as u16
            }
        })
        .collect();
    let labels = positions
        .iter()
        .map(|&p| part_of[owner(primitives, p)])
        .collect();
    Ok(Mesh {
        positions,
        triangles,
        labels,
        parts,
    })
}

const EDGES: [(usize, usize); 12] = [
    (0, 1),
    (2, 3),
    (4, 5),
    (6, 7),
    (0, 2),
    (1, 3),
    (4, 6),
    (5, 7),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

const FACES: [[usize; 4]; 6] = [
    [0, 2, 6, 4],
    [1, 3, 7, 5],
    [0, 1, 5, 4],
    [2, 3, 7, 6],
    [0, 1, 3, 2],
    [4, 5, 7, 6],
];

fn edge_between(a: usize, b: usize) -> usize {
    let (a, b) = (a.min(b), a.max(b));
    EDGES
        .iter()
        .position(|&e| e == (a, b))
        .expect("cube corners on one face share an edge")
}

fn edge_groups(corners: &[f64; 8]) -> [usize; 12] {
    let mut parent: [usize; 12] = std::array::from_fn(|e| e);
    fn root(parent: &mut [usize; 12], mut e: usize) -> usize {
        while parent[e] != e {
            parent[e] = parent[parent[e]];
            e = parent[e];
        }
        e
    }
    let mut join = |a: usize, b: usize| {
        let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
        parent[ra] = rb;
    };
    let crossing = |a: usize, b: usize| (corners[a] < 0.0) != (corners[b] < 0.0);
    for face in FACES {
        let edges: [usize; 4] = std::array::from_fn(|m| edge_between(face[m], face[(m + 1) % 4]));
        let cut: Vec<usize> = (0..4)
            .filter(|&m| crossing(face[m], face[(m + 1) % 4]))
            .collect();
        match cut.len() {
            2 => join(edges[cut[0]], edges[cut[1]]),
            4 => {
                let centre_inside = face.iter().map(|&c| corners[c]).sum::<f64>() < 0.0;
                for m in 0..4 {
                    let corner_inside = corners[face[(m + 1) % 4]] < 0.0;
                    if corner_inside != centre_inside {
                        join(edges[m], edges[(m + 1) % 4]);
                    }
                }
            }
            _ => {}
        }
    }
    std::array::from_fn(|e| root(&mut parent, e))
}

fn corner_offset(c: usize) -> P3 {
    [(c & 1) as f64, ((c >> 1) & 1) as f64, ((c >> 2) & 1) as f64]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sphere(r: f64) -> Primitive {
        Primitive {
            shape: Shape::Ellipsoid {
                centre: [0.0; 3],
                radii: [r; 3],
                rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            },
            blend: 0.0,
            combine: Combine::Add,
            part: "ball".into(),
        }
    }

    #[test]
    fn round_cone_is_a_capsule_with_equal_radii() {
        let d = round_cone([0.0, 3.0, 5.0], [0.0; 3], [0.0, 0.0, 10.0], 2.0, 2.0);
        assert!((d - 1.0).abs() < 1.0e-9, "{d}");
        let end = round_cone([0.0, 0.0, 13.0], [0.0; 3], [0.0, 0.0, 10.0], 2.0, 2.0);
        assert!((end - 1.0).abs() < 1.0e-9, "{end}");
    }

    #[test]
    fn first_primitive_with_a_blend_still_meshes() {
        let mut first = sphere(5.0);
        first.blend = 2.0;
        let mesh = remesh(&[first], 0.5).expect("meshes");
        assert!(
            mesh.triangles.len() > 100,
            "{} triangles",
            mesh.triangles.len()
        );
    }

    #[test]
    fn creature_blockout_is_closed_at_every_cell_size() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/mesh/creature.gcad"
        ))
        .expect("example");
        let lines: Vec<_> = crate::parse::parse_program(&text)
            .expect("parses")
            .into_iter()
            .take_while(|l| l.op != "remesh")
            .collect();
        let run = crate::model::run(&lines);
        for cell in [0.2, 0.25, 0.3, 0.4, 0.55] {
            let mesh = remesh(&run.model.mesh_body.blockout, cell).expect("meshes");
            assert_eq!(mesh.open_edges(), 0, "cell {cell}");
        }
    }

    #[test]
    fn surface_nets_sphere_is_closed_and_outward() {
        let mesh = remesh(&[sphere(5.0)], 0.5).expect("meshes");
        assert_eq!(mesh.open_edges(), 0);
        let exact = 4.0 / 3.0 * std::f64::consts::PI * 125.0;
        assert!(
            (mesh.volume() - exact).abs() < exact * 0.01,
            "{} vs {exact}",
            mesh.volume()
        );
    }
}
