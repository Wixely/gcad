//! Explicit selected-corner skin assignment with guarded source-face provenance.
use super::{
    Mesh,
    author::Asset,
    faces::{FaceMap, SelectionFile},
    scene::Scene,
    skin::{Skin, Weights},
};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boundary {
    /// Fail when selected and unselected triangles share a vertex index.
    RejectShared,
    /// Duplicate selected boundary tuples; deformation can open a seam.
    Split,
}

pub(crate) fn validate_influences(influences: &[(&str, f64)]) -> Result<()> {
    ensure!(
        (1..=4).contains(&influences.len()),
        "face weights require 1..4 influences"
    );
    let mut names = BTreeSet::new();
    for &(name, weight) in influences {
        ensure!(
            !name.trim().is_empty() && names.insert(name),
            "face weight joint repeated or empty"
        );
        ensure!(
            weight.is_finite() && weight > 0. && weight <= 1.,
            "face weights must be positive finite values"
        );
    }
    ensure!(
        (influences.iter().map(|(_, w)| *w).sum::<f64>() - 1.).abs() < 1e-8,
        "face weights must sum to one"
    );
    Ok(())
}

pub struct Edited {
    pub scene: Scene,
    pub skins: Vec<Skin>,
    pub face_maps: Vec<FaceMap>,
    pub source_vertices: Vec<Vec<usize>>,
    /// Output vertices receiving the replacement assignment, per primitive.
    pub assigned_vertices: Vec<Vec<bool>>,
    pub selected_triangles: usize,
    pub split_vertices: usize,
}

impl Edited {
    pub fn encode_glb(&self) -> Result<Vec<u8>> {
        super::glb::encode_skinned_scene(&self.scene, &self.skins)
    }
}

/// Replace selected face-corner weights using 1..4 named existing palette joints.
/// Values must be positive, finite and sum to one; no silent normalisation occurs.
/// Unselected corners, rest geometry, shading tuples and other skins stay exact.
pub fn edit_scene(
    asset: &Asset,
    node: usize,
    record: &SelectionFile,
    influences: &[(&str, f64)],
    boundary: Boundary,
) -> Result<Edited> {
    asset.scene.validate()?;
    validate_influences(influences)?;
    let selections = asset.resolve_face_primitives(node, record)?;
    let maps = asset.primitive_face_maps(node)?;
    let mut bound = BTreeSet::new();
    for skin in &asset.skins {
        skin.validate(&asset.scene)?;
        ensure!(bound.insert(skin.mesh_node), "duplicate node skin binding");
    }
    let skin = asset
        .skins
        .iter()
        .find(|s| s.mesh_node == node)
        .context("face weight editing requires an existing skin")?;
    ensure!(
        (1..=4).contains(&influences.len()),
        "face weights require 1..4 influences"
    );
    let mut assignment = Weights {
        joints: [0; 4],
        weights: [0.; 4],
    };
    let mut used = BTreeSet::new();
    for (i, &(name, weight)) in influences.iter().enumerate() {
        ensure!(
            weight.is_finite() && weight > 0. && weight <= 1.,
            "face weights must be positive finite values"
        );
        let joint = skin
            .joints
            .iter()
            .position(|&j| asset.scene.nodes[j].name == name)
            .context("face weight joint is not in the skin palette")?;
        ensure!(used.insert(joint), "face weight joint repeated");
        assignment.joints[i] = joint as u16;
        assignment.weights[i] = weight;
    }
    ensure!(
        (assignment.weights.iter().sum::<f64>() - 1.).abs() < 1e-8,
        "face weights must sum to one"
    );
    let selected_triangles = selections.iter().map(Vec::len).sum();
    ensure!(selected_triangles > 0, "face weight selection is empty");
    let mesh_index = asset.scene.nodes[node].mesh.unwrap();
    let original = &asset.scene.meshes[mesh_index];
    let mut mesh = original.clone();
    let mut source_vertices = Vec::new();
    let mut assigned_vertices = Vec::new();
    let mut face_maps = Vec::new();
    let mut weights = Vec::new();
    let mut split_vertices = 0;
    for (p, selection) in selections.iter().enumerate() {
        let geometry = &original.primitives[p].geometry;
        let (derived, sources, assigned) = isolate(geometry, selection, boundary)?;
        split_vertices += derived.positions.len() - geometry.positions.len();
        weights.push(
            sources
                .iter()
                .zip(&assigned)
                .map(|(&v, &selected)| {
                    if selected {
                        assignment
                    } else {
                        skin.weights[p][v]
                    }
                })
                .collect(),
        );
        let (derived, map) = maps[p].retain_triangles(geometry, derived)?;
        mesh.primitives[p].geometry = derived;
        source_vertices.push(sources);
        assigned_vertices.push(assigned);
        face_maps.push(map);
    }
    let mut scene = asset.scene.clone();
    if split_vertices > 0 {
        if scene
            .nodes
            .iter()
            .enumerate()
            .any(|(i, n)| i != node && n.mesh == Some(mesh_index))
        {
            mesh.name = format!("{} face weights", scene.nodes[node].name);
            scene.nodes[node].mesh = Some(scene.meshes.len());
            scene.meshes.push(mesh);
        } else {
            scene.meshes[mesh_index] = mesh;
        }
    }
    let rebound = Skin::bind(&scene, node, skin.joints.clone(), weights)?;
    ensure!(
        rebound.inverse_bind == skin.inverse_bind,
        "face weights changed inverse bind matrices"
    );
    let skins = asset
        .skins
        .iter()
        .map(|s| {
            if s.mesh_node == node {
                Ok(rebound.clone())
            } else {
                s.validate(&scene)?;
                Ok(s.clone())
            }
        })
        .collect::<Result<_>>()?;
    Ok(Edited {
        scene,
        skins,
        face_maps,
        source_vertices,
        assigned_vertices,
        selected_triangles,
        split_vertices,
    })
}

fn isolate(
    mesh: &Mesh,
    triangles: &[usize],
    boundary: Boundary,
) -> Result<(Mesh, Vec<usize>, Vec<bool>)> {
    mesh.validate()?;
    let mut chosen = BTreeSet::new();
    for &t in triangles {
        ensure!(
            t < mesh.triangles.len() && chosen.insert(t),
            "invalid or repeated face weight triangle"
        );
    }
    let mut selected = vec![false; mesh.positions.len()];
    let mut unselected = selected.clone();
    for (t, triangle) in mesh.triangles.iter().enumerate() {
        let flags = if chosen.contains(&t) {
            &mut selected
        } else {
            &mut unselected
        };
        for &v in triangle {
            flags[v as usize] = true;
        }
    }
    let mut output = mesh.clone();
    let mut sources: Vec<_> = (0..mesh.positions.len()).collect();
    let mut assigned = selected.clone();
    let mut remap: Vec<u32> = (0..mesh.positions.len())
        .map(u32::try_from)
        .collect::<std::result::Result<_, _>>()?;
    for v in 0..mesh.positions.len() {
        if !(selected[v] && unselected[v]) {
            continue;
        }
        ensure!(
            boundary == Boundary::Split,
            "face weight boundary shares vertices; explicitly choose split to allow a deformation seam"
        );
        remap[v] = u32::try_from(output.positions.len())?;
        output.positions.push(mesh.positions[v]);
        output.normals.push(mesh.normals[v]);
        if let Some(uv) = &mut output.texcoords {
            uv.push(mesh.texcoords.as_ref().unwrap()[v]);
        }
        if let Some(tangents) = &mut output.tangents {
            tangents.push(mesh.tangents.as_ref().unwrap()[v]);
        }
        assigned[v] = false;
        assigned.push(true);
        sources.push(v);
    }
    for &t in triangles {
        output.triangles[t] = mesh.triangles[t].map(|v| remap[v as usize]);
    }
    output.validate()?;
    Ok((output, sources, assigned))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_boundary_is_explicit_and_preserves_complete_tuples() {
        let mesh = Mesh {
            positions: vec![[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
            normals: vec![[0., 0., 1.]; 4],
            texcoords: Some(vec![[0., 0.], [1., 0.], [1., 1.], [0., 1.]]),
            tangents: Some(vec![[1., 0., 0., 1.]; 4]),
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        assert!(isolate(&mesh, &[0], Boundary::RejectShared).is_err());
        let (edited, sources, assigned) = isolate(&mesh, &[0], Boundary::Split).unwrap();
        assert_eq!(sources, vec![0, 1, 2, 3, 0, 2]);
        assert_eq!(assigned, vec![false, true, false, false, true, true]);
        assert_eq!(edited.triangles, vec![[4, 1, 5], [0, 2, 3]]);
        for (v, &old) in sources.iter().enumerate() {
            assert_eq!(edited.positions[v], mesh.positions[old]);
            assert_eq!(edited.normals[v], mesh.normals[old]);
            assert_eq!(
                edited.texcoords.as_ref().unwrap()[v],
                mesh.texcoords.as_ref().unwrap()[old]
            );
            assert_eq!(
                edited.tangents.as_ref().unwrap()[v],
                mesh.tangents.as_ref().unwrap()[old]
            );
        }
        let (all, _, flags) = isolate(&mesh, &[0, 1], Boundary::RejectShared).unwrap();
        assert_eq!(all, mesh);
        assert!(flags.iter().all(|x| *x));
        let (none, _, flags) = isolate(&mesh, &[], Boundary::RejectShared).unwrap();
        assert_eq!(none, mesh);
        assert!(flags.iter().all(|x| !*x));
        assert!(isolate(&mesh, &[2], Boundary::Split).is_err());
        assert!(isolate(&mesh, &[0, 0], Boundary::Split).is_err());
    }
}
