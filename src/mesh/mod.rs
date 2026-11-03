pub mod decimate;
pub mod sculpt;
pub mod sdf;

use anyhow::{Context, Result, anyhow, bail};
use std::collections::HashMap;

pub type P3 = [f64; 3];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub positions: Vec<P3>,
    pub triangles: Vec<[u32; 3]>,
    pub labels: Vec<u16>,
    pub parts: Vec<String>,
}

pub fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(a: P3, s: f64) -> P3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

pub fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: P3, b: P3) -> P3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn length(a: P3) -> f64 {
    dot(a, a).sqrt()
}

pub fn normalized(a: P3) -> P3 {
    let l = length(a);
    if l > 1.0e-300 { scale(a, 1.0 / l) } else { a }
}

impl Mesh {
    pub fn part_index(&mut self, name: &str) -> u16 {
        match self.parts.iter().position(|p| p == name) {
            Some(i) => i as u16,
            None => {
                self.parts.push(name.to_string());
                (self.parts.len() - 1) as u16
            }
        }
    }

    pub fn bounds(&self) -> Option<(P3, P3)> {
        let mut it = self.positions.iter();
        let first = *it.next()?;
        Some(it.fold((first, first), |(lo, hi), p| {
            (
                [lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])],
                [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])],
            )
        }))
    }

    pub fn corners(&self, t: [u32; 3]) -> [P3; 3] {
        t.map(|i| self.positions[i as usize])
    }

    pub fn volume(&self) -> f64 {
        self.triangles
            .iter()
            .map(|&t| {
                let [a, b, c] = self.corners(t);
                dot(a, cross(b, c)) / 6.0
            })
            .sum()
    }

    pub fn area(&self) -> f64 {
        self.triangles
            .iter()
            .map(|&t| {
                let [a, b, c] = self.corners(t);
                length(cross(sub(b, a), sub(c, a))) / 2.0
            })
            .sum()
    }

    pub fn vertex_normals(&self) -> Vec<P3> {
        let mut normals = vec![[0.0; 3]; self.positions.len()];
        for &t in &self.triangles {
            let [a, b, c] = self.corners(t);
            let n = cross(sub(b, a), sub(c, a));
            for i in t {
                normals[i as usize] = add(normals[i as usize], n);
            }
        }
        normals.into_iter().map(normalized).collect()
    }

    pub fn neighbours(&self) -> Vec<Vec<u32>> {
        let mut around: Vec<Vec<u32>> = vec![Vec::new(); self.positions.len()];
        for t in &self.triangles {
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                around[a as usize].push(b);
                around[b as usize].push(a);
            }
        }
        for list in &mut around {
            list.sort_unstable();
            list.dedup();
        }
        around
    }

    pub fn open_edges(&self) -> usize {
        let mut count: HashMap<(u32, u32), i32> = HashMap::new();
        for t in &self.triangles {
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                *count.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        count.values().filter(|&&n| n != 2).count()
    }

    pub fn merge(&mut self, other: &Mesh) {
        let base = self.positions.len() as u32;
        let remap: Vec<u16> = other.parts.iter().map(|p| self.part_index(p)).collect();
        self.positions.extend(&other.positions);
        self.labels.extend(
            other
                .labels
                .iter()
                .map(|&l| remap.get(l as usize).copied().unwrap_or(0)),
        );
        self.triangles
            .extend(other.triangles.iter().map(|t| t.map(|i| i + base)));
    }

    pub fn transformed(&self, f: impl Fn(P3) -> P3, flips: bool) -> Mesh {
        Mesh {
            positions: self.positions.iter().map(|&p| f(p)).collect(),
            triangles: if flips {
                self.triangles.iter().map(|t| [t[0], t[2], t[1]]).collect()
            } else {
                self.triangles.clone()
            },
            labels: self.labels.clone(),
            parts: self.parts.clone(),
        }
    }

    pub fn summary(&self) -> String {
        let (lo, hi) = self.bounds().unwrap_or_default();
        let tidy = |v: f64| if v.abs() < 5.0e-4 { 0.0 } else { v };
        let closed = match self.open_edges() {
            0 => String::new(),
            n => format!(", {n} open edges"),
        };
        format!(
            "mesh of {} triangles in {} part(s), volume {:.3}, bbox [{:.3}, {:.3}, {:.3}]..[{:.3}, {:.3}, {:.3}]{closed}",
            self.triangles.len(),
            self.parts.len().max(1),
            self.volume(),
            tidy(lo[0]),
            tidy(lo[1]),
            tidy(lo[2]),
            tidy(hi[0]),
            tidy(hi[1]),
            tidy(hi[2]),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Region {
    All,
    Part(String),
    Sphere(P3, f64),
    Box(P3, P3),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Selection {
    pub regions: Vec<Region>,
    pub falloff: f64,
    pub mirror: Option<usize>,
    pub weights: Vec<f64>,
    pub centre: P3,
}

fn box_distance(p: P3, lo: P3, hi: P3) -> f64 {
    let d: P3 = std::array::from_fn(|k| (lo[k] - p[k]).max(p[k] - hi[k]).max(0.0));
    length(d)
}

fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Selection {
    pub fn new(
        mesh: &Mesh,
        regions: Vec<Region>,
        falloff: f64,
        mirror: Option<usize>,
    ) -> Result<Selection> {
        for region in &regions {
            if let Region::Part(name) = region
                && !mesh.parts.iter().any(|p| p == name)
            {
                bail!(
                    "the mesh has no part `{name}`; its parts are {}",
                    if mesh.parts.is_empty() {
                        "none".to_string()
                    } else {
                        mesh.parts.join(", ")
                    }
                );
            }
        }
        let inside = |p: P3, region: &Region, label: u16| -> f64 {
            match region {
                Region::All => 0.0,
                Region::Part(name) => {
                    if mesh.parts.get(label as usize) == Some(name) {
                        0.0
                    } else {
                        f64::INFINITY
                    }
                }
                Region::Sphere(c, r) => (length(sub(p, *c)) - r).max(0.0),
                Region::Box(lo, hi) => box_distance(p, *lo, *hi),
            }
        };
        let part_regions: Vec<&Region> = regions
            .iter()
            .filter(|r| matches!(r, Region::Part(_)))
            .collect();
        let direct: Vec<bool> = mesh
            .positions
            .iter()
            .enumerate()
            .map(|(i, &p)| {
                let label = mesh.labels.get(i).copied().unwrap_or(0);
                regions.iter().any(|r| inside(p, r, label) == 0.0)
            })
            .collect();
        let core: Vec<bool> = mesh
            .positions
            .iter()
            .enumerate()
            .map(|(i, &p)| {
                let label = mesh.labels.get(i).copied().unwrap_or(0);
                direct[i]
                    || mirror.is_some_and(|axis| {
                        regions
                            .iter()
                            .any(|r| inside(reflect(p, axis), r, label) == 0.0)
                    })
            })
            .collect();
        let distance: Vec<f64> = if falloff <= 0.0 {
            core.iter()
                .map(|&c| if c { 0.0 } else { f64::INFINITY })
                .collect()
        } else if part_regions.is_empty() {
            mesh.positions
                .iter()
                .map(|&p| {
                    let near = |p: P3| {
                        regions
                            .iter()
                            .map(|r| inside(p, r, 0))
                            .fold(f64::INFINITY, f64::min)
                    };
                    let d = near(p);
                    mirror.map_or(d, |axis| d.min(near(reflect(p, axis))))
                })
                .collect()
        } else {
            surface_distance(mesh, &core, falloff)
        };
        let weights: Vec<f64> = distance
            .iter()
            .map(|&d| {
                if d <= 0.0 {
                    1.0
                } else if falloff > 0.0 && d < falloff {
                    1.0 - smoothstep(d / falloff)
                } else {
                    0.0
                }
            })
            .collect();
        let (sum, total) = mesh
            .positions
            .iter()
            .zip(&direct)
            .filter(|(_, c)| **c)
            .fold(([0.0; 3], 0.0), |(s, n), (p, _)| (add(s, *p), n + 1.0));
        if total == 0.0 {
            bail!("the selection holds no vertices of the mesh");
        }
        let centre = match &regions[..] {
            [Region::Sphere(c, _)] => *c,
            _ => scale(sum, 1.0 / total),
        };
        Ok(Selection {
            regions,
            falloff,
            mirror,
            weights,
            centre,
        })
    }

    pub fn count(&self) -> usize {
        self.weights.iter().filter(|&&w| w > 0.0).count()
    }
}

fn surface_distance(mesh: &Mesh, core: &[bool], limit: f64) -> Vec<f64> {
    let around = mesh.neighbours();
    let mut distance: Vec<f64> = core
        .iter()
        .map(|&c| if c { 0.0 } else { f64::INFINITY })
        .collect();
    let mut heap: std::collections::BinaryHeap<std::cmp::Reverse<(u64, u32)>> = distance
        .iter()
        .enumerate()
        .filter(|(_, d)| **d == 0.0)
        .map(|(i, _)| std::cmp::Reverse((0, i as u32)))
        .collect();
    while let Some(std::cmp::Reverse((key, i))) = heap.pop() {
        let d = f64::from_bits(key);
        if d > distance[i as usize] {
            continue;
        }
        for &j in &around[i as usize] {
            let step = d + length(sub(mesh.positions[i as usize], mesh.positions[j as usize]));
            if step < distance[j as usize] && step < limit {
                distance[j as usize] = step;
                heap.push(std::cmp::Reverse((step.to_bits(), j)));
            }
        }
    }
    distance
}

impl Mesh {
    pub fn blend_colours(&self, tints: &[[f64; 3]], soft: &[f64]) -> Vec<[f64; 3]> {
        let own = |i: usize| {
            tints
                .get(self.labels[i] as usize)
                .copied()
                .unwrap_or([0.7; 3])
        };
        let mut colours: Vec<[f64; 3]> = (0..self.positions.len()).map(own).collect();
        if soft.iter().all(|w| *w <= 0.0) {
            return colours;
        }
        let around = self.neighbours();
        for (part, &width) in soft.iter().enumerate() {
            if width <= 0.0 {
                continue;
            }
            let label = part as u16;
            let inside = |i: usize| self.labels[i] == label;
            let border: Vec<bool> = (0..self.positions.len())
                .map(|i| around[i].iter().any(|&j| inside(i) != inside(j as usize)))
                .collect();
            let reach = surface_distance(self, &border, width);
            let theirs = tints.get(part).copied().unwrap_or([0.7; 3]);
            let mut across: Vec<Option<[f64; 3]>> = (0..self.positions.len())
                .map(|i| {
                    if border[i] {
                        let other = if inside(i) {
                            around[i]
                                .iter()
                                .map(|&j| j as usize)
                                .find(|&j| !inside(j))
                                .map(own)
                        } else {
                            Some(own(i))
                        };
                        other
                    } else {
                        None
                    }
                })
                .collect();
            let mut order: Vec<usize> = (0..self.positions.len())
                .filter(|&i| reach[i].is_finite())
                .collect();
            order.sort_by(|&a, &b| reach[a].total_cmp(&reach[b]));
            for &i in &order {
                if across[i].is_some() {
                    continue;
                }
                across[i] = around[i]
                    .iter()
                    .map(|&j| j as usize)
                    .filter(|&j| reach[j] < reach[i] && inside(j) == inside(i))
                    .find_map(|j| across[j]);
            }
            let outside_colour: Vec<[f64; 3]> = (0..self.positions.len())
                .map(|i| if inside(i) { across[i].unwrap_or(colours[i]) } else { colours[i] })
                .collect();
            for i in 0..self.positions.len() {
                let d = reach[i];
                if !d.is_finite() || d >= width {
                    continue;
                }
                let side = if inside(i) { 1.0 } else { -1.0 };
                let t = 0.5 + 0.5 * side * smoothstep(d / width);
                let other = outside_colour[i];
                colours[i] = std::array::from_fn(|k| other[k] + (theirs[k] - other[k]) * t);
            }
        }
        colours
    }
}

pub fn reflect(p: P3, axis: usize) -> P3 {
    let mut q = p;
    q[axis] = -q[axis];
    q
}

pub fn rotation(axis: P3, degrees: f64) -> [[f64; 3]; 3] {
    let [x, y, z] = normalized(axis);
    let (s, c) = degrees.to_radians().sin_cos();
    let t = 1.0 - c;
    [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ]
}

pub fn apply(m: &[[f64; 3]; 3], v: P3) -> P3 {
    [dot(m[0], v), dot(m[1], v), dot(m[2], v)]
}

pub fn on_far_side(selection: &Selection, p: P3) -> Option<usize> {
    selection
        .mirror
        .filter(|&axis| p[axis] * selection.centre[axis] < 0.0)
}

impl Mesh {
    pub fn deform(&mut self, selection: &Selection, f: impl Fn(P3) -> P3) -> usize {
        let mut moved = 0;
        for (i, p) in self.positions.iter_mut().enumerate() {
            let w = selection.weights[i];
            if w <= 0.0 {
                continue;
            }
            let target = match on_far_side(selection, *p) {
                Some(axis) => reflect(f(reflect(*p, axis)), axis),
                None => f(*p),
            };
            *p = add(*p, scale(sub(target, *p), w));
            moved += 1;
        }
        moved
    }

    pub fn inflate(&mut self, selection: &Selection, by: f64) -> usize {
        let normals = self.vertex_normals();
        let mut moved = 0;
        for (i, p) in self.positions.iter_mut().enumerate() {
            let w = selection.weights[i];
            if w > 0.0 {
                *p = add(*p, scale(normals[i], by * w));
                moved += 1;
            }
        }
        moved
    }

    pub fn smooth(&mut self, selection: &Selection, passes: usize) -> usize {
        let around = self.neighbours();
        for _ in 0..passes {
            let before = self.positions.clone();
            for (i, p) in self.positions.iter_mut().enumerate() {
                let w = selection.weights[i];
                if w <= 0.0 || around[i].is_empty() {
                    continue;
                }
                let sum = around[i]
                    .iter()
                    .fold([0.0; 3], |s, &j| add(s, before[j as usize]));
                let mean = scale(sum, 1.0 / around[i].len() as f64);
                *p = add(*p, scale(sub(mean, *p), 0.5 * w));
            }
        }
        selection.count()
    }
}

pub fn read_obj(text: &str) -> Result<Mesh> {
    let mut mesh = Mesh::default();
    let mut part = 0u16;
    let mut has_parts = false;
    for (n, line) in text.lines().enumerate() {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("v") => {
                let p: Vec<f64> = words
                    .take(3)
                    .map(str::parse)
                    .collect::<std::result::Result<_, _>>()
                    .with_context(|| format!("line {}: a `v` needs three numbers", n + 1))?;
                let [x, y, z] = p[..] else {
                    bail!("line {}: a `v` needs three numbers", n + 1);
                };
                mesh.positions.push([x, y, z]);
                mesh.labels.push(part);
            }
            Some("o" | "g") => {
                let name = words.collect::<Vec<_>>().join("_");
                if !name.is_empty() {
                    part = mesh.part_index(&name);
                    has_parts = true;
                }
            }
            Some("f") => {
                let count = mesh.positions.len() as i64;
                let corners: Vec<u32> = words
                    .map(|w| {
                        let index: i64 = w
                            .split('/')
                            .next()
                            .unwrap_or_default()
                            .parse()
                            .map_err(|_| anyhow!("line {}: `{w}` is not a vertex index", n + 1))?;
                        let index = if index < 0 { count + index } else { index - 1 };
                        if index < 0 || index >= count {
                            bail!("line {}: vertex {w} does not exist yet", n + 1);
                        }
                        Ok(index as u32)
                    })
                    .collect::<Result<_>>()?;
                for k in 1..corners.len().saturating_sub(1) {
                    mesh.triangles
                        .push([corners[0], corners[k], corners[k + 1]]);
                }
                if has_parts {
                    for &c in &corners {
                        mesh.labels[c as usize] = part;
                    }
                }
            }
            _ => {}
        }
    }
    if mesh.triangles.is_empty() {
        bail!("the file holds no faces");
    }
    if mesh.parts.is_empty() {
        mesh.parts.push("mesh".to_string());
    }
    Ok(mesh)
}

pub fn from_triangles(points: Vec<[P3; 3]>) -> Mesh {
    let mut index: HashMap<[i64; 3], u32> = HashMap::new();
    let scale_of = points
        .iter()
        .flatten()
        .flatten()
        .fold(1.0_f64, |m, v| m.max(v.abs()));
    let step = scale_of * 1.0e-9;
    let mut mesh = Mesh {
        parts: vec!["mesh".to_string()],
        ..Default::default()
    };
    for tri in points {
        let ids = tri.map(|p| {
            let key = p.map(|c| (c / step).round() as i64);
            *index.entry(key).or_insert_with(|| {
                mesh.positions.push(p);
                mesh.labels.push(0);
                (mesh.positions.len() - 1) as u32
            })
        });
        if ids[0] != ids[1] && ids[1] != ids[2] && ids[0] != ids[2] {
            mesh.triangles.push(ids);
        }
    }
    mesh
}

pub fn write_obj(mesh: &Mesh) -> String {
    write_obj_with(mesh, false)
}

pub fn write_obj_with(mesh: &Mesh, materials: bool) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for p in &mesh.positions {
        let _ = writeln!(out, "v {} {} {}", p[0], p[1], p[2]);
    }
    let mut by_part: Vec<Vec<[u32; 3]>> = vec![Vec::new(); mesh.parts.len().max(1)];
    let last = by_part.len() - 1;
    for &t in &mesh.triangles {
        let label = mesh.labels.get(t[0] as usize).copied().unwrap_or(0) as usize;
        by_part[label.min(last)].push(t);
    }
    for (k, tris) in by_part.iter().enumerate() {
        if tris.is_empty() {
            continue;
        }
        if let Some(name) = mesh.parts.get(k) {
            let _ = writeln!(out, "g {name}");
            if materials {
                let _ = writeln!(out, "usemtl {name}");
            }
        }
        for t in tris {
            let _ = writeln!(out, "f {} {} {}", t[0] + 1, t[1] + 1, t[2] + 1);
        }
    }
    out
}

