//! X-ray rig overlay using the exact world transforms of the displayed frame.
use crate::asset::author::Asset;
use monstertruck::modeling::Matrix4;
use std::collections::BTreeSet;

pub(super) struct Node {
    pub index: usize,
    pub parent: Option<usize>,
    pub name: String,
    pub joint: bool,
    pub point: Option<[f32; 3]>,
}

pub(super) fn nodes(asset: &Asset, world: &[Matrix4]) -> Vec<Node> {
    let joints: BTreeSet<_> = asset
        .skins
        .iter()
        .flat_map(|skin| skin.joints.iter().copied())
        .collect();
    let mut ancestors = BTreeSet::new();
    for &joint in &joints {
        let mut at = Some(joint);
        while let Some(index) = at {
            if !ancestors.insert(index) {
                break;
            }
            at = asset.scene.nodes[index].parent;
        }
    }
    ancestors
        .into_iter()
        .map(|index| {
            let node = &asset.scene.nodes[index];
            let p = world[index].w;
            Node {
                index,
                parent: node.parent,
                name: node.name.clone(),
                joint: joints.contains(&index),
                point: super::v3([p.x, p.y, p.z]).ok(),
            }
        })
        .collect()
}

pub(super) fn paint(
    ui: &egui::Ui,
    rect: egui::Rect,
    projector: &egui_bench::viewer3d::Projector,
    nodes: &[Node],
    labels: bool,
) {
    let painter = ui.painter_at(rect);
    let projected: std::collections::BTreeMap<_, _> = nodes
        .iter()
        .filter_map(|node| {
            node.point
                .and_then(|point| projector.project(point))
                .filter(|p| p.x.is_finite() && p.y.is_finite())
                .map(|point| (node.index, point))
        })
        .collect();
    for node in nodes {
        let Some(&point) = projected.get(&node.index) else {
            continue;
        };
        if let Some(parent) = node.parent.and_then(|index| projected.get(&index)) {
            painter.line_segment([*parent, point], egui::Stroke::new(2., egui::Color32::GOLD));
        }
        let colour = if node.joint {
            egui::Color32::GOLD
        } else {
            egui::Color32::GRAY
        };
        painter.circle_filled(point, if node.joint { 4. } else { 2.5 }, colour);
        if labels && rect.contains(point) {
            painter.text(
                point + egui::vec2(6., -6.),
                egui::Align2::LEFT_BOTTOM,
                &node.name,
                egui::FontId::proportional(12.),
                colour,
            );
        }
    }
    if nodes.iter().any(|node| node.point.is_none()) {
        painter.text(
            rect.left_top() + egui::vec2(8., 8.),
            egui::Align2::LEFT_TOP,
            "Some rig positions exceed preview range",
            egui::FontId::proportional(13.),
            egui::Color32::LIGHT_RED,
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::asset::animation::{Interpolation, Track, Values};
    #[test]
    fn overlay_uses_animated_world_pose_and_retains_parent_links() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut asset = crate::asset::author::load(
            &root.join("examples/game-assets/plate-skinned.gasset"),
            &[],
        )
        .unwrap();
        let base = asset
            .scene
            .nodes
            .iter()
            .position(|n| n.name == "base")
            .unwrap();
        let tip = asset
            .scene
            .nodes
            .iter()
            .position(|n| n.name == "tip")
            .unwrap();
        asset.scene.clips[0].tracks.push(Track {
            node: base,
            times: vec![0., 1.],
            interpolation: Interpolation::Linear,
            values: Values::Translation(vec![[0.; 3], [10., 20., 30.]]),
        });
        let posed = super::super::frame(&asset, Some(0), 1.).unwrap();
        let base_node = posed.rig.iter().find(|n| n.index == base).unwrap();
        let tip_node = posed.rig.iter().find(|n| n.index == tip).unwrap();
        assert_eq!(base_node.point, Some([10., 20., 30.]));
        assert_eq!(tip_node.point, Some([10., 35., 30.]));
        assert_eq!(tip_node.parent, Some(base));
        assert!(base_node.joint && tip_node.joint);
        let rest = super::super::frame(&asset, None, 0.).unwrap();
        assert_eq!(
            rest.rig.iter().find(|n| n.index == base).unwrap().point,
            Some([0.; 3])
        );
    }
}
