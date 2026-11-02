//! Explicit image inputs and their declared provenance; no licence inference.
use super::material::Material;
use crate::model::texture::Map;
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    BaseColour,
    Roughness,
    Metallic,
    Normal,
    Occlusion,
    Emissive,
}
impl Slot {
    pub fn parse(value: &str) -> Result<Self> {
        Ok(match value {
            "base_colour" => Self::BaseColour,
            "roughness" => Self::Roughness,
            "metallic" => Self::Metallic,
            "normal" => Self::Normal,
            "occlusion" => Self::Occlusion,
            "emissive" => Self::Emissive,
            _ => bail!("unknown texture slot `{value}`"),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Input {
    pub slot: Slot,
    pub path: String,
    pub license: String,
    pub origin: String,
}

/// Original authored inputs, not a claim that exported/atlas pixels are unchanged.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Record {
    pub slot: Slot,
    pub project_path: String,
    pub sha256: String,
    pub license: String,
    pub origin: String,
}
impl Record {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.project_path.is_empty()
                && !self.project_path.contains(['\\', ':'])
                && self
                    .project_path
                    .split('/')
                    .all(|p| !p.is_empty() && p != "." && p != ".."),
            "texture provenance must use a portable project-relative path"
        );
        ensure!(
            self.sha256.len() == 64 && self.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid texture source hash"
        );
        ensure!(
            !self.license.trim().is_empty() && !self.origin.trim().is_empty(),
            "texture license and origin declarations are required"
        );
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Loaded {
    pub record: Record,
    pub map: Map,
}
impl Loaded {
    pub fn apply(&self, material: &mut Material) {
        let slot = match self.record.slot {
            Slot::BaseColour => &mut material.colour_map,
            Slot::Roughness => &mut material.roughness_map,
            Slot::Metallic => &mut material.metallic_map,
            Slot::Normal => &mut material.normal_map,
            Slot::Occlusion => &mut material.occlusion_map,
            Slot::Emissive => &mut material.emissive_map,
        };
        *slot = Some(self.map.clone());
        material.texture_sources.push(self.record.clone());
    }
}

pub fn load(
    input: &Input,
    directory: &std::path::Path,
    project: &std::path::Path,
    rgba_budget: usize,
) -> Result<Loaded> {
    use sha2::{Digest, Sha256};
    use std::path::Path;
    ensure!(
        !Path::new(&input.path).is_absolute(),
        "texture paths must be relative to the asset document"
    );
    let path = directory.join(&input.path);
    let root = crate::host::canonicalize(project).context("resolving texture project root")?;
    let resolved = crate::host::canonicalize(&path)
        .with_context(|| format!("resolving texture {}", path.display()))?;
    let relative = resolved
        .strip_prefix(&root)
        .context("texture lies outside the project root")?;
    let bytes =
        crate::host::read(&path).with_context(|| format!("reading texture {}", path.display()))?;
    let record = Record {
        slot: input.slot,
        project_path: relative
            .to_str()
            .context("texture path is not UTF-8")?
            .replace('\\', "/"),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        license: input.license.clone(),
        origin: input.origin.clone(),
    };
    record.validate()?;
    // Inspect dimensions before decoding the same immutable bytes.
    let reader = image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format()?;
    let (width, height) = reader
        .into_dimensions()
        .context("reading texture dimensions")?;
    ensure!(
        width > 0
            && height > 0
            && width <= 8192
            && height <= 8192
            && u64::from(width) * u64::from(height) * 4 <= rgba_budget as u64,
        "texture exceeds 8192 pixels per axis or 256 MiB RGBA budget"
    );
    let decoded = image::load_from_memory(&bytes)
        .with_context(|| format!("decoding texture {}", path.display()))?
        .into_rgba8();
    Ok(Loaded {
        record,
        map: Map {
            width: decoded.width() as usize,
            height: decoded.height() as usize,
            rgba: decoded.into_raw(),
        },
    })
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    #[test]
    fn image_limits_and_project_boundaries_are_enforced() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let directory = repo.join("examples/game-assets");
        let mut input = Input {
            slot: Slot::BaseColour,
            path: "maps/rgba.png".into(),
            license: "GPL-3.0-or-later".into(),
            origin: "gcad-test-pattern".into(),
        };
        let loaded = load(&input, &directory, repo, 16).unwrap();
        assert_eq!(loaded.map.rgba.len(), 16);
        assert_eq!(
            loaded.record.project_path,
            "examples/game-assets/maps/rgba.png"
        );
        assert!(load(&input, &directory, repo, 15).is_err());
        assert!(load(&input, &directory, &repo.join("src"), 16).is_err());
        input.path = directory
            .join("maps/rgba.png")
            .to_string_lossy()
            .into_owned();
        assert!(load(&input, &directory, repo, 16).is_err());
        input.path = "maps/rgba.png".into();
        input.license.clear();
        assert!(load(&input, &directory, repo, 16).is_err());
    }
}
