use anyhow::{Context, Result, bail};
use std::path::Path;

pub const PATTERNS: &[&str] = &[
    "none",
    "wood",
    "osb",
    "concrete",
    "stone",
    "brick",
    "sand",
    "fabric",
    "foam",
    "rubber",
    "brushed",
    "carbon-fibre",
    "polished",
    "cast",
    "plaster",
    "granulate",
    "marble",
    "asphalt",
];

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Map {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

impl Map {
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = crate::host::read(path)
            .with_context(|| format!("reading texture {}", path.display()))?;
        let image = image::load_from_memory(&bytes)
            .with_context(|| format!("decoding texture {}", path.display()))?
            .into_rgba8();
        Ok(Self {
            width: image.width() as usize,
            height: image.height() as usize,
            rgba: image.into_raw(),
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Texture {
    pub pattern: Option<String>,
    pub tile: Option<f64>,
    pub turn: Option<f64>,
    pub colour: Option<Map>,
    pub normal: Option<Map>,
    pub roughness: Option<Map>,
    pub metallic: Option<Map>,
}

impl Texture {
    pub fn shown(&self) -> bool {
        self.pattern.as_deref().is_some_and(|p| p != "none")
            || self.colour.is_some()
            || self.normal.is_some()
            || self.roughness.is_some()
            || self.metallic.is_some()
    }

    pub fn scale(&self) -> f64 {
        self.tile.unwrap_or(100.0)
    }

    pub fn rotation(&self) -> f64 {
        self.turn.unwrap_or(0.0)
    }

    pub fn over(self, base: Texture) -> Texture {
        Texture {
            pattern: self.pattern.or(base.pattern),
            tile: self.tile.or(base.tile),
            turn: self.turn.or(base.turn),
            colour: self.colour.or(base.colour),
            normal: self.normal.or(base.normal),
            roughness: self.roughness.or(base.roughness),
            metallic: self.metallic.or(base.metallic),
        }
    }

    pub fn check(&self) -> Result<()> {
        if let Some(pattern) = &self.pattern
            && !PATTERNS.contains(&pattern.as_str())
        {
            bail!(
                "unknown texture `{pattern}`; textures are {}",
                PATTERNS.join(", ")
            );
        }
        if !self.scale().is_finite() || self.scale() <= 0.0 {
            bail!("texture_scale must be a finite positive size in mm");
        }
        if !self.rotation().is_finite() {
            bail!("texture_rotation must be finite");
        }
        Ok(())
    }
}

// Shared CPU pattern generator; fixed 256-pixel tiles preserve viewer output.
fn noise(x: i32, y: i32, cells: i32) -> f32 {
    let x = x.rem_euclid(cells) as u32;
    let y = y.rem_euclid(cells) as u32;
    let mut n = x
        .wrapping_mul(374761393)
        .wrapping_add(y.wrapping_mul(668265263));
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    ((n ^ (n >> 16)) & 65535) as f32 / 65535.0
}

fn smooth_noise(u: f32, v: f32, cells: i32) -> f32 {
    let x = u * cells as f32;
    let y = v * cells as f32;
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let blend = |t: f32| t * t * (3.0 - 2.0 * t);
    let (a, b) = (blend(x.fract()), blend(y.fract()));
    let low = noise(ix, iy, cells) * (1.0 - a) + noise(ix + 1, iy, cells) * a;
    let high = noise(ix, iy + 1, cells) * (1.0 - a) + noise(ix + 1, iy + 1, cells) * a;
    low * (1.0 - b) + high * b
}

pub(crate) fn pattern(name: &str) -> Map {
    let size = 256;
    let mut rgba = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let (u, v) = (x as f32 / size as f32, y as f32 / size as f32);
            let wave = |t: f32| (t * std::f32::consts::TAU).sin() * 0.5 + 0.5;
            let fine = noise(x as i32, y as i32, size as i32);
            let coarse = smooth_noise(u, v, 16);
            let value = match name {
                "wood" => 0.68 + 0.25 * wave(v * 32.0 + 2.0 * wave(u)) + 0.06 * fine,
                "osb" => {
                    let row = (v * 24.0).floor();
                    0.55 + 0.37 * smooth_noise(u + row / 48.0, v, 24) + 0.07 * fine
                }
                "brick" => {
                    let row = (v * 4.0).floor();
                    let s = (u * 2.0 + row * 0.5).fract();
                    let t = (v * 4.0).fract();
                    if s < 0.035 || t < 0.08 {
                        0.52
                    } else {
                        0.8 + 0.18 * fine
                    }
                }
                "brushed" => 0.85 + 0.14 * noise(0, y as i32, size as i32),
                "fabric" => 0.65 + 0.18 * wave(u * 64.0) * wave(v * 64.0) + 0.16 * fine,
                "carbon-fibre" => {
                    let cross = ((u * 16.0).floor() as i32 + (v * 16.0).floor() as i32) % 2;
                    0.5 + 0.4
                        * if cross == 0 {
                            wave(u * 64.0)
                        } else {
                            wave(v * 64.0)
                        }
                }
                "foam" => 0.73 + 0.2 * coarse + 0.06 * fine,
                "polished" => 0.93 + 0.05 * smooth_noise(u, v, 4) + 0.015 * fine,
                "cast" => 0.55 + 0.28 * smooth_noise(u, v, 24) + 0.12 * coarse + 0.06 * fine,
                "plaster" => 0.8 + 0.1 * coarse + 0.06 * smooth_noise(u, v, 32) + 0.03 * fine,
                "granulate" => 0.78 + 0.12 * fine + 0.07 * coarse,
                "asphalt" => {
                    0.4 + 0.42 * fine * fine + 0.1 * coarse + 0.08 * smooth_noise(u, v, 64)
                }
                "marble" => {
                    let vein = ((u * 3.0 + v * 2.0 + 1.6 * smooth_noise(u, v, 6))
                        * std::f32::consts::TAU)
                        .sin()
                        .abs();
                    0.9 - 0.4 * vein.powf(6.0) + 0.04 * fine
                }
                "sand" => 0.65 + 0.34 * fine,
                "rubber" => 0.89 + 0.1 * fine,
                "stone" => 0.5 + 0.28 * smooth_noise(u, v, 8) + 0.15 * coarse + 0.06 * fine,
                _ => 0.7 + 0.2 * coarse + 0.09 * fine,
            };
            let c = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
            rgba.extend([c, c, c, 255]);
        }
    }
    Map {
        width: size,
        height: size,
        rgba,
    }
}
