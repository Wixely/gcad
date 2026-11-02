//! Picking references address authored node names and guarded prepared geometry.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Default)]
pub(super) struct Selection {
    face_mode: bool,
    pub node: Option<String>,
    face: Option<Face>,
}

struct Face {
    primitive: usize,
    triangle: usize,
    source_face: Option<usize>,
    triangles: Vec<usize>,
    fingerprint: String,
}

fn fingerprint(mesh: &crate::asset::Mesh) -> String {
    let bytes = serde_json::to_vec(&(
        &mesh.positions,
        &mesh.triangles,
        &mesh.normals,
        &mesh.texcoords,
    ))
    .expect("validated mesh");
    format!("{:x}", Sha256::digest(bytes))
}

// Surface groups retain their original ordering when consumer visibility removes surfaces.
fn owner(asset: &Asset, group: usize) -> Option<(usize, usize)> {
    let mut next = 0;
    for (node, n) in asset.scene.nodes.iter().enumerate() {
        if let Some(mesh) = n.mesh {
            for primitive in 0..asset.scene.meshes[mesh].primitives.len() {
                if next == group {
                    return Some((node, primitive));
                }
                next += 1;
            }
        }
    }
    None
}

fn ray_triangle(origin: [f32; 3], direction: [f32; 3], t: &[[f32; 3]]) -> Option<f32> {
    let sub = |a: [f32; 3], b: [f32; 3]| std::array::from_fn::<_, 3, _>(|i| a[i] - b[i]);
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let dot = |a: [f32; 3], b: [f32; 3]| (0..3).map(|i| a[i] * b[i]).sum::<f32>();
    let e1 = sub(t[1], t[0]);
    let e2 = sub(t[2], t[0]);
    let p = cross(direction, e2);
    let determinant = dot(e1, p);
    if determinant.abs() < 1e-10 {
        return None;
    }
    let s = sub(origin, t[0]);
    let u = dot(s, p) / determinant;
    let q = cross(s, e1);
    let v = dot(direction, q) / determinant;
    let distance = dot(e2, q) / determinant;
    (u >= -1e-5 && v >= -1e-5 && u + v <= 1.00001 && distance >= 0.).then_some(distance)
}

impl Selection {
    pub fn invalidate(&mut self) {
        self.face = None;
    }

    pub fn select_node(&mut self, name: Option<String>) {
        if self.node != name {
            self.node = name;
            self.face = None;
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, asset: &Asset) {
        if self
            .node
            .as_ref()
            .is_some_and(|name| !asset.scene.nodes.iter().any(|n| &n.name == name))
        {
            self.node = None;
            self.face = None;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Select:");
            if ui
                .selectable_value(&mut self.face_mode, false, "Item")
                .changed()
            {
                self.face = None;
            }
            if ui
                .selectable_value(&mut self.face_mode, true, "Face")
                .changed()
            {
                self.face = None;
            }
            if ui.button("Clear selection").clicked() {
                self.node = None;
                self.face = None;
            }
            if let Some(name) = &self.node {
                ui.label(format!("Node: {name}"));
                if let Some(face) = &self.face {
                    ui.label(match face.source_face {
                        Some(id) => format!("Source face {id} · primitive {}", face.primitive),
                        None => {
                            format!("Triangle {} · primitive {}", face.triangle, face.primitive)
                        }
                    });
                }
                if ui.button("Copy reference").clicked() {
                    ui.ctx().copy_text(self.reference());
                }
            }
        });
        ui.small("Click to select; drag to orbit. Face outlines are X-ray. References use zero-based indices; copy again after geometry edits.");
    }

    fn reference(&self) -> String {
        let mut value =
            serde_json::json!({"kind":"gcad-viewer-selection","version":1,"node":self.node});
        if let Some(f) = &self.face {
            value["primitive"] = f.primitive.into();
            value["triangle"] = f.triangle.into();
            value["source_face"] = f.source_face.into();
            value["mesh_sha256"] = f.fingerprint.clone().into();
        }
        serde_json::to_string_pretty(&value).unwrap()
    }

    pub fn pick(
        &mut self,
        asset: &Asset,
        viewer: &Viewer<Finish>,
        rect: egui::Rect,
        at: egui::Pos2,
    ) {
        let Some(hit) = viewer.pick(rect, at) else {
            self.node = None;
            self.face = None;
            return;
        };
        let surface = &viewer.scene().surfaces[hit.surface];
        let Some((node, primitive)) = owner(asset, surface.group) else {
            return;
        };
        self.node = Some(asset.scene.nodes[node].name.clone());
        self.face = None;
        if !self.face_mode {
            return;
        }
        let (origin, direction) = viewer.projector(rect).ray(at);
        let look = viewer.look(surface.group);
        let triangle = surface
            .positions
            .chunks_exact(3)
            .enumerate()
            .filter_map(|(i, t)| {
                let placed = [look.place(t[0]), look.place(t[1]), look.place(t[2])];
                ray_triangle(origin, direction, &placed).map(|d| (i, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        let Some(triangle) = triangle else {
            return;
        };
        let mesh = &asset.scene.meshes[asset.scene.nodes[node].mesh.unwrap()].primitives[primitive]
            .geometry;
        let labels = asset
            .primitive_face_maps(node)
            .ok()
            .and_then(|maps| maps[primitive].triangle_faces(mesh).ok());
        let source_face = labels.and_then(|l| l.get(triangle)).copied();
        let triangles = match (labels, source_face) {
            (Some(labels), Some(face)) => labels
                .iter()
                .enumerate()
                .filter_map(|(i, &f)| (f == face).then_some(i))
                .collect(),
            _ => vec![triangle],
        };
        self.face = Some(Face {
            primitive,
            triangle,
            source_face,
            triangles,
            fingerprint: fingerprint(mesh),
        });
    }

    pub fn looks(&self, asset: &Asset, frame: &Frame) -> Vec<Look> {
        let mut looks = frame.looks.clone();
        if self.node.is_none() {
            return looks;
        }
        if self.face.is_none() {
            for surface in &frame.scene.surfaces {
                if let Some((node, _)) = owner(asset, surface.group) {
                    looks[surface.group].selected =
                        self.node.as_ref() == Some(&asset.scene.nodes[node].name);
                }
            }
        }
        looks
    }

    pub fn paint(&self, ui: &egui::Ui, asset: &Asset, viewer: &Viewer<Finish>, rect: egui::Rect) {
        let Some(face) = &self.face else {
            return;
        };
        let projector = viewer.projector(rect);
        let painter = ui.painter().with_clip_rect(rect);
        for surface in &viewer.scene().surfaces {
            let Some((node, primitive)) = owner(asset, surface.group) else {
                continue;
            };
            if self.node.as_ref() != Some(&asset.scene.nodes[node].name)
                || primitive != face.primitive
            {
                continue;
            }
            let look = viewer.look(surface.group);
            if !look.visible() {
                continue;
            }
            for &triangle in &face.triangles {
                let Some(t) = surface.positions.get(triangle * 3..triangle * 3 + 3) else {
                    continue;
                };
                let points: Vec<_> = t
                    .iter()
                    .filter_map(|&p| projector.project(look.place(p)))
                    .collect();
                if points.len() == 3 {
                    painter.add(egui::Shape::closed_line(
                        points,
                        egui::Stroke::new(1.5, egui::Color32::YELLOW),
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picking_tracks_instances_turntable_and_source_faces() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/game-assets/plate-unwrapped.gasset");
        let mut asset = crate::asset::author::load_with_faces(&path, &[]).unwrap();
        let mut instance = asset.scene.nodes[0].clone();
        instance.name = "second-instance".into();
        instance.transform.translation[0] += 120.;
        asset.scene.nodes.push(instance);
        let frame = super::super::frame(&asset, None, 0.).unwrap();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 800.));
        for ortho in [true, false] {
            for angle in [0., 90., 225.] {
                let stage = super::super::stage::Stage {
                    model: angle,
                    ..Default::default()
                };
                let viewer = Viewer::new(
                    frame.scene.clone(),
                    stage.camera(Camera {
                        ortho,
                        ..Default::default()
                    }),
                )
                .looks(frame.looks.clone());
                // Find an exposed pixel on each instance, independently of face order.
                for group in 0..2 {
                    let surface = &frame.scene.surfaces[group];
                    let at = surface
                        .positions
                        .chunks_exact(3)
                        .find_map(|t| {
                            let centre =
                                std::array::from_fn(|k| (t[0][k] + t[1][k] + t[2][k]) / 3.);
                            let at = viewer.projector(rect).project(centre)?;
                            let hit = viewer.pick(rect, at)?;
                            (viewer.scene().surfaces[hit.surface].group == group).then_some(at)
                        })
                        .expect("instance has an exposed triangle");
                    let mut selection = Selection::default();
                    selection.pick(&asset, &viewer, rect, at);
                    assert_eq!(
                        selection.node.as_deref(),
                        Some(asset.scene.nodes[group].name.as_str())
                    );
                    let looks = selection.looks(&asset, &frame);
                    assert!(looks[group].selected);
                    assert!(!looks[1 - group].selected);
                    selection.face_mode = true;
                    selection.pick(&asset, &viewer, rect, at);
                    let f = selection.face.as_ref().unwrap();
                    assert!(!f.triangles.is_empty());
                    if group == 0 {
                        assert!(f.source_face.is_some());
                        let mesh = &asset.scene.meshes[0].primitives[0].geometry;
                        let labels = asset.face_map(0).unwrap().triangle_faces(mesh).unwrap();
                        assert!(
                            f.triangles
                                .iter()
                                .all(|&t| Some(labels[t]) == f.source_face)
                        );
                    } else {
                        assert!(f.source_face.is_none());
                    }
                    let reference: serde_json::Value =
                        serde_json::from_str(&selection.reference()).unwrap();
                    assert_eq!(reference["node"], asset.scene.nodes[group].name);
                    assert_eq!(reference["mesh_sha256"].as_str().unwrap().len(), 64);
                    selection.invalidate();
                    assert!(selection.face.is_none());
                    assert!(selection.node.is_some());
                }
            }
        }
        // Consumer filtering removes surfaces but must not renumber node ownership.
        let mut filtered = frame.scene.surfaces.clone();
        filtered.remove(0);
        assert_eq!(owner(&asset, filtered[0].group), Some((1, 0)));
        let mut selection = Selection::default();
        selection.node = Some("plate".into());
        let viewer = Viewer::new(frame.scene.clone(), Camera::default());
        selection.pick(&asset, &viewer, rect, egui::pos2(-10000., -10000.));
        assert!(selection.node.is_none());
    }

    #[test]
    fn fingerprint_changes_with_geometry_and_uvs_and_ray_chooses_forward_hits() {
        let asset = crate::asset::author::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples/game-assets/plate.gasset"),
            &[],
        )
        .unwrap();
        let mesh = &asset.scene.meshes[0].primitives[0].geometry;
        let before = fingerprint(mesh);
        let mut changed = mesh.clone();
        changed.positions[0][0] += 1.;
        assert_ne!(before, fingerprint(&changed));
        let mut changed = mesh.clone();
        changed.texcoords.as_mut().unwrap()[0][0] += 0.1;
        assert_ne!(before, fingerprint(&changed));
        let t = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]];
        assert_eq!(ray_triangle([0.2, 0.2, 1.], [0., 0., -1.], &t), Some(1.));
        assert_eq!(ray_triangle([0.2, 0.2, 1.], [0., 0., 1.], &t), None);
    }
}
