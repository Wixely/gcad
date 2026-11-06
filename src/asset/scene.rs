//! Derived scene graph. Local transforms use column vectors and T * R * S;
//! world transforms compose parent * local. CAD coordinates remain mm, Z-up.
use super::{Mesh, material::Material};
use anyhow::{Result, ensure};
use monstertruck::modeling::{InnerSpace, Matrix3, Matrix4, SquareMatrix, Vector3};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub translation: [f64; 3],
    /// Unit quaternion, xyzw.
    pub rotation: [f64; 4],
    pub scale: [f64; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        }
    }
}

impl Transform {
    /// Decompose an affine CAD placement; reject shear rather than changing it.
    pub fn from_matrix(matrix: Matrix4) -> Result<Self> {
        let raw: &[[f64; 4]; 4] = matrix.as_ref();
        ensure!(
            raw.iter().flatten().all(|x| x.is_finite()),
            "placement must be finite"
        );
        ensure!(
            matrix.x.w == 0. && matrix.y.w == 0. && matrix.z.w == 0. && matrix.w.w == 1.,
            "placement must be affine"
        );
        let mut columns = [
            matrix.x.truncate(),
            matrix.y.truncate(),
            matrix.z.truncate(),
        ];
        let mut scale = columns.map(|c| c.magnitude());
        ensure!(
            scale.iter().all(|s| s.is_finite() && *s > 0.),
            "placement is singular"
        );
        if Matrix3::from_cols(columns[0], columns[1], columns[2]).determinant() < 0. {
            scale[0] = -scale[0];
        }
        for i in 0..3 {
            columns[i] /= scale[i];
        }
        ensure!(
            columns[0].dot(columns[1]).abs() < 1e-10
                && columns[0].dot(columns[2]).abs() < 1e-10
                && columns[1].dot(columns[2]).abs() < 1e-10,
            "sheared placement cannot be represented as TRS"
        );
        let m = Matrix3::from_cols(columns[0], columns[1], columns[2]);
        let trace = m.x.x + m.y.y + m.z.z;
        let q = if trace > 0. {
            let s = (trace + 1.).sqrt() * 2.;
            [
                (m.y.z - m.z.y) / s,
                (m.z.x - m.x.z) / s,
                (m.x.y - m.y.x) / s,
                s / 4.,
            ]
        } else if m.x.x > m.y.y && m.x.x > m.z.z {
            let s = (1. + m.x.x - m.y.y - m.z.z).sqrt() * 2.;
            [
                s / 4.,
                (m.y.x + m.x.y) / s,
                (m.z.x + m.x.z) / s,
                (m.y.z - m.z.y) / s,
            ]
        } else if m.y.y > m.z.z {
            let s = (1. + m.y.y - m.x.x - m.z.z).sqrt() * 2.;
            [
                (m.y.x + m.x.y) / s,
                s / 4.,
                (m.z.y + m.y.z) / s,
                (m.z.x - m.x.z) / s,
            ]
        } else {
            let s = (1. + m.z.z - m.x.x - m.y.y).sqrt() * 2.;
            [
                (m.z.x + m.x.z) / s,
                (m.z.y + m.y.z) / s,
                s / 4.,
                (m.x.y - m.y.x) / s,
            ]
        };
        let length = q.iter().map(|v| v * v).sum::<f64>().sqrt();
        let result = Self {
            translation: [matrix.w.x, matrix.w.y, matrix.w.z],
            rotation: q.map(|v| v / length),
            scale,
        };
        let rebuilt = result.matrix()?;
        let actual: &[[f64; 4]; 4] = rebuilt.as_ref();
        ensure!(
            raw.iter()
                .flatten()
                .zip(actual.iter().flatten())
                .all(|(a, b)| (a - b).abs() <= 1e-9 * a.abs().max(1.)),
            "placement decomposition changed matrix"
        );
        Ok(result)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.translation
                .iter()
                .chain(&self.rotation)
                .chain(&self.scale)
                .all(|v| v.is_finite()),
            "transform values must be finite"
        );
        ensure!(
            self.scale.iter().all(|v| *v != 0.0),
            "singular transform: scale must be nonzero"
        );
        let norm: f64 = self.rotation.iter().map(|v| v * v).sum();
        ensure!(
            (norm - 1.0).abs() <= 1e-6,
            "rotation must be a unit xyzw quaternion"
        );
        Ok(())
    }

    pub fn matrix(&self) -> Result<Matrix4> {
        self.validate()?;
        let [x, y, z, w] = self.normalized_rotation();
        let [sx, sy, sz] = self.scale;
        let t = self.translation;
        Ok(Matrix4::from_translation(Vector3::new(t[0], t[1], t[2]))
            * Matrix4::new(
                1.0 - 2.0 * (y * y + z * z),
                2.0 * (x * y + w * z),
                2.0 * (x * z - w * y),
                0.0,
                2.0 * (x * y - w * z),
                1.0 - 2.0 * (x * x + z * z),
                2.0 * (y * z + w * x),
                0.0,
                2.0 * (x * z + w * y),
                2.0 * (y * z - w * x),
                1.0 - 2.0 * (x * x + y * y),
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
            )
            * Matrix4::from_nonuniform_scale(sx, sy, sz))
    }

    pub(super) fn normalized_rotation(&self) -> [f64; 4] {
        let norm = self.rotation.iter().map(|v| v * v).sum::<f64>().sqrt();
        self.rotation.map(|v| v / norm)
    }
}

#[derive(Clone, Debug)]
pub struct Primitive {
    pub geometry: Mesh,
    pub material: usize,
}

#[derive(Clone, Debug)]
pub struct SceneMesh {
    pub name: String,
    pub primitives: Vec<Primitive>,
}

#[derive(Clone, Debug, Default)]
pub struct Node {
    pub name: String,
    pub parent: Option<usize>,
    pub mesh: Option<usize>,
    pub transform: Transform,
}

#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub consumer: Option<super::consumer::Profile>,
    pub collision: Vec<super::collision::Collision>,
    pub nodes: Vec<Node>,
    pub meshes: Vec<SceneMesh>,
    pub materials: Vec<Material>,
    pub clips: Vec<super::animation::Clip>,
}

impl Scene {
    pub fn validate(&self) -> Result<()> {
        let mut clips = HashSet::new();
        for clip in &self.clips {
            clip.validate(self.nodes.len())?;
            ensure!(clips.insert(&clip.name), "clip names must be unique");
        }
        ensure!(!self.nodes.is_empty(), "scene has no nodes");
        ensure!(
            self.nodes.iter().any(|n| n.mesh.is_some()),
            "scene has no mesh instances"
        );
        for material in &self.materials {
            material.validate()?;
        }
        for mesh in &self.meshes {
            ensure!(
                !mesh.primitives.is_empty(),
                "mesh `{}` has no primitives",
                mesh.name
            );
            for primitive in &mesh.primitives {
                primitive.geometry.validate()?;
                ensure!(
                    primitive.material < self.materials.len(),
                    "mesh `{}` has an invalid material reference",
                    mesh.name
                );
                let material = &self.materials[primitive.material];
                if let Some(chain) = &material.mip_chain {
                    ensure!(
                        &primitive.geometry == chain.geometry(),
                        "explicit mip geometry changed; rebake its chain"
                    );
                }
                ensure!(
                    !material.has_textures()
                        || (primitive.geometry.texcoords.is_some()
                            && primitive.geometry.tangents.is_some()),
                    "textured primitive requires UVs and tangents"
                );
            }
        }
        self.world_transforms()?;
        if let Some(profile)=&self.consumer {profile.validate(self)?;}
        super::collision::validate(self)?;
        Ok(())
    }

    /// Resolve arbitrary node order iteratively, rejecting cycles and numeric
    /// overflow/underflow in composed transforms before export.
    pub fn world_transforms(&self) -> Result<Vec<Matrix4>> {
        self.world_transforms_with_pose(None)
    }

    /// Resolve a sampled pose without copying meshes or texture payloads.
    pub fn posed_world_transforms(&self, pose: &[Transform]) -> Result<Vec<Matrix4>> {
        ensure!(pose.len() == self.nodes.len(), "pose node count differs from scene");
        self.world_transforms_with_pose(Some(pose))
    }

    fn world_transforms_with_pose(&self, pose: Option<&[Transform]>) -> Result<Vec<Matrix4>> {
        let mut names = HashSet::new();
        let mut children = vec![Vec::new(); self.nodes.len()];
        let mut ready = Vec::new();
        for (i, node) in self.nodes.iter().enumerate() {
            ensure!(
                !node.name.trim().is_empty() && names.insert(node.name.as_str()),
                "node names must be nonempty and unique: `{}`",
                node.name
            );
            pose.map_or(node.transform, |p| p[i]).validate()?;
            ensure!(
                node.mesh.is_none_or(|m| m < self.meshes.len()),
                "node `{}` has an invalid mesh reference",
                node.name
            );
            if let Some(parent) = node.parent {
                ensure!(
                    parent < self.nodes.len(),
                    "node `{}` has an invalid parent",
                    node.name
                );
                children[parent].push(i);
            } else {
                ready.push(i);
            }
        }
        let mut world = vec![Matrix4::identity(); self.nodes.len()];
        let mut visited = 0;
        while let Some(i) = ready.pop() {
            let node = &self.nodes[i];
            let local = pose.map_or(node.transform, |p| p[i]).matrix()?;
            let matrix = node.parent.map_or(local, |p| world[p] * local);
            let columns: &[[f64; 4]; 4] = matrix.as_ref();
            ensure!(
                columns.iter().flatten().all(|v| v.is_finite()),
                "node `{}` world transform exceeds numeric range",
                node.name
            );
            let determinant = matrix.determinant();
            ensure!(
                determinant.is_finite() && determinant != 0.0,
                "node `{}` world transform is singular or exceeds numeric range",
                node.name
            );
            world[i] = matrix;
            visited += 1;
            ready.extend(&children[i]);
        }
        ensure!(
            visited == self.nodes.len(),
            "scene hierarchy contains a cycle"
        );
        Ok(world)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    #[test]
    fn placement_decomposition_preserves_reflections_and_half_turns_and_rejects_shear() {
        for rotation in [
            [0., 0., 0., 1.],
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0.5, 0.5, 0.5, 0.5],
        ] {
            for scale in [[2., 3., 4.], [-2., 3., 4.], [2., -3., -4.]] {
                let original = Transform {
                    translation: [12., -34., 56.],
                    rotation,
                    scale,
                }
                .matrix()
                .unwrap();
                let rebuilt = Transform::from_matrix(original).unwrap().matrix().unwrap();
                let a: &[[f64; 4]; 4] = original.as_ref();
                let b: &[[f64; 4]; 4] = rebuilt.as_ref();
                assert!(
                    a.iter()
                        .flatten()
                        .zip(b.iter().flatten())
                        .all(|(a, b)| (a - b).abs() < 1e-10)
                );
            }
        }
        let mut shear = Matrix4::identity();
        shear.x.y = 0.25;
        assert!(Transform::from_matrix(shear).is_err());
        assert!(Transform::from_matrix(Matrix4::from_nonuniform_scale(0., 1., 1.)).is_err());
    }
    use super::*;
    use monstertruck::modeling::{Point3, Transform as _};

    pub(crate) fn fixture() -> Scene {
        Scene {
            consumer:None, collision: Vec::new(),
            nodes: vec![
                Node {
                    name: "child".into(),
                    parent: Some(1),
                    mesh: Some(0),
                    transform: Transform {
                        translation: [1000.0, 0.0, 0.0],
                        scale: [-2.0, 3.0, 4.0],
                        ..Default::default()
                    },
                },
                Node {
                    name: "parent".into(),
                    transform: Transform {
                        translation: [0.0, 2000.0, 0.0],
                        rotation: [
                            0.0,
                            0.0,
                            std::f64::consts::FRAC_1_SQRT_2,
                            std::f64::consts::FRAC_1_SQRT_2,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            meshes: vec![SceneMesh {
                name: "triangle".into(),
                primitives: vec![Primitive {
                    geometry: super::super::tests::triangle(),
                    material: 0,
                }],
            }],
            materials: vec![Material::default()],
            clips: Vec::new(),
        }
    }

    #[test]
    fn hierarchy_composes_rotation_translation_nonuniform_and_mirrored_scale() {
        let scene = fixture();
        scene.validate().unwrap();
        let world = scene.world_transforms().unwrap();
        let p = world[0].transform_point(Point3::new(100.0, 200.0, 300.0));
        assert!(
            (p.x + 600.0).abs() < 1e-9
                && (p.y - 2800.0).abs() < 1e-9
                && (p.z - 1200.0).abs() < 1e-9
        );
        assert!(world[0].determinant() < 0.0);
    }

    #[test]
    fn invalid_hierarchy_references_and_transforms_are_rejected() {
        for change in [
            |s: &mut Scene| s.nodes[1].parent = Some(0),
            |s: &mut Scene| s.nodes[0].parent = Some(8),
            |s: &mut Scene| s.nodes[0].mesh = Some(8),
            |s: &mut Scene| s.nodes[0].name = "parent".into(),
            |s: &mut Scene| s.nodes[0].transform.rotation = [0.0; 4],
            |s: &mut Scene| s.nodes[0].transform.scale[0] = 0.0,
            |s: &mut Scene| s.nodes[0].transform.translation[0] = f64::NAN,
            |s: &mut Scene| s.meshes[0].primitives[0].material = 8,
        ] {
            let mut scene = fixture();
            change(&mut scene);
            assert!(scene.validate().is_err());
        }
    }
}
