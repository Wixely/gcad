//! GPU sprite sheets with explicit per-frame presentation and pose metadata.
use super::{stage::Stage, *};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shadows {
    #[default]
    Off,
    Hard,
    Soft,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LightSettings {
    pub key_intensity: f32,
    pub fill_intensity: f32,
    pub ambient: f32,
    pub exposure: f32,
    pub key_colour: [u8; 3],
    pub fill_colour: [u8; 3],
    pub shadows: Shadows,
    pub shadow_samples: u32,
    pub softness: f32,
}
impl Default for LightSettings {
    fn default() -> Self {
        let l = Lighting::STUDIO;
        Self {
            key_intensity: l.key.intensity,
            fill_intensity: l.fill.intensity,
            ambient: l.ambient,
            exposure: 1.,
            key_colour: [255; 3],
            fill_colour: [255; 3],
            shadows: Shadows::Off,
            shadow_samples: 32,
            softness: 0.08,
        }
    }
}
impl LightSettings {
    fn lighting(&self, stage: Stage) -> Lighting {
        let mut l = stage.lighting();
        l.key.intensity = self.key_intensity;
        l.fill.intensity = self.fill_intensity;
        l.ambient = self.ambient;
        l.exposure = self.exposure;
        l.key.colour =
            egui::Color32::from_rgb(self.key_colour[0], self.key_colour[1], self.key_colour[2]);
        l.fill.colour = egui::Color32::from_rgb(
            self.fill_colour[0],
            self.fill_colour[1],
            self.fill_colour[2],
        );
        l.key.spread = if self.shadows == Shadows::Soft {
            self.softness
        } else {
            0.
        };
        l.fill.spread = l.key.spread;
        l
    }
    fn trace(&self) -> Option<viewer3d::Trace> {
        (self.shadows != Shadows::Off).then_some(viewer3d::Trace {
            samples: self.shadow_samples,
            bounces: 1,
            per_frame: self.shadow_samples,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Sprite {
    pub view_azimuth: f32,
    pub view_elevation: f32,
    pub model_angle: f32,
    pub light_angle: f32,
    pub light_elevation: f32,
    pub time: f64,
}
impl Default for Sprite {
    fn default() -> Self {
        Self {
            view_azimuth: Camera::default().yaw.to_degrees(),
            view_elevation: Camera::default().pitch.to_degrees(),
            model_angle: 0.,
            light_angle: 0.,
            light_elevation: Stage::default().elevation,
            time: 0.,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Recipe {
    pub lighting: LightSettings,
    pub normal_sheet: bool,
    pub normal_y_negative: bool,
    pub width: u32,
    pub height: u32,
    pub columns: u32,
    pub padding: u32,
    pub clip: Option<String>,
    pub orthographic: bool,
    pub zoom: f32,
    pub frames: Vec<Sprite>,
}
impl Default for Recipe {
    fn default() -> Self {
        Self {
            lighting: LightSettings::default(),
            normal_sheet: true,
            normal_y_negative: false,
            width: 256,
            height: 256,
            columns: 4,
            padding: 2,
            clip: None,
            orthographic: true,
            zoom: 1.,
            frames: vec![Sprite::default()],
        }
    }
}
impl Recipe {
    fn dimensions(&self) -> Result<(u32, u32)> {
        let l = &self.lighting;
        ensure!(
            [l.key_intensity, l.fill_intensity, l.ambient, l.exposure]
                .iter()
                .all(|v| v.is_finite() && (0.0..=8.0).contains(v)),
            "lighting values must be finite and 0..8"
        );
        ensure!(
            (1..=128).contains(&l.shadow_samples)
                && l.softness.is_finite()
                && (0.0..=0.5).contains(&l.softness),
            "shadow samples must be 1..128 and softness 0..0.5"
        );
        ensure!(
            (1..=256).contains(&self.frames.len()),
            "sprite count must be 1..256"
        );
        ensure!(
            (16..=2048).contains(&self.width) && (16..=2048).contains(&self.height),
            "cell dimensions must be 16..2048"
        );
        ensure!(
            (1..=256).contains(&self.columns) && self.padding <= 32,
            "columns must be 1..256 and padding 0..32"
        );
        ensure!(
            self.zoom.is_finite() && (0.1..=4.).contains(&self.zoom),
            "zoom must be 0.1..4"
        );
        let cols = self.columns.min(self.frames.len() as u32);
        let rows = (self.frames.len() as u32).div_ceil(cols);
        let w = cols * (self.width + 2 * self.padding);
        let h = rows * (self.height + 2 * self.padding);
        ensure!(
            w <= 8192 && h <= 8192 && w as u64 * h as u64 <= 16_777_216,
            "sheet exceeds 8192-axis or 16-megapixel budget"
        );
        for f in &self.frames {
            ensure!(
                [
                    f.view_azimuth,
                    f.view_elevation,
                    f.model_angle,
                    f.light_angle,
                    f.light_elevation
                ]
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 360_000.),
                "angles must be finite and bounded"
            );
            ensure!(
                f.view_elevation.abs() <= 90. && f.light_elevation.abs() <= 90.,
                "elevations must be -90..90 degrees"
            );
            ensure!(
                f.time.is_finite() && f.time >= 0.,
                "frame time must be finite and nonnegative"
            );
        }
        Ok((w, h))
    }
}

/// Install the same renderer used by the viewer for command-line readback.
pub fn install_headless() -> Result<()> {
    use eframe::egui_wgpu;
    // The viewer caches device resources in thread-local storage. Keep the
    // owning instance/adapter alive beyond worker TLS destruction (in particular
    // Windows driver DLL lifetime), and install on the actual render thread.
    static STATE: std::sync::OnceLock<egui_wgpu::RenderState> = std::sync::OnceLock::new();
    if STATE.get().is_none() {
        let mut config = egui_wgpu::WgpuConfiguration::default();
        if let egui_wgpu::WgpuSetup::CreateNew(setup) = &mut config.wgpu_setup {
            // Tracing packs material texels into storage buffers. Request the
            // adapter's supported binding limits, as the interactive viewer does,
            // rather than WebGPU's smaller default (128 MiB per binding).
            let descriptor = setup.device_descriptor.clone();
            setup.device_descriptor = Arc::new(move |adapter| {
                let mut d = descriptor(adapter);
                let limits = adapter.limits();
                d.required_limits.max_buffer_size = limits.max_buffer_size;
                d.required_limits.max_storage_buffer_binding_size =
                    limits.max_storage_buffer_binding_size;
                d
            });
            setup.instance_descriptor.backends = if cfg!(target_os = "windows") {
                wgpu::Backends::DX12
            } else {
                wgpu::Backends::VULKAN
            };
        }
        let instance = pollster::block_on(config.wgpu_setup.new_instance());
        let state = pollster::block_on(egui_wgpu::RenderState::create(
            &config,
            &instance,
            None,
            egui_wgpu::RendererOptions::PREDICTABLE,
        ))?;
        let _ = STATE.set(state);
    }
    viewer3d::install(STATE.get().context("headless renderer not initialized")?);
    Ok(())
}

// The pinned renderer clears alpha to one. Dual-background readback recovers
// coverage without colour-keying, including black objects and antialiased edges.
fn matte(black: [u8; 4], white: [u8; 4]) -> [u8; 4] {
    let transmission = (0..3)
        .map(|i| (white[i] as f32 - black[i] as f32).clamp(0., 255.))
        .sum::<f32>()
        / 3.;
    let alpha = (1. - transmission / 255.).clamp(0., 1.);
    if alpha < 0.5 / 255. {
        return [0; 4];
    }
    [
        (black[0] as f32 / alpha).round().clamp(0., 255.) as u8,
        (black[1] as f32 / alpha).round().clamp(0., 255.) as u8,
        (black[2] as f32 / alpha).round().clamp(0., 255.) as u8,
        (alpha * 255.).round() as u8,
    ]
}

// Traced samples are accumulated in linear light before display encoding;
// raster blending happens in the renderer's display-byte target instead.
fn traced_matte(black: [u8; 4], white: [u8; 4]) -> [u8; 4] {
    let linear = |b: u8| {
        let v = b as f32 / 255.;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let alpha = (1.
        - (0..3)
            .map(|i| (linear(white[i]) - linear(black[i])).clamp(0., 1.))
            .sum::<f32>()
            / 3.)
        .clamp(0., 1.);
    if alpha < 0.5 / 255. {
        return [0; 4];
    }
    let encode = |b: u8| {
        let v = (linear(b) / alpha).clamp(0., 1.);
        let s = if v <= 0.0031308 {
            v * 12.92
        } else {
            1.055 * v.powf(1. / 2.4) - 0.055
        };
        (s * 255.).round() as u8
    };
    [
        encode(black[0]),
        encode(black[1]),
        encode(black[2]),
        (alpha * 255.).round() as u8,
    ]
}

fn render_pass(
    f: &Frame,
    scene: Arc<Scene<Finish>>,
    camera: Camera,
    lighting: Lighting,
    mode: RenderMode,
    trace: Option<viewer3d::Trace>,
    background: egui::Color32,
    recipe: &Recipe,
) -> Result<egui::ColorImage> {
    let viewer = Viewer::new(scene, camera)
        .looks(f.looks.clone())
        .edges(false)
        .style(viewer3d::Style {
            background,
            ..Default::default()
        })
        .lighting(lighting)
        .mode(mode)
        .trace(trace);
    if let Some(error) = viewer.explicit_image_error() {
        anyhow::bail!("Sprite render: {error}. Turn shadows off for this material.");
    }
    viewer
        .render_gpu(recipe.width as usize, recipe.height as usize)
        .context("GPU sprite readback unavailable")
}

fn albedo_scene(f: &Frame) -> Result<Arc<Scene<Finish>>> {
    let mut scene = Scene {
        surfaces: f.scene.surfaces.clone(),
        images: f.scene.images.clone(),
        explicit_images: f
            .scene
            .explicit_images
            .iter()
            .map(|(&i, m)| {
                viewer3d::ExplicitImage::new(m.levels().to_vec(), m.encoding())
                    .map(|m| (i, m))
                    .map_err(anyhow::Error::msg)
            })
            .collect::<Result<_>>()?,
        centre: f.scene.centre,
        radius: f.scene.radius,
        ..Default::default()
    };
    for s in &mut scene.surfaces {
        // Constant white ambient with no direct/specular/emissive or AO term.
        for i in [3, 4, 5, 8] {
            s.material.0[i] = 0f32.to_bits();
        }
    }
    Ok(Arc::new(scene))
}

fn sprite_normal(
    pixel: [u8; 4],
    alpha: u8,
    right: [f32; 3],
    up: [f32; 3],
    forward: [f32; 3],
    negative_y: bool,
) -> [u8; 4] {
    if alpha == 0 {
        return [128, 128, 255, 0];
    }
    let a = alpha as f32 / 255.;
    let n: [f32; 3] = std::array::from_fn(|i| 2. * (pixel[i] as f32 / 255. / a).clamp(0., 1.) - 1.);
    let dot = |v: [f32; 3]| (0..3).map(|i| n[i] * v[i]).sum::<f32>();
    let v = [
        dot(right),
        dot(up) * if negative_y { -1. } else { 1. },
        -dot(forward),
    ];
    let length = v.iter().map(|v| v * v).sum::<f32>().sqrt();
    if length < 1e-5 {
        return [128, 128, 255, alpha];
    }
    let encode = |x: f32| ((x / length * 0.5 + 0.5) * 255.).round().clamp(0., 255.) as u8;
    [encode(v[0]), encode(v[1]), encode(v[2]), alpha]
}

// Extend RGB two texels into transparent pixels, keeping coverage unchanged and
// never crossing a sprite-cell boundary. This protects straight-alpha filtering.
fn bleed_cell(map: &mut image::RgbaImage, x: u32, y: u32, width: u32, height: u32) {
    let mut seen = vec![false; (width * height) as usize];
    let mut queue = std::collections::VecDeque::new();
    for cy in 0..height {
        for cx in 0..width {
            if map.get_pixel(x + cx, y + cy)[3] > 0 {
                seen[(cy * width + cx) as usize] = true;
                queue.push_back((cx, cy, 0u8));
            }
        }
    }
    while let Some((cx, cy, d)) = queue.pop_front() {
        if d == 2 {
            continue;
        }
        let rgb = map.get_pixel(x + cx, y + cy).0;
        for (nx, ny) in [
            (cx as i32 - 1, cy as i32),
            (cx as i32 + 1, cy as i32),
            (cx as i32, cy as i32 - 1),
            (cx as i32, cy as i32 + 1),
        ] {
            if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                continue;
            }
            let (nx, ny) = (nx as u32, ny as u32);
            let index = (ny * width + nx) as usize;
            if seen[index] {
                continue;
            }
            seen[index] = true;
            map.put_pixel(x + nx, y + ny, image::Rgba([rgb[0], rgb[1], rgb[2], 0]));
            queue.push_back((nx, ny, d + 1));
        }
    }
}

pub fn export(asset: &Asset, recipe: &Recipe, output: &Path, cancelled: &AtomicBool) -> Result<()> {
    let (width, height) = recipe.dimensions()?;
    ensure!(
        !output.as_os_str().is_empty() && !output.exists(),
        "choose a new output directory; existing output is preserved"
    );
    let clip = recipe
        .clip
        .as_ref()
        .map(|name| {
            asset
                .scene
                .clips
                .iter()
                .position(|c| &c.name == name)
                .context("unknown animation clip")
        })
        .transpose()?;
    if let Some(c) = clip {
        ensure!(
            recipe
                .frames
                .iter()
                .all(|f| f.time <= asset.scene.clips[c].duration()),
            "frame time exceeds clip duration"
        );
    }
    let playback = Playback::new(asset)?;
    let consumer = consumer::ConsumerView::default();
    let mut low = [f32::INFINITY; 3];
    let mut high = [f32::NEG_INFINITY; 3];
    let mut bounds = Vec::new();
    let mut sampled = std::collections::HashSet::new();
    // Fixed framing across all sampled poses: no per-frame zoom pumping.
    for spec in &recipe.frames {
        ensure!(
            !cancelled.load(Ordering::Relaxed),
            "sprite export cancelled"
        );
        if !sampled.insert(spec.time.to_bits()) {
            continue;
        }
        let f = consumer.apply(asset, playback.frame(clip, spec.time)?);
        for k in 0..3 {
            low[k] = low[k].min(f.scene.centre[k] - f.scene.radius);
            high[k] = high[k].max(f.scene.centre[k] + f.scene.radius);
        }
        bounds.push((f.scene.centre, f.scene.radius));
    }
    let centre = std::array::from_fn(|k| (low[k] + high[k]) * 0.5);
    let radius = bounds
        .iter()
        .map(|(c, r)| {
            (0..3)
                .map(|k| (c[k] - centre[k]).powi(2))
                .sum::<f32>()
                .sqrt()
                + r
        })
        .fold(0f32, f32::max);
    ensure!(
        radius.is_finite() && radius > 0.,
        "no renderable sprite bounds"
    );
    let mut sheet = image::RgbaImage::new(width, height);
    let mut normals = recipe
        .normal_sheet
        .then(|| image::RgbaImage::from_pixel(width, height, image::Rgba([128, 128, 255, 0])));
    let mut albedo = recipe
        .normal_sheet
        .then(|| image::RgbaImage::new(width, height));
    let cols = recipe.columns.min(recipe.frames.len() as u32);
    let mut metadata = Vec::new();
    let mut cached: Option<(u64, Frame, Arc<Scene<Finish>>)> = None;
    for (i, spec) in recipe.frames.iter().enumerate() {
        ensure!(
            !cancelled.load(Ordering::Relaxed),
            "sprite export cancelled"
        );
        if cached
            .as_ref()
            .is_none_or(|(time, _, _)| *time != spec.time.to_bits())
        {
            // Offline readback has no UI frame boundary to age out renderer
            // uploads. Release the old pose's GPU cache before uploading another
            // one; angle/light-only samples keep sharing their existing buffers.
            if cached.is_some() {
                install_headless()?;
            }
            let mut f = consumer.apply(asset, playback.frame(clip, spec.time)?);
            let scene = Arc::get_mut(&mut f.scene).context("sprite frame unexpectedly shared")?;
            scene.centre = centre;
            scene.radius = radius;
            let unlit = albedo_scene(&f)?;
            cached = Some((spec.time.to_bits(), f, unlit));
        }
        let f = &cached.as_ref().unwrap().1;
        let stage = Stage {
            model: spec.model_angle,
            light: spec.light_angle,
            elevation: spec.light_elevation,
            ..Default::default()
        };
        let camera = stage.camera(Camera {
            yaw: spec.view_azimuth.to_radians(),
            pitch: spec.view_elevation.to_radians(),
            zoom: recipe.zoom,
            ortho: recipe.orthographic,
            ..Default::default()
        });
        let lighting = recipe.lighting.lighting(stage);
        let render = |background| {
            render_pass(
                f,
                f.scene.clone(),
                camera,
                lighting,
                RenderMode::Shaded,
                recipe.lighting.trace(),
                background,
                recipe,
            )
        };
        let black = render(egui::Color32::BLACK)?;
        ensure!(
            !cancelled.load(Ordering::Relaxed),
            "sprite export cancelled"
        );
        let white = render(egui::Color32::WHITE)?;
        let unlit = cached.as_ref().unwrap().2.clone();
        let flat_lighting = Lighting {
            key: viewer3d::Light {
                intensity: 0.,
                ..Lighting::STUDIO.key
            },
            fill: viewer3d::Light {
                intensity: 0.,
                ..Lighting::STUDIO.fill
            },
            ambient: 1.,
            exposure: 1.,
            ..Lighting::STUDIO
        };
        // Raster coverage is shared by colour, albedo and normals even when the
        // beauty pass uses stochastic traced shadows.
        let raw_black = render_pass(
            f,
            unlit.clone(),
            camera,
            flat_lighting,
            RenderMode::Shaded,
            None,
            egui::Color32::BLACK,
            recipe,
        )?;
        let raw_white = render_pass(
            f,
            unlit,
            camera,
            flat_lighting,
            RenderMode::Shaded,
            None,
            egui::Color32::WHITE,
            recipe,
        )?;
        let normal_pixels = if recipe.normal_sheet {
            Some(render_pass(
                f,
                f.scene.clone(),
                camera,
                lighting,
                RenderMode::Normals,
                None,
                egui::Color32::BLACK,
                recipe,
            )?)
        } else {
            None
        };
        let view = viewer3d::view(
            &f.scene,
            &camera,
            egui::vec2(recipe.width as f32, recipe.height as f32),
        );
        let x = (i as u32 % cols) * (recipe.width + 2 * recipe.padding) + recipe.padding;
        let y = (i as u32 / cols) * (recipe.height + 2 * recipe.padding) + recipe.padding;
        for (p, (b, w)) in black.pixels.iter().zip(&white.pixels).enumerate() {
            let px = x + p as u32 % recipe.width;
            let py = y + p as u32 / recipe.width;
            let base = matte(
                raw_black.pixels[p].to_array(),
                raw_white.pixels[p].to_array(),
            );
            let mut beauty = if recipe.lighting.shadows == Shadows::Off {
                matte(b.to_array(), w.to_array())
            } else {
                traced_matte(b.to_array(), w.to_array())
            };
            beauty[3] = base[3];
            sheet.put_pixel(px, py, image::Rgba(beauty));
            if let Some(map) = &mut albedo {
                map.put_pixel(px, py, image::Rgba(base));
            }
            if let (Some(map), Some(pixels)) = (&mut normals, &normal_pixels) {
                map.put_pixel(
                    px,
                    py,
                    image::Rgba(sprite_normal(
                        pixels.pixels[p].to_array(),
                        base[3],
                        view.right,
                        view.up,
                        view.forward,
                        recipe.normal_y_negative,
                    )),
                );
            }
        }
        for map in std::iter::once(&mut sheet)
            .chain(albedo.iter_mut())
            .chain(normals.iter_mut())
        {
            bleed_cell(
                map,
                x - recipe.padding,
                y - recipe.padding,
                recipe.width + 2 * recipe.padding,
                recipe.height + 2 * recipe.padding,
            );
        }
        metadata.push(
            serde_json::json!({"index":i,"rect":[x,y,recipe.width,recipe.height],"sample":spec}),
        );
    }
    ensure!(
        !cancelled.load(Ordering::Relaxed),
        "sprite export cancelled"
    );
    // Reserve only after rendering succeeds. Roll back our files on write failure.
    std::fs::create_dir(output)?;
    let result = (|| -> Result<()> {
        sheet.save(output.join("sprites.png"))?;
        if let Some(map) = &normals {
            map.save(output.join("normals.png"))?;
        }
        if let Some(map) = &albedo {
            map.save(output.join("albedo.png"))?;
        }
        std::fs::write(
            output.join("sprites.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "version":2,"image":"sprites.png","size":[width,height],"alpha":"straight",
                "albedo_image":recipe.normal_sheet.then_some("albedo.png"),
                "normal_image":recipe.normal_sheet.then_some("normals.png"),
                "normal_encoding":{"space":"sprite-view","rgb":"linear-unorm","x":"screen-right","y":if recipe.normal_y_negative {"screen-down"}else{"screen-up"},"z":"toward-viewer","decode":"normalize(rgb * 2 - 1)","includes_material_normal_maps":true},
                "origin":"top-left","bounds_centre_mm":centre,"bounds_radius_mm":radius,
                "recipe":recipe,"frames":metadata
            }))?,
        )?;
        Ok(())
    })();
    if result.is_err() {
        for name in ["sprites.png", "sprites.json", "normals.png", "albedo.png"] {
            let _ = std::fs::remove_file(output.join(name));
        }
        let _ = std::fs::remove_dir(output);
    }
    result
}

pub(super) struct Panel {
    output: String,
    models: String,
    lights: String,
    times: String,
    recipe: Recipe,
    status: String,
    pending: Option<mpsc::Receiver<Result<()>>>,
    cancelled: Arc<AtomicBool>,
}
impl Default for Panel {
    fn default() -> Self {
        Self {
            output: String::new(),
            models: "0,45,90,135,180,225,270,315".into(),
            lights: "0".into(),
            times: "0".into(),
            recipe: Recipe::default(),
            status: String::new(),
            pending: None,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}
fn numbers(text: &str) -> Result<Vec<f64>> {
    let values = text
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<f64>()
                .context("use comma-separated numbers")
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        values.len() <= 256 && values.iter().all(|v| v.is_finite()),
        "lists must contain 1..256 finite numbers"
    );
    Ok(values)
}
impl Panel {
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        asset: &Asset,
        clip: Option<usize>,
        time: f64,
        camera: Camera,
        stage: Stage,
    ) {
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok(result) => {
                    self.status = match result {
                        Ok(()) => "Sprite sheet exported.".into(),
                        Err(e) => format!("Export failed: {e:#}"),
                    };
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.status = "Sprite worker stopped.".into();
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => ui
                    .ctx()
                    .request_repaint_after(std::time::Duration::from_millis(100)),
            }
        }
        ui.collapsing("Export sprite sheet",|ui| {
            ui.label("Exports the loaded asset and selected clip, with fixed framing and transparent padding. Uses normal material appearance and LOD 0.");
            ui.add_enabled_ui(self.pending.is_none(),|ui| {
                if ui.button("Use current angles and frame").clicked() {self.models=stage.model.to_string();self.lights=stage.light.to_string();self.times=time.to_string();}
                for (label,value) in [("Model angles (degrees)",&mut self.models),("Light angles (degrees)",&mut self.lights),("Times (seconds)",&mut self.times)] {
                    ui.horizontal(|ui| {ui.label(label);ui.text_edit_singleline(value);});
                }
                ui.label("Comma-separated lists: every combination is exported; time varies fastest. View and light elevation use the current sliders. Panning is reset for export.");
                ui.collapsing("Export lighting and shadows",|ui| {
                    let l=&mut self.recipe.lighting;
                    ui.horizontal(|ui| {ui.label("Key colour");ui.color_edit_button_srgb(&mut l.key_colour);ui.add(egui::Slider::new(&mut l.key_intensity,0.0..=4.).text("Key intensity"));});
                    ui.horizontal(|ui| {ui.label("Fill colour");ui.color_edit_button_srgb(&mut l.fill_colour);ui.add(egui::Slider::new(&mut l.fill_intensity,0.0..=4.).text("Fill intensity"));});
                    ui.add(egui::Slider::new(&mut l.ambient,0.0..=2.).text("Ambient"));
                    ui.add(egui::Slider::new(&mut l.exposure,0.0..=4.).text("Exposure"));
                    ui.horizontal(|ui| {ui.label("Shadows");ui.selectable_value(&mut l.shadows,Shadows::Off,"Off");ui.selectable_value(&mut l.shadows,Shadows::Hard,"Hard");ui.selectable_value(&mut l.shadows,Shadows::Soft,"Soft");});
                    if l.shadows!=Shadows::Off {
                        ui.add(egui::Slider::new(&mut l.shadow_samples,1..=128).text("Shadow samples"));
                        if l.shadows==Shadows::Soft {ui.add(egui::Slider::new(&mut l.softness,0.0..=0.5).text("Light angular spread"));}
                        ui.label("Traced self/cast shadows on the model. Alpha materials, UV shaders, explicit mips and AO/emission maps require shadows off. No ground plane is added.");
                    }
                });
                ui.checkbox(&mut self.recipe.normal_sheet,"Export unlit colour and normal sheets for 2D lighting");
                if self.recipe.normal_sheet {
                    ui.checkbox(&mut self.recipe.normal_y_negative,"Flip normal green channel (Y down)");
                    ui.label("Use albedo.png + normals.png in-game. Normals include surface detail; import as linear data with identical sprite rectangles.");
                }
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::DragValue::new(&mut self.recipe.width).range(16..=2048).prefix("Width "));
                    ui.add(egui::DragValue::new(&mut self.recipe.height).range(16..=2048).prefix("Height "));
                    ui.add(egui::DragValue::new(&mut self.recipe.columns).range(1..=256).prefix("Columns "));
                    ui.add(egui::DragValue::new(&mut self.recipe.padding).range(0..=32).prefix("Padding "));
                });
                ui.label("New output directory");ui.text_edit_singleline(&mut self.output);
                if ui.button("Export sprite sheet").clicked() {
                    let prepared=(||->Result<Recipe> {
                        let models=numbers(&self.models)?;let lights=numbers(&self.lights)?;let times=numbers(&self.times)?;
                        ensure!(models.len()*lights.len()*times.len()<=256,"maximum 256 frames per sheet");
                        let mut recipe=self.recipe.clone(); recipe.frames.clear();
                        recipe.clip=clip.map(|i|asset.scene.clips[i].name.clone());recipe.orthographic=camera.ortho;recipe.zoom=camera.zoom;
                        for m in models {for l in &lights {for t in &times {recipe.frames.push(Sprite {view_azimuth:camera.yaw.to_degrees(),view_elevation:camera.pitch.to_degrees(),model_angle:m as f32,light_angle:*l as f32,light_elevation:stage.elevation,time:*t});}}}
                        recipe.dimensions()?;Ok(recipe)
                    })();
                    match prepared {
                        Err(e)=>self.status=e.to_string(),
                        Ok(recipe)=>{
                            let scene=asset.scene.clone();let skins=asset.skins.clone();let output=std::path::PathBuf::from(self.output.trim());
                            self.cancelled=Arc::new(AtomicBool::new(false));let cancel=self.cancelled.clone();
                            let (tx,rx)=mpsc::channel();self.pending=Some(rx);self.status="Rendering sprite sheet...".into();let ctx=ui.ctx().clone();
                            std::thread::spawn(move||{let result=install_headless().and_then(|()|Asset::preview_scene(scene,skins)).and_then(|a|export(&a,&recipe,&output,&cancel));let _=tx.send(result);ctx.request_repaint();});
                        }
                    }
                }
            });
            if self.pending.is_some() && ui.button("Cancel sprite export").clicked(){self.cancelled.store(true,Ordering::Relaxed);}
            ui.label(&self.status);
        });
    }
}
impl Drop for Panel {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matte_preserves_black_and_recovers_partial_coverage() {
        assert_eq!(matte([0, 0, 0, 255], [255; 4]), [0; 4]);
        assert_eq!(matte([0, 0, 0, 255], [0, 0, 0, 255]), [0, 0, 0, 255]);
        let p = matte([100, 50, 25, 255], [227, 177, 152, 255]);
        assert_eq!(p[3], 128);
        assert!(p[0].abs_diff(199) <= 1 && p[1].abs_diff(100) <= 1);
        let traced = traced_matte([137, 137, 137, 255], [225, 225, 225, 255]);
        assert!(traced[0].abs_diff(188) <= 2 && traced[3].abs_diff(128) <= 2);
    }
    #[test]
    fn recipe_limits_and_arbitrary_samples() {
        let mut r = Recipe::default();
        r.frames = vec![Sprite::default(); 5];
        assert_eq!(r.dimensions().unwrap(), (1040, 520));
        r.frames[0].time = f64::NAN;
        assert!(r.dimensions().is_err());
        r.frames[0].time = 0.;
        r.width = 2048;
        r.height = 2048;
        assert!(r.dimensions().is_err());
        assert!(numbers("1,NaN").is_err());
        assert!(serde_json::from_str::<Recipe>("{\"framse\":[]}").is_err());
    }

    #[test]
    fn normal_encoding_is_view_relative_linear_and_green_flip_is_explicit() {
        let n = sprite_normal(
            [128, 128, 255, 255],
            255,
            [1., 0., 0.],
            [0., 1., 0.],
            [0., 0., -1.],
            false,
        );
        assert_eq!(n, [128, 128, 255, 255]);
        let up = sprite_normal(
            [128, 218, 218, 255],
            255,
            [1., 0., 0.],
            [0., 1., 0.],
            [0., 0., -1.],
            false,
        );
        let down = sprite_normal(
            [128, 218, 218, 255],
            255,
            [1., 0., 0.],
            [0., 1., 0.],
            [0., 0., -1.],
            true,
        );
        assert_eq!(up[0], down[0]);
        assert_eq!(up[2], down[2]);
        assert_eq!(up[1] as u16 + down[1] as u16, 255);
        assert_eq!(
            sprite_normal([0; 4], 0, [1., 0., 0.], [0., 1., 0.], [0., 0., -1.], false),
            [128, 128, 255, 0]
        );
    }

    #[test]
    fn transparent_rgb_bleed_keeps_alpha_and_cell_boundary() {
        let mut map = image::RgbaImage::new(6, 3);
        map.put_pixel(2, 1, image::Rgba([200, 100, 50, 255]));
        bleed_cell(&mut map, 0, 0, 3, 3);
        assert_eq!(map.get_pixel(1, 1).0, [200, 100, 50, 0]);
        assert_eq!(map.get_pixel(3, 1).0, [0; 4]);
        assert_eq!(map.pixels().filter(|p| p[3] > 0).count(), 1);
    }

    #[cfg(feature = "gpu-tests")]
    #[test]
    fn sprite_normals_include_bump_and_traced_shadows_occlude_receivers() {
        install_headless().unwrap();
        let root = std::env::temp_dir().join(format!(
            "gcad-sprite-relight-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("part.gcad"), "rect 20 20\nextrude 10\n").unwrap();
        let source = root.join("asset.gasset");
        std::fs::write(&source,"asset 1\nsource box part.gcad body=main\ntessellate box tolerance=0.2\nuv box projection=box tile=10\nnode raised source=box translation=0,0,8\nnode receiver source=box translation=0,0,-2 scale=4,4,0.1\n").unwrap();
        let mut asset = crate::asset::author::load(&source, &[]).unwrap();
        let recipe = Recipe {
            width: 96,
            height: 96,
            frames: vec![Sprite {
                view_azimuth: 0.,
                view_elevation: 90.,
                ..Default::default()
            }],
            ..Default::default()
        };
        let f = frame(&asset, None, 0.).unwrap();
        let camera = Camera {
            yaw: 0.,
            pitch: std::f32::consts::FRAC_PI_2,
            ..Default::default()
        };
        let render_normals = |f: &Frame| {
            render_pass(
                f,
                f.scene.clone(),
                camera,
                Lighting::STUDIO,
                RenderMode::Normals,
                None,
                egui::Color32::BLACK,
                &recipe,
            )
            .unwrap()
        };
        let flat = render_normals(&f);
        asset.scene.materials[0].normal_map = Some(crate::model::texture::Map {
            width: 1,
            height: 1,
            rgba: vec![218, 128, 218, 255],
        });
        let bumped = frame(&asset, None, 0.).unwrap();
        let bump = render_normals(&bumped);
        assert!(
            flat.pixels
                .iter()
                .zip(&bump.pixels)
                .filter(|(a, b)| a != b)
                .count()
                > 100,
            "normal sheet must include material normal maps"
        );
        let settings = LightSettings {
            fill_intensity: 0.,
            ambient: 0.,
            shadows: Shadows::Hard,
            shadow_samples: 16,
            ..Default::default()
        };
        let light = settings.lighting(Stage {
            elevation: 45.,
            ..Default::default()
        });
        let render_shadow = |f: &Frame| {
            render_pass(
                f,
                f.scene.clone(),
                camera,
                light,
                RenderMode::Shaded,
                settings.trace(),
                egui::Color32::BLACK,
                &recipe,
            )
            .unwrap()
        };
        let shadowed = render_shadow(&f);
        let mut open = frame(&crate::asset::author::load(&source, &[]).unwrap(), None, 0.).unwrap();
        for surface in &open.scene.surfaces {
            if surface.positions.iter().all(|p| p[2] > 0.) {
                open.looks[surface.group].opacity = 0.;
            }
        }
        // Preserve camera bounds so only the occluder and its shadow change.
        Arc::get_mut(&mut open.scene).unwrap().centre = f.scene.centre;
        Arc::get_mut(&mut open.scene).unwrap().radius = f.scene.radius;
        let clear = render_shadow(&open);
        let occluded = shadowed
            .pixels
            .iter()
            .zip(&clear.pixels)
            .filter(|(a, b)| b.r() > 30 && (a.r() as i32 + 20) < b.r() as i32)
            .count();
        assert!(
            occluded > 20,
            "raised geometry must cast shadows on receiver: {occluded}"
        );
        let soft = LightSettings {
            shadows: Shadows::Soft,
            softness: 0.2,
            ..settings
        };
        let soft_pixels = render_pass(
            &f,
            f.scene.clone(),
            camera,
            soft.lighting(Stage {
                elevation: 45.,
                ..Default::default()
            }),
            RenderMode::Shaded,
            soft.trace(),
            egui::Color32::BLACK,
            &recipe,
        )
        .unwrap();
        assert_ne!(soft_pixels.pixels, shadowed.pixels);
        for name in ["part.gcad", "asset.gasset"] {
            std::fs::remove_file(root.join(name)).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }

    #[cfg(feature = "gpu-tests")]
    #[test]
    fn sprite_workers_initialize_and_release_thread_local_renderers() {
        let root = std::env::temp_dir().join(format!(
            "gcad-sprite-workers-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        // Two sequential workers exercise thread-local teardown and reuse of the
        // process-owned device without borrowing the viewer's UI-thread cache.
        for i in 0..2 {
            let output = root.join(i.to_string());
            let destination = output.clone();
            std::thread::spawn(move || -> Result<()> {
                install_headless()?;
                let asset = crate::asset::author::load(
                    &Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("examples/game-assets/plate.gasset"),
                    &[],
                )?;
                export(
                    &asset,
                    &Recipe {
                        width: 32,
                        height: 32,
                        ..Default::default()
                    },
                    &destination,
                    &AtomicBool::new(false),
                )
            })
            .join()
            .unwrap()
            .unwrap();
            assert!(output.join("sprites.png").is_file());
            for name in ["sprites.png", "sprites.json", "normals.png", "albedo.png"] {
                std::fs::remove_file(output.join(name)).unwrap();
            }
            std::fs::remove_dir(output).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }

    #[cfg(feature = "gpu-tests")]
    #[test]
    fn gpu_sprite_sheet_angles_lighting_pose_alpha_and_preservation() {
        let state = super::super::explicit_tests::requested_render_state();
        viewer3d::install(&state);
        let root = std::env::temp_dir().join(format!(
            "gcad-sprites-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("part.gcad"), "rect 30 10\nextrude 8\n").unwrap();
        let source = root.join("asset.gasset");
        std::fs::write(&source,"asset 1\nsource box part.gcad body=main\ntessellate box tolerance=0.2\nuv box projection=box tile=10\nnode box source=box\nclip move interpolation=linear\nkey move box time=0 translation=0,0,0\nkey move box time=1 translation=20,0,0\n").unwrap();
        let asset = crate::asset::author::load(&source, &[]).unwrap();
        let recipe = Recipe {
            width: 96,
            height: 96,
            columns: 4,
            clip: Some("move".into()),
            frames: vec![
                Sprite::default(),
                Sprite {
                    model_angle: 90.,
                    ..Default::default()
                },
                Sprite {
                    light_angle: 180.,
                    ..Default::default()
                },
                Sprite {
                    time: 1.,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let output = root.join("output");
        let cancel = AtomicBool::new(false);
        export(&asset, &recipe, &output, &cancel).unwrap();
        let image = image::open(output.join("sprites.png")).unwrap().to_rgba8();
        let normals = image::open(output.join("normals.png")).unwrap().to_rgba8();
        let albedo = image::open(output.join("albedo.png")).unwrap().to_rgba8();
        assert_eq!(image.dimensions(), normals.dimensions());
        for ((colour, normal), base) in image.pixels().zip(normals.pixels()).zip(albedo.pixels()) {
            assert_eq!(colour[3], normal[3]);
            assert_eq!(colour[3], base[3]);
            if normal[3] > 240 {
                let length = normal.0[..3]
                    .iter()
                    .map(|v| (*v as f32 / 255. * 2. - 1.).powi(2))
                    .sum::<f32>()
                    .sqrt();
                assert!((length - 1.).abs() < 0.015);
            }
        }
        for map in [&normals, &albedo] {
            assert_eq!(
                image::imageops::crop_imm(map, 2, 2, 96, 96).to_image(),
                image::imageops::crop_imm(map, 202, 2, 96, 96).to_image(),
                "runtime sheets must not bake light changes"
            );
        }
        assert_eq!(image.dimensions(), (400, 100));
        assert_eq!(image.get_pixel(0, 0).0, [0; 4]);
        let tiles: Vec<_> = (0..4)
            .map(|i| image::imageops::crop_imm(&image, 2 + i * 100, 2, 96, 96).to_image())
            .collect();
        for tile in &tiles {
            assert!(tile.pixels().any(|p| p[3] == 255));
            assert!(tile.pixels().any(|p| p[3] == 0));
        }
        for i in 1..4 {
            assert!(
                tiles[0]
                    .pixels()
                    .zip(tiles[i].pixels())
                    .filter(|(a, b)| a != b)
                    .count()
                    > 100,
                "sample {i} must change pixels"
            );
        }
        assert!(export(&asset, &recipe, &output, &cancel).is_err());
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(output.join("sprites.json")).unwrap()).unwrap();
        assert_eq!(json["frames"][3]["sample"]["time"], 1.);
        cancel.store(true, Ordering::Relaxed);
        assert!(export(&asset, &recipe, &root.join("cancelled"), &cancel).is_err());
        assert!(!root.join("cancelled").exists());
        for f in [
            output.join("sprites.png"),
            output.join("sprites.json"),
            output.join("normals.png"),
            output.join("albedo.png"),
            source,
            root.join("part.gcad"),
        ] {
            std::fs::remove_file(f).unwrap();
        }
        std::fs::remove_dir(output).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
