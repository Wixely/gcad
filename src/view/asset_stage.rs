//! Presentation transforms independent of asset geometry and animation.
use egui_bench::viewer3d::{Camera, Lighting};

#[derive(Clone, Copy, Debug)]
pub(super) struct Stage {
    pub model: f32,
    pub light: f32,
    pub elevation: f32,
    pub model_playing: bool,
    pub light_playing: bool,
    pub model_speed: f32,
    pub light_speed: f32,
}
impl Default for Stage {
    fn default() -> Self {
        Self {
            model: 0.,
            light: 0.,
            elevation: Lighting::STUDIO.key.elevation.to_degrees(),
            model_playing: false,
            light_playing: false,
            model_speed: 30.,
            light_speed: 30.,
        }
    }
}
impl Stage {
    pub fn advance(&mut self, dt: f32) {
        if !dt.is_finite() || dt < 0. {
            return;
        }
        if self.model_playing {
            self.model = (self.model + self.model_speed * dt).rem_euclid(360.);
        }
        if self.light_playing {
            self.light = (self.light + self.light_speed * dt).rem_euclid(360.);
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui, camera: &mut Camera) {
        self.advance(ui.input(|i| i.stable_dt));
        if self.model_playing || self.light_playing {
            ui.ctx().request_repaint();
        }
        ui.collapsing("Lighting and turntable", |ui| {
            for (label, angle, playing, speed) in [
                ("Light angle", &mut self.light, &mut self.light_playing, &mut self.light_speed),
                ("Model angle", &mut self.model, &mut self.model_playing, &mut self.model_speed),
            ] {
                ui.push_id(label, |ui| ui.horizontal_wrapped(|ui| {
                    if ui.add(egui::Slider::new(angle, 0.0..=360.).text(label).suffix("°")).changed() { *playing = false; }
                    if ui.button(if *playing { "Pause" } else { "Play" }).clicked() { *playing = !*playing; }
                    ui.add(egui::DragValue::new(speed).range(-180.0..=180.).suffix("°/s"));
                }));
            }
            ui.add(egui::Slider::new(&mut self.elevation, -90.0..=90.).text("Light elevation").suffix("°"));
            let mut yaw = camera.yaw.to_degrees().rem_euclid(360.);
            let mut pitch = camera.pitch.to_degrees();
            if ui.add(egui::Slider::new(&mut yaw, 0.0..=360.).text("View azimuth").suffix("°")).changed() { camera.yaw = yaw.to_radians(); }
            if ui.add(egui::Slider::new(&mut pitch, -89.9..=89.9).text("View elevation").suffix("°")).changed() { camera.pitch = pitch.to_radians(); }
            ui.checkbox(&mut camera.ortho, "Orthographic");
            if ui.button("Reset lighting and turntable").clicked() { *self = Self::default(); }
            ui.label("Both playback controls loop. Lighting stays fixed in the scene as the model turns.");
        });
    }
    // Inverse rotation of both camera and lights is equivalent to rotating the
    // model about its centre. Geometry, skin buffers and overlay coordinates stay
    // unchanged, so turntable playback does not rebuild or upload the scene.
    pub fn camera(&self, mut camera: Camera) -> Camera {
        camera.yaw -= self.model.to_radians();
        camera.shift = rotate(camera.shift, -self.model);
        camera
    }
    pub fn restore_camera(&self, mut camera: Camera) -> Camera {
        camera.yaw += self.model.to_radians();
        camera.shift = rotate(camera.shift, self.model);
        camera
    }
    pub fn lighting(&self) -> Lighting {
        let mut lighting = Lighting::STUDIO;
        lighting.follow = false;
        let angle = (self.light - self.model).to_radians();
        lighting.key.azimuth += angle;
        lighting.fill.azimuth += angle;
        lighting.key.elevation = self.elevation.to_radians();
        lighting
    }
}
fn rotate(p: [f32; 3], degrees: f32) -> [f32; 3] {
    let (s, c) = degrees.to_radians().sin_cos();
    [c * p[0] - s * p[1], s * p[0] + c * p[1], p[2]]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_loops_and_inverse_navigation() {
        let mut s = Stage {
            model: 359.,
            light: 2.,
            model_playing: true,
            light_playing: true,
            model_speed: 30.,
            light_speed: -30.,
            ..Default::default()
        };
        s.advance(0.1);
        assert!((s.model - 2.).abs() < 0.001 && (s.light - 359.).abs() < 0.001);
        s.model_playing = false;
        s.advance(1.);
        assert_eq!(s.model, 2.);
        let c = Camera {
            shift: [2., 3., 4.],
            ..Default::default()
        };
        let restored = s.restore_camera(s.camera(c));
        assert!((restored.yaw - c.yaw).abs() < 1e-6);
        for i in 0..3 {
            assert!((restored.shift[i] - c.shift[i]).abs() < 1e-5);
        }
        assert!(!s.lighting().follow);
    }
}
