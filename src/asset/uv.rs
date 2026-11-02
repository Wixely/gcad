use super::{Mesh, cross, dot, sub};
use anyhow::{Result, ensure};
use std::collections::HashMap;

impl Mesh {
    /// Return a box-projected mesh with one repeating tile per `tile_mm`.
    /// Dominant triangle normals select charts; chart boundaries split vertices.
    /// Replaces UVs/tangents. Apply before skin authoring: indices can change.
    pub fn project_box(&self, tile_mm: f64) -> Result<Self> {
        self.validate()?;
        ensure!(
            tile_mm.is_finite() && tile_mm > 0.0,
            "UV tile size must be finite and positive"
        );
        let mut mesh = Mesh {
            positions: Vec::new(),
            normals: Vec::new(),
            texcoords: Some(Vec::new()),
            tangents: None,
            triangles: Vec::new(),
        };
        let mut vertices = HashMap::<(u32, usize, bool), u32>::new();
        for triangle in &self.triangles {
            let [a, b, c] = triangle.map(|i| self.positions[i as usize]);
            let n = cross(sub(b, a), sub(c, a));
            let axis = (0..3)
                .max_by(|&a, &b| n[a].abs().total_cmp(&n[b].abs()))
                .unwrap();
            let negative = n[axis] < 0.0;
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            let mut output = [0; 3];
            for k in 0..3 {
                let old = triangle[k];
                let key = (old, axis, negative);
                output[k] = if let Some(&index) = vertices.get(&key) {
                    index
                } else {
                    let p = self.positions[old as usize];
                    let index = u32::try_from(mesh.positions.len())?;
                    mesh.positions.push(p);
                    mesh.normals.push(self.normals[old as usize]);
                    mesh.texcoords.as_mut().unwrap().push([
                        p[u] / tile_mm * if negative { -1.0 } else { 1.0 },
                        p[v] / tile_mm,
                    ]);
                    vertices.insert(key, index);
                    index
                };
            }
            mesh.triangles.push(output);
        }
        mesh.generate_tangents()
    }

    /// Average UV-derivative tangents within existing vertices, then orthogonalise.
    /// This is not MikkTSpace. Mirrored UV islands must have split vertices.
    pub fn generate_tangents(&self) -> Result<Self> {
        self.validate()?;
        let uvs = self
            .texcoords
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("tangent generation requires UVs"))?;
        let mut along_u = vec![[0.0; 3]; self.positions.len()];
        let mut along_v = along_u.clone();
        let mut signs = vec![0.0_f64; self.positions.len()];
        for triangle in &self.triangles {
            let [a, b, c] = triangle.map(|i| i as usize);
            let e1 = sub(self.positions[b], self.positions[a]);
            let e2 = sub(self.positions[c], self.positions[a]);
            let d1 = [uvs[b][0] - uvs[a][0], uvs[b][1] - uvs[a][1]];
            let d2 = [uvs[c][0] - uvs[a][0], uvs[c][1] - uvs[a][1]];
            let determinant = d1[0] * d2[1] - d1[1] * d2[0];
            ensure!(
                determinant.is_finite() && determinant != 0.0,
                "degenerate UV triangle; cannot generate tangents"
            );
            let t: [f64; 3] =
                std::array::from_fn(|k| (e1[k] * d2[1] - e2[k] * d1[1]) / determinant);
            let b: [f64; 3] =
                std::array::from_fn(|k| (e2[k] * d1[0] - e1[k] * d2[0]) / determinant);
            for &vertex in triangle {
                let i = vertex as usize;
                let orientation = dot(cross(self.normals[i], t), b);
                ensure!(
                    orientation.is_finite() && orientation != 0.0,
                    "degenerate tangent orientation"
                );
                let sign = orientation.signum();
                ensure!(
                    signs[i] == 0.0 || signs[i] == sign,
                    "mirrored UV seam must have split vertices"
                );
                signs[i] = sign;
                for k in 0..3 {
                    along_u[i][k] += t[k];
                    along_v[i][k] += b[k];
                }
            }
        }
        let mut mesh = self.clone();
        mesh.tangents = Some(
            self.normals
                .iter()
                .enumerate()
                .map(|(i, &n)| {
                    let projected = dot(along_u[i], n);
                    let t: [f64; 3] = std::array::from_fn(|k| along_u[i][k] - n[k] * projected);
                    let length = dot(t, t).sqrt();
                    ensure!(
                        length.is_finite() && length > 0.0,
                        "vertex {i} has no valid tangent; remove unused vertices or split seams"
                    );
                    let t = t.map(|v| v / length);
                    let handedness = dot(cross(n, t), along_v[i]);
                    ensure!(
                        handedness.is_finite() && handedness != 0.0,
                        "vertex {i} has a degenerate bitangent"
                    );
                    Ok([t[0], t[1], t[2], handedness.signum()])
                })
                .collect::<Result<Vec<_>>>()?,
        );
        mesh.validate()?;
        Ok(mesh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_projection_preserves_geometry_and_physical_tile_size() {
        let source = crate::parse::parse_program("rect 1000 500\nextrude 250\n").unwrap();
        let model = crate::model::run(&source).model;
        let original = Mesh::from_solid(model.solid.as_ref().unwrap(), 0.1).unwrap();
        let projected = original.project_box(250.0).unwrap();
        assert!(original.texcoords.is_none());
        assert_eq!(projected.positions.len(), 24);
        for (before, after) in original.triangles.iter().zip(&projected.triangles) {
            assert_eq!(
                before.map(|i| original.positions[i as usize]),
                after.map(|i| projected.positions[i as usize])
            );
            let uv = projected.texcoords.as_ref().unwrap();
            for k in 0..3 {
                let a = after[k] as usize;
                let b = after[(k + 1) % 3] as usize;
                let edge = sub(projected.positions[b], projected.positions[a]);
                let du = (uv[b][0] - uv[a][0]) * 250.0;
                let dv = (uv[b][1] - uv[a][1]) * 250.0;
                assert!((dot(edge, edge) - du * du - dv * dv).abs() < 1e-6);
            }
        }
        for size in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(original.project_box(size).is_err());
        }
    }

    #[test]
    fn mirrored_uvs_get_negative_handedness_and_degenerate_uvs_fail() {
        let mut mesh = super::super::tests::triangle();
        mesh.tangents = None;
        let positive = mesh.generate_tangents().unwrap();
        assert_eq!(positive.tangents.as_ref().unwrap()[0], [1.0, 0.0, 0.0, 1.0]);
        for uv in mesh.texcoords.as_mut().unwrap() {
            uv[0] = -uv[0];
        }
        let mirrored = mesh.generate_tangents().unwrap();
        assert_eq!(
            mirrored.tangents.as_ref().unwrap()[0],
            [-1.0, 0.0, 0.0, -1.0]
        );
        mesh.texcoords.as_mut().unwrap().fill([0.0; 2]);
        assert!(mesh.generate_tangents().is_err());
    }

    #[test]
    fn chart_boundaries_split_shared_vertices_and_unsplit_mirrors_fail() {
        let diagonal = 1.0_f64 / 2.0_f64.sqrt();
        let bent = Mesh {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
            normals: vec![
                [diagonal, 0.0, diagonal],
                [0.0, 0.0, 1.0],
                [diagonal, 0.0, diagonal],
                [1.0, 0.0, 0.0],
            ],
            texcoords: None,
            tangents: None,
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        let projected = bent.project_box(1.0).unwrap();
        assert_eq!(projected.positions.len(), 6);
        assert_eq!(bent.positions.len(), 4);
        for a in projected.triangles[0] {
            assert!(!projected.triangles[1].contains(&a));
        }

        let flat = Mesh {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
            ],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            texcoords: Some(vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [0.0, 0.0]]),
            tangents: None,
            triangles: vec![[0, 1, 2], [1, 3, 2]],
        };
        assert!(
            flat.generate_tangents()
                .unwrap_err()
                .to_string()
                .contains("split vertices")
        );
    }
}
