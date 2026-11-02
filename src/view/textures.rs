use crate::model::materials::Finish;
use crate::model::texture::{Map, Texture};
use egui_bench::viewer3d::{Image, Surface};
use monstertruck::modeling::*;

#[derive(Clone, Copy)]
pub struct Binding {
    pub colour: Option<usize>,
    pub normal: Option<usize>,
    pub metal_rough: Option<usize>,
    pub scale: f32,
    pub rotation: f32,
}

pub struct Library {
    pub images: Vec<Image>,
    entries: Vec<(Texture, [u32; 3], Binding)>,
}

fn tinted(image: &Image, colour: [f32; 3]) -> Image {
    const GAIN: f32 = 1.3;
    let mut rgba = image.rgba.clone();
    for pixel in rgba.chunks_exact_mut(4) {
        for channel in 0..3 {
            let value = pixel[channel] as f32 / 255.0 * colour[channel] * GAIN;
            pixel[channel] = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    Image {
        width: image.width,
        height: image.height,
        rgba,
    }
}

impl Library {
    pub fn new() -> Self {
        Self {
            images: Vec::new(),
            entries: Vec::new(),
        }
    }

    fn add(&mut self, image: Image) -> usize {
        let index = self.images.len();
        self.images.push(image);
        index
    }

    pub fn binding(&mut self, finish: &Finish, tint: [f32; 3]) -> Option<Binding> {
        let texture = finish.texture.as_ref().filter(|t| t.shown())?;
        let bits = tint.map(f32::to_bits);
        if let Some((_, _, binding)) = self
            .entries
            .iter()
            .find(|(key, seen, _)| key == texture && *seen == bits)
        {
            return Some(*binding);
        }
        let procedural = texture
            .pattern
            .as_deref()
            .filter(|p| *p != "none")
            .map(pattern);
        let colour = texture
            .colour
            .as_ref()
            .map(image)
            .or_else(|| procedural.as_ref().map(|image| tinted(image, tint)))
            .map(|image| self.add(image));
        let normal = texture
            .normal
            .as_ref()
            .map(image)
            .or_else(|| {
                procedural
                    .as_ref()
                    .map(|image| image.normal_from_height(1.5))
            })
            .map(|image| self.add(image));
        let metal_rough = if texture.roughness.is_some() || texture.metallic.is_some() {
            let width = texture
                .roughness
                .as_ref()
                .or(texture.metallic.as_ref())
                .unwrap()
                .width;
            let height = texture
                .roughness
                .as_ref()
                .or(texture.metallic.as_ref())
                .unwrap()
                .height;
            let mut rgba = Vec::with_capacity(width * height * 4);
            for y in 0..height {
                for x in 0..width {
                    let sample = |map: &Option<Map>, default: u8| {
                        map.as_ref().map_or(default, |m| {
                            let u = x * m.width / width;
                            let v = y * m.height / height;
                            m.rgba[(v * m.width + u) * 4]
                        })
                    };
                    rgba.extend([
                        255,
                        sample(&texture.roughness, 255),
                        sample(&texture.metallic, 255),
                        255,
                    ]);
                }
            }
            Some(self.add(Image {
                width,
                height,
                rgba,
            }))
        } else {
            None
        };
        let binding = Binding {
            colour,
            normal,
            metal_rough,
            scale: texture.scale() as f32,
            rotation: texture.rotation().to_radians() as f32,
        };
        self.entries.push((texture.clone(), bits, binding));
        Some(binding)
    }
}

fn image(map: &Map) -> Image {
    Image {
        width: map.width,
        height: map.height,
        rgba: map.rgba.clone(),
    }
}

fn pattern(name: &str) -> Image {
    image(&crate::model::texture::pattern(name))
}

pub fn apply<M>(surfaces: &mut [Surface<M>], binding: Binding, placement: Matrix4) {
    let inverse = placement.invert().unwrap_or_else(Matrix4::identity);
    let mut extent = [0.0_f32; 3];
    let mut low = [f32::INFINITY; 3];
    let mut high = [f32::NEG_INFINITY; 3];
    for p in surfaces.iter().flat_map(|s| &s.positions) {
        let p = inverse.transform_point(Point3::new(p[0] as f64, p[1] as f64, p[2] as f64));
        for axis in 0..3 {
            low[axis] = low[axis].min(p[axis] as f32);
            high[axis] = high[axis].max(p[axis] as f32);
            extent[axis] = high[axis] - low[axis];
        }
    }
    let (sin, cos) = binding.rotation.sin_cos();
    for surface in surfaces {
        surface.texture = binding.colour;
        surface.normal_map = binding.normal;
        surface.metal_rough_map = binding.metal_rough;
        surface.uvs.clear();
        for triangle in surface.positions.chunks(3) {
            if triangle.len() != 3 {
                continue;
            }
            let local = |p: &[f32; 3]| {
                inverse.transform_point(Point3::new(p[0] as f64, p[1] as f64, p[2] as f64))
            };
            let [a, b, c] = [
                local(&triangle[0]),
                local(&triangle[1]),
                local(&triangle[2]),
            ];
            let n = (b - a).cross(c - a);
            let skip = (0..3)
                .max_by(|&a, &b| n[a].abs().total_cmp(&n[b].abs()))
                .unwrap();
            let mut axes: Vec<_> = (0..3).filter(|&axis| axis != skip).collect();
            axes.sort_by(|&a, &b| extent[b].total_cmp(&extent[a]));
            for p in [a, b, c] {
                let u = p[axes[0]] as f32 / binding.scale;
                let v = p[axes[1]] as f32 / binding.scale;
                surface.uvs.push([u * cos - v * sin, u * sin + v * cos]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad(offset: [f32; 3]) -> Surface<()> {
        let p = |x: f32, y: f32| [x + offset[0], y + offset[1], offset[2]];
        Surface {
            positions: vec![
                p(0.0, 0.0),
                p(400.0, 0.0),
                p(400.0, 50.0),
                p(0.0, 0.0),
                p(400.0, 50.0),
                p(0.0, 50.0),
            ],
            normals: vec![[0.0, 0.0, 1.0]; 6],
            ..Default::default()
        }
    }

    fn binding(rotation: f32) -> Binding {
        Binding {
            colour: Some(0),
            normal: Some(1),
            metal_rough: None,
            scale: 100.0,
            rotation,
        }
    }

    #[test]
    fn uvs_follow_tile_size_and_long_axis() {
        let mut surfaces = [quad([0.0; 3])];
        apply(&mut surfaces, binding(0.0), Matrix4::identity());
        let s = &surfaces[0];
        assert_eq!((s.texture, s.normal_map), (Some(0), Some(1)));
        assert_eq!(s.uvs.len(), 6);
        assert_eq!(s.uvs[1], [4.0, 0.0]);
        assert_eq!(s.uvs[2], [4.0, 0.5]);
        let mut turned = [quad([0.0; 3])];
        apply(
            &mut turned,
            binding(std::f32::consts::FRAC_PI_2),
            Matrix4::identity(),
        );
        assert!((turned[0].uvs[1][0]).abs() < 1.0e-5 && (turned[0].uvs[1][1] - 4.0).abs() < 1.0e-5);
    }

    #[test]
    fn copies_of_a_part_share_their_texture_coordinates() {
        let shift = [1234.0, -50.0, 7.0];
        let mut seed = [quad([0.0; 3])];
        let mut copy = [quad(shift)];
        apply(&mut seed, binding(0.0), Matrix4::identity());
        let placement = Matrix4::from_translation(Vector3::new(
            shift[0] as f64,
            shift[1] as f64,
            shift[2] as f64,
        ));
        apply(&mut copy, binding(0.0), placement);
        for (a, b) in seed[0].uvs.iter().zip(&copy[0].uvs) {
            assert!(
                (a[0] - b[0]).abs() < 1.0e-4 && (a[1] - b[1]).abs() < 1.0e-4,
                "{a:?} {b:?}"
            );
        }
    }

    #[test]
    fn procedural_patterns_are_deterministic_and_distinct() {
        let wood = pattern("wood");
        assert_eq!(wood.rgba, pattern("wood").rgba);
        assert_ne!(wood.rgba, pattern("concrete").rgba);
        assert_eq!(wood.rgba.len(), 256 * 256 * 4);
        for kind in crate::model::texture::PATTERNS
            .iter()
            .filter(|p| **p != "none")
        {
            let map = pattern(kind);
            assert!(map.rgba.chunks_exact(4).all(|pixel| pixel[3] == 255));
            assert!(
                map.rgba
                    .chunks_exact(4)
                    .any(|pixel| pixel[0] != map.rgba[0])
            );
        }
    }
}
