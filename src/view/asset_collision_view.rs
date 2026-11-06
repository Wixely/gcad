//! Collision declarations in the preview: fitted shapes drawn as solid, pickable surfaces, and
//! a view mode that shows what renders, what physics sees, or both.
use super::{Finish, Look, Surface};
use crate::asset::{
    author::Asset,
    collision::{self, Shape},
};
use anyhow::Result;
use egui_bench::viewer3d::Scene;
use monstertruck::modeling::Matrix4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum CollisionView {
    /// What a game draws: collision-only nodes are hidden.
    #[default]
    Visual,
    /// Everything drawn, with fitted shapes as see-through overlays.
    Both,
    /// Only what physics sees, in one flat colour.
    Collision,
}

/// Linear colour for collision geometry.
pub(super) const COLOUR: [f32; 3] = [0.05, 0.55, 0.35];
const GHOST: f32 = 0.35;

/// Surfaces for mesh primitives use groups `0..primitive_count`; the shape for declaration `k`
/// uses group `primitive_count + k`.
pub(super) fn primitive_count(asset: &Asset) -> usize {
    asset
        .scene
        .nodes
        .iter()
        .filter_map(|n| n.mesh)
        .map(|m| asset.scene.meshes[m].primitives.len())
        .sum()
}

/// The declaration a group draws, when it is a fitted shape rather than a mesh primitive.
pub(super) fn shape_of(asset: &Asset, group: usize) -> Option<usize> {
    group
        .checked_sub(primitive_count(asset))
        .filter(|&k| k < asset.scene.collision.len())
}

fn finish() -> Finish {
    Finish([0.8, 0., 1., 0., 0., 0., -1., 1., 1., 1., 1., 0., 0.].map(f32::to_bits))
}

/// Append one look per declaration and a surface for each fitted shape, posed by `world`.
pub(super) fn add_shapes(
    asset: &Asset,
    world: &[Matrix4],
    scene: &mut Scene<Finish>,
    looks: &mut Vec<Look>,
) -> Result<()> {
    for collision in &asset.scene.collision {
        let group = looks.len();
        looks.push(Look::default());
        if collision.shape == Shape::Mesh {
            continue;
        }
        let node = asset
            .scene
            .nodes
            .iter()
            .position(|n| n.name == collision.node)
            .expect("validated collision node");
        let fitted = collision::fit(&asset.scene, collision)?;
        let triangles = collision::place(world[node], &collision::display_triangles(&fitted));
        let mut surface = Surface {
            colour: COLOUR,
            material: finish(),
            group,
            ..Default::default()
        };
        for t in &triangles {
            let normal = super::v3(collision::triangle_normal(t))?;
            for &p in t {
                surface.positions.push(super::v3(p)?);
                surface.normals.push(normal);
            }
        }
        scene.surfaces.push(surface);
    }
    Ok(())
}

impl CollisionView {
    pub fn controls(&mut self, ui: &mut egui::Ui, asset: &Asset) {
        if asset.scene.collision.is_empty() {
            return;
        }
        ui.horizontal(|ui| {
            ui.label("Collision:");
            ui.selectable_value(self, CollisionView::Visual, "Visual")
                .on_hover_text("What a game draws; collision-only nodes are hidden");
            ui.selectable_value(self, CollisionView::Both, "Both")
                .on_hover_text("Everything, with fitted shapes drawn see-through");
            ui.selectable_value(self, CollisionView::Collision, "Collision")
                .on_hover_text("Only what physics sees. Click a shape to select its node");
        });
        if *self == CollisionView::Visual {
            return;
        }
        for collision in &asset.scene.collision {
            let detail = match collision::fit(&asset.scene, collision) {
                Ok(collision::Fitted::Hull { points, error, .. }) => {
                    format!("{} points, up to {error:.2} mm short", points.len())
                }
                Ok(collision::Fitted::Mesh { triangles }) => format!("{triangles} triangles"),
                Ok(_) => String::new(),
                Err(e) => format!("{e:#}"),
            };
            ui.small(format!(
                "{}: {}{}{}",
                collision.node,
                collision.shape.name(),
                if collision.render {
                    ", rendered"
                } else {
                    ", collision only"
                },
                if detail.is_empty() {
                    String::new()
                } else {
                    format!(" ({detail})")
                }
            ));
        }
    }

    /// Visibility of every group (mesh primitives, then shapes) for this mode, given the groups
    /// other roles already hide.
    pub fn visibility(self, asset: &Asset, primitive_visible: Vec<bool>) -> Vec<bool> {
        let mut visible = primitive_visible;
        let mut group = 0;
        for node in &asset.scene.nodes {
            let Some(mesh) = node.mesh else {
                continue;
            };
            let declared = asset.scene.collision.iter().find(|c| c.node == node.name);
            for _ in &asset.scene.meshes[mesh].primitives {
                let shown = match (self, declared) {
                    (CollisionView::Visual, Some(c)) => c.render,
                    (CollisionView::Visual | CollisionView::Both, _) => true,
                    (CollisionView::Collision, Some(c)) => c.shape == Shape::Mesh,
                    (CollisionView::Collision, None) => false,
                };
                visible[group] &= shown;
                group += 1;
            }
        }
        visible.extend(
            asset
                .scene
                .collision
                .iter()
                .map(|c| self != CollisionView::Visual && c.shape != Shape::Mesh),
        );
        visible
    }

    /// Colour what stays for this mode: one flat colour in collision view, see-through shapes
    /// in both.
    pub fn style(self, asset: &Asset, scene: &mut Scene<Finish>, looks: &mut [Look]) {
        let first = primitive_count(asset);
        match self {
            CollisionView::Visual => {}
            CollisionView::Both => {
                for look in looks.iter_mut().skip(first) {
                    look.opacity = GHOST;
                }
            }
            CollisionView::Collision => {
                for surface in &mut scene.surfaces {
                    surface.colour = COLOUR;
                    surface.material = finish();
                    surface.texture = None;
                    surface.normal_map = None;
                    surface.metal_rough_map = None;
                    surface.occlusion_map = None;
                    surface.emission_map = None;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::consumer::ConsumerView;
    use super::*;

    fn fixture() -> Asset {
        crate::asset::author::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples/game-assets/plate-collision.gasset"),
            &[],
        )
        .unwrap()
    }

    fn groups_of(asset: &Asset, view: CollisionView) -> (Vec<String>, usize) {
        let frame = ConsumerView {
            physics: view,
            ..Default::default()
        }
        .apply(asset, super::super::frame(asset, None, 0.).unwrap());
        let first = primitive_count(asset);
        let mut nodes: Vec<String> = Vec::new();
        let mut shapes = 0;
        for surface in &frame.scene.surfaces {
            if surface.group >= first {
                shapes += 1;
                if view == CollisionView::Both {
                    assert_eq!(frame.looks[surface.group].opacity, GHOST);
                }
            } else {
                let mut group = 0;
                for node in &asset.scene.nodes {
                    let Some(mesh) = node.mesh else { continue };
                    for _ in &asset.scene.meshes[mesh].primitives {
                        if group == surface.group && !nodes.contains(&node.name) {
                            nodes.push(node.name.clone());
                        }
                        group += 1;
                    }
                }
            }
            if view == CollisionView::Collision {
                assert_eq!(surface.colour, COLOUR);
                assert!(surface.texture.is_none());
            }
        }
        (nodes, shapes)
    }

    #[test]
    fn modes_show_what_renders_what_collides_or_both() {
        let asset = fixture();
        let source = asset.encode_glb().unwrap();
        let raw = super::super::frame(&asset, None, 0.).unwrap();
        assert_eq!(raw.looks.len(), primitive_count(&asset) + 5);
        assert_eq!(
            groups_of(&asset, CollisionView::Visual),
            (vec!["plate".to_string(), "lid".to_string()], 0)
        );
        let (both, shapes) = groups_of(&asset, CollisionView::Both);
        assert_eq!(both.len(), 5);
        assert_eq!(
            shapes, 4,
            "box, sphere, hull and capsule; the mesh shape is the mesh"
        );
        assert_eq!(
            groups_of(&asset, CollisionView::Collision),
            (vec!["exact".to_string()], 4)
        );
        assert_eq!(asset.encode_glb().unwrap(), source);
    }

    #[test]
    fn shapes_follow_their_node_through_a_clip() {
        let asset = fixture();
        let lid = asset
            .scene
            .collision
            .iter()
            .position(|c| c.node == "lid")
            .unwrap();
        let group = primitive_count(&asset) + lid;
        let at = |time| {
            let frame = super::super::frame(&asset, Some(0), time).unwrap();
            frame
                .scene
                .surfaces
                .iter()
                .find(|s| s.group == group)
                .unwrap()
                .positions
                .clone()
        };
        let (start, end) = (at(0.), at(1.));
        assert_eq!(start.len(), end.len());
        assert!(
            start
                .iter()
                .zip(&end)
                .any(|(a, b)| (a[0] - b[0]).abs() > 1.)
        );
    }
}
