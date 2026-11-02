//! UV layouts and bounded, raw-channel texture thumbnails for asset inspection.
use crate::asset::{material::Material, scene::SceneMesh};
#[path = "asset_texture_detail.rs"]
mod texture_detail;

#[derive(Default)]
pub(super) struct Visuals {
    primitive: usize,
    channel: usize,
    mip: usize,
    texture: Option<((usize, usize, usize), egui::TextureHandle)>,
    source_pixels: bool,
    detail: texture_detail::Detail,
}

impl Visuals {
    pub fn invalidate(&mut self) {
        self.texture = None;
        self.detail.invalidate();
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, mesh: &SceneMesh, materials: &[Material]) {
        self.primitive = self.primitive.min(mesh.primitives.len().saturating_sub(1));
        if mesh.primitives.is_empty() {
            return;
        }
        ui.separator();
        ui.strong("UVs and texture maps");
        egui::ComboBox::from_id_salt("visual-primitive")
            .selected_text(format!("Primitive {}", self.primitive))
            .show_ui(ui, |ui| {
                for i in 0..mesh.primitives.len() {
                    ui.selectable_value(&mut self.primitive, i, format!("Primitive {i}"));
                }
            });
        // Resolve after the selector so geometry and texture cache use the same primitive.
        let primitive = &mesh.primitives[self.primitive];
        ui.collapsing("UV layout", |ui| {
            let Some(uvs) = &primitive.geometry.texcoords else {
                ui.label("No UV coordinates.");
                return;
            };
            ui.label("U right · V up · orange outline is the 0–1 tile. All tiles fit the view.");
            let side = ui.available_width().clamp(1., 300.);
            let size = egui::vec2(side, side);
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0., egui::Color32::from_gray(18));
            let fit = UvFit::new(uvs);
            let project = |uv| {
                let p = fit.project(uv);
                egui::pos2(
                    rect.left() + p[0] as f32 * rect.width(),
                    rect.top() + p[1] as f32 * rect.height(),
                )
            };
            for triangle in &primitive.geometry.triangles {
                for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                    painter.line_segment(
                        [
                            project(uvs[triangle[a] as usize]),
                            project(uvs[triangle[b] as usize]),
                        ],
                        egui::Stroke::new(0.7, egui::Color32::LIGHT_BLUE),
                    );
                }
            }
            let corners = [[0., 0.], [1., 0.], [1., 1.], [0., 1.]];
            for i in 0..4 {
                painter.line_segment(
                    [project(corners[i]), project(corners[(i + 1) % 4])],
                    egui::Stroke::new(1.5, egui::Color32::ORANGE),
                );
            }
        });
        ui.collapsing("Texture image", |ui| {
            let material = &materials[primitive.material];
            let levels = material.mip_chain.as_ref().map(|chain| chain.levels());
            let count = levels.map_or(1, |levels| levels.len());
            self.mip = self.mip.min(count - 1);
            ui.add(egui::Slider::new(&mut self.mip, 0..=count - 1).text("Mip level"));
            let material = levels.map_or(material, |levels| &levels[self.mip]);
            let maps = [
                ("Base colour", material.colour_map.as_ref()), ("Normal", material.normal_map.as_ref()),
                ("Metallic", material.metallic_map.as_ref()), ("Roughness", material.roughness_map.as_ref()),
                ("Occlusion", material.occlusion_map.as_ref()), ("Emission", material.emissive_map.as_ref()),
            ];
            egui::ComboBox::from_id_salt("texture-channel").selected_text(maps[self.channel].0).show_ui(ui, |ui| {
                for (i, (name, _)) in maps.iter().enumerate() { ui.selectable_value(&mut self.channel, i, *name); }
            });
            let Some(map) = maps[self.channel].1 else { ui.label("No image in this channel; the material uses its factor."); return; };
            let key = (self.primitive, self.channel, self.mip);
            if self.texture.as_ref().is_none_or(|(old, _)| *old != key) {
                self.detail.invalidate();
                self.texture = Some((key, ui.ctx().load_texture("asset-map-inspection", thumbnail(map), egui::TextureOptions::NEAREST)));
            }
            let texture = &self.texture.as_ref().unwrap().1;
            ui.label(format!("Source: {} × {} · thumbnail: {} × {}", map.width, map.height, texture.size()[0], texture.size()[1]));
            ui.label("Raw RGB bytes, alpha shown as transparency. Numeric/normal maps are diagnostic colours, not shaded output.");
            ui.checkbox(&mut self.source_pixels, "Inspect source pixels");
            if self.source_pixels {
                self.detail.ui(ui, map);
                return;
            }
            ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(ui.available_width(), ui.available_width() * texture.size()[1] as f32 / texture.size()[0] as f32)));
        });
    }
}

/// Normalise before subtracting so extreme but finite tiled UVs cannot overflow.
struct UvFit {
    magnitude: f64,
    low: [f64; 2],
    span: f64,
}
impl UvFit {
    fn new(uvs: &[[f64; 2]]) -> Self {
        let magnitude = uvs.iter().flatten().fold(1f64, |m, v| m.max(v.abs()));
        let mut low = [0f64; 2];
        let mut high = [1. / magnitude; 2];
        for uv in uvs {
            for k in 0..2 {
                low[k] = low[k].min(uv[k] / magnitude);
                high[k] = high[k].max(uv[k] / magnitude);
            }
        }
        let span = (high[0] - low[0]).max(high[1] - low[1]);
        Self {
            magnitude,
            low,
            span,
        }
    }
    fn project(&self, uv: [f64; 2]) -> [f64; 2] {
        let u = (uv[0] / self.magnitude - self.low[0]) / self.span;
        let v = (uv[1] / self.magnitude - self.low[1]) / self.span;
        [0.05 + u * 0.9, 0.95 - v * 0.9]
    }
}

fn thumbnail(map: &crate::model::texture::Map) -> egui::ColorImage {
    let scale = (map.width.max(map.height) as f64 / 512.).max(1.);
    let width = ((map.width as f64 / scale).round() as usize).max(1);
    let height = ((map.height as f64 / scale).round() as usize).max(1);
    let mut bytes = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let offset = ((y * map.height / height) * map.width + x * map.width / width) * 4;
            bytes.extend_from_slice(&map.rgba[offset..offset + 4]);
        }
    }
    egui::ColorImage::from_rgba_unmultiplied([width, height], &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uv_fit_handles_tiles_degenerate_coordinates_and_extreme_ranges() {
        for uvs in [
            vec![[0., 0.]; 3],
            vec![[-4., 7.], [8., -2.]],
            vec![[-f64::MAX, f64::MAX], [f64::MAX, -f64::MAX]],
        ] {
            let fit = UvFit::new(&uvs);
            for uv in uvs {
                assert!(
                    fit.project(uv)
                        .iter()
                        .all(|v| v.is_finite() && (0.04..=0.96).contains(v))
                );
            }
        }
        let fit = UvFit::new(&[[0., 0.], [1., 1.]]);
        assert!(fit.project([0., 0.])[1] > fit.project([0., 1.])[1]);
    }
    #[test]
    fn thumbnail_preserves_small_rgba_images_and_bounds_large_allocations() {
        let map = crate::model::texture::Map {
            width: 2,
            height: 1,
            rgba: vec![255, 0, 0, 255, 0, 255, 0, 128],
        };
        let image = thumbnail(&map);
        assert_eq!(image.size, [2, 1]);
        assert_eq!(image.pixels[0], egui::Color32::RED);
        assert_eq!(
            image.pixels[1],
            egui::Color32::from_rgba_unmultiplied(0, 255, 0, 128)
        );
        let map = crate::model::texture::Map {
            width: 2048,
            height: 1,
            rgba: vec![255; 2048 * 4],
        };
        assert_eq!(thumbnail(&map).size, [512, 1]);
    }
}
