//! Explicit, opt-in consumer roles. Node names remain ordinary scene identities;
//! no role is inferred from a naming prefix. Version 1 targets the Unity adapter.
use super::scene::Scene;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub version: u32,
    pub engine: String,
    pub sockets: Vec<Socket>,
    pub colliders: Vec<Collider>,
    pub lods: Vec<Lod>,
}

#[cfg(test)]
mod tests {
    fn fixture() -> super::super::author::Asset {
        super::super::author::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples/game-assets/plate-consumer-profile.gasset"),
            &[],
        )
        .unwrap()
    }
    #[test]
    fn authored_profile_preserves_generic_geometry_and_exports_roles() {
        let asset = fixture();
        let profile = asset.scene.consumer.as_ref().unwrap();
        profile.validate(&asset.scene).unwrap();
        assert_eq!(profile.sockets[0].name, "handle");
        assert_eq!(profile.lods[0].levels.len(), 2);
        let bytes = asset.encode_glb().unwrap();
        let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let glb: serde_json::Value = serde_json::from_slice(&bytes[20..20 + length]).unwrap();
        assert_eq!(glb["nodes"].as_array().unwrap().len(), 5);
        assert_eq!(
            glb["extras"]["gcad_consumer_profile"],
            serde_json::to_value(profile).unwrap()
        );
        let mut ordinary = asset.scene.clone();
        ordinary.consumer = None;
        let plain = super::super::glb::encode_scene(&ordinary).unwrap();
        let length = u32::from_le_bytes(plain[12..16].try_into().unwrap()) as usize;
        let plain: serde_json::Value = serde_json::from_slice(&plain[20..20 + length]).unwrap();
        assert_eq!(plain["nodes"], glb["nodes"]);
        assert_eq!(plain["meshes"], glb["meshes"]);
        assert!(plain["extras"].get("gcad_consumer_profile").is_none());
    }
    #[test]
    fn roles_reject_conflicts_and_bad_lod_membership() {
        let asset = fixture();
        let original = asset.scene.consumer.clone().unwrap();
        let mut bad = original.clone();
        bad.sockets[0].node = "absent".into();
        assert!(bad.validate(&asset.scene).is_err());
        let mut bad = original.clone();
        bad.lods[0].levels[1].screen_height = 0.8;
        assert!(bad.validate(&asset.scene).is_err());
        let mut bad = original.clone();
        bad.lods[0].levels[0].screen_height = 0.6000000001;
        bad.lods[0].levels[1].screen_height = 0.6;
        assert!(bad.validate(&asset.scene).is_err());
        let mut bad = original.clone();
        bad.lods[0].levels[1].screen_height = 1e-100;
        assert!(bad.validate(&asset.scene).is_err());
        let mut bad = original.clone();
        bad.lods[0].levels[1].nodes = vec!["collision".into()];
        assert!(bad.validate(&asset.scene).is_err());
        let mut bad = original.clone();
        bad.colliders[0].node = "asset".into();
        assert!(bad.validate(&asset.scene).is_err());
        let mut bad = original.clone();
        bad.sockets.push(bad.sockets[0].clone());
        assert!(bad.validate(&asset.scene).is_err());
        let mut changed = asset.scene.clone();
        changed
            .nodes
            .iter_mut()
            .find(|n| n.name == "detail_low")
            .unwrap()
            .parent = None;
        assert!(original.validate(&changed).is_err());
        assert!(super::super::author::Document::parse("asset 1\nsocket handle node=grip").is_err());
        assert!(
            super::super::author::Document::parse("asset 1\nconsumer unity version=2").is_err()
        );
    }
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            version: 1,
            engine: "unity".into(),
            sockets: Vec::new(),
            colliders: Vec::new(),
            lods: Vec::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Socket {
    pub name: String,
    pub node: String,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    Box,
    Mesh,
    Convex,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Collider {
    pub node: String,
    pub shape: Shape,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Lod {
    pub name: String,
    pub root: String,
    pub levels: Vec<Level>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Level {
    pub screen_height: f64,
    pub nodes: Vec<String>,
}

fn named(value: &str) -> Result<()> {
    ensure!(
        !value.trim().is_empty() && value.len() <= 4096 && !value.chars().any(char::is_control),
        "invalid consumer reference name"
    );
    Ok(())
}
impl Profile {
    pub fn validate(&self, scene: &Scene) -> Result<()> {
        self.validate_graph(&scene.nodes, |mesh| {
            Ok(scene
                .meshes
                .get(mesh)
                .context("invalid profile mesh")?
                .primitives
                .iter()
                .map(|p| p.geometry.triangles.len())
                .sum())
        })
    }
    pub(super) fn validate_graph(
        &self,
        nodes: &[super::scene::Node],
        triangle_count: impl Fn(usize) -> Result<usize>,
    ) -> Result<()> {
        ensure!(
            self.version == 1 && self.engine == "unity",
            "unsupported consumer profile"
        );
        ensure!(
            self.sockets.len() + self.colliders.len() + self.lods.len() <= 4096,
            "consumer role limit exceeded"
        );
        let resolve = |name: &str| -> Result<usize> {
            named(name)?;
            nodes
                .iter()
                .position(|n| n.name == name)
                .context("consumer node is absent")
        };
        let mut roles = HashSet::new();
        let mut sockets = HashSet::new();
        for socket in &self.sockets {
            named(&socket.name)?;
            let index = resolve(&socket.node)?;
            ensure!(
                sockets.insert(&socket.name) && roles.insert(index),
                "duplicate socket name or consumer role"
            );
            ensure!(
                nodes[index].mesh.is_none(),
                "socket requires an explicit transform node"
            );
        }
        for collider in &self.colliders {
            let index = resolve(&collider.node)?;
            ensure!(roles.insert(index), "duplicate collider or consumer role");
            let mesh = nodes[index].mesh.context("collider requires a mesh node")?;
            ensure!(
                !nodes.iter().any(|n| n.parent == Some(index)),
                "collider must be a dedicated leaf mesh node"
            );
            if collider.shape == Shape::Convex {
                let triangles = triangle_count(mesh)?;
                ensure!(
                    triangles <= 255,
                    "Unity convex collision source exceeds the profile's 255-triangle limit"
                );
            }
        }
        let mut groups = HashSet::new();
        let mut roots = HashSet::new();
        for lod in &self.lods {
            named(&lod.name)?;
            let root = resolve(&lod.root)?;
            ensure!(
                groups.insert(&lod.name) && roots.insert(root) && roles.insert(root),
                "duplicate LOD name, root or consumer role"
            );
            ensure!(
                nodes[root].mesh.is_none(),
                "LOD root must be a transform node"
            );
            ensure!(
                (2..=8).contains(&lod.levels.len()),
                "LOD group requires two to eight levels"
            );
            let mut previous = 1.1_f32;
            for level in &lod.levels {
                ensure!(
                    level.screen_height.is_finite()
                        && level.screen_height > 0.
                        && level.screen_height <= 1.
                        && (level.screen_height as f32) > 0.
                        && (level.screen_height as f32) < previous,
                    "LOD heights must descend within (0,1]"
                );
                previous = level.screen_height as f32;
                ensure!(
                    !level.nodes.is_empty() && level.nodes.len() <= 4096,
                    "LOD level requires bounded mesh members"
                );
                for name in &level.nodes {
                    let node = resolve(name)?;
                    ensure!(
                        nodes[node].mesh.is_some() && roles.insert(node),
                        "LOD member must be a unique visual mesh, not a collider"
                    );
                    let mut parent = nodes[node].parent;
                    let mut found = false;
                    for _ in 0..nodes.len() {
                        match parent {
                            Some(p) => {
                                if p == root {
                                    found = true;
                                    break;
                                }
                                parent = nodes[p].parent
                            }
                            None => break,
                        }
                    }
                    ensure!(found, "LOD member is outside its group root");
                }
            }
        }
        for &root in &roots {
            let mut parent = nodes[root].parent;
            for _ in 0..nodes.len() {
                match parent {
                    Some(p) => {
                        ensure!(!roots.contains(&p), "nested LOD groups are unsupported");
                        parent = nodes[p].parent
                    }
                    None => break,
                }
            }
        }
        Ok(())
    }
    pub fn validate_skins(&self, scene: &Scene, skins: &[super::skin::Skin]) -> Result<()> {
        self.validate(scene)?;
        for skin in skins {
            ensure!(
                !self
                    .colliders
                    .iter()
                    .any(|c| c.node == scene.nodes[skin.mesh_node].name),
                "collision shapes cannot use skinned geometry"
            );
        }
        Ok(())
    }
}
