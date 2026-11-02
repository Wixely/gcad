//! CPU baking of existing CAD texture patterns into portable 256-pixel tiles.
use super::{Mesh, material::Material};
use crate::model::{materials::Finish, texture::Map};
use anyhow::Result;

/// Produce derived geometry/material without modifying CAD appearance or solids.
/// Explicit image maps override generated maps. Projection is the asset box
/// projection, not the viewer's extent-dependent long-axis projection.
pub fn finish(
    mesh: &Mesh,
    source: &Finish,
    colour: [f64; 3],
    opacity: f64,
) -> Result<(Mesh, Material)> {
    let mut portable = source.clone();
    let mut projected = mesh.clone();
    if let Some(texture) = &mut portable.texture {
        texture.check()?;
        if texture.shown() || texture.tile.is_some() || texture.turn.is_some() {
            projected = mesh.project_box(texture.scale())?;
            let (sin, cos) = texture.rotation().rem_euclid(360.0).to_radians().sin_cos();
            for uv in projected.texcoords.as_mut().unwrap() {
                let [u, v] = *uv;
                *uv = [u * cos - v * sin, u * sin + v * cos];
            }
            projected.tangents = None;
            projected = projected.generate_tangents()?;
        }
        if let Some(name) = texture.pattern.as_deref().filter(|n| *n != "none") {
            let height = crate::model::texture::pattern(name);
            // Retain the finish colour as a linear material factor. This uses
            // the same pattern, but intentionally avoids the viewer's 1.3 tint gain.
            if texture.colour.is_none() {
                texture.colour = Some(height.clone());
            }
            if texture.normal.is_none() {
                texture.normal = Some(normal_from_height(&height));
            }
        }
        texture.pattern = None;
        texture.tile = None;
        texture.turn = None;
    }
    let material = Material::from_finish(&portable, colour, opacity)?;
    projected.validate()?;
    Ok((projected, material))
}

/// Periodic central differences; +Y follows increasing texture V. Height is
/// numeric red-channel data, not gamma-decoded colour. Strength is 1.5 per texel.
fn normal_from_height(height: &Map) -> Map {
    let mut rgba = Vec::with_capacity(height.rgba.len());
    let w = height.width;
    let h = height.height;
    let sample = |x: usize, y: usize| height.rgba[(y * w + x) * 4] as f64 / 255.0;
    for y in 0..h {
        for x in 0..w {
            let dx = (sample((x + 1) % w, y) - sample((x + w - 1) % w, y)) * 0.75;
            let dy = (sample(x, (y + 1) % h) - sample(x, (y + h - 1) % h)) * 0.75;
            let length = (dx * dx + dy * dy + 1.0).sqrt();
            for value in [-dx / length, -dy / length, 1.0 / length] {
                rgba.push(((value * 0.5 + 0.5) * 255.0).round() as u8);
            }
            rgba.push(255);
        }
    }
    Map {
        width: w,
        height: h,
        rgba,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::texture::{PATTERNS, Texture};

    #[test]
    fn all_patterns_bake_to_portable_maps_without_mutating_sources() {
        let mesh = crate::asset::tests::triangle();
        for name in PATTERNS.iter().filter(|n| **n != "none") {
            let source = Finish {
                texture: Some(Texture {
                    pattern: Some((*name).into()),
                    tile: Some(250.0),
                    turn: Some(90.0),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let before = source.clone();
            let (geometry, material) = finish(&mesh, &source, [0.5; 3], 1.0).unwrap();
            assert_eq!(source, before);
            assert_eq!(mesh, crate::asset::tests::triangle());
            let uv = geometry.texcoords.as_ref().unwrap();
            assert!((uv[1][0]).abs() < 1e-12 && (uv[1][1] - 4.0).abs() < 1e-12);
            assert!((uv[2][0] + 4.0).abs() < 1e-12 && (uv[2][1]).abs() < 1e-12);
            let tangent = geometry.tangents.as_ref().unwrap()[0];
            assert!(tangent[0].abs() < 1e-12 && (tangent[1] + 1.0).abs() < 1e-12);
            assert_eq!(material.colour_map.as_ref().unwrap().width, 256);
            assert_eq!(
                material.normal_map.as_ref().unwrap().rgba.len(),
                256 * 256 * 4
            );
            assert_eq!(material, finish(&mesh, &source, [0.5; 3], 1.0).unwrap().1);
            assert!(super::super::glb::encode(&geometry, &material).is_ok());
        }
    }

    #[test]
    fn untextured_finish_still_applies_explicit_uv_settings() {
        let mesh = crate::asset::tests::triangle();
        let source = Finish {
            texture: Some(Texture {
                tile: Some(250.0),
                turn: Some(90.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (mesh, material) = finish(&mesh, &source, [1.0; 3], 1.0).unwrap();
        assert!(material.colour_map.is_none() && material.normal_map.is_none());
        let uv = mesh.texcoords.unwrap();
        assert!(uv[1][0].abs() < 1e-12 && (uv[1][1] - 4.0).abs() < 1e-12);
        assert!((uv[2][0] + 4.0).abs() < 1e-12 && uv[2][1].abs() < 1e-12);
    }

    #[test]
    fn flat_height_is_neutral_and_slopes_follow_positive_v() {
        let flat = Map {
            width: 3,
            height: 3,
            rgba: [100, 100, 100, 255].repeat(9),
        };
        assert_eq!(
            normal_from_height(&flat).rgba,
            [128, 128, 255, 255].repeat(9)
        );
        let mut slope = flat;
        for y in 0..3 {
            for x in 0..3 {
                slope.rgba[(y * 3 + x) * 4] = (y * 100) as u8;
            }
        }
        let normal = normal_from_height(&slope);
        assert_eq!(normal.rgba[16], 128);
        assert!(normal.rgba[17] < 128);
        assert!(normal.rgba[18] > 128);
    }

    #[test]
    fn explicit_images_override_patterns_and_bad_features_still_fail() {
        let mesh = crate::asset::tests::triangle();
        let map = Map {
            width: 1,
            height: 1,
            rgba: vec![128, 128, 255, 255],
        };
        let mut source = Finish {
            texture: Some(Texture {
                pattern: Some("wood".into()),
                colour: Some(map.clone()),
                normal: Some(map.clone()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (_, material) = finish(&mesh, &source, [1.0; 3], 1.0).unwrap();
        assert_eq!(material.colour_map, Some(map.clone()));
        assert_eq!(material.normal_map, Some(map));
        source.texture.as_mut().unwrap().pattern = Some("unknown".into());
        assert!(finish(&mesh, &source, [1.0; 3], 1.0).is_err());
        source.texture.as_mut().unwrap().pattern = None;
        source.transmission = Some(0.5);
        assert!(finish(&mesh, &source, [1.0; 3], 1.0).is_err());
    }
}
