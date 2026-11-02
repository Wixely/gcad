//! Experimental derived render data. CAD solids remain authoritative and unchanged.
//!
//! Coordinates are right-handed, Z-up, in millimetres. Export adapters own the
//! conversion to their destination coordinates. Vertex indices address complete
//! attribute tuples: UV seams and hard normal edges must retain separate vertices.

use anyhow::{Result, ensure};
use sha2::{Digest, Sha256};

pub mod animation;
mod assembly_tree;
pub mod atlas;
pub mod texel_density;
pub mod unwrap;
#[cfg(not(target_arch = "wasm32"))]
pub mod paint_guide;
pub mod author;
#[cfg(not(target_arch = "wasm32"))]
pub mod fbx_helper;
#[cfg(not(target_arch = "wasm32"))]
pub mod fbx_package;
#[cfg(not(target_arch = "wasm32"))]
mod fbx_response;
pub mod bake;
pub mod capabilities;
pub mod faces;
pub mod face_author;
pub mod face_material;
pub mod face_weights;
pub mod glb;
pub mod material;
pub mod lod;
pub mod motion;
pub mod provenance;
pub mod projection;
pub mod quantize;
pub mod reduce;
mod refine;
mod normals;
mod slice;
pub mod report;
pub mod root_motion;
pub mod rigid_motion;
pub mod scene;
pub mod consumer;
pub mod shader;
#[cfg(not(target_arch = "wasm32"))]
mod shader_receipt;
#[cfg(not(target_arch = "wasm32"))]
mod motion_receipt;
#[cfg(not(target_arch = "wasm32"))]
mod fbx_receipts;
#[cfg(not(target_arch = "wasm32"))]
mod consumer_receipt;
pub mod silhouette;
pub mod skin;
pub mod surface;
mod surface_settings;
pub mod texture_input;
mod uv;
pub mod uv_edit;

/// One indexed triangle primitive, before any format-specific conversion.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    pub positions: Vec<[f64; 3]>,
    pub normals: Vec<[f64; 3]>,
    pub texcoords: Option<Vec<[f64; 2]>>,
    /// Tangent xyz and handedness w; bitangent = w * cross(normal, tangent).
    pub tangents: Option<Vec<[f64; 4]>>,
    pub triangles: Vec<[u32; 3]>,
}

/// Exact binding guard for the derived vertex order, geometry and tessellation.
/// This is not a persistent CAD face identity or a promise of stable regeneration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BindingStamp([u8; 32]);

impl Mesh {
    /// Tessellate a borrowed CAD solid without changing it. Surface parameter UVs
    /// are deliberately not treated as portable texture coordinates. Analytic
    /// normals are retained, with flat normals where the kernel supplies none.
    pub fn from_solid(solid: &monstertruck::modeling::Solid, tolerance_mm: f64) -> Result<Self> {
        ensure!(
            tolerance_mm.is_finite() && tolerance_mm > 0.0,
            "tessellation tolerance must be finite and positive"
        );
        let source = crate::geometry::mesh(solid, tolerance_mm);
        Self::from_polygon(&source)
    }

    pub(crate) fn from_polygon(source: &monstertruck::meshing::prelude::PolygonMesh) -> Result<Self> {
        let mut result = Self {
            positions: Vec::new(),
            normals: Vec::new(),
            texcoords: None,
            tangents: None,
            triangles: Vec::new(),
        };
        // Key by the complete position/normal tuple, never just the position.
        let mut vertices = std::collections::HashMap::<[u64; 6], u32>::new();
        for triangle in source.faces().triangle_iter() {
            let positions = triangle.map(|v| {
                let p = source.positions()[v.pos];
                [p.x, p.y, p.z]
            });
            let flat = cross(
                sub(positions[1], positions[0]),
                sub(positions[2], positions[0]),
            );
            let mut indices = [0; 3];
            for k in 0..3 {
                let normal = match triangle[k].nor {
                    Some(i) => {
                        let n = source.normals()[i];
                        [n.x, n.y, n.z]
                    }
                    None => flat,
                };
                let length = dot(normal, normal).sqrt();
                ensure!(
                    length.is_finite() && length > 0.0,
                    "tessellation produced an invalid normal"
                );
                let normal = normal.map(|v| v / length);
                let p = positions[k];
                let key = [p[0], p[1], p[2], normal[0], normal[1], normal[2]]
                    .map(|v| if v == 0.0 { 0 } else { v.to_bits() });
                indices[k] = match vertices.get(&key) {
                    Some(&index) => index,
                    None => {
                        let index = u32::try_from(result.positions.len())?;
                        vertices.insert(key, index);
                        result.positions.push(p);
                        result.normals.push(normal);
                        index
                    }
                };
            }
            result.triangles.push(indices);
        }
        result.validate()?;
        Ok(result)
    }

    /// Reject invalid data before any exporter creates an output file.
    pub fn validate(&self) -> Result<()> {
        let count = self.positions.len();
        ensure!(count > 0, "mesh has no vertices");
        ensure!(
            u32::try_from(count).is_ok(),
            "mesh exceeds u32 vertex capacity"
        );
        ensure!(!self.triangles.is_empty(), "mesh has no triangles");
        ensure!(
            self.normals.len() == count,
            "normal count differs from vertex count"
        );
        for (i, p) in self.positions.iter().enumerate() {
            ensure!(p.iter().all(|v| v.is_finite()), "vertex {i} is not finite");
        }
        for (i, n) in self.normals.iter().enumerate() {
            ensure!(unit(*n), "normal {i} must be finite and unit length");
        }
        if let Some(uvs) = &self.texcoords {
            ensure!(uvs.len() == count, "UV count differs from vertex count");
            ensure!(
                uvs.iter().flatten().all(|v| v.is_finite()),
                "UVs must be finite"
            );
        }
        if let Some(tangents) = &self.tangents {
            ensure!(self.texcoords.is_some(), "tangents require UVs");
            ensure!(
                tangents.len() == count,
                "tangent count differs from vertex count"
            );
            for (i, t) in tangents.iter().enumerate() {
                let xyz = [t[0], t[1], t[2]];
                ensure!(unit(xyz), "tangent {i} must be finite and unit length");
                ensure!(
                    t[3] == -1.0 || t[3] == 1.0,
                    "tangent {i} handedness must be -1 or 1"
                );
                ensure!(
                    dot(xyz, self.normals[i]).abs() <= 1.0e-6,
                    "tangent {i} is not perpendicular to its normal"
                );
            }
        }
        for (i, indices) in self.triangles.iter().enumerate() {
            ensure!(
                indices.iter().all(|&v| (v as usize) < count),
                "triangle {i} has an out-of-range vertex"
            );
            let [a, b, c] = indices.map(|v| self.positions[v as usize]);
            let ab = sub(b, a);
            let ac = sub(c, a);
            let area = cross(ab, ac);
            let area_squared = dot(area, area);
            ensure!(
                area_squared.is_finite() && area_squared > 0.0,
                "triangle {i} is degenerate or outside numeric range"
            );
        }
        Ok(())
    }

    /// Conservative invalidation: even moving vertices without changing triangle
    /// count requires rebinding. UVs/tangents are excluded since they are bindings.
    pub fn binding_stamp(&self) -> Result<BindingStamp> {
        self.validate()?;
        let mut hash = Sha256::new();
        hash.update(b"gcad-render-binding-v1\0");
        hash.update((self.positions.len() as u64).to_le_bytes());
        hash.update((self.triangles.len() as u64).to_le_bytes());
        for value in self.positions.iter().chain(&self.normals).flatten() {
            // Treat negative zero as the same geometric coordinate.
            hash.update(if *value == 0.0 { 0.0_f64 } else { *value }.to_le_bytes());
        }
        for value in self.triangles.iter().flatten() {
            hash.update(value.to_le_bytes());
        }
        Ok(BindingStamp(hash.finalize().into()))
    }

    pub fn require_binding(&self, expected: BindingStamp) -> Result<()> {
        ensure!(
            self.binding_stamp()? == expected,
            "render mesh changed; rebind UVs and skin weights before exporting"
        );
        Ok(())
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}

fn unit(v: [f64; 3]) -> bool {
    v.iter().all(|v| v.is_finite()) && (dot(v, v) - 1.0).abs() <= 1.0e-6
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn triangle() -> Mesh {
        Mesh {
            positions: vec![[0.0, 0.0, 0.0], [1000.0, 0.0, 0.0], [0.0, 1000.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            texcoords: Some(vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]),
            tangents: Some(vec![[1.0, 0.0, 0.0, 1.0]; 3]),
            triangles: vec![[0, 1, 2]],
        }
    }

    #[test]
    fn validates_complete_attribute_tuples() {
        let original = triangle();
        original.validate().unwrap();
        for mutate in [
            |m: &mut Mesh| {
                m.normals.pop();
            },
            |m: &mut Mesh| {
                m.texcoords.as_mut().unwrap().pop();
            },
            |m: &mut Mesh| {
                m.tangents.as_mut().unwrap().pop();
            },
            |m: &mut Mesh| m.positions[0][0] = f64::NAN,
            |m: &mut Mesh| m.normals[0] = [0.0; 3],
            |m: &mut Mesh| m.texcoords.as_mut().unwrap()[0][0] = f64::INFINITY,
            |m: &mut Mesh| m.tangents.as_mut().unwrap()[0][3] = 0.0,
            |m: &mut Mesh| m.tangents.as_mut().unwrap()[0] = [0.0, 0.0, 1.0, 1.0],
            |m: &mut Mesh| m.texcoords = None,
            |m: &mut Mesh| m.triangles[0][2] = 3,
            |m: &mut Mesh| m.triangles[0][2] = 1,
            |m: &mut Mesh| m.positions[2] = [500.0, 0.0, 0.0],
            |m: &mut Mesh| m.triangles.clear(),
            |m: &mut Mesh| m.positions.clear(),
        ] {
            let mut mesh = original.clone();
            mutate(&mut mesh);
            assert!(mesh.validate().is_err(), "accepted invalid mesh: {mesh:?}");
        }
    }

    #[test]
    fn rejects_changed_geometry_order_and_normals_without_mutation() {
        let original = triangle();
        let stamp = original.binding_stamp().unwrap();
        original.require_binding(stamp).unwrap();
        for mutate in [
            |m: &mut Mesh| m.positions[1][0] += 1.0,
            |m: &mut Mesh| m.triangles[0].swap(1, 2),
            |m: &mut Mesh| m.normals[0][2] = -1.0,
        ] {
            let mut changed = original.clone();
            mutate(&mut changed);
            assert!(changed.require_binding(stamp).is_err());
        }
        assert_eq!(original, triangle());
    }

    #[test]
    fn binding_updates_and_negative_zero_preserve_stamp() {
        let mut mesh = triangle();
        let stamp = mesh.binding_stamp().unwrap();
        mesh.positions[0][0] = -0.0;
        mesh.texcoords.as_mut().unwrap()[0] = [0.5, 0.5];
        mesh.tangents.as_mut().unwrap()[0][3] = -1.0;
        mesh.require_binding(stamp).unwrap();
    }

    #[test]
    fn cad_tessellation_preserves_units_hard_edges_and_solid() {
        let lines = crate::parse::parse_program("rect 1000 500\nextrude 250\n").unwrap();
        let model = crate::model::run(&lines).model;
        let solid = model.solid.as_ref().unwrap();
        let volume = crate::geometry::volume(solid);
        let mesh = Mesh::from_solid(solid, 0.1).unwrap();
        assert!(
            mesh.positions.len() >= 24,
            "box hard edges must not be welded"
        );
        assert_eq!(mesh.triangles.len(), 12);
        for (axis, expected) in [1000.0, 500.0, 250.0].into_iter().enumerate() {
            let low = mesh
                .positions
                .iter()
                .map(|p| p[axis])
                .fold(f64::INFINITY, f64::min);
            let high = mesh
                .positions
                .iter()
                .map(|p| p[axis])
                .fold(f64::NEG_INFINITY, f64::max);
            assert!((high - low - expected).abs() < 1.0e-9);
        }
        assert!(mesh.texcoords.is_none());
        assert!(mesh.tangents.is_none());
        assert_eq!(crate::geometry::volume(solid), volume);
        for tolerance in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(Mesh::from_solid(solid, tolerance).is_err());
        }
    }
}
