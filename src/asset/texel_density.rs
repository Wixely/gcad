//! Non-destructive rest-pose, area-weighted texture density for delivery assets.
use super::{
    material::{AlphaMode, Material},
    scene::Scene,
};
use crate::model::texture::Map;
use anyhow::{Result, ensure};
use monstertruck::modeling::{InnerSpace as _, Point3, Transform as _};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub pixels_per_metre: f64,
    pub max_dimension: u32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            pixels_per_metre: 512.,
            max_dimension: 2048,
        }
    }
}
impl Settings {
    pub fn validate(self) -> Result<()> {
        ensure!(
            self.pixels_per_metre.is_finite() && (1.0..=16384.).contains(&self.pixels_per_metre),
            "texel density must be 1..16384 pixels per metre"
        );
        ensure!(
            (4..=4096).contains(&self.max_dimension),
            "maximum texture dimension must be 4..4096"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Entry {
    pub node: String,
    pub primitive: usize,
    pub map: String,
    pub source_size: [usize; 2],
    pub output_size: [usize; 2],
    pub achieved_pixels_per_metre: f64,
}
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Report {
    pub settings: Settings,
    pub maps: Vec<Entry>,
}

fn dimensions(map: &Map, ratio: f64, settings: Settings) -> [usize; 2] {
    let desired = settings.pixels_per_metre / ((map.width * map.height) as f64 * ratio).sqrt();
    let scale = desired
        .min(1.)
        .min(settings.max_dimension as f64 / map.width.max(map.height) as f64);
    [
        (map.width as f64 * scale).round().max(1.) as usize,
        (map.height as f64 * scale).round().max(1.) as usize,
    ]
}

// Filter colour in linear light with premultiplied alpha; numeric channels stay
// linear. Filter normal vectors and renormalize rather than gamma-correcting RGB.
fn resize(map: &Map, size: [usize; 2], colour: bool, normal: bool) -> Map {
    if size == [map.width, map.height] {
        return map.clone();
    }
    let decode = |x: f32| {
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    let encode = |x: f32| {
        if x <= 0.0031308 {
            x * 12.92
        } else {
            1.055 * x.powf(1. / 2.4) - 0.055
        }
    };
    let input = image::Rgba32FImage::from_fn(map.width as u32, map.height as u32, |x, y| {
        let offset = (y as usize * map.width + x as usize) * 4;
        let mut v = std::array::from_fn(|i| map.rgba[offset + i] as f32 / 255.);
        for i in 0..3 {
            v[i] = if normal {
                v[i] * 2. - 1.
            } else if colour {
                decode(v[i]) * v[3]
            } else {
                v[i]
            };
        }
        image::Rgba(v)
    });
    let filtered = image::imageops::resize(
        &input,
        size[0] as u32,
        size[1] as u32,
        image::imageops::FilterType::Triangle,
    );
    let mut rgba = Vec::with_capacity(size[0] * size[1] * 4);
    for pixel in filtered.pixels() {
        let mut v = pixel.0;
        if normal {
            let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            for i in 0..3 {
                v[i] = if length > 1e-6 {
                    v[i] / length * 0.5 + 0.5
                } else if i == 2 {
                    1.
                } else {
                    0.5
                };
            }
        } else if colour {
            for i in 0..3 {
                v[i] = if v[3] > 1e-6 {
                    encode((v[i] / v[3]).clamp(0., 1.))
                } else {
                    0.
                };
            }
        }
        rgba.extend(v.map(|x| (x.clamp(0., 1.) * 255.).round() as u8));
    }
    Map {
        width: size[0],
        height: size[1],
        rgba,
    }
}

fn maps(material: &Material) -> [(&str, &Option<Map>, bool, bool); 6] {
    [
        ("base_colour", &material.colour_map, true, false),
        ("normal", &material.normal_map, false, true),
        ("metallic", &material.metallic_map, false, false),
        ("roughness", &material.roughness_map, false, false),
        ("occlusion", &material.occlusion_map, false, false),
        ("emission", &material.emissive_map, true, false),
    ]
}

pub fn normalize(source: &Scene, settings: Settings) -> Result<(Scene, Report)> {
    settings.validate()?;
    source.validate()?;
    let world = source.world_transforms()?;
    let mut scene = source.clone();
    scene.meshes.clear();
    scene.materials.clear();
    let mut materials = BTreeMap::new();
    let mut meshes = BTreeMap::new();
    let mut report = Report {settings, ..Default::default()};
    let mut pixels = 0usize;
    for (node_index, node) in source.nodes.iter().enumerate() {
        let Some(mesh_index) = node.mesh else {
            continue;
        };
        let mut mesh = source.meshes[mesh_index].clone();
        let mut material_ids = Vec::new();
        for (primitive_index, primitive) in mesh.primitives.iter_mut().enumerate() {
            let material = &source.materials[primitive.material];
            let textured = maps(material).iter().any(|(_, m, _, _)| m.is_some());
            let mut sizes = Vec::new();
            if textured {
                ensure!(
                    material.shader.is_none() && material.mip_chain.is_none(),
                    "density normalization requires ordinary maps; UV shaders and explicit mip chains are unsupported"
                );
                ensure!(
                    !matches!(material.alpha, AlphaMode::Mask { .. }),
                    "density normalization of alpha-mask materials requires coverage preservation and is not yet supported"
                );
                let geometry = &primitive.geometry;
                let uv = geometry
                    .texcoords
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("textured primitive has no UVs"))?;
                let mut area = 0.;
                let mut uv_area = 0.;
                for t in &geometry.triangles {
                    let points = t.map(|i| {
                        let p = geometry.positions[i as usize];
                        world[node_index].transform_point(Point3::new(p[0], p[1], p[2]))
                    });
                    area += (points[1] - points[0])
                        .cross(points[2] - points[0])
                        .magnitude()
                        * 0.5
                        / 1_000_000.;
                    let u = t.map(|i| uv[i as usize]);
                    uv_area += ((u[1][0] - u[0][0]) * (u[2][1] - u[0][1])
                        - (u[1][1] - u[0][1]) * (u[2][0] - u[0][0]))
                        .abs()
                        * 0.5;
                }
                ensure!(
                    area.is_finite() && area > 0. && uv_area.is_finite() && uv_area > 0.,
                    "density normalization needs nondegenerate geometry and UV area"
                );
                let ratio = uv_area / area;
                for (name, map, _, _) in maps(material) {
                    if let Some(map) = map {
                        ensure!(
                            map.width
                                .checked_mul(map.height)
                                .is_some_and(|n| n <= 16_777_216),
                            "density resampling input exceeds 16 megapixels"
                        );
                        let size = dimensions(map, ratio, settings);
                        report.maps.push(Entry {
                            node: node.name.clone(),
                            primitive: primitive_index,
                            map: name.into(),
                            source_size: [map.width, map.height],
                            output_size: size,
                            achieved_pixels_per_metre: ((size[0] * size[1]) as f64 * ratio).sqrt(),
                        });
                        sizes.push(size);
                    } else {
                        sizes.push([0, 0]);
                    }
                }
            } else {
                sizes.resize(6, [0, 0]);
            }
            let key = (primitive.material, sizes.clone());
            let id = if let Some(&id) = materials.get(&key) {
                id
            } else {
                let mut derived = material.clone();
                let mut output = Vec::new();
                for (i, (_, map, colour, normal)) in maps(material).iter().enumerate() {
                    if let Some(map) = map {
                        pixels += sizes[i][0] * sizes[i][1];
                        ensure!(
                            pixels <= 67_108_864,
                            "normalized textures exceed 256 MiB RGBA output budget"
                        );
                        output.push(Some(resize(map, sizes[i], *colour, *normal)));
                    } else {
                        output.push(None);
                    }
                }
                let mut output = output.into_iter();
                derived.colour_map = output.next().unwrap();
                derived.normal_map = output.next().unwrap();
                derived.metallic_map = output.next().unwrap();
                derived.roughness_map = output.next().unwrap();
                derived.occlusion_map = output.next().unwrap();
                derived.emissive_map = output.next().unwrap();
                let id = scene.materials.len();
                scene.materials.push(derived);
                materials.insert(key, id);
                id
            };
            primitive.material = id;
            material_ids.push(id);
        }
        let key = (mesh_index, material_ids);
        let id = if let Some(&id) = meshes.get(&key) {
            id
        } else {
            let id = scene.meshes.len();
            mesh.name = format!("{}-density-{id}", mesh.name);
            scene.meshes.push(mesh);
            meshes.insert(key, id);
            id
        };
        scene.nodes[node_index].mesh = Some(id);
    }
    scene.validate()?;
    Ok((scene, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scaled_instances_get_matching_density_without_changing_source_or_uvs() {
        let asset = super::super::author::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples/game-assets/plate.gasset"),
            &[],
        )
        .unwrap();
        let mut source = asset.scene;
        source.materials[0].colour_map = Some(Map {
            width: 512,
            height: 512,
            rgba: vec![255; 512 * 512 * 4],
        });
        let mut node = source.nodes[0].clone();
        node.name = "large".into();
        node.transform.scale = [2.; 3];
        source.nodes.push(node);
        let (result, report) = normalize(
            &source,
            Settings {
                pixels_per_metre: 512.,
                max_dimension: 2048,
            },
        )
        .unwrap();
        assert_eq!(source.materials[0].colour_map.as_ref().unwrap().width, 512);
        assert_eq!(report.maps.len(), 2);
        let a = &report.maps[0];
        let b = &report.maps[1];
        assert!((b.output_size[0] as i64 - a.output_size[0] as i64 * 2).abs() <= 1);
        assert!((a.achieved_pixels_per_metre - b.achieved_pixels_per_metre).abs() < 25.);
        for node in &result.nodes {
            let geometry = &result.meshes[node.mesh.unwrap()].primitives[0].geometry;
            assert_eq!(
                geometry.positions,
                source.meshes[0].primitives[0].geometry.positions
            );
            assert_eq!(
                geometry.texcoords,
                source.meshes[0].primitives[0].geometry.texcoords
            );
        }
        let low = Map {
            width: 4,
            height: 4,
            rgba: vec![255; 64],
        };
        assert_eq!(dimensions(&low, 1., Settings::default()), [4, 4]);
        source.materials[0].alpha = AlphaMode::Mask { cutoff: 0.5 };
        assert!(normalize(&source, Settings::default()).is_err());
    }
    #[test]
    fn filtering_respects_colour_alpha_and_normal_vectors() {
        let map = Map {
            width: 2,
            height: 1,
            rgba: vec![0, 0, 0, 255, 255, 255, 255, 255],
        };
        let colour = resize(&map, [1, 1], true, false);
        assert!((colour.rgba[0] as i32 - 188).abs() <= 1);
        let numeric = resize(&map, [1, 1], false, false);
        assert!((numeric.rgba[0] as i32 - 128).abs() <= 1);
        let alpha = Map {
            width: 2,
            height: 1,
            rgba: vec![255, 0, 0, 0, 0, 0, 255, 255],
        };
        let alpha = resize(&alpha, [1, 1], true, false);
        assert_eq!(&alpha.rgba[..3], &[0, 0, 255]);
        let normals = Map {
            width: 2,
            height: 1,
            rgba: vec![218, 128, 218, 255, 128, 218, 218, 255],
        };
        let normal = resize(&normals, [1, 1], false, true);
        let length = normal.rgba[..3]
            .iter()
            .map(|v| (*v as f64 / 255. * 2. - 1.).powi(2))
            .sum::<f64>()
            .sqrt();
        assert!((length - 1.).abs() < 0.015);
    }
}
