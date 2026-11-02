use super::*;
use eframe::egui_wgpu;

pub(super) fn requested_render_state() -> egui_wgpu::RenderState {
    let (backends, expected) = match std::env::var("GCAD_RENDER_BACKEND").as_deref() {
        Ok("vulkan") => (wgpu::Backends::VULKAN, wgpu::Backend::Vulkan),
        Ok("dx12") | Err(std::env::VarError::NotPresent) => {
            (wgpu::Backends::DX12, wgpu::Backend::Dx12)
        }
        other => panic!("unsupported GCAD_RENDER_BACKEND: {other:?}"),
    };
    let egui_wgpu::WgpuSetup::CreateNew(mut setup) = egui_kittest::wgpu::default_wgpu_setup()
    else {
        panic!("expected a new headless renderer");
    };
    setup.instance_descriptor.backends = backends;
    let state = egui_kittest::wgpu::create_render_state(
        egui_wgpu::WgpuSetup::CreateNew(setup),
        egui_wgpu::RendererOptions::PREDICTABLE,
    );
    assert_eq!(
        state.adapter.get_info().backend,
        expected,
        "backend fallback is forbidden"
    );
    println!("Verified requested backend: {expected:?}");
    state
}

fn authored() -> Asset {
    crate::asset::author::load(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/game-assets/atlas-mips.gasset"),
        &[],
    )
    .unwrap()
}

#[test]
fn gcad_preview_explicit_textured_animation_matches_analytic_poses() {
    check_textured_animation(false);
}

#[test]
fn gcad_preview_explicit_textured_skin_matches_analytic_blends() {
    check_textured_animation(true);
}

fn check_textured_animation(skinned: bool) {
    let mut asset = crate::asset::author::load(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/game-assets/atlas-extra-motion.gasset"),
        &[],
    )
    .unwrap();
    if skinned {
        use crate::asset::{
            animation::{Clip, Interpolation, Track, Values},
            scene::Node,
            skin::{Skin, Weights},
        };
        let joint0 = asset.scene.nodes.len();
        for (name, translation) in [
            ("fixed-joint", [-1., 1., 0.]),
            ("moving-joint", [1., 0., 2.]),
        ] {
            asset.scene.nodes.push(Node {
                name: name.into(),
                parent: None,
                mesh: None,
                transform: Transform {
                    translation,
                    ..Default::default()
                },
            });
        }
        let track = |values| Track {
            node: joint0 + 1,
            times: vec![0., 1.],
            interpolation: Interpolation::Linear,
            values,
        };
        asset.scene.clips = vec![Clip {
            name: "blended".into(),
            tracks: vec![
                track(Values::Translation(vec![[1., 0., 2.], [1.5, -0.25, 2.75]])),
                track(Values::Rotation(vec![
                    [0., 0., 0., 1.],
                    [0., 0., 0.5, 3f64.sqrt() / 2.],
                ])),
                track(Values::Scale(vec![[1.; 3], [1.2, 0.8, 1.1]])),
            ],
        }];
        let weights = asset.scene.meshes[asset.scene.nodes[0].mesh.unwrap()]
            .primitives
            .iter()
            .map(|p| {
                p.geometry
                    .positions
                    .iter()
                    .map(|point| {
                        let w = 0.25 + point[2] * 0.125;
                        assert!((0.25..=0.75).contains(&w));
                        Weights {
                            joints: [0, 1, 0, 0],
                            weights: [1. - w, w, 0., 0.],
                        }
                    })
                    .collect()
            })
            .collect();
        let skin = Skin::bind(&asset.scene, 0, vec![joint0, joint0 + 1], weights).unwrap();
        assert!(skin.inverse_bind.iter().all(|m| m.w.x != 0.));
        asset.skins.push(skin);
    }
    let rest = frame(&asset, None, 0.).unwrap();
    assert_eq!(rest.scene.surfaces.len(), 2);
    assert_eq!(rest.scene.explicit_images.len(), 5);
    for (slot, image) in &rest.scene.explicit_images {
        let unique: std::collections::HashSet<_> = image.levels()[0].rgba.chunks_exact(4).collect();
        println!(
            "Animated map slot {slot}: {} distinct RGBA values",
            unique.len()
        );
        assert!(unique.len() > 32, "fixture map lacks spatial variation");
    }
    let state = requested_render_state();
    viewer3d::install(&state);
    let mut maximum = 0;
    let mut compared = 0;
    let mut texture_changes = [0; 5];
    let mut motion_changes = 0;
    let mut previous: Option<egui::ColorImage> = None;
    for time in [0., 0.25, 0.5, 0.75, 1.] {
        let mut actual = frame(&asset, Some(0), time).unwrap();
        let mut reference = frame(&asset, None, 0.).unwrap();
        let reference_scene = Arc::get_mut(&mut reference.scene).unwrap();
        // Independent closed-form TRS, without clip sampling or scene matrices.
        let angle = if skinned {
            std::f64::consts::FRAC_PI_3
        } else {
            std::f64::consts::FRAC_PI_2
        };
        let (sin, cos) = (angle * time).sin_cos();
        let scale = [1. + 0.2 * time, 1. - 0.2 * time, 1. + 0.1 * time];
        for point in &mut reference_scene.surfaces[0].positions {
            if skinned {
                let p = point.map(f64::from);
                let w = 0.25 + p[2] * 0.125;
                let [x, y, z] = [
                    (p[0] - 1.) * scale[0],
                    p[1] * scale[1],
                    (p[2] - 2.) * scale[2],
                ];
                let moved = [
                    cos * x - sin * y + 1. + 0.5 * time,
                    sin * x + cos * y - 0.25 * time,
                    z + 2. + 0.75 * time,
                ];
                *point = std::array::from_fn(|i| ((1. - w) * p[i] + w * moved[i]) as f32);
                continue;
            }
            let [x, y, z] = std::array::from_fn(|i| point[i] as f64 * scale[i]);
            *point = [
                (cos * x - sin * y + 2. * time) as f32,
                (sin * x + cos * y + 3. * time) as f32,
                (z + time) as f32,
            ];
        }
        for (index, normal) in reference_scene.surfaces[0].normals.iter_mut().enumerate() {
            if skinned {
                let w = 0.25 + rest.scene.surfaces[0].positions[index][2] as f64 * 0.125;
                // Inverse transpose of (1-w)I + w Rz*S, computed as scalars.
                let a = 1. - w + w * cos * scale[0];
                let b = -w * sin * scale[1];
                let c = w * sin * scale[0];
                let d = 1. - w + w * cos * scale[1];
                let e = 1. - w + w * scale[2];
                let det = a * d - b * c;
                let [nx, ny, nz] = normal.map(f64::from);
                let n = [(d * nx - c * ny) / det, (-b * nx + a * ny) / det, nz / e];
                let length = n[0].hypot(n[1]).hypot(n[2]);
                *normal = n.map(|n| (n / length) as f32);
                continue;
            }
            let [x, y, z] = std::array::from_fn(|i| normal[i] as f64 / scale[i]);
            let length = x.hypot(y).hypot(z);
            *normal = [
                ((cos * x - sin * y) / length) as f32,
                ((sin * x + cos * y) / length) as f32,
                (z / length) as f32,
            ];
        }
        for (a, b) in actual.scene.surfaces.iter().zip(&reference_scene.surfaces) {
            for (a, b) in a
                .positions
                .iter()
                .flatten()
                .zip(b.positions.iter().flatten())
            {
                assert!((a - b).abs() < 0.000002, "analytic position mismatch");
            }
            for (a, b) in a.normals.iter().flatten().zip(b.normals.iter().flatten()) {
                assert!((a - b).abs() < 0.000002, "analytic normal mismatch");
            }
            assert_eq!(a.uvs, b.uvs);
            assert_eq!(a.material.0, b.material.0);
            assert_eq!(a.material.pbr().occlusion_strength, 0.4);
            assert_eq!(a.material.pbr().emission, [0.3, 0.2, 0.1]);
            assert_eq!(
                (
                    a.texture,
                    a.normal_map,
                    a.metal_rough_map,
                    a.occlusion_map,
                    a.emission_map
                ),
                (Some(0), Some(1), Some(2), Some(3), Some(4))
            );
        }
        assert_eq!(
            actual.scene.surfaces[1].positions,
            rest.scene.surfaces[1].positions
        );
        for (slot, image) in &actual.scene.explicit_images {
            for (a, b) in image
                .levels()
                .iter()
                .zip(rest.scene.explicit_images[slot].levels())
            {
                assert_eq!(a.rgba, b.rgba);
            }
        }
        // Match interactive playback's fixed bounds, with room for the motion.
        let actual_scene = Arc::get_mut(&mut actual.scene).unwrap();
        actual_scene.centre = rest.scene.centre;
        actual_scene.radius = rest.scene.radius * 1.4;
        reference_scene.centre = actual_scene.centre;
        reference_scene.radius = actual_scene.radius;
        let mut controls = Vec::new();
        for slot in 0..5 {
            let mut control = frame(&asset, Some(0), time).unwrap();
            let control_scene = Arc::get_mut(&mut control.scene).unwrap();
            control_scene.centre = actual_scene.centre;
            control_scene.radius = actual_scene.radius;
            let mut levels = control_scene.explicit_images[&slot].levels().to_vec();
            for image in &mut levels {
                for pixel in image.rgba.chunks_exact_mut(4) {
                    pixel[..3].copy_from_slice(
                        &[[137; 3], [128, 128, 255], [255, 255, 0], [255; 3], [255; 3]][slot],
                    );
                }
            }
            control_scene.images[slot] = levels[0].clone();
            control_scene.explicit_images.insert(
                slot,
                viewer3d::ExplicitImage::new(
                    levels,
                    if slot == 0 || slot == 4 {
                        viewer3d::ImageEncoding::Srgb
                    } else {
                        viewer3d::ImageEncoding::Linear
                    },
                )
                .unwrap(),
            );
            controls.push(control);
        }
        let viewer = |frame: Frame| {
            Viewer::new(frame.scene, Camera::default())
                .looks(frame.looks)
                .edges(false)
        };
        let actual = viewer(actual);
        let reference = viewer(reference);
        let controls: Vec<_> = controls.into_iter().map(viewer).collect();
        for size in [128, 32] {
            let a = actual
                .render_gpu(size, size)
                .expect("animation requires GPU");
            let b = reference
                .render_gpu(size, size)
                .expect("analytic reference requires GPU");
            for (slot, control) in controls.iter().enumerate() {
                let c = control
                    .render_gpu(size, size)
                    .expect("texture control requires GPU");
                assert!(
                    a.pixels.iter().any(|p| *p != a.pixels[0]),
                    "empty animated image"
                );
                let changed = a
                    .pixels
                    .iter()
                    .zip(&c.pixels)
                    .filter(|(a, b)| a != b)
                    .count();
                assert!(changed > 0, "map slot {slot} has no visible effect");
                texture_changes[slot] += changed;
            }
            if size == 128 {
                if let Some(previous) = &previous {
                    let changed = a
                        .pixels
                        .iter()
                        .zip(&previous.pixels)
                        .filter(|(a, b)| a != b)
                        .count();
                    assert!(changed > 0, "animation did not affect pixels");
                    motion_changes += changed;
                }
                previous = Some(a.clone());
            }
            for (a, b) in a.pixels.iter().zip(&b.pixels) {
                for (a, b) in a.to_array().into_iter().zip(b.to_array()) {
                    maximum = maximum.max(a.abs_diff(b));
                }
                compared += 1;
            }
        }
    }
    assert!(
        maximum <= 2,
        "animated appearance maximum byte error {maximum}"
    );
    let label = if skinned {
        "Textured skin"
    } else {
        "Textured animation"
    };
    println!(
        "{label}: {compared} pixels across five poses, maximum byte error {maximum}; map controls {texture_changes:?}, motion control {motion_changes}; backend {:?}",
        state.adapter.get_info().backend
    );
}

#[test]
fn gcad_preview_explicit_preserves_authored_levels_and_instances() {
    let asset = authored();
    let chain = asset.scene.materials[0].mip_chain.as_ref().unwrap().clone();
    let prepared = mip_images(&chain).unwrap();
    let actual = frame(&asset, None, 0.).unwrap();
    assert_eq!(actual.scene.surfaces.len(), 2);
    assert_eq!(actual.scene.images.len(), 5);
    assert_eq!(actual.scene.explicit_images.len(), 5);
    let start = std::time::Instant::now();
    for _ in 0..2000 {
        std::hint::black_box(&actual.scene)
            .validate_explicit_images()
            .unwrap();
    }
    println!(
        "Explicit validation timing: 2000 checks of 327680 base bytes in {:?}",
        start.elapsed()
    );
    // A larger diagnostic exposes byte-scan scaling without timing assertions.
    let mut large: Scene = Scene::default();
    let image = viewer3d::Image {
        width: 2048,
        height: 2048,
        rgba: vec![128; 2048 * 2048 * 4],
    };
    large.images.push(image.clone());
    large.explicit_images.insert(
        0,
        viewer3d::ExplicitImage::new(vec![image], viewer3d::ImageEncoding::Srgb).unwrap(),
    );
    large.surfaces.push(Surface {
        texture: Some(0),
        ..Default::default()
    });
    let start = std::time::Instant::now();
    for _ in 0..200 {
        std::hint::black_box(&large)
            .validate_explicit_images()
            .unwrap();
    }
    println!(
        "Explicit validation timing: 200 checks of 16777216 base bytes in {:?}",
        start.elapsed()
    );
    *large.images[0].rgba.last_mut().unwrap() ^= 1;
    assert!(large.validate_explicit_images().is_err());
    for surface in &actual.scene.surfaces {
        assert_eq!(
            (
                surface.texture,
                surface.normal_map,
                surface.metal_rough_map,
                surface.occlusion_map,
                surface.emission_map
            ),
            (Some(0), Some(1), Some(2), Some(3), Some(4))
        );
    }
    for (slot, expected) in prepared.iter().enumerate() {
        let image = &actual.scene.explicit_images[&slot];
        assert_eq!(image.max_lod(), 2);
        assert_eq!(
            image.encoding(),
            if slot == 0 || slot == 4 {
                viewer3d::ImageEncoding::Srgb
            } else {
                viewer3d::ImageEncoding::Linear
            }
        );
        assert_eq!(image.levels().len(), expected.len());
        for (a, b) in image.levels().iter().zip(expected) {
            assert_eq!((a.width, a.height), (b.width, b.height));
            assert_eq!(a.rgba, b.rgba);
        }
    }
    assert!(Arc::ptr_eq(
        &chain,
        asset.scene.materials[0].mip_chain.as_ref().unwrap()
    ));
    asset.scene.validate().unwrap();
    let warnings = differences(&asset);
    assert!(!warnings.iter().any(|s| s.contains("custom sampler")));
    assert!(!warnings.iter().any(|s| s.contains("texture alpha")));
    let path =
        std::env::temp_dir().join(format!("gcad-explicit-snapshot-{}.png", std::process::id()));
    assert!(!path.exists());
    let error = snapshot(&asset, None, 0., &path).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("software snapshot does not support")
    );
    assert!(!path.exists());
    println!(
        "Authored adapter: two instances share five maps with three exact levels; snapshot refusal leaves no output"
    );
}

#[test]
fn gcad_preview_explicit_authored_gpu_matches_constant_material() {
    let asset = authored();
    let actual = frame(&asset, None, 0.).unwrap();
    let mut reference = frame(&asset, None, 0.).unwrap();
    let mut control = frame(&asset, None, 0.).unwrap();
    let control_scene = Arc::get_mut(&mut control.scene).unwrap();
    let mut levels = control_scene.explicit_images[&0].levels().to_vec();
    for level in &mut levels[1..] {
        for pixel in level.rgba.chunks_exact_mut(4) {
            pixel[..3].copy_from_slice(&[0, 255, 0]);
        }
    }
    control_scene.explicit_images.insert(
        0,
        viewer3d::ExplicitImage::new(levels, viewer3d::ImageEncoding::Srgb).unwrap(),
    );
    control_scene.validate_explicit_images().unwrap();
    let scene = Arc::get_mut(&mut reference.scene).unwrap();
    scene.explicit_images.clear();
    // Analytic 25% white stripes: linear-light colour = 137, numeric = 64.
    // All chart interiors have this average; unused atlas pixels must never leak.
    scene.images = [
        [137, 137, 137, 64],
        [128, 128, 255, 255],
        [255, 64, 64, 255],
        [64; 4],
        [137, 137, 137, 64],
    ]
    .into_iter()
    .map(|rgba| viewer3d::Image {
        width: 1,
        height: 1,
        rgba: rgba.to_vec(),
    })
    .collect();
    let state = requested_render_state();
    viewer3d::install(&state);
    // The fixture has uniform emission=1. Avoid saturation masking texture errors.
    let lighting = Lighting {
        exposure: 0.2,
        ..Lighting::STUDIO
    };
    let actual = Viewer::new(actual.scene, Camera::default())
        .looks(actual.looks)
        .lighting(lighting)
        .edges(false);
    let expected = Viewer::new(reference.scene, Camera::default())
        .looks(reference.looks)
        .lighting(lighting)
        .edges(false);
    let control = Viewer::new(control.scene, Camera::default())
        .looks(control.looks)
        .lighting(lighting)
        .edges(false);
    let mut maximum = 0;
    let mut compared = 0;
    let mut changed = 0;
    for size in [128, 32, 8] {
        let a = actual
            .render_gpu(size, size)
            .expect("authored adapter requires GPU");
        let b = expected
            .render_gpu(size, size)
            .expect("reference requires GPU");
        let c = control
            .render_gpu(size, size)
            .expect("control requires GPU");
        changed += a
            .pixels
            .iter()
            .zip(&c.pixels)
            .filter(|(x, y)| x != y)
            .count();
        assert!(
            b.pixels.iter().any(|p| *p != b.pixels[0]),
            "empty reference"
        );
        for (x, y) in a.pixels.iter().zip(&b.pixels) {
            for (x, y) in x.to_array().into_iter().zip(y.to_array()) {
                maximum = maximum.max(x.abs_diff(y));
            }
            compared += 1;
        }
    }
    assert!(
        maximum <= 2,
        "authored adapter maximum byte error {maximum}"
    );
    assert!(
        changed > 0,
        "supplied lower levels did not affect the authored render"
    );
    println!("Authored supplied-mip control: {changed} changed pixels");
    println!(
        "Authored adapter GPU: {compared} pixels, maximum byte error {maximum}, backend {:?}",
        state.adapter.get_info().backend
    );
}
