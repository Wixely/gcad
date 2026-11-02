//! Experimental asset preview on the existing renderer; export remains authoritative.
#[cfg(all(test, feature = "gpu-tests"))]
#[path = "asset_alpha_tests.rs"]
mod alpha_tests;
#[cfg(all(test, feature = "gpu-tests"))]
#[path = "asset_explicit_tests.rs"]
mod explicit_tests;
#[cfg(all(test, feature = "gpu-tests"))]
#[path = "asset_extra_maps_tests.rs"]
mod extra_maps_tests;
#[cfg(all(test, feature = "gpu-tests"))]
#[path = "asset_shader_tests.rs"]
mod shader_tests;
#[path = "asset_images.rs"]
pub mod image_budget;
#[path = "asset_inspector.rs"]
mod inspector;
#[path = "asset_consumer.rs"]
mod consumer;
#[path = "asset_collision.rs"]
mod collision;
#[path = "asset_lod.rs"]
mod lod;
#[path = "asset_motion.rs"]
mod motion;
#[path = "asset_reload.rs"]
mod reload;
#[path = "asset_export.rs"]
mod export;
#[path = "asset_stage.rs"]
mod stage;
#[path = "asset_sprites.rs"]
pub mod sprites;
#[path = "asset_rig.rs"]
mod rig;
#[path = "asset_visuals.rs"]
mod visuals;
#[path = "asset_weights.rs"]
mod weights;
#[cfg(test)]
#[path = "asset_live_tests.rs"]
mod live_tests;
use crate::asset::{
    author::Asset,
    material::{AlphaMode, Sampler},
    scene::Transform,
};
use anyhow::{Context, Result, ensure};
use egui_bench::viewer3d::{
    self, Camera, Lighting, Look, Material, Orbit, Pbr, RenderMode, Scene, Surface, Viewer,
};
use monstertruck::modeling::{Matrix as _, Matrix3, Point3, SquareMatrix, Transform as _, Vector3};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Finish([u32; 13]);
impl Default for Finish {
    fn default() -> Self {
        let mut bits = [0; 13];
        bits[6] = (-1f32).to_bits();
        bits[7] = 1f32.to_bits();
        bits[8] = 1f32.to_bits();
        bits[9] = 1f32.to_bits();
        bits[10] = 1f32.to_bits();
        Self(bits)
    }
}
impl Material for Finish {
    fn pbr(&self) -> Pbr {
        let [
            roughness,
            metallic,
            normal_scale,
            r,
            g,
            b,
            mode,
            base_alpha,
            occlusion_strength,
            scale_u, scale_v, offset_u, offset_v,
        ] = self.0.map(f32::from_bits);
        Pbr {
            uv_transform: [scale_u, scale_v, offset_u, offset_v],
            roughness,
            metallic,
            normal_scale,
            emission: [r, g, b],
            alpha: if mode == -2. {
                viewer3d::AlphaMode::Blend
            } else if mode >= 0. {
                viewer3d::AlphaMode::Mask { cutoff: mode }
            } else {
                viewer3d::AlphaMode::Opaque
            },
            base_alpha,
            occlusion_strength,
            ..Default::default()
        }
    }
}

pub struct Frame {
    pub scene: Arc<Scene<Finish>>,
    pub looks: Vec<Look>,
    rig: Vec<rig::Node>,
    collision_boxes: Vec<collision::BoxVolume>,
}

/// Keep navigation stable during playback/reload; only an explicit refit adopts
/// the new geometry's bounds and resets navigation.
fn replace_frame(
    current: &mut Frame,
    mut next: Frame,
    camera: &mut Camera,
    orbit: &mut Orbit,
    refit: bool,
) {
    if refit {
        *camera = Camera::default();
        *orbit = Orbit::default();
    } else {
        let scene = Arc::get_mut(&mut next.scene).expect("new preview frame is unshared");
        scene.centre = current.scene.centre;
        scene.radius = current.scene.radius;
    }
    *current = next;
}

pub fn differences(asset: &Asset) -> Vec<String> {
    let mut result = vec![
        "Preview uses renderer-generated tangent frames; export uses authored tangents.".into(),
        "Preview draws both sides; exported materials retain their double-sided setting.".into(),
    ];
    for (i, m) in asset.scene.materials.iter().enumerate() {
        if m.mip_chain.is_none() && m.sampler != Sampler::default() {
            result.push(format!(
                "Material {i}: custom sampler settings are not displayed."
            ));
        }
        if m.alpha == AlphaMode::Blend {
            result.push(format!("Material {i}: blend preview sorts batches; intersecting transparent surfaces can differ."));
        }
    }
    result
}

fn srgb(x: f64) -> f32 {
    (if x <= 0.0031308 {
        12.92 * x
    } else {
        1.055 * x.powf(1. / 2.4) - 0.055
    }) as f32
}
fn linear(x: f64) -> f64 {
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

/// Prepare the five maps supported by the preview renderer. Only colour RGB
/// absorbs its factor; alpha, normal scale and scalar factors remain separate.
pub fn material_images(
    material: &crate::asset::material::Material,
) -> Result<[Option<crate::model::texture::Map>; 5]> {
    material.validate()?;
    let mut colour = material.colour_map.clone();
    if let Some(map) = &mut colour {
        for p in map.rgba.chunks_exact_mut(4) {
            for c in 0..3 {
                p[c] = (srgb(linear(p[c] as f64 / 255.) * material.base_colour[c]).clamp(0., 1.)
                    * 255.)
                    .round() as u8;
            }
        }
    }
    Ok([
        colour,
        material.portable_normal()?,
        material.packed_metal_rough()?,
        material.occlusion_map.clone(),
        material.emissive_map.clone(),
    ])
}

/// Apply the same preview conversion to every explicit level. Empty vectors
/// represent absent slots; no level is generated or dropped by this adapter.
pub fn mip_images(
    chain: &crate::asset::atlas::MipChain,
) -> Result<[Vec<crate::model::texture::Map>; 5]> {
    let mut images: [Vec<_>; 5] = std::array::from_fn(|_| Vec::new());
    for level in chain.levels() {
        for (slot, map) in material_images(level)?.into_iter().enumerate() {
            if let Some(map) = map {
                images[slot].push(map);
            }
        }
    }
    ensure!(
        images
            .iter()
            .all(|slot| slot.is_empty() || slot.len() == chain.levels().len()),
        "preview mip slots must be contiguous"
    );
    Ok(images)
}
fn v3(x: [f64; 3]) -> Result<[f32; 3]> {
    let result = x.map(|x| x as f32);
    ensure!(
        result.iter().all(|x| x.is_finite()),
        "preview exceeds float32 range"
    );
    Ok(result)
}

pub fn frame(asset: &Asset, clip: Option<usize>, time: f64) -> Result<Frame> {
    Playback::new(asset)?.frame(clip, time)
}

/// An immutable asset borrow keeps load-time validation valid across frames.
pub struct Playback<'a> {
    asset: &'a Asset,
}
impl<'a> Playback<'a> {
    pub fn new(asset: &'a Asset) -> Result<Self> {
        asset.scene.validate()?;
        for skin in &asset.skins { skin.validate(&asset.scene)?; }
        image_budget::usage(&asset.scene.materials, image_budget::ImageLimits::default())?;
        Ok(Self { asset })
    }

    pub fn frame(&self, clip: Option<usize>, time: f64) -> Result<Frame> {
        loaded_frame(self.asset, clip, time)
    }
}

// Only for immutable assets already validated by author::load/preview_scene or
// Playback::new. The live document replaces the asset on reload/LOD generation.
fn loaded_frame(asset: &Asset, clip: Option<usize>, time: f64) -> Result<Frame> {
    let pose = match clip {
        Some(i) => asset
            .scene
            .clips
            .get(i)
            .context("unknown preview clip")?
            .sample(&asset.scene, time, false)?,
        None => asset.scene.nodes.iter().map(|n| n.transform).collect(),
    };
    build(asset, &pose, time)
}

fn preview_duration(asset: &Asset, clip: Option<usize>) -> f64 {
    clip.map_or_else(|| if asset.scene.materials.iter().any(|m| m.shader.is_some()) { 10. } else { 0. },
        |i| asset.scene.clips[i].duration())
}

fn build(asset: &Asset, pose: &[Transform], time: f64) -> Result<Frame> {
    image_budget::usage(&asset.scene.materials, image_budget::ImageLimits::default())?;

    let world = asset.scene.posed_world_transforms(pose)?;
    let mut result = Scene::default();
    let mut looks = Vec::new();
    let mut images = Vec::new();
    for m in &asset.scene.materials {
        if let Some(chain) = &m.mip_chain {
            let mut slots = [None; 5];
            for (slot, maps) in mip_images(chain)?.into_iter().enumerate() {
                if maps.is_empty() {
                    continue;
                }
                let levels: Vec<_> = maps
                    .into_iter()
                    .map(|map| viewer3d::Image {
                        width: map.width,
                        height: map.height,
                        rgba: map.rgba,
                    })
                    .collect();
                let index = result.images.len();
                result.images.push(levels[0].clone());
                let encoding = if slot == 0 || slot == 4 {
                    viewer3d::ImageEncoding::Srgb
                } else {
                    viewer3d::ImageEncoding::Linear
                };
                let explicit =
                    viewer3d::ExplicitImage::new(levels, encoding).map_err(anyhow::Error::msg)?;
                result.explicit_images.insert(index, explicit);
                slots[slot] = Some(index);
            }
            images.push((slots[0], slots[1], slots[2], slots[3], slots[4]));
            continue;
        }
        let [colour, normal, metal_rough, occlusion, emission] = material_images(m)?;
        let mut add = |map: Option<crate::model::texture::Map>| {
            map.map(|map| {
                let i = result.images.len();
                result.images.push(viewer3d::Image {
                    width: map.width,
                    height: map.height,
                    rgba: map.rgba,
                });
                i
            })
        };
        images.push((
            add(colour),
            add(normal),
            add(metal_rough),
            add(occlusion),
            add(emission),
        ));
    }
    let mut low = [f32::INFINITY; 3];
    let mut high = [f32::NEG_INFINITY; 3];
    for (node_index, node) in asset.scene.nodes.iter().enumerate() {
        let Some(mesh_index) = node.mesh else {
            continue;
        };
        let skin = asset.skins.iter().find(|s| s.mesh_node == node_index);
        let matrices = skin.map(|s| s.matrices_from_world(&world));
        let deformed = skin
            .map(|s| s.positions_from_matrices(&asset.scene, matrices.as_ref().unwrap()))
            .transpose()?;
        let basis = skin
            .map(|s| s.basis_from_matrices(&asset.scene, matrices.as_ref().unwrap()))
            .transpose()?;
        let matrix = world[node_index];
        let normal_matrix = Matrix3::from_cols(
            matrix.x.truncate(),
            matrix.y.truncate(),
            matrix.z.truncate(),
        )
        .invert()
        .context("singular preview transform")?
        .transpose();
        for (p, primitive) in asset.scene.meshes[mesh_index].primitives.iter().enumerate() {
            let m = &asset.scene.materials[primitive.material];
            let group = looks.len();
            looks.push(Look {
                opacity: 1.,
                ..Default::default()
            });
            let (texture, normal_map, metal_rough_map, occlusion_map, emission_map) =
                images[primitive.material];
            let uv_transform = match &m.shader {
                Some(shader) => {
                    let transform = shader.at(time)?;
                    for uv in primitive.geometry.texcoords.as_deref().unwrap_or(&[]) { transform.apply(*uv)?; }
                    [transform.scale[0] as f32, transform.scale[1] as f32, transform.offset[0] as f32, transform.offset[1] as f32]
                }
                None => [1.,1.,0.,0.],
            };
            let mut surface = Surface {
                colour: [
                    srgb(m.base_colour[0]),
                    srgb(m.base_colour[1]),
                    srgb(m.base_colour[2]),
                ],
                material: Finish(
                    [
                        m.roughness as f32,
                        m.metallic as f32,
                        m.normal_scale as f32,
                        m.emissive[0] as f32,
                        m.emissive[1] as f32,
                        m.emissive[2] as f32,
                        match m.alpha {
                            AlphaMode::Opaque => -1.,
                            AlphaMode::Blend => -2.,
                            AlphaMode::Mask { cutoff } => cutoff as f32,
                        },
                        m.base_colour[3] as f32,
                        m.occlusion_strength as f32,
                        uv_transform[0], uv_transform[1], uv_transform[2], uv_transform[3],
                    ]
                    .map(f32::to_bits),
                ),
                group,
                texture,
                normal_map,
                metal_rough_map,
                occlusion_map,
                emission_map,
                ..Default::default()
            };
            for triangle in &primitive.geometry.triangles {
                for &index in triangle {
                    let i = index as usize;
                    let point = if let Some(positions) = &deformed {
                        positions[p][i]
                    } else {
                        let a = primitive.geometry.positions[i];
                        let v = matrix.transform_point(Point3::new(a[0], a[1], a[2]));
                        [v.x, v.y, v.z]
                    };
                    let normal = if let Some(basis) = &basis {
                        basis[p].normals[i]
                    } else {
                        let n = primitive.geometry.normals[i];
                        let v = normal_matrix * Vector3::new(n[0], n[1], n[2]);
                        let length = v.x.hypot(v.y).hypot(v.z);
                        ensure!(length.is_finite() && length > 0., "invalid preview normal");
                        [v.x / length, v.y / length, v.z / length]
                    };
                    let point = v3(point)?;
                    for k in 0..3 {
                        low[k] = low[k].min(point[k]);
                        high[k] = high[k].max(point[k]);
                    }
                    surface.positions.push(point);
                    surface.normals.push(v3(normal)?);
                    if let Some(uv) = &primitive.geometry.texcoords {
                        let uv = uv[i].map(|x| x as f32);
                        ensure!(
                            uv.iter().all(|v| v.is_finite()),
                            "preview UV exceeds float32 range"
                        );
                        surface.uvs.push(uv);
                    }
                }
            }
            result.surfaces.push(surface);
        }
    }
    ensure!(!result.surfaces.is_empty(), "preview has no surfaces");
    result.centre = std::array::from_fn(|k| low[k] * 0.5 + high[k] * 0.5);
    result.radius = ((high[0] - low[0])
        .hypot(high[1] - low[1])
        .hypot(high[2] - low[2])
        * 0.5)
        .max(0.001);
    ensure!(
        result.radius.is_finite(),
        "preview bounds exceed float32 range"
    );
    result
        .validate_explicit_images()
        .map_err(anyhow::Error::msg)?;
    Ok(Frame {
        collision_boxes: collision::boxes(asset, &world)?,
        scene: Arc::new(result),
        looks,
        rig: rig::nodes(asset, &world),
    })
}

pub fn snapshot(
    asset: &Asset,
    clip: Option<usize>,
    time: f64,
    path: &std::path::Path,
) -> Result<()> {
    let frame = consumer::ConsumerView::default().apply(asset,frame(asset, clip, time)?);
    ensure!(!frame.scene.uses_uv_transform(), "software snapshot does not support UV shader transforms");
    ensure!(
        frame.scene.explicit_images.is_empty(),
        "software snapshot does not support explicit mip chains"
    );
    ensure!(
        !frame.scene.uses_material_alpha(),
        "software snapshot does not support material alpha"
    );
    ensure!(
        !frame.scene.uses_extra_maps(),
        "software snapshot does not support occlusion/emission maps"
    );
    let viewer = Viewer::new(frame.scene, Camera::default())
        .looks(frame.looks)
        .lighting(Lighting::STUDIO);
    let image = viewer.render_soft(960, 720);
    let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
    image::save_buffer_with_format(
        path,
        &bytes,
        960,
        720,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )?;
    Ok(())
}

struct Preview {
    stage: stage::Stage,
    sprites: sprites::Panel,
    consumer:consumer::ConsumerView,
    lod: lod::Comparison,
    motion: motion::MotionPreview,
    skeleton: bool,
    joint_labels: bool,
    weight_joint: Option<String>,
    inspector: inspector::Inspector,
    inspector_open: bool,
    asset: Asset,
    frame: Frame,
    clip: Option<usize>,
    time: f64,
    playing: bool,
    looping: bool,
    camera: Camera,
    orbit: Orbit,
    mode: RenderMode,
    error: Option<String>,
    differences: Vec<String>,
}
/// Game-asset document hosted by the existing gcad App and window.
pub(super) struct Document {
    export: export::Export,
    preview: Option<Preview>,
    reload: reload::Reload,
    loading: bool,
    error: Option<String>,
    initial_clip: Option<String>,
    initial_time: f64,
}

impl Document {
    #[cfg(test)]
    pub(super) fn ready(&self) -> bool {
        self.preview.is_some() && !self.loading && self.error.is_none()
    }

    pub fn open(path: std::path::PathBuf, vars: Vec<(String, f64)>, ctx: egui::Context) -> Self {
        Self {
            export: export::Export::new(path.clone(), vars.clone()),
            preview: None,
            reload: reload::Reload::start(path, vars, crate::host::Reads::default(), ctx),
            loading: true,
            error: None,
            initial_clip: None,
            initial_time: 0.,
        }
    }

    pub fn initial_pose(&mut self, clip: Option<String>, time: f64) {
        self.initial_clip = clip;
        self.initial_time = time;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        while let Ok(event) = self.reload.events.try_recv() {
            match event {
                reload::Event::Loading => self.loading = true,
                reload::Event::Failed(error) => {
                    self.loading = false;
                    self.error = Some(error);
                }
                reload::Event::Loaded(asset, reads) => {
                    if !reads.unchanged() {
                        self.loading = true;
                        continue;
                    }
                    let name = self
                        .preview
                        .as_ref()
                        .and_then(|p| p.clip.map(|i| p.asset.scene.clips[i].name.as_str()))
                        .or_else(|| {
                            self.preview
                                .is_none()
                                .then_some(self.initial_clip.as_deref())
                                .flatten()
                        });
                    let clip =
                        name.and_then(|name| asset.scene.clips.iter().position(|c| c.name == name));
                    let old_time = self.preview.as_ref().map_or(self.initial_time, |p| p.time);
                    let time = old_time.min(preview_duration(&asset, clip));
                    let weight_joint = self
                        .preview
                        .as_ref()
                        .and_then(|p| p.weight_joint.clone())
                        .filter(|name| {
                            asset.skins.iter().any(|skin| {
                                skin.joints
                                    .iter()
                                    .any(|&i| asset.scene.nodes[i].name == *name)
                            })
                        });
                    let consumer=self.preview.as_ref().map(|p|p.consumer).unwrap_or_default();
                    match weights::frame(&asset, clip, time, weight_joint.as_deref()).map(|frame|consumer.apply(&asset,frame)) {
                        Ok(next) => {
                            let differences = differences(&asset);
                            if let Some(p) = &mut self.preview {
                                p.inspector.invalidate();
                                p.motion.invalidate();
                                p.lod.invalidate();
                                replace_frame(
                                    &mut p.frame,
                                    next,
                                    &mut p.camera,
                                    &mut p.orbit,
                                    false,
                                );
                                p.differences = differences;
                                p.asset = asset;
                                p.weight_joint = weight_joint;
                                p.clip = clip;
                                p.time = time;
                                p.playing &= preview_duration(&p.asset, clip) > 0.;
                                p.error = None;
                            } else {
                                self.preview = Some(Preview {
                                    stage: stage::Stage::default(),
                                    sprites: sprites::Panel::default(),
                                    consumer,
                                    lod: lod::Comparison::default(),
                                    motion: motion::MotionPreview::default(),
                                    skeleton: false,
                                    joint_labels: true,
                                    weight_joint: None,
                                    inspector: inspector::Inspector::default(),
                                    inspector_open: true,
                                    asset,
                                    frame: next,
                                    clip,
                                    time,
                                    playing: false,
                                    looping: true,
                                    camera: Camera::default(),
                                    orbit: Orbit::default(),
                                    mode: RenderMode::Shaded,
                                    error: None,
                                    differences,
                                });
                            }
                            self.error = None;
                        }
                        Err(error) => self.error = Some(format!("Preview unavailable: {error:#}")),
                    }
                    self.loading = false;
                }
            }
        }
        egui::Panel::top("asset-status").show(ui, |ui| {
            self.export.ui(ui, self.preview.is_some() && !self.loading && self.error.is_none());
            if self.loading {
                ui.label("Updating game asset...");
            }
            if let Some(error) = &self.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
                if self.preview.is_some() {
                    ui.label("Showing last valid model.");
                }
            }
        });
        if let Some(preview) = &mut self.preview {
            preview.ui(ui);
        } else {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.label(
                    "Waiting for a valid game asset. Correct the source to reload automatically.",
                );
            });
        }
    }
}

impl Preview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let old = (self.clip, self.time);
        let old_weight = self.weight_joint.clone();
        let old_consumer=self.consumer;
        let duration = preview_duration(&self.asset, self.clip);
        if self.playing && duration > 0. {
            self.time += ui.input(|i| i.stable_dt) as f64;
            if self.time >= duration {
                if self.looping {
                    self.time = self.time.rem_euclid(duration);
                } else {
                    self.time = duration;
                    self.playing = false;
                }
            }
            ui.ctx().request_repaint();
        }
        let mut refit = false;
        let mut lod_changed = false;
        egui::Panel::top("asset-controls").show(ui, |ui| {
            ui.horizontal(|ui| {
                egui::ComboBox::from_label("Animation")
                    .selected_text(
                        self.clip
                            .map_or("Rest pose", |i| &self.asset.scene.clips[i].name),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.clip, None, "Rest pose");
                        for (i, c) in self.asset.scene.clips.iter().enumerate() {
                            ui.selectable_value(&mut self.clip, Some(i), &c.name);
                        }
                    });
                if ui
                    .add_enabled(
                        preview_duration(&self.asset, self.clip) > 0.,
                        egui::Button::new(if self.playing { "Pause" } else { "Play" }),
                    )
                    .clicked()
                {
                    self.playing = !self.playing;
                }
                ui.checkbox(&mut self.looping, "Loop");
                let duration = preview_duration(&self.asset, self.clip);
                ui.add(egui::Slider::new(&mut self.time, 0.0..=duration).text("Seconds"));
                refit = ui
                    .button("Fit model")
                    .on_hover_text("Reset the camera to fit the current pose")
                    .clicked();
            });
            if self.clip != old.0 {
                self.time = 0.;
                self.playing = false;
            }
            self.stage.ui(ui, &mut self.camera);
            self.sprites.ui(ui, &self.asset, self.clip, self.time, self.camera, self.stage);
            viewer3d::mode_controls(ui, &mut self.mode);
            if !self.asset.skins.is_empty() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.skeleton, "Skeleton (X-ray)");
                    if self.skeleton { ui.checkbox(&mut self.joint_labels, "Joint labels"); }
                });
                egui::ComboBox::from_label("Joint weights")
                    .selected_text(self.weight_joint.as_deref().map_or("Material appearance".to_owned(), |name| format!("Weights: {name}")))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.weight_joint, None, "Material appearance");
                        let joints: std::collections::BTreeSet<_> = self.asset.skins.iter().flat_map(|skin| skin.joints.iter().copied()).collect();
                        for joint in joints {
                            let name = &self.asset.scene.nodes[joint].name;
                            ui.selectable_value(&mut self.weight_joint, Some(name.clone()), format!("Weights: {name}"));
                        }
                    });
                if self.weight_joint.is_some() {
                    ui.label("Joint influence: blue = 0, red = 1; unskinned meshes are grey. Use shaded view. Lighting affects colours.");
                }
            }
            ui.checkbox(&mut self.inspector_open, "Scene inspector");
            self.consumer.controls(ui,&self.asset);
            self.motion.controls(ui, &self.asset, self.clip, self.time);
            lod_changed = self.lod.controls(ui, &self.asset);
            ui.collapsing("Preview differences", |ui| {
                for text in &self.differences {
                    ui.label(text);
                }
            });
            if let Some(error) = &self.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
        });
        if lod_changed {
            self.inspector.invalidate();
        }
        if refit || old != (self.clip, self.time) || old_weight != self.weight_joint || lod_changed || old_consumer!=self.consumer
        {
            match weights::loaded_frame(
                self.lod.displayed(&self.asset),
                self.clip,
                self.time,
                self.weight_joint.as_deref(),
            ).map(|frame|self.consumer.apply(self.lod.displayed(&self.asset),frame)) {
                Ok(frame) => {
                    replace_frame(
                        &mut self.frame,
                        frame,
                        &mut self.camera,
                        &mut self.orbit,
                        refit,
                    );
                    self.error = None;
                }
                Err(error) => {
                    self.error = Some(format!(
                        "Pose unavailable; showing previous pose: {error:#}"
                    ));
                    self.playing = false;
                }
            }
        }
        if self.inspector_open {
            egui::Panel::right("asset-inspector")
                .resizable(true)
                .default_size(300.)
                .show(ui, |ui| {
                    self.inspector
                        .ui(ui, self.lod.displayed(&self.asset), self.clip, self.time);
                });
        }
        egui::CentralPanel::default().show(ui, |ui| {
            let rect = ui.max_rect();
            let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
            let mut viewer = Viewer::new(self.frame.scene.clone(), self.stage.camera(self.camera))
                .looks(self.frame.looks.clone())
                .lighting(self.stage.lighting())
                .mode(self.mode);
            viewer.navigate(ui, &response, &mut self.orbit);
            self.camera = self.stage.restore_camera(viewer.camera());
            viewer.paint(ui, rect);
            if self.consumer.boxes {
                collision::paint(ui, rect, &viewer.projector(rect), &self.frame.collision_boxes);
            }
            if self.skeleton {
                rig::paint(
                    ui,
                    rect,
                    &viewer.projector(rect),
                    &self.frame.rig,
                    self.joint_labels,
                );
            }
            if self.error.is_none() {
                self.motion
                    .paint(ui, rect, &viewer.projector(rect), self.time);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn playback_matches_checked_skin_deformation_and_rejects_stale_rest_data() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/game-assets/plate-skinned.gasset");
        let mut asset = crate::asset::author::load(&path, &[]).unwrap();
        let playback = Playback::new(&asset).unwrap();
        for time in [0., 0.3, 0.7] {
            let pose = asset.scene.clips[0].sample(&asset.scene, time, false).unwrap();
            let actual = playback.frame(Some(0), time).unwrap();
            let skin = &asset.skins[0];
            let positions = skin.deform_positions(&asset.scene, &pose).unwrap();
            let basis = skin.deform_basis(&asset.scene, &pose).unwrap();
            let mesh = asset.scene.nodes[skin.mesh_node].mesh.unwrap();
            for (p, primitive) in asset.scene.meshes[mesh].primitives.iter().enumerate() {
                let expected_positions: Vec<_> = primitive.geometry.triangles.iter().flatten()
                    .map(|&i| v3(positions[p][i as usize]).unwrap()).collect();
                let expected_normals: Vec<_> = primitive.geometry.triangles.iter().flatten()
                    .map(|&i| v3(basis[p].normals[i as usize]).unwrap()).collect();
                assert_eq!(actual.scene.surfaces[p].positions, expected_positions);
                assert_eq!(actual.scene.surfaces[p].normals, expected_normals);
            }
        }
        assert!(playback.frame(Some(0), f64::NAN).is_err());
        asset.scene.nodes[asset.skins[0].joints[0]].transform.translation[0] += 1.;
        assert!(Playback::new(&asset).is_err());
    }

    #[test]
    fn live_document_recovers_texture_and_preserves_camera_and_clip() {
        check_live_texture(false);
    }

    #[test]
    fn unwrapped_live_document_recovers_texture_without_moving_uvs() {
        check_live_texture(true);
    }

    fn check_live_texture(unwrap: bool) {
        use egui_kittest::kittest::Queryable as _;
        use std::{
            path::Path,
            time::{Duration, Instant, SystemTime, UNIX_EPOCH},
        };

        fn wait(
            harness: &mut egui_kittest::Harness<'_, Option<Document>>,
            condition: impl Fn(&Document) -> bool,
        ) {
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                harness.step();
                let document = harness.state().as_ref().unwrap();
                if condition(document) {
                    return;
                }
                assert!(
                    Instant::now() < deadline,
                    "live document timed out: {:?}",
                    document.error
                );
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        fn pixel(document: &Document) -> [u8; 4] {
            let frame = &document.preview.as_ref().unwrap().frame;
            let texture = frame.scene.surfaces[0].texture.unwrap();
            frame.scene.images[texture].rgba[..4].try_into().unwrap()
        }
        fn png(path: &Path, width: u32, colour: [u8; 4]) {
            image::save_buffer_with_format(
                path,
                &colour.repeat(width as usize),
                width,
                1,
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            )
            .unwrap();
        }

        let root = std::env::temp_dir().join(format!(
            "gcad-live-document-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let part = root.join("part.gcad");
        let texture = root.join("colour.png");
        let path = root.join("model.gasset");
        std::fs::write(&part, "rect 20 10\nextrude 2\n").unwrap();
        png(&texture, 1, [255, 0, 0, 255]);
        let source = "asset 1\nproject .\nsource plate part.gcad body=main\ntessellate plate tolerance=0.1\nuv plate projection=box tile=10\nsurface plate base_colour=1,1,1,1\nmap plate base_colour colour.png license=CC0-1.0 origin=test\nnode plate source=plate\nclip resize interpolation=linear\nkey resize plate time=0 scale=1,1,1\nkey resize plate time=2 scale=2,2,2\n";
        let source = if unwrap {
            source.replace("projection=box tile=10", "projection=unwrap resolution=128 padding=4")
        } else { source.to_owned() };
        std::fs::write(&path, &source).unwrap();
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1000., 800.))
            .with_context_setup(egui_bench::install)
            .build_ui_state(
                |ui, document: &mut Option<Document>| {
                    let document = document.get_or_insert_with(|| {
                        let mut document = Document::open(path.clone(), vec![], ui.ctx().clone());
                        document.initial_pose(Some("resize".into()), 1.5);
                        document
                    });
                    document.ui(ui);
                },
                None,
            );
        wait(&mut harness, Document::ready);
        assert_eq!(pixel(harness.state().as_ref().unwrap()), [255, 0, 0, 255]);
        let original_uvs = harness.state().as_ref().unwrap().preview.as_ref().unwrap()
            .asset.scene.meshes[0].primitives[0].geometry.texcoords.clone();
        let camera = Camera {
            zoom: 1.7,
            shift: [2., 3., 0.],
            ..Camera::default()
        };
        harness
            .state_mut()
            .as_mut()
            .unwrap()
            .preview
            .as_mut()
            .unwrap()
            .camera = camera;

        // Only the texture changes: no asset-source save is needed.
        png(&texture, 2, [0, 255, 0, 255]);
        wait(&mut harness, |d| d.ready() && pixel(d) == [0, 255, 0, 255]);
        let preview = harness.state().as_ref().unwrap().preview.as_ref().unwrap();
        assert_eq!(preview.camera, camera);
        assert_eq!(preview.time, 1.5);
        assert_eq!(preview.asset.scene.meshes[0].primitives[0].geometry.texcoords, original_uvs);
        assert_eq!(
            preview.asset.scene.clips[preview.clip.unwrap()].name,
            "resize"
        );

        std::fs::write(&texture, b"incomplete PNG save").unwrap();
        wait(&mut harness, |d| d.error.is_some() && !d.loading);
        assert_eq!(pixel(harness.state().as_ref().unwrap()), [0, 255, 0, 255]);
        assert!(
            harness
                .query_by_label("Showing last valid model.")
                .is_some()
        );
        png(&texture, 3, [0, 0, 255, 255]);
        wait(&mut harness, |d| d.ready() && pixel(d) == [0, 0, 255, 255]);
        assert_eq!(
            harness
                .state()
                .as_ref()
                .unwrap()
                .preview
                .as_ref()
                .unwrap()
                .camera,
            camera
        );

        // Shortening an existing clip clamps its time; removing it returns to rest.
        std::fs::write(&path, source.replace("time=2", "time=1")).unwrap();
        wait(&mut harness, |d| {
            d.ready() && d.preview.as_ref().unwrap().time == 1.
        });
        let rest = source
            .lines()
            .filter(|line| !line.starts_with("clip ") && !line.starts_with("key "))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, rest).unwrap();
        wait(&mut harness, |d| {
            d.ready() && d.preview.as_ref().unwrap().clip.is_none()
        });
        let preview = harness.state().as_ref().unwrap().preview.as_ref().unwrap();
        assert_eq!(preview.time, 0.);
        assert!(!preview.playing);
        assert_eq!(preview.camera, camera);
        drop(harness);
        for file in [&part, &texture, &path] {
            std::fs::remove_file(file).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn mip_preview_converts_every_level_without_double_applying_factors() {
        use crate::{
            asset::{
                atlas,
                material::{Material, NormalConvention},
            },
            model::texture::Map,
        };
        let mesh = crate::asset::Mesh {
            positions: vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
            normals: vec![[0., 0., 1.]; 3],
            texcoords: Some(vec![[0., 0.], [1., 0.], [0., 1.]]),
            tangents: None,
            triangles: vec![[0, 1, 2]],
        };
        let atlas = atlas::pack(&mesh, 32, 16, 2, 4.).unwrap();
        let map = |pixel: [u8; 4]| {
            Some(Map {
                width: 1,
                height: 1,
                rgba: pixel.to_vec(),
            })
        };
        let material = Material {
            base_colour: [0.25, 0.5, 1., 0.75],
            roughness: 0.3,
            metallic: 0.6,
            normal_scale: 0.4,
            colour_map: map([255, 255, 255, 128]),
            normal_map: map([128, 64, 255, 255]),
            roughness_map: map([64; 4]),
            metallic_map: map([192; 4]),
            normal_convention: NormalConvention::NegativeY,
            ..Material::default()
        };
        let original = material.clone();
        let chain = atlas.rebake_chain(&material, 3).unwrap();
        let before = chain.levels().to_vec();
        let prepared = mip_images(&chain).unwrap();
        assert!(prepared[..3].iter().all(|slot| slot.len() == 4));
        assert!(prepared[3..].iter().all(Vec::is_empty));
        for level in 0..4 {
            assert_eq!(
                (prepared[0][level].width, prepared[0][level].height),
                (32 >> level, 16 >> level)
            );
            let mut covered = 0;
            for (i, pixel) in prepared[0][level].rgba.chunks_exact(4).enumerate() {
                if pixel[3] == 0 {
                    continue;
                }
                covered += 1;
                assert_eq!(pixel, [137, 188, 255, 128]);
                assert_eq!(
                    &prepared[1][level].rgba[4 * i..4 * i + 4],
                    &[128, 191, 255, 255]
                );
                assert_eq!(
                    &prepared[2][level].rgba[4 * i..4 * i + 4],
                    &[255, 64, 192, 255]
                );
            }
            assert!(covered > 0);
        }
        assert_eq!(chain.levels(), before);
        assert_eq!(material, original);
        let empty = atlas.rebake_chain(&Material::default(), 3).unwrap();
        assert!(mip_images(&empty).unwrap().iter().all(Vec::is_empty));
    }
    #[test]
    fn textured_preview_tints_copies_and_connects_portable_numeric_maps() {
        use crate::asset::material::NormalConvention;
        use crate::model::texture::Map;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut asset =
            crate::asset::author::load(&root.join("examples/game-assets/plate.gasset"), &[])
                .unwrap();
        let material = &mut asset.scene.materials[0];
        material.base_colour = [0.25, 0.5, 1., 1.];
        material.colour_map = Some(Map {
            width: 2,
            height: 2,
            rgba: vec![
                255, 255, 255, 255, 20, 40, 80, 255, 20, 40, 80, 255, 255, 255, 255, 255,
            ],
        });
        material.normal_map = Some(Map {
            width: 2,
            height: 2,
            rgba: [128, 55, 240, 255].repeat(4),
        });
        material.normal_convention = NormalConvention::NegativeY;
        material.roughness_map = Some(Map {
            width: 2,
            height: 2,
            rgba: [128, 0, 0, 255].repeat(4),
        });
        material.metallic_map = Some(Map {
            width: 2,
            height: 2,
            rgba: [64, 0, 0, 255].repeat(4),
        });
        let original = material.clone();
        let frame = frame(&asset, None, 0.).unwrap();
        let surface = &frame.scene.surfaces[0];
        let colour = &frame.scene.images[surface.texture.unwrap()];
        assert_eq!(&colour.rgba[..4], &[137, 188, 255, 255]);
        assert_eq!(
            &frame.scene.images[surface.normal_map.unwrap()].rgba[..4],
            &[128, 200, 240, 255]
        );
        assert_eq!(
            &frame.scene.images[surface.metal_rough_map.unwrap()].rgba[..4],
            &[255, 128, 64, 255]
        );
        assert_eq!(surface.uvs.len(), surface.positions.len());
        assert_eq!(asset.scene.materials[0], original);
        snapshot(
            &asset,
            None,
            0.,
            &root.join("target/game-asset-textured-preview.png"),
        )
        .unwrap();
        asset.scene.materials[0].occlusion_map = Some(Map {
            width: 1,
            height: 1,
            rgba: vec![255; 4],
        });
        assert!(!differences(&asset).iter().any(|s| s.contains("occlusion")));
    }
    #[test]
    fn refit_uses_changed_geometry_bounds_without_resetting_ordinary_updates() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let asset =
            crate::asset::author::load(&root.join("examples/game-assets/plate-scale.gasset"), &[])
                .unwrap();
        let clip = asset
            .scene
            .clips
            .iter()
            .position(|c| c.name == "resize")
            .unwrap();
        let mut current = frame(&asset, Some(clip), 0.).unwrap();
        let initial_radius = current.scene.radius;
        let mut camera = Camera {
            zoom: 2.,
            shift: [10., 20., 0.],
            ..Camera::default()
        };
        let saved_camera = camera;
        let mut orbit = Orbit::default();
        let next = frame(&asset, Some(clip), 2.).unwrap();
        let enlarged_radius = next.scene.radius;
        assert!(enlarged_radius > initial_radius * 1.5);
        replace_frame(&mut current, next, &mut camera, &mut orbit, false);
        assert_eq!(current.scene.radius, initial_radius);
        assert_eq!(camera, saved_camera);
        let next = frame(&asset, Some(clip), 2.).unwrap();
        let centre = next.scene.centre;
        replace_frame(&mut current, next, &mut camera, &mut orbit, true);
        assert_eq!(current.scene.radius, enlarged_radius);
        assert_eq!(current.scene.centre, centre);
        assert_eq!(camera, Camera::default());
    }

    #[test]
    fn preview_preserves_instances_and_uses_sampled_clip_positions() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let asset = crate::asset::author::load(
            &root.join("examples/game-assets/plate-instances.gasset"),
            &[],
        )
        .unwrap();
        let frame = frame(&asset, None, 0.).unwrap();
        let stored: usize = asset
            .scene
            .meshes
            .iter()
            .flat_map(|m| &m.primitives)
            .map(|p| p.geometry.triangles.len())
            .sum();
        assert_eq!(frame.scene.triangles(), stored * 2);
        assert_eq!(frame.looks.len(), 2);
        let asset =
            crate::asset::author::load(&root.join("examples/game-assets/plate-motion.gasset"), &[])
                .unwrap();
        let clip = asset
            .scene
            .clips
            .iter()
            .position(|c| c.name == "slide")
            .unwrap();
        let first = super::frame(&asset, Some(clip), 0.).unwrap();
        let last = super::frame(&asset, Some(clip), 2.).unwrap();
        for (a, b) in first.scene.surfaces[0]
            .positions
            .iter()
            .zip(&last.scene.surfaces[0].positions)
        {
            assert!((b[0] - a[0] - 100.).abs() < 1e-4);
            assert!((b[1] - a[1]).abs() < 1e-4 && (b[2] - a[2]).abs() < 1e-4);
        }
        assert!(super::frame(&asset, Some(999), 0.).is_err());
    }
}
