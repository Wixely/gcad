//! Optional pre-binding refinement of the derived triangle surface, not CAD retopology.
use super::{Mesh, dot};
use anyhow::{Result, ensure};
use std::collections::HashMap;

impl Mesh {
    /// Split every triangle into four per level, preserving winding and existing
    /// hard-edge vertex splits. Run before UV generation and skin assignment.
    /// Midpoints lie on the existing triangles; this does not reproject onto CAD.
    pub fn refined(&self, levels: u32, triangle_budget: usize) -> Result<Self> {
        Ok(self.refined_with_labels(levels, triangle_budget, None)?.0)
    }

    /// Optional triangle labels are emitted with their children, never inferred
    /// from positions after refinement. Ordinary refinement allocates no labels.
    pub(crate) fn refined_with_labels(
        &self,
        levels: u32,
        triangle_budget: usize,
        labels: Option<&[usize]>,
    ) -> Result<(Self, Option<Vec<usize>>)> {
        self.validate()?;
        ensure!(
            labels.is_none_or(|labels| labels.len() == self.triangles.len()),
            "refinement label count must match triangle count"
        );
        ensure!(
            self.texcoords.is_none() && self.tangents.is_none(),
            "refine before UV generation and skin binding"
        );
        ensure!(levels <= 8, "refinement supports at most eight levels");
        let expected = self
            .triangles
            .len()
            .checked_mul(4usize.pow(levels))
            .ok_or_else(|| anyhow::anyhow!("refined triangle count overflows"))?;
        ensure!(
            expected <= triangle_budget,
            "refinement exceeds triangle budget: {expected} > {triangle_budget}"
        );
        let mut mesh = self.clone();
        let mut labels = labels.map(<[usize]>::to_vec);
        for _ in 0..levels {
            let mut midpoints = HashMap::new();
            let mut triangles = Vec::with_capacity(mesh.triangles.len() * 4);
            let mut child_labels = labels
                .as_ref()
                .map(|_| Vec::with_capacity(mesh.triangles.len() * 4));
            for (parent, [a, b, c]) in mesh.triangles.clone().into_iter().enumerate() {
                let ab = midpoint(&mut mesh, &mut midpoints, a, b)?;
                let bc = midpoint(&mut mesh, &mut midpoints, b, c)?;
                let ca = midpoint(&mut mesh, &mut midpoints, c, a)?;
                triangles.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
                if let (Some(labels), Some(children)) = (&labels, &mut child_labels) {
                    children.extend([labels[parent]; 4]);
                }
            }
            mesh.triangles = triangles;
            labels = child_labels;
        }
        mesh.validate()?;
        Ok((mesh, labels))
    }
}

fn midpoint(mesh: &mut Mesh, cache: &mut HashMap<(u32, u32), u32>, a: u32, b: u32) -> Result<u32> {
    let edge = (a.min(b), a.max(b));
    if let Some(&index) = cache.get(&edge) {
        return Ok(index);
    }
    let p = mesh.positions[a as usize];
    let q = mesh.positions[b as usize];
    let n = mesh.normals[a as usize];
    let m = mesh.normals[b as usize];
    let normal = std::array::from_fn(|k| n[k] + m[k]);
    let length = dot(normal, normal).sqrt();
    ensure!(
        length.is_finite() && length > 1e-12,
        "refinement cannot interpolate opposite normals"
    );
    let index = u32::try_from(mesh.positions.len())?;
    mesh.positions
        .push(std::array::from_fn(|k| p[k] * 0.5 + q[k] * 0.5));
    mesh.normals.push(normal.map(|x| x / length));
    cache.insert(edge, index);
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refinement_shares_edges_preserves_surface_and_enforces_budget() {
        let mesh = Mesh {
            positions: vec![[0., 0., 0.], [2., 0., 0.], [2., 2., 0.], [0., 2., 0.]],
            normals: vec![[0., 0., 1.]; 4],
            texcoords: None,
            tangents: None,
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        let original = mesh.clone();
        let result = mesh.refined(1, 8).unwrap();
        assert_eq!(result.positions.len(), 9);
        assert_eq!(result.triangles.len(), 8);
        let area: f64 = result
            .triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| result.positions[i as usize]);
                let cross = super::super::cross(super::super::sub(b, a), super::super::sub(c, a));
                assert!(cross[2] > 0.);
                cross[2] * 0.5
            })
            .sum();
        assert_eq!(area, 4.);
        assert_eq!(mesh.refined(2, 32).unwrap().triangles.len(), 32);
        assert!(mesh.refined(2, 31).is_err());
        assert!(mesh.refined(9, usize::MAX).is_err());
        assert_eq!(mesh, original);
        assert_eq!(result, mesh.refined(1, 8).unwrap());
        let mut split = mesh.clone();
        split.positions.extend_from_slice(&mesh.positions[..3]);
        split.normals.extend_from_slice(&[[0., 1., 0.]; 3]);
        split.triangles.push([4, 5, 6]);
        let split = split.refined(1, 12).unwrap();
        assert_eq!(
            split
                .positions
                .iter()
                .filter(|&&p| p == [1., 0., 0.])
                .count(),
            2
        );
        let mut textured = mesh;
        textured.texcoords = Some(vec![[0., 0.]; 4]);
        assert!(textured.refined(1, 8).is_err());
    }
}
