use crate::model::texture::Map;
use anyhow::{Result, ensure};
use image::ImageEncoder;

/// Linear factors, sRGB base-colour pixels and linear numeric map pixels.
/// Images and UVs use a top-left origin. Separate scalar maps use their red
/// channel; packing does not apply gamma or silently resample mismatched images.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    /// Explicit constrained effect, retained through derived material operations.
    pub shader: Option<super::shader::Description>,
    /// Immutable explicitly baked levels. Consumers must honor their LOD bound.
    pub mip_chain: Option<std::sync::Arc<super::atlas::MipChain>>,
    /// Original explicit image inputs, retained across derived baking operations.
    pub texture_sources: Vec<super::texture_input::Record>,
    pub base_colour: [f64; 4],
    pub metallic: f64,
    pub roughness: f64,
    pub colour_map: Option<Map>,
    pub roughness_map: Option<Map>,
    pub metallic_map: Option<Map>,
    /// Tangent-space normal map; +Y is the portable/glTF convention.
    pub normal_map: Option<Map>,
    pub normal_convention: NormalConvention,
    pub normal_scale: f64,
    pub occlusion_map: Option<Map>,
    pub occlusion_strength: f64,
    pub emissive: [f64; 3],
    pub emissive_map: Option<Map>,
    pub alpha: AlphaMode,
    pub double_sided: bool,
    pub sampler: Sampler,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum AlphaMode {
    #[default]
    Opaque,
    Mask {
        cutoff: f64,
    },
    Blend,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NormalConvention {
    #[default]
    PositiveY,
    NegativeY,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Wrap {
    #[default]
    Repeat,
    Clamp,
    Mirror,
}

impl Wrap {
    pub(super) fn gltf(self) -> u32 {
        match self {
            Self::Repeat => 10497,
            Self::Clamp => 33071,
            Self::Mirror => 33648,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filter {
    Nearest,
    #[default]
    Linear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sampler {
    pub wrap_u: Wrap,
    pub wrap_v: Wrap,
    pub magnify: Filter,
    pub minify: Filter,
    /// None disables mipmaps; otherwise selects nearest or linear mip blending.
    pub mipmap: Option<Filter>,
}

impl Default for Sampler {
    fn default() -> Self {
        Self {
            wrap_u: Wrap::Repeat,
            wrap_v: Wrap::Repeat,
            magnify: Filter::Linear,
            minify: Filter::Linear,
            mipmap: Some(Filter::Linear),
        }
    }
}

impl Sampler {
    pub(super) fn magnification(self) -> u32 {
        match self.magnify {
            Filter::Nearest => 9728,
            Filter::Linear => 9729,
        }
    }
    pub(super) fn minification(self) -> u32 {
        match (self.minify, self.mipmap) {
            (Filter::Nearest, None) => 9728,
            (Filter::Linear, None) => 9729,
            (Filter::Nearest, Some(Filter::Nearest)) => 9984,
            (Filter::Linear, Some(Filter::Nearest)) => 9985,
            (Filter::Nearest, Some(Filter::Linear)) => 9986,
            (Filter::Linear, Some(Filter::Linear)) => 9987,
        }
    }
}

impl Default for Material {
    fn default() -> Self {
        Self {
            shader: None,
            mip_chain: None,
            texture_sources: Vec::new(),
            base_colour: [1.0; 4],
            metallic: 0.0,
            roughness: 1.0,
            colour_map: None,
            roughness_map: None,
            metallic_map: None,
            normal_map: None,
            normal_convention: NormalConvention::PositiveY,
            normal_scale: 1.0,
            occlusion_map: None,
            occlusion_strength: 1.0,
            emissive: [0.0; 3],
            emissive_map: None,
            alpha: AlphaMode::Opaque,
            double_sided: false,
            sampler: Sampler::default(),
        }
    }
}

fn map_shape(map: &Map) -> Result<(u32, u32)> {
    let size = map
        .width
        .checked_mul(map.height)
        .and_then(|n| n.checked_mul(4));
    ensure!(
        map.width > 0 && map.height > 0 && size == Some(map.rgba.len()),
        "invalid RGBA image dimensions or data length"
    );
    Ok((u32::try_from(map.width)?, u32::try_from(map.height)?))
}

pub(super) fn png(map: &Map) -> Result<Vec<u8>> {
    let (width, height) = map_shape(map)?;
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes).write_image(
        &map.rgba,
        width,
        height,
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(bytes)
}

impl Material {
    /// Import core appearance from an existing CAD finish. Unsupported fields
    /// are reported together; no extension or procedural appearance is discarded.
    /// Colour inputs are sRGB; exported factors are linear. Missing roughness
    /// uses 1 and missing metallic uses 0, the asset material defaults.
    pub fn from_finish(
        finish: &crate::model::materials::Finish,
        fallback_colour: [f64; 3],
        opacity: f64,
    ) -> Result<Self> {
        let colour = finish.colour.unwrap_or(fallback_colour);
        ensure!(
            colour
                .into_iter()
                .chain([opacity])
                .all(|v| v.is_finite() && (0.0..=1.0).contains(&v)),
            "finish colour and opacity must be finite and between zero and one"
        );
        let mut unsupported = Vec::new();
        for (name, present) in [
            ("specular", finish.specular.is_some()),
            ("clearcoat", finish.clearcoat.is_some()),
            ("clearcoat_roughness", finish.clearcoat_roughness.is_some()),
            ("transmission", finish.transmission.is_some()),
            ("ior", finish.ior.is_some()),
        ] {
            if present {
                unsupported.push(name);
            }
        }
        if let Some(texture) = &finish.texture {
            texture.check()?;
            if texture.pattern.as_deref().is_some_and(|p| p != "none") {
                unsupported.push("procedural texture (bake first)");
            }
            // Projection controls live on the mesh; do not claim to have applied them.
            if texture.tile.is_some() {
                unsupported.push("texture_scale (apply UV projection explicitly)");
            }
            if texture.turn.is_some() {
                unsupported.push("texture_rotation (apply UV rotation explicitly)");
            }
        }
        ensure!(
            unsupported.is_empty(),
            "unsupported finish features: {}",
            unsupported.join(", ")
        );
        let linear = colour.map(|v| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        });
        let emission = finish.emission.unwrap_or(0.0);
        ensure!(
            emission.is_finite() && (0.0..=1.0).contains(&emission),
            "emission outside zero to one requires an emissive-strength extension"
        );
        let texture = finish.texture.as_ref();
        let material = Self {
            base_colour: [linear[0], linear[1], linear[2], opacity],
            roughness: finish.roughness.unwrap_or(1.0),
            metallic: finish.metallic.unwrap_or(0.0),
            emissive: linear.map(|v| v * emission),
            alpha: if opacity < 1.0 {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            },
            colour_map: texture.and_then(|t| t.colour.clone()),
            roughness_map: texture.and_then(|t| t.roughness.clone()),
            metallic_map: texture.and_then(|t| t.metallic.clone()),
            normal_map: texture.and_then(|t| t.normal.clone()),
            ..Self::default()
        };
        material.validate()?;
        Ok(material)
    }

    fn same_base(&self, other: &Self) -> bool {
        self.shader == other.shader
            && self.texture_sources == other.texture_sources
            && self.base_colour == other.base_colour
            && self.metallic == other.metallic
            && self.roughness == other.roughness
            && self.colour_map == other.colour_map
            && self.roughness_map == other.roughness_map
            && self.metallic_map == other.metallic_map
            && self.normal_map == other.normal_map
            && self.normal_convention == other.normal_convention
            && self.normal_scale == other.normal_scale
            && self.occlusion_map == other.occlusion_map
            && self.occlusion_strength == other.occlusion_strength
            && self.emissive == other.emissive
            && self.emissive_map == other.emissive_map
            && self.alpha == other.alpha
            && self.double_sided == other.double_sided
            && self.sampler == other.sampler
    }

    pub fn validate(&self) -> Result<()> {
        if let Some(shader) = &self.shader { shader.validate()?; }
        if let Some(chain) = &self.mip_chain {
            let base = &chain.levels()[0];
            ensure!(
                self.same_base(base),
                "explicit mip material changed; rebake its chain"
            );
        }
        for source in &self.texture_sources {
            source.validate()?;
        }
        ensure!(
            self.base_colour
                .into_iter()
                .chain([self.metallic, self.roughness, self.occlusion_strength])
                .chain(self.emissive)
                .all(|v| v.is_finite() && (0.0..=1.0).contains(&v)),
            "PBR factors must be finite and between zero and one"
        );
        ensure!(
            self.normal_scale.is_finite() && self.normal_scale >= 0.0,
            "normal scale must be finite and nonnegative"
        );
        if let AlphaMode::Mask { cutoff } = self.alpha {
            ensure!(
                cutoff.is_finite() && cutoff >= 0.0,
                "alpha cutoff must be finite and nonnegative"
            );
        }
        for map in [
            &self.colour_map,
            &self.roughness_map,
            &self.metallic_map,
            &self.normal_map,
            &self.occlusion_map,
            &self.emissive_map,
        ]
        .into_iter()
        .flatten()
        {
            map_shape(map)?;
        }
        if let (Some(r), Some(m)) = (&self.roughness_map, &self.metallic_map) {
            ensure!(
                (r.width, r.height) == (m.width, m.height),
                "roughness and metallic map dimensions differ; resample explicitly before export"
            );
        }
        Ok(())
    }

    pub fn has_textures(&self) -> bool {
        [
            &self.colour_map,
            &self.roughness_map,
            &self.metallic_map,
            &self.normal_map,
            &self.occlusion_map,
            &self.emissive_map,
        ]
        .iter()
        .any(|m| m.is_some())
    }

    /// Convert the normal-map convention on a copy, preserving the source image.
    pub fn portable_normal(&self) -> Result<Option<Map>> {
        self.validate()?;
        Ok(self.normal_map.as_ref().map(|source| {
            let mut map = source.clone();
            if self.normal_convention == NormalConvention::NegativeY {
                for pixel in map.rgba.chunks_exact_mut(4) {
                    pixel[1] = 255 - pixel[1];
                }
            }
            map
        }))
    }

    pub fn packed_metal_rough(&self) -> Result<Option<Map>> {
        self.validate()?;
        let Some(shape) = self.roughness_map.as_ref().or(self.metallic_map.as_ref()) else {
            return Ok(None);
        };
        let mut rgba = Vec::with_capacity(shape.rgba.len());
        for i in 0..shape.width * shape.height {
            let r = self
                .roughness_map
                .as_ref()
                .map_or(255, |map| map.rgba[i * 4]);
            let m = self
                .metallic_map
                .as_ref()
                .map_or(255, |map| map.rgba[i * 4]);
            rgba.extend_from_slice(&[255, r, m, 255]);
        }
        Ok(Some(Map {
            width: shape.width,
            height: shape.height,
            rgba,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_convention_and_channel_packing_preserve_input() {
        let source = Map {
            width: 1,
            height: 1,
            rgba: vec![120, 40, 240, 255],
        };
        let material = Material {
            normal_map: Some(source.clone()),
            normal_convention: NormalConvention::NegativeY,
            roughness_map: Some(source.clone()),
            metallic_map: Some(source.clone()),
            ..Material::default()
        };
        assert_eq!(
            material.portable_normal().unwrap().unwrap().rgba,
            [120, 215, 240, 255]
        );
        assert_eq!(
            material.packed_metal_rough().unwrap().unwrap().rgba,
            [255, 120, 120, 255]
        );
        assert_eq!(material.normal_map.unwrap(), source);
    }

    #[test]
    fn rejects_invalid_extended_material_values_and_map_sizes() {
        for modify in [
            |m: &mut Material| m.normal_scale = f64::NAN,
            |m: &mut Material| m.occlusion_strength = -0.1,
            |m: &mut Material| m.emissive[0] = 1.1,
            |m: &mut Material| {
                m.alpha = AlphaMode::Mask {
                    cutoff: f64::INFINITY,
                }
            },
            |m: &mut Material| {
                m.normal_map = Some(Map {
                    width: 1,
                    height: 1,
                    rgba: vec![],
                })
            },
        ] {
            let mut material = Material::default();
            modify(&mut material);
            assert!(material.validate().is_err());
        }
        let material = Material {
            roughness_map: Some(Map {
                width: 1,
                height: 1,
                rgba: vec![0; 4],
            }),
            metallic_map: Some(Map {
                width: 2,
                height: 1,
                rgba: vec![0; 8],
            }),
            ..Material::default()
        };
        assert!(material.packed_metal_rough().is_err());
    }

    #[test]
    fn converts_finish_colours_and_reports_unsupported_fields() {
        let mut finish = crate::model::materials::Finish {
            colour: Some([0.5, 0.0, 1.0]),
            emission: Some(0.5),
            ..Default::default()
        };
        let material = Material::from_finish(&finish, [1.0; 3], 0.4).unwrap();
        assert!((material.base_colour[0] - 0.21404114048223255).abs() < 1e-12);
        assert_eq!(material.alpha, AlphaMode::Blend);
        assert_eq!(material.emissive[2], 0.5);
        finish.transmission = Some(0.5);
        finish.ior = Some(1.5);
        let error = Material::from_finish(&finish, [1.0; 3], 1.0)
            .unwrap_err()
            .to_string();
        assert!(error.contains("transmission") && error.contains("ior"));
    }
}
