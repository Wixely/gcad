//! Bounded source-texel inspection: never upload a second full-resolution map.
use crate::model::texture::Map;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Region {
    origin: [usize; 2],
    size: [usize; 2],
}

impl Region {
    fn new(map: &Map, origin: [usize; 2], size: [usize; 2]) -> Self {
        let size = [size[0].clamp(1, map.width), size[1].clamp(1, map.height)];
        Self {
            origin: [
                origin[0].min(map.width - size[0]),
                origin[1].min(map.height - size[1]),
            ],
            size,
        }
    }

    fn image(self, map: &Map) -> egui::ColorImage {
        let mut rgba = Vec::with_capacity(self.size[0] * self.size[1] * 4);
        for y in self.origin[1]..self.origin[1] + self.size[1] {
            let start = (y * map.width + self.origin[0]) * 4;
            rgba.extend_from_slice(&map.rgba[start..start + self.size[0] * 4]);
        }
        egui::ColorImage::from_rgba_unmultiplied(self.size, &rgba)
    }
}

pub(super) struct Detail {
    origin: [usize; 2],
    zoom: f32,
    drag_origin: [usize; 2],
    texture: Option<(Region, egui::TextureHandle)>,
}

impl Default for Detail {
    fn default() -> Self {
        Self {
            origin: [0, 0],
            zoom: 1.,
            drag_origin: [0, 0],
            texture: None,
        }
    }
}

impl Detail {
    pub fn invalidate(&mut self) {
        self.texture = None;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, map: &Map) {
        ui.label(
            "Source pixels: drag to pan. X/Y start at the top-left. Zoom uses UI points per texel.",
        );
        ui.add(egui::Slider::new(&mut self.zoom, 1.0..=16.0).text("Texel zoom"));
        let viewport = egui::vec2(ui.available_width().clamp(1., 512.), 256.);
        let size = [
            (viewport.x / self.zoom).ceil() as usize,
            (viewport.y / self.zoom).ceil() as usize,
        ];
        let region = Region::new(map, self.origin, size);
        self.origin = region.origin;
        ui.add(
            egui::Slider::new(&mut self.origin[0], 0..=map.width - region.size[0]).text("Source X"),
        );
        ui.add(
            egui::Slider::new(&mut self.origin[1], 0..=map.height - region.size[1])
                .text("Source Y"),
        );
        let region = Region::new(map, self.origin, size);
        if self.texture.as_ref().is_none_or(|(old, _)| *old != region) {
            let image = region.image(map);
            if let Some((old, texture)) = &mut self.texture {
                texture.set(image, egui::TextureOptions::NEAREST);
                *old = region;
            } else {
                self.texture = Some((
                    region,
                    ui.ctx().load_texture(
                        "asset-source-region",
                        image,
                        egui::TextureOptions::NEAREST,
                    ),
                ));
            }
        }
        let (rect, response) = ui.allocate_exact_size(viewport, egui::Sense::drag());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::accesskit::Role::Image,
                ui.is_enabled(),
                "Source texture pixels",
            )
        });
        let painter = ui.painter_at(rect);
        // Checkerboard makes transparency distinguishable from black texels.
        for y in 0..(viewport.y / 16.).ceil() as usize {
            for x in 0..(viewport.x / 16.).ceil() as usize {
                let tile = egui::Rect::from_min_size(
                    rect.min + egui::vec2(x as f32 * 16., y as f32 * 16.),
                    egui::vec2(16., 16.),
                );
                painter.rect_filled(
                    tile,
                    0.,
                    egui::Color32::from_gray(if (x + y) % 2 == 0 { 48 } else { 80 }),
                );
            }
        }
        let image_rect = egui::Rect::from_min_size(
            rect.min,
            egui::vec2(region.size[0] as f32, region.size[1] as f32) * self.zoom,
        );
        painter.image(
            self.texture.as_ref().unwrap().1.id(),
            image_rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
            egui::Color32::WHITE,
        );
        if let Some(at) = response.hover_pos().filter(|at| image_rect.contains(*at)) {
            let local = (at - rect.min) / self.zoom;
            let x = region.origin[0] + (local.x as usize).min(region.size[0] - 1);
            let y = region.origin[1] + (local.y as usize).min(region.size[1] - 1);
            let offset = (y * map.width + x) * 4;
            ui.label(format!(
                "Pixel ({x}, {y}) · RGBA {:?}",
                &map.rgba[offset..offset + 4]
            ));
        } else {
            ui.label("Hover to read exact source RGBA bytes (before material factors).");
        }
        if response.drag_started() {
            self.drag_origin = self.origin;
        }
        if let Some(delta) = response.total_drag_delta() {
            let delta = delta / self.zoom;
            for (axis, delta) in [delta.x, delta.y].into_iter().enumerate() {
                self.origin[axis] =
                    (self.drag_origin[axis] as f32 - delta).round().max(0.) as usize;
            }
            self.origin = Region::new(map, self.origin, size).origin;
            ui.ctx().request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_pixel_navigation_accumulates_small_drags_at_both_dpi_scales() {
        use egui_kittest::kittest::Queryable as _;
        for dpi in [1., 2.] {
            let map = Map {
                width: 1024,
                height: 512,
                rgba: [37, 89, 123, 0].repeat(1024 * 512),
            };
            let detail = Detail {
                origin: [500, 200],
                zoom: 16.,
                ..Default::default()
            };
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(600., 460.))
                .with_pixels_per_point(dpi)
                .build_ui_state(
                    |ui, detail| {
                        egui::ScrollArea::vertical().show(ui, |ui| detail.ui(ui, &map));
                    },
                    detail,
                );
            harness.run();
            let rect = harness.get_by_label("Source texture pixels").rect();
            let start = egui::pos2(rect.min.x as f32 + 64., rect.min.y as f32 + 64.);
            harness
                .input_mut()
                .events
                .push(egui::Event::PointerMoved(start));
            harness.step();
            assert!(
                harness
                    .query_by_label_contains("RGBA [37, 89, 123, 0]")
                    .is_some(),
                "transparent source bytes must remain readable at DPI {dpi}"
            );
            harness.input_mut().events.push(egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
            harness.step();
            // Every individual move is below one source texel at this zoom.
            for offset in 1..=32 {
                harness.input_mut().events.push(egui::Event::PointerMoved(
                    start + egui::vec2(offset as f32, 0.),
                ));
                harness.step();
            }
            harness.input_mut().events.push(egui::Event::PointerButton {
                pos: start + egui::vec2(32., 0.),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            });
            harness.run();
            assert_eq!(harness.state().origin, [498, 200], "DPI {dpi}");
            assert_eq!(
                harness.state().texture.as_ref().unwrap().0.origin,
                [498, 200]
            );
        }
    }

    #[cfg(feature = "gpu-tests")]
    #[test]
    fn source_pixel_ui_renders_and_reloads_the_visible_region() {
        let mut map = Map {
            width: 2048,
            height: 512,
            rgba: Vec::with_capacity(2048 * 512 * 4),
        };
        for y in 0..map.height {
            for x in 0..map.width {
                map.rgba.extend_from_slice(&[
                    (x % 256) as u8,
                    (y % 256) as u8,
                    180,
                    if (x / 8 + y / 8) % 2 == 0 { 255 } else { 64 },
                ]);
            }
        }
        let detail = Detail {
            origin: [1537, 256],
            zoom: 4.,
            ..Default::default()
        };
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(600., 460.))
            .build_ui_state(
                |ui, (detail, map): &mut (Detail, Map)| detail.ui(ui, map),
                (detail, map),
            );
        harness.run();
        let region = harness.state().0.texture.as_ref().unwrap().0;
        assert_eq!(region.origin, [1537, 256]);
        assert_eq!(region.size, [128, 64]);
        let before = harness.render().expect("offscreen source-pixel rendering");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
        before
            .save(root.join("game-asset-source-pixels.png"))
            .unwrap();
        harness
            .input_mut()
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(2.);
        harness.run();
        let high_dpi = harness.render().unwrap();
        assert_eq!(high_dpi.dimensions(), (1200, 920));
        assert_eq!(harness.state().0.texture.as_ref().unwrap().0, region);
        high_dpi
            .save(root.join("game-asset-source-pixels-2x.png"))
            .unwrap();
        harness
            .input_mut()
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(1.);
        harness.run();
        let (detail, map) = harness.state_mut();
        map.rgba.fill(255);
        detail.invalidate();
        harness.run();
        let after = harness.render().unwrap();
        let changed = before
            .pixels()
            .zip(after.pixels())
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            changed > 100_000,
            "reloaded region did not reach rendered UI: {changed}"
        );
    }

    #[test]
    fn source_region_preserves_texels_beyond_thumbnail_resolution_and_clamps_edges() {
        let mut map = Map {
            width: 2048,
            height: 2,
            rgba: vec![0; 2048 * 2 * 4],
        };
        let offset = (2048 + 1537) * 4;
        map.rgba[offset..offset + 4].copy_from_slice(&[91, 203, 17, 128]);
        let region = Region::new(&map, [1537, 1], [1, 1]);
        let image = region.image(&map);
        assert_eq!(image.size, [1, 1]);
        assert_eq!(
            image.pixels[0],
            egui::Color32::from_rgba_unmultiplied(91, 203, 17, 128)
        );
        let edge = Region::new(&map, [usize::MAX; 2], [512, 256]);
        assert_eq!(edge.origin, [1536, 0]);
        assert_eq!(edge.size, [512, 2]);
        assert_eq!(edge.image(&map).pixels.len(), 1024);
    }
}
