use super::{Mesh, cross, dot, sub};
use anyhow::{Result, ensure};
use std::collections::BTreeMap;

fn root(parents: &mut [usize], mut i: usize) -> usize {
    while parents[i] != i {
        parents[i] = parents[parents[i]];
        i = parents[i];
    }
    i
}

impl Mesh {
    /// Rebuild area-weighted normals over connected smooth face fans. Only
    /// exact-position, oppositely directed manifold edges can join fans; point
    /// contacts, boundaries and nonmanifold edges remain split. Apply before UVs
    /// and skinning: complete vertex tuples and their binding stamp change.
    /// The angle is a local edge threshold, not a whole-fan angular bound.
    pub fn crease_normals(&self, angle_degrees: f64) -> Result<Self> {
        self.validate()?;
        ensure!(
            angle_degrees.is_finite() && (0.0..=180.0).contains(&angle_degrees),
            "normal crease angle must be between 0 and 180 degrees"
        );
        ensure!(
            self.texcoords.is_none() && self.tangents.is_none(),
            "rebuild normals before UV and tangent authoring"
        );
        let count = self
            .triangles
            .len()
            .checked_mul(3)
            .ok_or_else(|| anyhow::anyhow!("too many triangle corners"))?;
        let mut parents: Vec<_> = (0..count).collect();
        let mut positions = BTreeMap::new();
        let mut ids = Vec::with_capacity(self.positions.len());
        for p in &self.positions {
            let bits = p.map(|x| if x == 0.0 { 0 } else { x.to_bits() });
            let next = positions.len();
            ids.push(*positions.entry(bits).or_insert(next));
        }
        let mut edges = BTreeMap::<(usize, usize), Vec<(usize, usize)>>::new();
        let mut areas = Vec::new();
        let mut unit = Vec::new();
        for (face, triangle) in self.triangles.iter().enumerate() {
            let [a, b, c] = triangle.map(|i| self.positions[i as usize]);
            let n = cross(sub(b, a), sub(c, a));
            let length = dot(n, n).sqrt();
            ensure!(
                length.is_finite() && length > 0.0,
                "cannot rebuild normals for a degenerate triangle"
            );
            areas.push(n);
            unit.push(n.map(|x| x / length));
            for k in 0..3 {
                let a = ids[triangle[k] as usize];
                let b = ids[triangle[(k + 1) % 3] as usize];
                edges
                    .entry((a.min(b), a.max(b)))
                    .or_default()
                    .push((face, k));
            }
        }
        let cosine = angle_degrees.to_radians().cos();
        for adjacent in edges.values() {
            if let [(a, ka), (b, kb)] = adjacent.as_slice() {
                let ta = self.triangles[*a];
                let tb = self.triangles[*b];
                if ids[ta[*ka] as usize] != ids[tb[(*kb + 1) % 3] as usize]
                    || dot(unit[*a], unit[*b]) + 16.0 * f64::EPSILON < cosine
                {
                    continue;
                }
                for (ca, cb) in [
                    (a * 3 + ka, b * 3 + (kb + 1) % 3),
                    (a * 3 + (ka + 1) % 3, b * 3 + kb),
                ] {
                    let ra = root(&mut parents, ca);
                    let rb = root(&mut parents, cb);
                    parents[ra.max(rb)] = ra.min(rb);
                }
            }
        }
        let mut sums = vec![[0.0; 3]; count];
        for corner in 0..count {
            let r = root(&mut parents, corner);
            for axis in 0..3 {
                sums[r][axis] += areas[corner / 3][axis];
            }
        }
        let mut output = Mesh {
            positions: Vec::new(),
            normals: Vec::new(),
            texcoords: None,
            tangents: None,
            triangles: Vec::new(),
        };
        let mut vertices = BTreeMap::new();
        for (face, triangle) in self.triangles.iter().enumerate() {
            let mut indices = [0; 3];
            for k in 0..3 {
                let r = root(&mut parents, face * 3 + k);
                indices[k] = if let Some(&index) = vertices.get(&r) {
                    index
                } else {
                    let index = u32::try_from(output.positions.len())?;
                    let n = sums[r];
                    let length = dot(n, n).sqrt();
                    ensure!(
                        length.is_finite() && length > 0.0,
                        "smooth fan has an undefined normal"
                    );
                    output.positions.push(self.positions[triangle[k] as usize]);
                    output.normals.push(n.map(|x| x / length));
                    vertices.insert(r, index);
                    index
                };
            }
            output.triangles.push(indices);
        }
        output.validate()?;
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wedge() -> Mesh {
        Mesh {
            positions: vec![
                [0., 0., 0.],
                [1., 0., 0.],
                [0., 1., 0.],
                [1., 0., 0.],
                [0., 0., 0.],
                [0., 0., 1.],
            ],
            normals: vec![[0., 0., 1.]; 6],
            texcoords: None,
            tangents: None,
            triangles: vec![[0, 1, 2], [3, 4, 5]],
        }
    }
    #[test]
    fn crease_splits_and_smooth_fans_join_exact_edges_without_moving_faces() {
        let input = wedge();
        let hard = input.crease_normals(89.).unwrap();
        let smooth = input.crease_normals(90.).unwrap();
        assert_eq!(hard.positions.len(), 6);
        assert_eq!(smooth.positions.len(), 4);
        for mesh in [&hard, &smooth] {
            for (original, changed) in input.triangles.iter().zip(&mesh.triangles) {
                assert_eq!(
                    original.map(|i| input.positions[i as usize]),
                    changed.map(|i| mesh.positions[i as usize])
                );
            }
        }
        let expected = std::f64::consts::FRAC_1_SQRT_2;
        assert!((smooth.normals[0][1] - expected).abs() < 1e-12);
        assert!((smooth.normals[0][2] - expected).abs() < 1e-12);
        assert_ne!(
            input.binding_stamp().unwrap(),
            smooth.binding_stamp().unwrap()
        );
        assert_eq!(smooth, input.crease_normals(90.).unwrap());
    }
    #[test]
    fn nonmanifold_edges_and_point_contacts_do_not_join() {
        let mut mesh = wedge();
        mesh.positions.push([0., -1., 0.]);
        mesh.normals.push([0., 0., 1.]);
        mesh.triangles.push([0, 1, 6]);
        assert_eq!(mesh.crease_normals(180.).unwrap().positions.len(), 9);
        mesh.positions.push([-1., 0., 0.]);
        mesh.normals.push([0., 0., 1.]);
        mesh.triangles[2] = [0, 6, 7];
        assert_eq!(mesh.crease_normals(90.).unwrap().positions.len(), 7);
    }
    #[test]
    fn rejects_invalid_angles_and_existing_uvs() {
        for angle in [-1., 181., f64::NAN, f64::INFINITY] {
            assert!(wedge().crease_normals(angle).is_err());
        }
        let mut mesh = wedge();
        mesh.texcoords = Some(vec![[0., 0.]; mesh.positions.len()]);
        assert!(mesh.crease_normals(45.).is_err());
    }
}
