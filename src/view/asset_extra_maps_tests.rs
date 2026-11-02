use super::*;

#[test]
fn gcad_preview_extra_maps_authored_payloads_budget_and_gpu() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/game-assets/atlas-extra-maps.gasset");
    let state = super::explicit_tests::requested_render_state();
    viewer3d::install(&state);
    for explicit in [false, true] {
        let mut asset = crate::asset::author::load(&path, &[]).unwrap();
        if !explicit {
            for m in &mut asset.scene.materials {
                m.mip_chain = None;
            }
        }
        let before = asset.scene.materials.clone();
        let actual = frame(&asset, None, 0.).unwrap();
        assert_eq!(actual.scene.images.len(), 5);
        assert_eq!(actual.scene.surfaces.len(), 2);
        for surface in &actual.scene.surfaces {
            assert_eq!(
                (surface.occlusion_map, surface.emission_map),
                (Some(3), Some(4))
            );
            assert_eq!(surface.material.pbr().occlusion_strength, 0.4);
            assert_eq!(surface.material.pbr().emission, [0.3, 0.2, 0.1]);
        }
        let material = &asset.scene.materials[0];
        for (slot, source) in [
            (3, material.occlusion_map.as_ref().unwrap()),
            (4, material.emissive_map.as_ref().unwrap()),
        ] {
            assert_eq!(actual.scene.images[slot].rgba, source.rgba);
            let unique: std::collections::HashSet<_> = source.rgba.chunks_exact(4).collect();
            assert!(unique.len() > 32, "extra map lacks spatial variation");
            if explicit {
                let image = &actual.scene.explicit_images[&slot];
                assert_eq!(
                    image.encoding(),
                    if slot == 3 {
                        viewer3d::ImageEncoding::Linear
                    } else {
                        viewer3d::ImageEncoding::Srgb
                    }
                );
                let levels = material.mip_chain.as_ref().unwrap().levels();
                assert_eq!(image.levels().len(), levels.len());
                for (image, m) in image.levels().iter().zip(levels) {
                    let source = if slot == 3 {
                        m.occlusion_map.as_ref()
                    } else {
                        m.emissive_map.as_ref()
                    }
                    .unwrap();
                    assert_eq!(image.rgba, source.rgba);
                }
            }
        }
        let usage =
            image_budget::usage(&asset.scene.materials, image_budget::ImageLimits::default())
                .unwrap();
        assert_eq!(usage.images, 5);
        let retained: usize = actual
            .scene
            .images
            .iter()
            .map(|m| m.rgba.len())
            .sum::<usize>()
            + actual
                .scene
                .explicit_images
                .values()
                .flat_map(|i| i.levels())
                .map(|m| m.rgba.len())
                .sum::<usize>();
        assert_eq!(usage.retained_bytes, retained);
        let exact = image_budget::ImageLimits {
            images: 5,
            cpu_bytes: usage.retained_bytes + usage.upload_scratch_bytes,
            gpu_bytes: usage.gpu_bytes,
        };
        assert!(image_budget::usage(&asset.scene.materials, exact).is_ok());
        for limits in [
            image_budget::ImageLimits { images: 4, ..exact },
            image_budget::ImageLimits {
                cpu_bytes: exact.cpu_bytes - 1,
                ..exact
            },
            image_budget::ImageLimits {
                gpu_bytes: exact.gpu_bytes - 1,
                ..exact
            },
        ] {
            assert!(image_budget::usage(&asset.scene.materials, limits).is_err());
        }
        let mut controls = Vec::new();
        for slot in [3, 4] {
            let mut control = frame(&asset, None, 0.).unwrap();
            for s in &mut Arc::get_mut(&mut control.scene).unwrap().surfaces {
                if slot == 3 {
                    s.occlusion_map = None;
                } else {
                    s.emission_map = None;
                }
            }
            controls.push(control);
        }
        let render = |f: &Frame, size| {
            Viewer::new(f.scene.clone(), Camera::default())
                .looks(f.looks.clone())
                .edges(false)
                .render_gpu(size, size)
                .unwrap()
        };
        let mut changed = [0; 2];
        for size in [128, 32] {
            let a = render(&actual, size);
            for (slot, control) in controls.iter().enumerate() {
                let b = render(control, size);
                let changes = a
                    .pixels
                    .iter()
                    .zip(&b.pixels)
                    .filter(|(a, b)| a != b)
                    .count();
                assert!(changes > 0, "extra-map control ineffective at size {size}");
                changed[slot] += changes;
            }
        }
        let output = std::env::temp_dir().join(format!(
            "gcad-extra-map-refusal-{}-{explicit}.png",
            std::process::id()
        ));
        assert!(!output.exists());
        assert!(snapshot(&asset, None, 0., &output).is_err());
        assert!(!output.exists());
        assert!(
            !differences(&asset)
                .iter()
                .any(|s| s.contains("occlusion and emissive"))
        );
        assert_eq!(asset.scene.materials, before);
        println!(
            "Authored extra maps explicit {explicit}: five shared slots, exact payloads, budget CPU {} GPU {}, controls {changed:?}, software refusal; backend {:?}",
            usage.retained_bytes + usage.upload_scratch_bytes,
            usage.gpu_bytes,
            state.adapter.get_info().backend
        );
    }
}
