//! Read-only inspection for game-asset documents. CAD panels are independent.
use crate::asset::author::Asset;

#[derive(Default)]
pub(super) struct Inspector {
    selected: Option<String>,
    visual_node: Option<String>,
    visuals: super::visuals::Visuals,
}

impl Inspector {
    pub fn invalidate(&mut self) {
        self.visuals.invalidate();
    }
    fn selection(&mut self, asset: &Asset) -> Option<usize> {
        let found = self
            .selected
            .as_ref()
            .and_then(|name| asset.scene.nodes.iter().position(|node| &node.name == name));
        if found.is_none() {
            self.selected = None;
        }
        found
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, asset: &Asset, clip: Option<usize>, time: f64) {
        let selected = self.selection(asset);
        let scene = &asset.scene;
        ui.heading("Asset scene");
        ui.label(format!(
            "{} nodes · {} meshes · {} skins",
            scene.nodes.len(),
            scene.meshes.len(),
            asset.skins.len()
        ));
        egui::ScrollArea::vertical()
            .id_salt("asset-nodes")
            .max_height(220.)
            .show_rows(ui, 22., scene.nodes.len(), |ui, rows| {
                for index in rows {
                    let node = &scene.nodes[index];
                    let role = if scene.consumer.as_ref().is_some_and(|p|p.sockets.iter().any(|s|s.node==node.name)) {
                        "socket"
                    } else if scene.consumer.as_ref().is_some_and(|p|p.colliders.iter().any(|c|c.node==node.name)) {
                        "collision"
                    } else if scene.consumer.as_ref().is_some_and(|p|p.lods.iter().any(|g|g.levels.iter().any(|l|l.nodes.contains(&node.name)))) {
                        "LOD mesh"
                    } else if asset.skins.iter().any(|s| s.joints.contains(&index)) {
                        "joint"
                    } else if node.mesh.is_some() {
                        "mesh"
                    } else {
                        "group"
                    };
                    if ui
                        .selectable_label(
                            selected == Some(index),
                            format!("{} ({role})", node.name),
                        )
                        .clicked()
                    {
                        self.selected = Some(node.name.clone());
                    }
                }
            });
        let Some(index) = self.selection(asset) else {
            ui.label("Select a node to inspect its pose, mesh and rig.");
            return;
        };
        let node = &scene.nodes[index];
        if self.visual_node != self.selected {
            self.visuals.invalidate();
            self.visual_node = self.selected.clone();
        }
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("asset-node-details")
            .show(ui, |ui| {
                ui.strong(&node.name);
                ui.horizontal(|ui| {
                    ui.label("Parent:");
                    if let Some(parent) = node.parent {
                        if ui.link(&scene.nodes[parent].name).clicked() {
                            self.selected = Some(scene.nodes[parent].name.clone());
                        }
                    } else {
                        ui.label("Scene root");
                    }
                });
                let pose = clip.map_or_else(
                    || Ok(node.transform),
                    |clip| {
                        scene.clips[clip]
                            .sample(scene, time, false)
                            .map(|pose| pose[index])
                    },
                );
                match pose {
                    Ok(pose) => {
                        ui.label("Local pose (relative to parent)");
                        ui.label(format!("Translation (mm): {:.3?}", pose.translation));
                        ui.label(format!("Rotation (xyzw): {:.3?}", pose.rotation));
                        ui.label(format!("Scale: {:.3?}", pose.scale));
                    }
                    Err(error) => {
                        ui.colored_label(
                            egui::Color32::LIGHT_RED,
                            format!("Pose unavailable: {error:#}"),
                        );
                    }
                }
                if let Some(mesh) = node.mesh {
                    let mesh = &scene.meshes[mesh];
                    ui.separator();
                    ui.label(format!("Mesh: {}", mesh.name));
                    for (i, primitive) in mesh.primitives.iter().enumerate() {
                        ui.collapsing(
                            format!("Primitive {i} · material {}", primitive.material),
                            |ui| {
                                let geometry = &primitive.geometry;
                                ui.label(format!(
                                    "{} vertices · {} triangles",
                                    geometry.positions.len(),
                                    geometry.triangles.len()
                                ));
                                ui.label(format!(
                                    "UVs: {} · tangents: {}",
                                    geometry.texcoords.is_some(),
                                    geometry.tangents.is_some()
                                ));
                                let material = &scene.materials[primitive.material];
                                ui.label(format!("Base colour: {:.3?}", material.base_colour));
                                ui.label(format!(
                                    "Metallic: {:.3} · roughness: {:.3}",
                                    material.metallic, material.roughness
                                ));
                                ui.label(format!("Alpha: {:?}", material.alpha));
                                for (label, map) in [
                                    ("Base colour", &material.colour_map),
                                    ("Normal", &material.normal_map),
                                    ("Metallic", &material.metallic_map),
                                    ("Roughness", &material.roughness_map),
                                    ("Occlusion", &material.occlusion_map),
                                    ("Emission", &material.emissive_map),
                                ] {
                                    if let Some(map) = map {
                                        ui.label(format!(
                                            "{label} map: {} × {}",
                                            map.width, map.height
                                        ));
                                    }
                                }
                            },
                        );
                    }
                }
                if let Some(mesh) = node.mesh {
                    self.visuals.ui(ui, &scene.meshes[mesh], &scene.materials);
                }
                for skin in asset.skins.iter().filter(|s| s.mesh_node == index) {
                    ui.separator();
                    ui.label(format!("Skin: {} joints", skin.joints.len()));
                    for &joint in &skin.joints {
                        if ui.link(&scene.nodes[joint].name).clicked() {
                            self.selected = Some(scene.nodes[joint].name.clone());
                        }
                    }
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_follows_node_identity_across_reload_and_clears_removed_nodes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut asset =
            crate::asset::author::load(&root.join("examples/game-assets/plate.gasset"), &[])
                .unwrap();
        let name = asset.scene.nodes[0].name.clone();
        let mut inspector = Inspector {
            selected: Some(name.clone()),
            ..Default::default()
        };
        assert_eq!(inspector.selection(&asset), Some(0));
        asset.scene.nodes.insert(
            0,
            crate::asset::scene::Node {
                name: "new-group".into(),
                ..Default::default()
            },
        );
        assert_eq!(inspector.selection(&asset), Some(1));
        asset.scene.nodes.retain(|n| n.name != name);
        assert_eq!(inspector.selection(&asset), None);
        assert!(inspector.selected.is_none());
    }
}
