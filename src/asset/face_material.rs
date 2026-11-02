//! Explicit derived material assignment from a validated source-face record.
use super::{
    Mesh,
    author::{Asset, SurfaceSettings},
    faces::SelectionFile,
    scene::{Primitive, Scene, SceneMesh},
    skin::Skin,
};
use anyhow::{Result, ensure};
use std::collections::BTreeSet;

pub struct Assigned {
    pub scene: Scene,
    pub skins: Vec<Skin>,
    /// Original primitive index for each output primitive; other maps are local to it.
    pub source_primitives: Vec<usize>,
    /// Per-output-primitive original vertex and triangle indices on the selected node.
    pub source_vertices: Vec<Vec<usize>>,
    pub source_triangles: Vec<Vec<usize>>,
    pub selected_triangles: usize,
}

impl Assigned {
    pub fn encode_glb(&self) -> Result<Vec<u8>> {
        super::glb::encode_skinned_scene(&self.scene, &self.skins)
    }
}

fn subset(mesh: &Mesh, triangles: &[usize]) -> Result<(Mesh, Vec<usize>)> {
    let vertices: Vec<_> = triangles
        .iter()
        .flat_map(|&i| mesh.triangles[i])
        .map(|v| v as usize)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut packed = vec![0; mesh.positions.len()];
    for (i, &v) in vertices.iter().enumerate() {
        packed[v] = u32::try_from(i)?;
    }
    let output = Mesh {
        positions: vertices.iter().map(|&v| mesh.positions[v]).collect(),
        normals: vertices.iter().map(|&v| mesh.normals[v]).collect(),
        texcoords: mesh
            .texcoords
            .as_ref()
            .map(|a| vertices.iter().map(|&v| a[v]).collect()),
        tangents: mesh
            .tangents
            .as_ref()
            .map(|a| vertices.iter().map(|&v| a[v]).collect()),
        triangles: triangles
            .iter()
            .map(|&i| mesh.triangles[i].map(|v| packed[v as usize]))
            .collect(),
    };
    output.validate()?;
    Ok((output, vertices))
}

/// Create a new scene, splitting the selected node into material primitives and
/// copying complete vertex tuples and weights exactly. Other shared instances
/// retain their original mesh. Source asset and persisted records are untouched.
pub fn assign(
    asset: &Asset,
    node: usize,
    record: &SelectionFile,
    settings: &SurfaceSettings,
) -> Result<Assigned> {
    assign_regions(asset, node, &[(record, settings)])
}

/// Resolve all regions before partitioning. Overlapping selections are errors,
/// including overlaps with identical settings; declaration order is not priority.
pub fn assign_regions(
    asset: &Asset,
    node: usize,
    regions: &[(&SelectionFile, &SurfaceSettings)],
) -> Result<Assigned> {
    asset.scene.validate()?;
    ensure!(!regions.is_empty(), "material assignment needs regions");
    let mut selected = BTreeSet::new();
    let mut resolved = Vec::new();
    for (record, settings) in regions {
        let triangles = asset.resolve_face_primitives(node, record)?;
        ensure!(
            triangles.iter().any(|t| !t.is_empty()),
            "material face selection is empty"
        );
        for (primitive, indices) in triangles.iter().enumerate() {
            for &triangle in indices {
                ensure!(
                    selected.insert((primitive, triangle)),
                    "face-material selections overlap"
                );
            }
        }
        resolved.push((triangles, *settings));
    }
    let mut bound = BTreeSet::new();
    for skin in &asset.skins {
        skin.validate(&asset.scene)?;
        ensure!(bound.insert(skin.mesh_node), "duplicate node skin binding");
    }
    let mesh_index = asset.scene.nodes[node].mesh.unwrap();
    let originals = &asset.scene.meshes[mesh_index].primitives;
    let mut scene = asset.scene.clone();
    let mut partitions = Vec::new();
    for (p, original) in originals.iter().enumerate() {
        let rest: Vec<_> = (0..original.geometry.triangles.len())
            .filter(|i| !selected.contains(&(p, *i)))
            .collect();
        partitions.push((p, rest, original.material));
        for (triangles, settings) in &resolved {
            if triangles[p].is_empty() {
                continue;
            }
            let material = settings.apply(&asset.scene.materials[original.material])?;
            partitions.push((p, triangles[p].clone(), scene.materials.len()));
            scene.materials.push(material);
        }
    }
    let mut source_primitives = Vec::new();
    let mut source_vertices = Vec::new();
    let mut source_triangles = Vec::new();
    let mut primitives = Vec::new();
    for (p, triangles, material) in partitions {
        if triangles.is_empty() {
            continue;
        }
        let (geometry, vertices) = subset(&originals[p].geometry, &triangles)?;
        primitives.push(Primitive { geometry, material });
        source_vertices.push(vertices);
        source_triangles.push(triangles);
        source_primitives.push(p);
    }
    let mesh = SceneMesh {
        name: format!("{} face material", scene.nodes[node].name),
        primitives,
    };
    if scene
        .nodes
        .iter()
        .enumerate()
        .any(|(i, n)| i != node && n.mesh == Some(mesh_index))
    {
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
                .zip(&source_primitives)
                .map(|(vertices, &p)| vertices.iter().map(|&v| skin.weights[p][v]).collect())
                .collect();
            let rebound = Skin::bind(&scene, node, skin.joints.clone(), weights)?;
            ensure!(
                rebound.inverse_bind == skin.inverse_bind,
                "material assignment changed inverse bind matrices"
            );
            Ok(rebound)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Assigned {
        scene,
        skins,
        source_primitives,
        source_vertices,
        source_triangles,
        selected_triangles: selected.len(),
    })
}
