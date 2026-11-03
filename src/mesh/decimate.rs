use super::{Mesh, P3, Selection, add, cross, dot, length, normalized, scale, sub};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

type Quadric = [f64; 10];
type Candidate = Reverse<(u64, u32, u32, u32, u32)>;

fn plane_quadric(n: P3, d: f64, w: f64) -> Quadric {
    let [a, b, c] = n;
    [
        a * a * w,
        a * b * w,
        a * c * w,
        a * d * w,
        b * b * w,
        b * c * w,
        b * d * w,
        c * c * w,
        c * d * w,
        d * d * w,
    ]
}

fn plus(a: &Quadric, b: &Quadric) -> Quadric {
    std::array::from_fn(|k| a[k] + b[k])
}

fn error(q: &Quadric, p: P3) -> f64 {
    let [x, y, z] = p;
    q[0] * x * x
        + 2.0 * q[1] * x * y
        + 2.0 * q[2] * x * z
        + 2.0 * q[3] * x
        + q[4] * y * y
        + 2.0 * q[5] * y * z
        + 2.0 * q[6] * y
        + q[7] * z * z
        + 2.0 * q[8] * z
        + q[9]
}

fn optimum(q: &Quadric, a: P3, b: P3) -> P3 {
    let m = [[q[0], q[1], q[2]], [q[1], q[4], q[5]], [q[2], q[5], q[7]]];
    let rhs = [-q[3], -q[6], -q[8]];
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let scale_of = m.iter().flatten().fold(0.0f64, |s, v| s.max(v.abs()));
    if det.abs() > 1.0e-9 * scale_of.powi(3).max(1.0e-30) {
        let solve = |col: usize| {
            let mut r = m;
            for row in 0..3 {
                r[row][col] = rhs[row];
            }
            (r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1])
                - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0])
                + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0]))
                / det
        };
        let p = [solve(0), solve(1), solve(2)];
        let span = length(sub(a, b));
        let mid = scale(add(a, b), 0.5);
        if length(sub(p, mid)) <= 2.0 * span {
            return p;
        }
    }
    [a, scale(add(a, b), 0.5), b]
        .into_iter()
        .min_by(|x, y| error(q, *x).total_cmp(&error(q, *y)))
        .expect("three candidates")
}

impl Mesh {
    pub fn decimate(&mut self, selection: &Selection, target: usize) -> usize {
        let start = self.triangles.len();
        if target >= start {
            return 0;
        }
        let n = self.positions.len();
        let mut quadric = vec![[0.0; 10]; n];
        for &t in &self.triangles {
            let [a, b, c] = self.corners(t);
            let raw = cross(sub(b, a), sub(c, a));
            let area = length(raw) / 2.0;
            if area <= 0.0 {
                continue;
            }
            let normal = normalized(raw);
            let q = plane_quadric(normal, -dot(normal, a), area);
            for i in t {
                quadric[i as usize] = plus(&quadric[i as usize], &q);
            }
        }
        let mut around: Vec<HashSet<u32>> = vec![HashSet::new(); n];
        for (f, t) in self.triangles.iter().enumerate() {
            for &i in t {
                around[i as usize].insert(f as u32);
            }
        }
        let mut alive = vec![true; self.triangles.len()];
        let mut gone = vec![false; n];
        let mut stamp = vec![0u32; n];
        let free = |i: u32| selection.weights[i as usize] > 0.0;
        let mut heap: BinaryHeap<Candidate> = BinaryHeap::new();
        let push = |heap: &mut BinaryHeap<_>,
                    quadric: &[Quadric],
                    positions: &[P3],
                    stamp: &[u32],
                    a: u32,
                    b: u32| {
            let q = plus(&quadric[a as usize], &quadric[b as usize]);
            let p = optimum(&q, positions[a as usize], positions[b as usize]);
            let cost = error(&q, p).max(0.0);
            heap.push(Reverse((
                cost.to_bits(),
                a,
                b,
                stamp[a as usize],
                stamp[b as usize],
            )));
        };
        let mut seen = HashSet::new();
        for t in &self.triangles {
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                if free(a) && free(b) && seen.insert((a.min(b), a.max(b))) {
                    push(
                        &mut heap,
                        &quadric,
                        &self.positions,
                        &stamp,
                        a.min(b),
                        a.max(b),
                    );
                }
            }
        }
        let mut count = start;
        while count > target {
            let Some(Reverse((_, a, b, sa, sb))) = heap.pop() else {
                break;
            };
            if gone[a as usize]
                || gone[b as usize]
                || stamp[a as usize] != sa
                || stamp[b as usize] != sb
            {
                continue;
            }
            let ring = |v: u32, around: &[HashSet<u32>], tris: &[[u32; 3]]| -> HashSet<u32> {
                around[v as usize]
                    .iter()
                    .flat_map(|&f| tris[f as usize])
                    .filter(|&x| x != v)
                    .collect()
            };
            let (ra, rb) = (
                ring(a, &around, &self.triangles),
                ring(b, &around, &self.triangles),
            );
            let shared: Vec<u32> = ra.intersection(&rb).copied().collect();
            let both: Vec<u32> = around[a as usize]
                .intersection(&around[b as usize])
                .copied()
                .collect();
            if shared.len() != 2 || both.len() != 2 {
                continue;
            }
            let q = plus(&quadric[a as usize], &quadric[b as usize]);
            let p = optimum(&q, self.positions[a as usize], self.positions[b as usize]);
            let flips = [a, b].iter().any(|&v| {
                around[v as usize].iter().any(|&f| {
                    let t = self.triangles[f as usize];
                    if t.contains(&a) && t.contains(&b) {
                        return false;
                    }
                    let before = cross(
                        sub(self.positions[t[1] as usize], self.positions[t[0] as usize]),
                        sub(self.positions[t[2] as usize], self.positions[t[0] as usize]),
                    );
                    let moved = t.map(|i| {
                        if i == v {
                            p
                        } else {
                            self.positions[i as usize]
                        }
                    });
                    let after = cross(sub(moved[1], moved[0]), sub(moved[2], moved[0]));
                    dot(normalized(before), normalized(after)) < 0.2 || length(after) < 1.0e-14
                })
            });
            if flips {
                continue;
            }
            for &f in &both {
                alive[f as usize] = false;
                for i in self.triangles[f as usize] {
                    around[i as usize].remove(&f);
                }
                count -= 1;
            }
            let moving: Vec<u32> = around[b as usize].iter().copied().collect();
            for f in moving {
                for i in self.triangles[f as usize].iter_mut() {
                    if *i == b {
                        *i = a;
                    }
                }
                around[a as usize].insert(f);
            }
            around[b as usize].clear();
            gone[b as usize] = true;
            self.positions[a as usize] = p;
            quadric[a as usize] = q;
            stamp[a as usize] += 1;
            let neighbours: HashSet<u32> = ring(a, &around, &self.triangles);
            for v in neighbours {
                if free(a) && free(v) {
                    push(
                        &mut heap,
                        &quadric,
                        &self.positions,
                        &stamp,
                        a.min(v),
                        a.max(v),
                    );
                }
            }
        }
        let mut remap: HashMap<u32, u32> = HashMap::new();
        let mut positions = Vec::new();
        let mut labels = Vec::new();
        let mut triangles = Vec::new();
        for (f, t) in self.triangles.iter().enumerate() {
            if !alive[f] {
                continue;
            }
            triangles.push(t.map(|i| {
                *remap.entry(i).or_insert_with(|| {
                    positions.push(self.positions[i as usize]);
                    labels.push(self.labels[i as usize]);
                    (positions.len() - 1) as u32
                })
            }));
        }
        self.positions = positions;
        self.labels = labels;
        self.triangles = triangles;
        start - self.triangles.len()
    }
}
