use super::{Mesh, P3, Selection, add, dot, length, normalized, on_far_side, reflect, scale, sub};
use std::collections::HashMap;

fn edge(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

fn curved_midpoint(a: P3, na: P3, b: P3, nb: P3) -> P3 {
    let d = sub(b, a);
    let bulge = sub(scale(na, dot(d, na)), scale(nb, dot(d, nb)));
    sub(scale(add(a, b), 0.5), scale(bulge, 0.125))
}

impl Mesh {
    pub fn refine_to(&mut self, selection: &Selection, target: f64, rounds: usize) -> usize {
        let mut added = 0;
        let mut selection = selection.clone();
        for _ in 0..rounds {
            let more = self.refine(&selection, target);
            if more == 0 {
                break;
            }
            added += more;
            selection = Selection::new(
                self,
                selection.regions.clone(),
                selection.falloff,
                selection.mirror,
            )
            .unwrap_or(selection);
        }
        added
    }

    pub fn refine(&mut self, selection: &Selection, target: f64) -> usize {
        let normals = self.vertex_normals();
        let picked = |t: &[u32; 3]| t.iter().any(|&i| selection.weights[i as usize] > 0.0);
        let mut split: HashMap<(u32, u32), u32> = HashMap::new();
        for t in self.triangles.iter().filter(|t| picked(t)) {
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                let long = length(sub(self.positions[a as usize], self.positions[b as usize]));
                if long > target {
                    split.entry(edge(a, b)).or_insert(u32::MAX);
                }
            }
        }
        if split.is_empty() {
            return 0;
        }
        loop {
            let mut more = Vec::new();
            for t in &self.triangles {
                let marked: Vec<usize> = (0..3)
                    .filter(|&k| split.contains_key(&edge(t[k], t[(k + 1) % 3])))
                    .collect();
                if marked.len() == 2 {
                    let k = (0..3).find(|k| !marked.contains(k)).expect("one unmarked");
                    more.push(edge(t[k], t[(k + 1) % 3]));
                }
            }
            if more.is_empty() {
                break;
            }
            for e in more {
                split.insert(e, u32::MAX);
            }
        }
        let mut keys: Vec<(u32, u32)> = split.keys().copied().collect();
        keys.sort_unstable();
        for (a, b) in keys {
            let (pa, pb) = (self.positions[a as usize], self.positions[b as usize]);
            let p = curved_midpoint(pa, normals[a as usize], pb, normals[b as usize]);
            let label = self.labels[a as usize];
            self.positions.push(p);
            self.labels.push(label);
            split.insert((a, b), (self.positions.len() - 1) as u32);
        }
        let mid = |a: u32, b: u32| split.get(&edge(a, b)).copied();
        let mut triangles = Vec::with_capacity(self.triangles.len() * 2);
        for &t in &self.triangles {
            let m: [Option<u32>; 3] = std::array::from_fn(|k| mid(t[k], t[(k + 1) % 3]));
            match m.iter().filter(|x| x.is_some()).count() {
                0 => triangles.push(t),
                3 => {
                    let [ab, bc, ca] = m.map(|x| x.expect("all split"));
                    triangles.push([t[0], ab, ca]);
                    triangles.push([ab, t[1], bc]);
                    triangles.push([ca, bc, t[2]]);
                    triangles.push([ab, bc, ca]);
                }
                1 => {
                    let k = (0..3).find(|&k| m[k].is_some()).expect("one split");
                    let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                    let x = m[k].expect("split");
                    triangles.push([a, x, c]);
                    triangles.push([x, b, c]);
                }
                _ => unreachable!("two split edges were closed to three above"),
            }
        }
        let added = split.len();
        self.triangles = triangles;
        added
    }

    pub fn crease(
        &mut self,
        selection: &Selection,
        line: (P3, P3),
        depth: f64,
        width: f64,
    ) -> usize {
        let normals = self.vertex_normals();
        let (a, b) = line;
        let ab = sub(b, a);
        let l2 = dot(ab, ab).max(1.0e-18);
        let mut moved = 0;
        for (i, p) in self.positions.iter_mut().enumerate() {
            let w = selection.weights[i];
            if w <= 0.0 {
                continue;
            }
            let (a, ab) = match on_far_side(selection, *p) {
                Some(axis) => (reflect(a, axis), reflect(ab, axis)),
                None => (a, ab),
            };
            let t = (dot(sub(*p, a), ab) / l2).clamp(0.0, 1.0);
            let off = sub(*p, add(a, scale(ab, t)));
            let n = normals[i];
            let height = dot(off, n);
            if height.abs() > 2.0 * width {
                continue;
            }
            let d = length(sub(off, scale(n, height)));
            if d >= width {
                continue;
            }
            let profile = {
                let s = d / width;
                (1.0 - s * s).powi(2)
            };
            *p = sub(*p, scale(normals[i], depth * w * profile));
            moved += 1;
        }
        moved
    }

    pub fn pinch(&mut self, selection: &Selection, toward: P3, strength: f64) -> usize {
        let normals = self.vertex_normals();
        let mut moved = 0;
        for (i, p) in self.positions.iter_mut().enumerate() {
            let w = selection.weights[i];
            if w <= 0.0 {
                continue;
            }
            let target = match on_far_side(selection, *p) {
                Some(axis) => reflect(toward, axis),
                None => toward,
            };
            let pull = sub(target, *p);
            let n = normals[i];
            let along = sub(pull, scale(n, dot(pull, n)));
            *p = add(*p, scale(along, (strength * w).clamp(-1.0, 1.0)));
            moved += 1;
        }
        moved
    }

    pub fn flatten(&mut self, selection: &Selection, plane: Option<(P3, P3)>, cut: bool) -> usize {
        let (origin, normal) = plane.unwrap_or_else(|| self.fit_plane(selection));
        let n = normalized(normal);
        let mut moved = 0;
        for (i, p) in self.positions.iter_mut().enumerate() {
            let w = selection.weights[i];
            if w <= 0.0 {
                continue;
            }
            let h = dot(sub(*p, origin), n);
            if cut && h >= 0.0 {
                continue;
            }
            *p = sub(*p, scale(n, h * w));
            moved += 1;
        }
        moved
    }

    fn fit_plane(&self, selection: &Selection) -> (P3, P3) {
        let normals = self.vertex_normals();
        let (mut c, mut n, mut total) = ([0.0; 3], [0.0; 3], 0.0);
        for (i, p) in self.positions.iter().enumerate() {
            let w = selection.weights[i];
            if w > 0.0 {
                c = add(c, scale(*p, w));
                n = add(n, scale(normals[i], w));
                total += w;
            }
        }
        (scale(c, 1.0 / total.max(1.0e-12)), normalized(n))
    }

    pub fn noise(&mut self, selection: &Selection, amplitude: f64, size: f64, seed: u32) -> usize {
        let normals = self.vertex_normals();
        let mut moved = 0;
        for (i, p) in self.positions.iter_mut().enumerate() {
            let w = selection.weights[i];
            if w <= 0.0 {
                continue;
            }
            let h = fbm(scale(*p, 1.0 / size), seed);
            *p = add(*p, scale(normals[i], amplitude * h * w));
            moved += 1;
        }
        moved
    }

    pub fn relax(&mut self, selection: &Selection, passes: usize) -> usize {
        let around = self.neighbours();
        for _ in 0..passes {
            let normals = self.vertex_normals();
            let before = self.positions.clone();
            for (i, p) in self.positions.iter_mut().enumerate() {
                let w = selection.weights[i];
                if w <= 0.0 || around[i].is_empty() {
                    continue;
                }
                let sum = around[i]
                    .iter()
                    .fold([0.0; 3], |s, &j| add(s, before[j as usize]));
                let step = sub(scale(sum, 1.0 / around[i].len() as f64), *p);
                let n = normals[i];
                let along = sub(step, scale(n, dot(step, n)));
                *p = add(*p, scale(along, 0.5 * w));
            }
        }
        selection.count()
    }

    pub fn edge_lengths(&self, selection: &Selection) -> (f64, f64) {
        let mut seen = std::collections::HashSet::new();
        let (mut sum, mut count, mut most) = (0.0f64, 0.0f64, 0.0f64);
        for t in &self.triangles {
            if !t.iter().any(|&i| selection.weights[i as usize] > 0.0) {
                continue;
            }
            for k in 0..3 {
                let e = edge(t[k], t[(k + 1) % 3]);
                if seen.insert(e) {
                    let l = length(sub(
                        self.positions[e.0 as usize],
                        self.positions[e.1 as usize],
                    ));
                    sum += l;
                    count += 1.0;
                    most = most.max(l);
                }
            }
        }
        (sum / count.max(1.0), most)
    }
}

fn hash(x: i64, y: i64, z: i64, seed: u32) -> f64 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (z as u64).wrapping_mul(0x1656_67B1_9E37_79F9)
        ^ (seed as u64).wrapping_mul(0x27D4_EB2F_1656_67C5);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 29;
    (h >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

fn value_noise(p: P3, seed: u32) -> f64 {
    let f = p.map(f64::floor);
    let t = std::array::from_fn::<f64, 3, _>(|k| {
        let u = p[k] - f[k];
        u * u * (3.0 - 2.0 * u)
    });
    let [x, y, z] = f.map(|v| v as i64);
    let mut out = 0.0;
    for c in 0..8 {
        let (dx, dy, dz) = (c & 1, (c >> 1) & 1, (c >> 2) & 1);
        let w = [dx, dy, dz]
            .iter()
            .zip(t)
            .map(|(&d, t)| if d == 1 { t } else { 1.0 - t })
            .product::<f64>();
        out += w * hash(x + dx as i64, y + dy as i64, z + dz as i64, seed);
    }
    out
}

pub fn fbm(p: P3, seed: u32) -> f64 {
    let (mut sum, mut amp, mut norm, mut q) = (0.0, 1.0, 0.0, p);
    for octave in 0..4 {
        sum += amp * value_noise(q, seed.wrapping_add(octave));
        norm += amp;
        amp *= 0.5;
        q = scale(q, 2.03);
    }
    sum / norm
}
