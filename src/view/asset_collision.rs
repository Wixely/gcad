//! Box bounds are computed in mesh-local space, then follow the displayed pose.
//! Convex cooking remains engine-owned; a source mesh is not a cooked hull.
use crate::asset::{author::Asset, consumer::Shape};
use monstertruck::modeling::{Matrix4, Point3, Transform as _};

pub(super) struct BoxVolume {
    pub name: String,
    pub corners: [[f32; 3]; 8],
}

pub(super) fn boxes(asset: &Asset, world: &[Matrix4]) -> anyhow::Result<Vec<BoxVolume>> {
    let mut result = Vec::new();
    let Some(profile) = &asset.scene.consumer else {
        return Ok(result);
    };
    for collider in &profile.colliders {
        if collider.shape != Shape::Box {
            continue;
        }
        let (index, node) = asset
            .scene
            .nodes
            .iter()
            .enumerate()
            .find(|(_, node)| node.name == collider.node)
            .unwrap();
        let mesh = &asset.scene.meshes[node.mesh.unwrap()];
        let mut low = [f64::INFINITY; 3];
        let mut high = [f64::NEG_INFINITY; 3];
        for primitive in &mesh.primitives {
            for point in &primitive.geometry.positions {
                for k in 0..3 {
                    low[k] = low[k].min(point[k]);
                    high[k] = high[k].max(point[k]);
                }
            }
        }
        let mut corners = [[0.; 3]; 8];
        for (i, corner) in corners.iter_mut().enumerate() {
            let p: [f64; 3] =
                std::array::from_fn(|k| if i & (1 << k) == 0 { low[k] } else { high[k] });
            let p = world[index].transform_point(Point3::new(p[0], p[1], p[2]));
            *corner = super::v3([p.x, p.y, p.z])?;
        }
        result.push(BoxVolume {
            name: collider.node.clone(),
            corners,
        });
    }
    Ok(result)
}

pub(super) fn paint(
    ui: &egui::Ui,
    rect: egui::Rect,
    projector: &egui_bench::viewer3d::Projector,
    boxes: &[BoxVolume],
) {
    let painter = ui.painter_at(rect);
    let colour = egui::Color32::from_rgb(60, 220, 160);
    for volume in boxes {
        let points = volume.corners.map(|p| {
            projector
                .project(p)
                .filter(|p| p.x.is_finite() && p.y.is_finite())
        });
        for i in 0..8 {
            for axis in 0..3 {
                let j = i ^ (1 << axis);
                if i < j {
                    if let (Some(a), Some(b)) = (points[i], points[j]) {
                        painter.line_segment([a, b], egui::Stroke::new(1.5, colour));
                    }
                }
            }
        }
        if let Some(p) = points[7].filter(|p| rect.contains(*p)) {
            painter.text(
                p + egui::vec2(5., -5.),
                egui::Align2::LEFT_BOTTOM,
                format!("{} (box)", volume.name),
                egui::FontId::proportional(12.),
                colour,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::asset::animation::{Clip, Interpolation, Track, Values};

    #[test]
    fn box_overlay_uses_local_bounds_reflection_rotation_and_animation() {
        let mut asset = crate::asset::author::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples/game-assets/plate-consumer-profile.gasset"),
            &[],
        )
        .unwrap();
        let node = asset
            .scene
            .nodes
            .iter()
            .position(|n| n.name == "collision")
            .unwrap();
        asset.scene.nodes[node].transform.translation = [5., 0., 0.];
        asset.scene.nodes[node].transform.scale = [-2., 3., 4.];
        let half = std::f64::consts::FRAC_1_SQRT_2;
        asset.scene.nodes[node].transform.rotation = [0., 0., half, half];
        asset.scene.clips.push(Clip {
            name: "lift collision".into(),
            tracks: vec![Track {
                node,
                times: vec![0., 1.],
                interpolation: Interpolation::Linear,
                values: Values::Translation(vec![[5., 0., 0.], [5., 0., 10.]]),
            }],
        });
        let encoded = asset.encode_glb().unwrap();
        let frame = super::super::frame(&asset, Some(0), 1.).unwrap();
        assert_eq!(frame.collision_boxes.len(), 1);
        let volume = &frame.collision_boxes[0];
        assert_eq!(volume.name, "collision");
        for (axis, expected) in [(-30., 60.), (-20., 60.), (40., 52.)]
            .into_iter()
            .enumerate()
        {
            let min = volume
                .corners
                .iter()
                .map(|p| p[axis])
                .fold(f32::INFINITY, f32::min);
            let max = volume
                .corners
                .iter()
                .map(|p| p[axis])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!((min - expected.0).abs() < 1e-4, "axis {axis}: {min}");
            assert!((max - expected.1).abs() < 1e-4, "axis {axis}: {max}");
        }
        let without = super::super::consumer::ConsumerView::default()
            .apply(&asset, super::super::frame(&asset, Some(0), 1.).unwrap());
        let with = super::super::consumer::ConsumerView {
            boxes: true,
            ..Default::default()
        }
        .apply(&asset, frame);
        assert_eq!(
            with.scene.surfaces.len(),
            1,
            "overlay does not expose collision source mesh"
        );
        assert!(with.scene.radius > without.scene.radius);
        assert_eq!(asset.encode_glb().unwrap(), encoded);
    }
}
