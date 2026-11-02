//! Plane-aligned subdivision of derived triangles, not CAD surface retopology.
use super::{Mesh, dot};
use anyhow::{Result, ensure};
use std::collections::HashMap;

impl Mesh {
    /// Split triangles at ordered mesh-local axis planes before UVs and binding.
    /// Intersections stay on the input surface; complete-tuple edges share cuts,
    /// while hard-edge vertex splits remain separate. The source is untouched.
    pub fn sliced(&self, axis: usize, planes_mm: &[f64], triangle_budget: usize) -> Result<Self> {
        Ok(self
            .sliced_with_labels(axis, planes_mm, triangle_budget, None)?
            .0)
    }

    pub(crate) fn sliced_with_labels(
        &self,
        axis: usize,
        planes_mm: &[f64],
        triangle_budget: usize,
        labels: Option<&[usize]>,
    ) -> Result<(Self, Option<Vec<usize>>)> {
        self.validate()?;
        ensure!(
            labels.is_none_or(|v| v.len() == self.triangles.len()),
            "slice label count differs from triangle count"
        );
        let mut labels = labels.map(<[usize]>::to_vec);
        ensure!(
            self.texcoords.is_none() && self.tangents.is_none(),
            "slice before UV generation and skin binding"
        );
        ensure!(
            axis < 3 && !planes_mm.is_empty() && planes_mm.len() <= 1024,
            "slice requires an axis and 1..1024 planes"
        );
        ensure!(
            planes_mm.iter().all(|v| v.is_finite()) && planes_mm.windows(2).all(|w| w[1] > w[0]),
            "slice planes must be finite and strictly increasing"
        );
        ensure!(
            self.triangles.len() <= triangle_budget,
            "slice exceeds triangle budget"
        );
        let mut result = self.clone();
        let mut visits = 0usize;
        for &plane in planes_mm {
            // Treat only roundoff-sized offsets as on-plane, without moving the
            // original vertex. Otherwise CAD vertices near an exact plane can
            // produce zero-area slivers when their intersections round together.
            let magnitude = result
                .positions
                .iter()
                .map(|p| p[axis].abs())
                .fold(plane.abs().max(1.), f64::max);
            let tolerance = 64. * f64::EPSILON * magnitude;
            visits = visits
                .checked_add(result.triangles.len())
                .ok_or_else(|| anyhow::anyhow!("slice work overflow"))?;
            ensure!(
                visits <= 20_000_000,
                "slice exceeds 20 million triangle visits"
            );
            let mut triangles = Vec::new();
            let mut child_labels = labels.as_ref().map(|_| Vec::new());
            let mut cuts = HashMap::new();
            for (parent, triangle) in result.triangles.clone().into_iter().enumerate() {
                let sides = triangle.map(|i| {
                    let d = result.positions[i as usize][axis] - plane;
                    if d.abs() <= tolerance { 0. } else { d }
                });
                let positive = sides.iter().any(|&d| d > 0.);
                let negative = sides.iter().any(|&d| d < 0.);
                if !positive || !negative {
                    ensure!(
                        triangles.len() < triangle_budget,
                        "slice exceeds triangle budget"
                    );
                    triangles.push(triangle);
                    if let (Some(source), Some(children)) = (&labels, &mut child_labels) {
                        children.push(source[parent]);
                    }
                    continue;
                }
                ensure!(
                    sides.iter().all(|v| v.is_finite()),
                    "slice coordinate range overflow"
                );
                for keep_positive in [false, true] {
                    let mut polygon = Vec::with_capacity(4);
                    for edge in 0..3 {
                        let a = triangle[edge];
                        let b = triangle[(edge + 1) % 3];
                        let da = sides[edge];
                        let db = sides[(edge + 1) % 3];
                        if (keep_positive && da >= 0.) || (!keep_positive && da <= 0.) {
                            polygon.push(a);
                        }
                        if (da < 0. && db > 0.) || (da > 0. && db < 0.) {
                            polygon.push(cut(&mut result, &mut cuts, a, b, axis, plane)?);
                        }
                    }
                    for i in 1..polygon.len() - 1 {
                        ensure!(
                            triangles.len() < triangle_budget,
                            "slice exceeds triangle budget"
                        );
                        triangles.push([polygon[0], polygon[i], polygon[i + 1]]);
                        if let (Some(source), Some(children)) = (&labels, &mut child_labels) {
                            children.push(source[parent]);
                        }
                    }
                }
            }
            result.triangles = triangles;
            labels = child_labels;
        }
        result.validate()?;
        Ok((result, labels))
    }
}

fn cut(
    mesh: &mut Mesh,
    cuts: &mut HashMap<(u32, u32), u32>,
    a: u32,
    b: u32,
    axis: usize,
    plane: f64,
) -> Result<u32> {
    let key = (a.min(b), a.max(b));
    if let Some(&index) = cuts.get(&key) {
        return Ok(index);
    }
    let p = mesh.positions[key.0 as usize];
    let q = mesh.positions[key.1 as usize];
    let denominator = q[axis] - p[axis];
    ensure!(denominator.is_finite(), "slice edge range overflow");
    let t = (plane - p[axis]) / denominator;
    ensure!(
        t.is_finite() && t > 0. && t < 1.,
        "slice intersection collapses at numeric precision"
    );
    let mut position = std::array::from_fn(|k| p[k] * (1. - t) + q[k] * t);
    position[axis] = plane;
    let n = mesh.normals[key.0 as usize];
    let m = mesh.normals[key.1 as usize];
    let normal = std::array::from_fn(|k| n[k] * (1. - t) + m[k] * t);
    let length = dot(normal, normal).sqrt();
    ensure!(
        length.is_finite() && length > 1e-12,
        "slice cannot interpolate opposite normals"
    );
    let index = u32::try_from(mesh.positions.len())?;
    mesh.positions.push(position);
    mesh.normals.push(normal.map(|v| v / length));
    cuts.insert(key, index);
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn square() -> Mesh {
        Mesh {
            positions: vec![[0., 0., 0.], [2., 0., 0.], [2., 2., 0.], [0., 2., 0.]],
            normals: vec![[0., 0., 1.]; 4],
            texcoords: None,
            tangents: None,
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        }
    }
    #[test]
    fn slices_preserve_area_winding_shared_edges_and_source() {
        let original = square();
        let sliced = original.sliced(1, &[0., 0.5, 1., 1.5, 2.], 100).unwrap();
        let mut area = 0.;
        for t in &sliced.triangles {
            let [a, b, c] = t.map(|i| sliced.positions[i as usize]);
            let cross = super::super::cross(super::super::sub(b, a), super::super::sub(c, a));
            assert!(cross[2] > 0.);
            area += cross[2] * 0.5;
            for plane in [0.5, 1., 1.5] {
                assert!(
                    !([a, b, c].iter().any(|p| p[1] < plane)
                        && [a, b, c].iter().any(|p| p[1] > plane))
                );
            }
        }
        assert!((area - 4.).abs() < 1e-12);
        assert_eq!(
            sliced
                .positions
                .iter()
                .filter(|&&p| p == [1., 1., 0.])
                .count(),
            1
        );
        assert_eq!(original, square());
        assert_eq!(
            sliced,
            original.sliced(1, &[0., 0.5, 1., 1.5, 2.], 100).unwrap()
        );
        assert!(original.sliced(1, &[1.], 2).is_err());
        assert_eq!(original.sliced(1, &[-1., 0., 2., 3.], 2).unwrap(), original);
        let mut split = original.clone();
        split.positions.extend_from_slice(&original.positions);
        split.normals.extend_from_slice(&[[0., 1., 0.]; 4]);
        split.triangles.extend([[4, 5, 6], [4, 6, 7]]);
        let split = split.sliced(1, &[1.], 100).unwrap();
        assert_eq!(
            split
                .positions
                .iter()
                .filter(|&&p| p == [1., 1., 0.])
                .count(),
            2
        );
    }
    #[test]
    fn near_plane_vertices_do_not_create_roundoff_slivers() {
        let mut mesh = square();
        mesh.positions[0][1] = -1e-16;
        let sliced = mesh.sliced(1, &[0., 1.], 100).unwrap();
        assert_eq!(sliced.positions[0], mesh.positions[0]);
        assert!(sliced.triangles.iter().all(|t| {
            let [a, b, c] = t.map(|i| sliced.positions[i as usize]);
            super::super::cross(super::super::sub(b, a), super::super::sub(c, a))[2] > 1e-10
        }));
    }
    #[test]
    fn invalid_slice_settings_fail() {
        for planes in [
            vec![],
            vec![1., 1.],
            vec![2., 1.],
            vec![f64::NAN],
            vec![f64::INFINITY],
        ] {
            assert!(square().sliced(1, &planes, 100).is_err());
        }
        assert!(square().sliced(3, &[1.], 100).is_err());
        let mut textured = square();
        textured.texcoords = Some(vec![[0., 0.]; 4]);
        assert!(textured.sliced(1, &[1.], 100).is_err());
    }
}
