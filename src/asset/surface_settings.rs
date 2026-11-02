//! Typed, explicit overrides for derived core PBR materials.
use super::material::{AlphaMode, Filter, Material, NormalConvention, Wrap};
use anyhow::{Result, bail, ensure};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct SurfaceSettings(Vec<Setting>);

#[derive(Clone, Debug)]
enum Setting {
    Base([f64; 4]),
    Metallic(f64),
    Roughness(f64),
    Emissive([f64; 3]),
    Alpha(AlphaMode),
    DoubleSided(bool),
    NormalScale(f64),
    NormalY(NormalConvention),
    OcclusionStrength(f64),
    WrapU(Wrap),
    WrapV(Wrap),
    Magnify(Filter),
    Minify(Filter),
    Mipmap(Option<Filter>),
}

fn vector<const N: usize>(value: &str) -> Result<[f64; N]> {
    let values = value
        .split(',')
        .map(str::parse::<f64>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    values
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected {N} comma-separated factors"))
}
fn filter(value: &str) -> Result<Filter> {
    match value {
        "nearest" => Ok(Filter::Nearest),
        "linear" => Ok(Filter::Linear),
        _ => bail!("filter must be nearest or linear"),
    }
}
fn wrap(value: &str) -> Result<Wrap> {
    match value {
        "repeat" => Ok(Wrap::Repeat),
        "clamp" => Ok(Wrap::Clamp),
        "mirror" => Ok(Wrap::Mirror),
        _ => bail!("wrap must be repeat, clamp or mirror"),
    }
}

impl SurfaceSettings {
    pub fn parse(named: &BTreeMap<String, String>) -> Result<Self> {
        ensure!(!named.is_empty(), "surface requires at least one setting");
        let mask = named.get("alpha").is_some_and(|v| v == "mask");
        ensure!(
            mask == named.contains_key("cutoff"),
            "cutoff is required exactly when alpha=mask"
        );
        let mut settings = Vec::new();
        for (key, value) in named {
            settings.push(match key.as_str() {
                "base_colour" => Setting::Base(vector(value)?),
                "metallic" => Setting::Metallic(value.parse()?),
                "roughness" => Setting::Roughness(value.parse()?),
                "emissive" => Setting::Emissive(vector(value)?),
                "alpha" => Setting::Alpha(match value.as_str() {
                    "opaque" => AlphaMode::Opaque,
                    "blend" => AlphaMode::Blend,
                    "mask" => AlphaMode::Mask {
                        cutoff: named["cutoff"].parse()?,
                    },
                    _ => bail!("alpha must be opaque, mask or blend"),
                }),
                "cutoff" => continue,
                "double_sided" => Setting::DoubleSided(value.parse()?),
                "normal_scale" => Setting::NormalScale(value.parse()?),
                "normal_y" => Setting::NormalY(match value.as_str() {
                    "positive" => NormalConvention::PositiveY,
                    "negative" => NormalConvention::NegativeY,
                    _ => bail!("normal_y must be positive or negative"),
                }),
                "occlusion_strength" => Setting::OcclusionStrength(value.parse()?),
                "wrap_u" => Setting::WrapU(wrap(value)?),
                "wrap_v" => Setting::WrapV(wrap(value)?),
                "magnify" => Setting::Magnify(filter(value)?),
                "minify" => Setting::Minify(filter(value)?),
                "mipmap" => Setting::Mipmap(if value == "none" {
                    None
                } else {
                    Some(filter(value)?)
                }),
                _ => bail!("unknown surface setting `{key}`"),
            });
        }
        let settings = Self(settings);
        settings.apply(&Material::default())?;
        Ok(settings)
    }

    /// Copy first, so invalid source materials or overrides cannot partially mutate input.
    pub fn apply(&self, source: &Material) -> Result<Material> {
        source.validate()?;
        let mut result = source.clone();
        for setting in &self.0 {
            match *setting {
                Setting::Base(v) => result.base_colour = v,
                Setting::Metallic(v) => result.metallic = v,
                Setting::Roughness(v) => result.roughness = v,
                Setting::Emissive(v) => result.emissive = v,
                Setting::Alpha(v) => result.alpha = v,
                Setting::DoubleSided(v) => result.double_sided = v,
                Setting::NormalScale(v) => result.normal_scale = v,
                Setting::NormalY(v) => result.normal_convention = v,
                Setting::OcclusionStrength(v) => result.occlusion_strength = v,
                Setting::WrapU(v) => result.sampler.wrap_u = v,
                Setting::WrapV(v) => result.sampler.wrap_v = v,
                Setting::Magnify(v) => result.sampler.magnify = v,
                Setting::Minify(v) => result.sampler.minify = v,
                Setting::Mipmap(v) => result.sampler.mipmap = v,
            }
        }
        result.validate()?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settings(pairs: &[(&str, &str)]) -> Result<SurfaceSettings> {
        SurfaceSettings::parse(
            &pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }
    #[test]
    fn overrides_preserve_maps_and_unspecified_values_without_mutation() {
        let mut source = Material::default();
        source.roughness = 0.7;
        source.normal_map = Some(crate::model::texture::Map {
            width: 1,
            height: 1,
            rgba: vec![128, 64, 255, 255],
        });
        let before = source.clone();
        let result = settings(&[
            ("base_colour", "0.1,0.2,0.3,0.4"),
            ("alpha", "mask"),
            ("cutoff", "0.25"),
            ("double_sided", "true"),
            ("normal_y", "negative"),
            ("wrap_u", "mirror"),
            ("mipmap", "none"),
        ])
        .unwrap()
        .apply(&source)
        .unwrap();
        assert_eq!(source, before);
        assert_eq!(result.base_colour, [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(result.roughness, 0.7);
        assert_eq!(result.normal_map, source.normal_map);
        assert_eq!(
            result.portable_normal().unwrap().unwrap().rgba,
            [128, 191, 255, 255]
        );
        assert_eq!(result.alpha, AlphaMode::Mask { cutoff: 0.25 });
        assert!(result.double_sided);
        assert_eq!(result.sampler.wrap_u, Wrap::Mirror);
        assert_eq!(result.sampler.wrap_v, Wrap::Repeat);
        assert_eq!(result.sampler.mipmap, None);
    }
    #[test]
    fn texture_controls_reach_the_glb_document() {
        let map = crate::model::texture::Map {
            width: 1,
            height: 1,
            rgba: vec![128, 64, 255, 255],
        };
        let source = Material {
            normal_map: Some(map.clone()),
            occlusion_map: Some(map),
            ..Default::default()
        };
        let material = settings(&[
            ("normal_scale", "0.7"),
            ("normal_y", "negative"),
            ("occlusion_strength", "0.4"),
            ("wrap_u", "clamp"),
            ("wrap_v", "mirror"),
            ("magnify", "nearest"),
            ("minify", "linear"),
            ("mipmap", "nearest"),
            ("metallic", "0.25"),
            ("roughness", "0.75"),
            ("emissive", "0.1,0.2,0.3"),
        ])
        .unwrap()
        .apply(&source)
        .unwrap();
        let bytes = crate::asset::glb::encode(&crate::asset::tests::triangle(), &material).unwrap();
        let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let document: serde_json::Value = serde_json::from_slice(&bytes[20..20 + length]).unwrap();
        assert_eq!(document["materials"][0]["normalTexture"]["scale"], 0.7);
        assert_eq!(
            document["materials"][0]["occlusionTexture"]["strength"],
            0.4
        );
        assert_eq!(
            document["materials"][0]["pbrMetallicRoughness"]["metallicFactor"],
            0.25
        );
        assert_eq!(
            document["materials"][0]["emissiveFactor"],
            serde_json::json!([0.1, 0.2, 0.3])
        );
        assert_eq!(
            document["samplers"][0],
            serde_json::json!({"wrapS":33071,"wrapT":33648,"magFilter":9728,"minFilter":9985})
        );
    }

    #[test]
    fn invalid_surface_settings_fail_early() {
        for pairs in [
            vec![],
            vec![("alpha", "mask")],
            vec![("cutoff", "0.5")],
            vec![("alpha", "blend"), ("cutoff", "0.5")],
            vec![("base_colour", "1,1,1")],
            vec![("metallic", "NaN")],
            vec![("roughness", "2")],
            vec![("normal_scale", "-1")],
            vec![("occlusion_strength", "2")],
            vec![("emissive", "1,2,0")],
            vec![("normal_y", "up")],
            vec![("double_sided", "yes")],
            vec![("wrap_u", "border")],
            vec![("mipmap", "cubic")],
            vec![("unknown", "1")],
        ] {
            assert!(settings(&pairs).is_err(), "accepted {pairs:?}");
        }
    }
}
