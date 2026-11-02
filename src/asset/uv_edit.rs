//! Explicit region UV edits on derived meshes, with complete-tuple remapping.
use super::Mesh;
use anyhow::{Result, ensure};
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub scale: [f64; 2],
    pub rotation_degrees: f64,
    pub offset: [f64; 2],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            scale: [1., 1.],
            rotation_degrees: 0.,
            offset: [0., 0.],
        }
    }
}

pub struct Edited {
    pub mesh: Mesh,
    /// Each output vertex inherits geometry/weights from this original vertex.
    pub source_vertices: Vec<usize>,
}

pub struct SceneEdited {
    pub scene: super::scene::Scene,
    pub skins: Vec<super::skin::Skin>,
    pub face_maps: Vec<super::faces::FaceMap>,
    pub source_vertices: Vec<Vec<usize>>,
    pub selected_triangles: usize,
}

impl SceneEdited {
    pub fn encode_glb(&self) -> Result<Vec<u8>> {
        super::glb::encode_skinned_scene(&self.scene, &self.skins)
    }
}

/// Edit a validated saved selection on one instance. Materials and texture pixels
/// stay unchanged; this transforms sampling coordinates, including atlas coordinates.
pub fn edit_scene(
    asset: &super::author::Asset,
    node: usize,
    record: &super::faces::SelectionFile,
    transform: Transform,
) -> Result<SceneEdited> {
    asset.scene.validate()?;
    transform.validate()?;
    let selections = asset.resolve_face_primitives(node, record)?;
    let maps = asset.primitive_face_maps(node)?;
    let selected_triangles = selections.iter().map(Vec::len).sum();
    ensure!(selected_triangles > 0, "UV face selection is empty");
    let mut bound = BTreeSet::new();
    for skin in &asset.skins {
        skin.validate(&asset.scene)?;
        ensure!(bound.insert(skin.mesh_node), "duplicate node skin binding");
    }
    let mesh_index = asset.scene.nodes[node].mesh.unwrap();
    let original = &asset.scene.meshes[mesh_index];
    let mut mesh = original.clone();
    let mut source_vertices = Vec::new();
    let mut face_maps = Vec::new();
    for (p, selection) in selections.iter().enumerate() {
        let geometry = &original.primitives[p].geometry;
        if selection.is_empty() {
            source_vertices.push((0..geometry.positions.len()).collect());
            face_maps.push(maps[p].clone());
        } else {
            let (edited, map) = maps[p].transformed_uvs(geometry, selection, transform)?;
            mesh.primitives[p].geometry = edited.mesh;
            source_vertices.push(edited.source_vertices);
            face_maps.push(map);
        }
    }
    let mut scene = asset.scene.clone();
    if scene
        .nodes
        .iter()
        .enumerate()
        .any(|(i, n)| i != node && n.mesh == Some(mesh_index))
    {
        mesh.name = format!("{} face UV", scene.nodes[node].name);
        scene.nodes[node].mesh = Some(scene.meshes.len());
        scene.meshes.push(mesh);
    } else {
        scene.meshes[mesh_index] = mesh;
    }
    scene.validate()?;
    let skins = asset
        .skins
        .iter()
        .map(|skin| {
            if skin.mesh_node != node {
                skin.validate(&scene)?;
                return Ok(skin.clone());
            }
            let weights = source_vertices
                .iter()
                .enumerate()
                .map(|(p, vertices)| vertices.iter().map(|&v| skin.weights[p][v]).collect())
                .collect();
            let rebound = super::skin::Skin::bind(&scene, node, skin.joints.clone(), weights)?;
            ensure!(
                rebound.inverse_bind == skin.inverse_bind,
                "UV editing changed inverse bind matrices"
            );
            Ok(rebound)
        })
        .collect::<Result<_>>()?;
    Ok(SceneEdited {
        scene,
        skins,
        face_maps,
        source_vertices,
        selected_triangles,
    })
}

impl Transform {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.scale.iter().all(|v| v.is_finite() && *v != 0.)
                && self.rotation_degrees.is_finite()
                && self.offset.iter().all(|v| v.is_finite()),
            "UV transform requires finite values and nonzero scales"
        );
        Ok(())
    }
    /// UV origin is the pivot: scale, then rotate, then translate. Negative scales
    /// deliberately mirror the chart and regenerate its tangent handedness.
    pub fn apply(&self, uv: [f64; 2]) -> [f64; 2] {
        let (sin, cos) = self
            .rotation_degrees
            .rem_euclid(360.)
            .to_radians()
            .sin_cos();
        let [u, v] = [uv[0] * self.scale[0], uv[1] * self.scale[1]];
        [
            u * cos - v * sin + self.offset[0],
            u * sin + v * cos + self.offset[1],
        ]
    }
}

impl Mesh {
    /// Split shared selected/unselected boundaries. Triangle order, positions and
    /// normals stay exact; unselected UVs and authored tangent tuples stay exact.
    /// The explicit vertex remap must be applied before reusing any skin weights.
    pub fn transformed_uvs(&self, triangles: &[usize], transform: Transform) -> Result<Edited> {
        self.validate()?;
        transform.validate()?;
        let uvs = self
            .texcoords
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("UV editing requires existing UVs"))?;
        let tangents = self
            .tangents
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("UV editing requires existing tangents"))?;
        let selected: BTreeSet<_> = triangles.iter().copied().collect();
        ensure!(
            !selected.is_empty()
                && selected.len() == triangles.len()
                && selected.iter().all(|&i| i < self.triangles.len()),
            "UV selection must contain unique valid triangle indices"
        );
        if transform.scale == [1., 1.]
            && transform.rotation_degrees.rem_euclid(360.) == 0.
            && transform.offset == [0., 0.]
        {
            return Ok(Edited {
                mesh: self.clone(),
                source_vertices: (0..self.positions.len()).collect(),
            });
        }
        let mut mesh = Mesh {
            positions: Vec::new(),
            normals: Vec::new(),
            texcoords: Some(Vec::new()),
            tangents: None,
            triangles: Vec::new(),
        };
        let mut vertices = HashMap::new();
        let mut source_vertices = Vec::new();
        let mut changed = Vec::new();
        for (i, triangle) in self.triangles.iter().enumerate() {
            let edit = selected.contains(&i);
            let mut output = [0; 3];
            for (corner, &old) in triangle.iter().enumerate() {
                output[corner] = if let Some(&v) = vertices.get(&(old, edit)) {
                    v
                } else {
                    let v = u32::try_from(mesh.positions.len())?;
                    let old_index = old as usize;
                    mesh.positions.push(self.positions[old_index]);
                    mesh.normals.push(self.normals[old_index]);
                    let uv = if edit {
                        transform.apply(uvs[old_index])
                    } else {
                        uvs[old_index]
                    };
                    ensure!(uv.iter().all(|v| v.is_finite()), "UV transform overflow");
                    mesh.texcoords.as_mut().unwrap().push(uv);
                    source_vertices.push(old_index);
                    changed.push(edit);
                    vertices.insert((old, edit), v);
                    v
                };
            }
            mesh.triangles.push(output);
        }
        let mut mesh = mesh.generate_tangents()?;
        for (i, &edit) in changed.iter().enumerate() {
            if !edit {
                mesh.tangents.as_mut().unwrap()[i] = tangents[source_vertices[i]];
            }
        }
        mesh.validate()?;
        Ok(Edited {
            mesh,
            source_vertices,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn square() -> Mesh {
        Mesh {
            positions: vec![[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
            normals: vec![[0., 0., 1.]; 4],
            texcoords: Some(vec![[0., 0.], [1., 0.], [1., 1.], [0., 1.]]),
            tangents: None,
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        }
        .generate_tangents()
        .unwrap()
    }
    #[test]
    fn selected_uvs_split_shared_vertices_preserve_other_tuples_and_remap_payloads() {
        let original = square();
        let transform = Transform {
            scale: [-2., 3.],
            rotation_degrees: 90.,
            offset: [4., 5.],
        };
        let edited = original.transformed_uvs(&[0], transform).unwrap();
        assert_eq!(edited.mesh.positions.len(), 6);
        assert!(
            !edited.mesh.triangles[0]
                .iter()
                .any(|v| edited.mesh.triangles[1].contains(v))
        );
        for (i, t) in edited.mesh.triangles.iter().enumerate() {
            assert_eq!(
                t.map(|v| edited.source_vertices[v as usize] as u32),
                original.triangles[i]
            );
            for &v in t {
                let v = v as usize;
                let old = edited.source_vertices[v];
                assert_eq!(edited.mesh.positions[v], original.positions[old]);
                assert_eq!(edited.mesh.normals[v], original.normals[old]);
                let uv = edited.mesh.texcoords.as_ref().unwrap()[v];
                if i == 0 {
                    assert_eq!(
                        uv,
                        transform.apply(original.texcoords.as_ref().unwrap()[old])
                    );
                    assert_eq!(edited.mesh.tangents.as_ref().unwrap()[v][3], -1.);
                } else {
                    assert_eq!(uv, original.texcoords.as_ref().unwrap()[old]);
                    assert_eq!(
                        edited.mesh.tangents.as_ref().unwrap()[v],
                        original.tangents.as_ref().unwrap()[old]
                    );
                }
            }
        }
        assert_eq!(original, square());
        assert_eq!(
            original
                .transformed_uvs(&[0], Transform::default())
                .unwrap()
                .mesh,
            original
        );
        assert_eq!(
            original
                .transformed_uvs(&[0, 1], transform)
                .unwrap()
                .mesh
                .positions
                .len(),
            4
        );
        for indices in [vec![], vec![2], vec![0, 0]] {
            assert!(original.transformed_uvs(&indices, transform).is_err());
        }
        for bad in [
            Transform {
                scale: [0., 1.],
                ..transform
            },
            Transform {
                rotation_degrees: f64::NAN,
                ..transform
            },
            Transform {
                offset: [f64::INFINITY, 0.],
                ..transform
            },
            Transform {
                offset: [1e100, 1e100],
                ..transform
            },
        ] {
            assert!(original.transformed_uvs(&[0], bad).is_err());
        }
    }
}
