//! Preflight for preview-owned RGBA payloads, before conversion or GPU upload.
use crate::asset::material::Material;
use anyhow::{Result, ensure};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ImageUsage {
    pub images: usize,
    pub retained_bytes: usize,
    pub upload_scratch_bytes: usize,
    pub gpu_bytes: usize,
}

#[derive(Clone, Copy)]
pub struct ImageLimits {
    pub images: usize,
    pub cpu_bytes: usize,
    pub gpu_bytes: usize,
}

impl Default for ImageLimits {
    fn default() -> Self {
        Self {
            images: 4096,
            cpu_bytes: 256 * 1024 * 1024,
            gpu_bytes: 256 * 1024 * 1024,
        }
    }
}

fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| anyhow::anyhow!("preview image byte count overflow"))
}

fn rgba_bytes(width: usize, height: usize) -> Result<usize> {
    ensure!(
        (1..=8192).contains(&width) && (1..=8192).contains(&height),
        "preview image dimensions must be 1..8192"
    );
    Ok(width * height * 4)
}

fn full_chain_bytes(mut width: usize, mut height: usize) -> Result<usize> {
    let mut bytes = 0;
    loop {
        bytes = add(bytes, rgba_bytes(width, height)?)?;
        if width == 1 && height == 1 {
            return Ok(bytes);
        }
        width = (width / 2).max(1);
        height = (height / 2).max(1);
    }
}

fn slots(material: &Material) -> [Option<&crate::model::texture::Map>; 5] {
    [
        material.colour_map.as_ref(),
        material.normal_map.as_ref(),
        material
            .roughness_map
            .as_ref()
            .or(material.metallic_map.as_ref()),
        material.occlusion_map.as_ref(),
        material.emissive_map.as_ref(),
    ]
}

impl ImageUsage {
    pub fn check(self, limits: ImageLimits) -> Result<Self> {
        ensure!(
            self.images <= limits.images,
            "preview image count exceeds {}",
            limits.images
        );
        let cpu = add(self.retained_bytes, self.upload_scratch_bytes)?;
        ensure!(
            cpu <= limits.cpu_bytes,
            "preview prepared image CPU payload {cpu} exceeds {} bytes",
            limits.cpu_bytes
        );
        ensure!(
            self.gpu_bytes <= limits.gpu_bytes,
            "preview image GPU payload {} exceeds {} bytes",
            self.gpu_bytes,
            limits.gpu_bytes
        );
        Ok(self)
    }
}

/// Counts per material, matching the adapter (instances share map slots).
/// Call after scene validation. No image pixels are read, cloned or allocated.
/// CPU includes explicit level-zero compatibility copies and the largest legacy
/// mip-generation scratch chain. GPU counts RGBA texels, not driver alignment,
/// staging buffers, render targets, geometry or retained source asset storage.
/// Legacy accounting conservatively includes the full chain through 1x1; the
/// pinned renderer caps automatic generation at twelve levels. Each frame is
/// counted independently; overlapping old/new frames and cached scenes are extra.
pub fn usage(materials: &[Material], limits: ImageLimits) -> Result<ImageUsage> {
    let mut usage = ImageUsage::default();
    for material in materials {
        for (slot, map) in slots(material).into_iter().enumerate() {
            let Some(map) = map else { continue };
            usage.images = add(usage.images, 1)?;
            let base = rgba_bytes(map.width, map.height)?;
            if let Some(chain) = &material.mip_chain {
                let mut bytes = 0;
                for level in chain.levels() {
                    let level = slots(level)[slot]
                        .ok_or_else(|| anyhow::anyhow!("missing preview mip slot"))?;
                    bytes = add(bytes, rgba_bytes(level.width, level.height)?)?;
                }
                usage.retained_bytes = add(usage.retained_bytes, add(base, bytes)?)?;
                usage.gpu_bytes = add(usage.gpu_bytes, bytes)?;
            } else {
                let bytes = full_chain_bytes(map.width, map.height)?;
                usage.retained_bytes = add(usage.retained_bytes, base)?;
                usage.gpu_bytes = add(usage.gpu_bytes, bytes)?;
                usage.upload_scratch_bytes = usage.upload_scratch_bytes.max(bytes);
            }
            usage.check(limits)?;
        }
    }
    usage.check(limits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::texture::Map;

    #[test]
    fn legacy_budget_counts_packed_slots_npot_mips_and_peak_scratch() {
        let map = Map {
            width: 3,
            height: 5,
            rgba: vec![255; 60],
        };
        let material = Material {
            colour_map: Some(map.clone()),
            normal_map: Some(map.clone()),
            metallic_map: Some(map.clone()),
            roughness_map: Some(map.clone()),
            occlusion_map: Some(map.clone()),
            emissive_map: Some(map),
            ..Default::default()
        };
        material.validate().unwrap();
        // 3x5 -> 1x2 -> 1x1: 60 + 8 + 4. Metallic/roughness share one slot.
        let expected = ImageUsage {
            images: 5,
            retained_bytes: 300,
            upload_scratch_bytes: 72,
            gpu_bytes: 360,
        };
        assert_eq!(
            usage(&[material.clone()], ImageLimits::default()).unwrap(),
            expected
        );
        let exact = ImageLimits {
            images: 5,
            cpu_bytes: 372,
            gpu_bytes: 360,
        };
        assert_eq!(usage(&[material.clone()], exact).unwrap(), expected);
        for limits in [
            ImageLimits { images: 4, ..exact },
            ImageLimits {
                cpu_bytes: 371,
                ..exact
            },
            ImageLimits {
                gpu_bytes: 359,
                ..exact
            },
        ] {
            assert!(usage(&[material.clone()], limits).is_err());
        }
        assert!(usage(&[material.clone(), material], exact).is_err());
    }

    #[test]
    fn authored_budget_matches_prepared_bytes_and_shared_instances() {
        let asset = crate::asset::author::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples/game-assets/atlas-mips.gasset"),
            &[],
        )
        .unwrap();
        let chain = asset.scene.materials[0].mip_chain.as_ref().unwrap();
        let maps = super::super::mip_images(chain).unwrap();
        let bytes: usize = maps.iter().flatten().map(|m| m.rgba.len()).sum();
        let base: usize = maps.iter().map(|slot| slot[0].rgba.len()).sum();
        let actual = usage(&asset.scene.materials, ImageLimits::default()).unwrap();
        assert_eq!(
            actual,
            ImageUsage {
                images: 5,
                retained_bytes: bytes + base,
                upload_scratch_bytes: 0,
                gpu_bytes: bytes
            }
        );
        assert_eq!(actual.retained_bytes, 757760);
        assert_eq!(actual.gpu_bytes, 430080);
        assert_eq!(
            asset
                .scene
                .nodes
                .iter()
                .filter(|n| n.mesh.is_some())
                .count(),
            2
        );
        let exact = ImageLimits {
            images: 5,
            cpu_bytes: actual.retained_bytes,
            gpu_bytes: bytes,
        };
        assert!(usage(&asset.scene.materials, exact).is_ok());
        assert!(
            usage(
                &asset.scene.materials,
                ImageLimits {
                    cpu_bytes: exact.cpu_bytes - 1,
                    ..exact
                }
            )
            .is_err()
        );
    }

    #[test]
    fn accounting_rejects_overflow_and_limits_without_large_allocations() {
        assert!(add(usize::MAX, 1).is_err());
        assert!(rgba_bytes(0, 1).is_err());
        assert!(rgba_bytes(8193, 1).is_err());
        let base = rgba_bytes(8192, 8192).unwrap();
        let gpu = full_chain_bytes(8192, 8192).unwrap();
        assert_eq!(gpu, 357913940);
        assert!(
            ImageUsage {
                images: 1,
                retained_bytes: base,
                upload_scratch_bytes: gpu,
                gpu_bytes: gpu
            }
            .check(ImageLimits::default())
            .is_err()
        );
        assert!(
            ImageUsage {
                retained_bytes: usize::MAX,
                upload_scratch_bytes: 1,
                ..Default::default()
            }
            .check(ImageLimits::default())
            .is_err()
        );
    }
}
