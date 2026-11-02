//! Weight diagnostics change only the preview material, never authored data.
use super::{Finish, Frame};
use crate::asset::author::Asset;
use anyhow::{Context, Result};

pub(super) fn frame(
    asset: &Asset,
    clip: Option<usize>,
    time: f64,
    joint: Option<&str>,
) -> Result<Frame> {
    let mut result = super::frame(asset, clip, time)?;
    apply(asset, &mut result, joint)?;
    Ok(result)
}

pub(super) fn loaded_frame(asset: &Asset, clip: Option<usize>, time: f64, joint: Option<&str>) -> Result<Frame> {
    let mut result = super::loaded_frame(asset, clip, time)?;
    apply(asset, &mut result, joint)?;
    Ok(result)
}

fn apply(asset: &Asset, result: &mut Frame, joint: Option<&str>) -> Result<()> {
    let Some(joint) = joint else {
        return Ok(());
    };
    let joint = asset
        .scene
        .nodes
        .iter()
        .position(|node| node.name == joint)
        .context("unknown weight-preview joint")?;
    let scene = std::sync::Arc::get_mut(&mut result.scene).unwrap();
    scene.images.clear();
    scene.explicit_images.clear();
    scene.images.push(egui_bench::viewer3d::Image {
        width: 256,
        height: 1,
        rgba: (0..=255u8).flat_map(|w| [w, 0, 255 - w, 255]).collect(),
    });
    let mut surface_index = 0;
    for (node_index, node) in asset.scene.nodes.iter().enumerate() {
        let Some(mesh) = node.mesh else {
            continue;
        };
        let skin = asset.skins.iter().find(|skin| skin.mesh_node == node_index);
        let palette_joint =
            skin.and_then(|skin| skin.joints.iter().position(|&index| index == joint));
        for (primitive_index, primitive) in asset.scene.meshes[mesh].primitives.iter().enumerate() {
            let surface = &mut scene.surfaces[surface_index];
            surface_index += 1;
            surface.colour = if skin.is_some() { [1.; 3] } else { [0.35; 3] };
            surface.texture = skin.map(|_| 0);
            surface.normal_map = None;
            surface.metal_rough_map = None;
            surface.occlusion_map = None;
            surface.emission_map = None;
            surface.material = Finish::default();
            surface.uvs.clear();
            if let Some(skin) = skin {
                for triangle in &primitive.geometry.triangles {
                    for &vertex in triangle {
                        let weights = &skin.weights[primitive_index][vertex as usize];
                        let value = palette_joint.map_or(0., |joint| {
                            weights
                                .joints
                                .iter()
                                .zip(weights.weights)
                                .filter(|(index, _)| **index as usize == joint)
                                .map(|(_, weight)| weight)
                                .sum::<f64>()
                        });
                        surface
                            .uvs
                            .push([((value.clamp(0., 1.) * 255. + 0.5) / 256.) as f32, 0.5]);
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weight_preview_matches_vertex_bindings_without_changing_pose_or_export() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let asset = crate::asset::author::load(
            &root.join("examples/game-assets/plate-skinned.gasset"),
            &[],
        )
        .unwrap();
        let before = asset.encode_glb().unwrap();
        let skin = &asset.skins[0];
        let palette = skin
            .joints
            .iter()
            .position(|&joint| asset.scene.nodes[joint].name == "tip")
            .unwrap();
        let ordinary = super::super::frame(&asset, Some(0), 0.6).unwrap();
        let diagnostic = frame(&asset, Some(0), 0.6, Some("tip")).unwrap();
        assert_eq!(
            ordinary.scene.surfaces[0].positions,
            diagnostic.scene.surfaces[0].positions
        );
        assert_eq!(
            ordinary.scene.surfaces[0].normals,
            diagnostic.scene.surfaces[0].normals
        );
        let primitive =
            &asset.scene.meshes[asset.scene.nodes[skin.mesh_node].mesh.unwrap()].primitives[0];
        for (&vertex, uv) in primitive
            .geometry
            .triangles
            .iter()
            .flatten()
            .zip(&diagnostic.scene.surfaces[0].uvs)
        {
            let weights = &skin.weights[0][vertex as usize];
            let expected: f64 = weights
                .joints
                .iter()
                .zip(weights.weights)
                .filter(|(joint, _)| **joint as usize == palette)
                .map(|(_, w)| w)
                .sum();
            let decoded = (uv[0] as f64 * 256. - 0.5) / 255.;
            assert!((decoded - expected).abs() < 1e-6);
        }
        assert_eq!(diagnostic.scene.images[0].rgba[0..4], [0, 0, 255, 255]);
        assert_eq!(
            diagnostic.scene.images[0].rgba[1020..1024],
            [255, 0, 0, 255]
        );
        assert_eq!(asset.encode_glb().unwrap(), before);
    }
}
