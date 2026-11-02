use super::*;

// Box average the binary high-resolution occupancy, keeping fractional values
// instead of thresholding them back to a low-resolution silhouette.
fn projected_mask_average(image: &egui::ColorImage, size: usize, scale: usize) -> Vec<f64> {
    assert!(scale > 0);
    assert_eq!(image.size, [size * scale; 2]);
    let mut result = vec![0.; size * size];
    for (index, pixel) in image.pixels.iter().enumerate() {
        if pixel.r().max(pixel.g()).max(pixel.b()) > 3 {
            let x = index % image.size[0] / scale;
            let y = index / image.size[0] / scale;
            result[y * size + x] += 1. / (scale * scale) as f64;
        }
    }
    result
}

#[test]
fn gcad_preview_mask_mips_characterize_projected_occupancy() {
    let black = egui::Color32::BLACK;
    let white = egui::Color32::WHITE;
    let mapping = egui::ColorImage::new(
        [4, 4],
        vec![
            white, black, white, white, black, black, white, white, white, black, black, black,
            white, black, black, black,
        ],
    );
    assert_eq!(projected_mask_average(&mapping, 2, 2), [0.25, 1., 0.5, 0.]);
    let state = super::explicit_tests::requested_render_state();
    viewer3d::install(&state);
    let load = |name| {
        crate::asset::author::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples/game-assets")
                .join(name),
            &[],
        )
        .unwrap()
    };
    let corrected = load("atlas-mask-coverage.gasset");
    let raw = load("atlas-mask-raw.gasset");
    let a = frame(&corrected, None, 0.).unwrap();
    let b = frame(&raw, None, 0.).unwrap();
    assert_eq!(a.scene.surfaces.len(), b.scene.surfaces.len());
    for (a, b) in a.scene.surfaces.iter().zip(&b.scene.surfaces) {
        assert_eq!(a.positions, b.positions);
        assert_eq!(a.normals, b.normals);
        assert_eq!(a.uvs, b.uvs);
        assert_eq!(a.material.0, b.material.0);
    }
    let mut changed_alpha = 0;
    assert_eq!(a.scene.explicit_images.len(), b.scene.explicit_images.len());
    for (slot, a) in &a.scene.explicit_images {
        let b = &b.scene.explicit_images[slot];
        assert_eq!(a.encoding(), b.encoding());
        assert_eq!(a.levels().len(), b.levels().len());
        for (level, (a, b)) in a.levels().iter().zip(b.levels()).enumerate() {
            assert_eq!((a.width, a.height), (b.width, b.height));
            assert_eq!(a.rgba.len(), b.rgba.len());
            for (i, (&a, &b)) in a.rgba.iter().zip(&b.rgba).enumerate() {
                if *slot == 0 && level > 0 && i % 4 == 3 {
                    changed_alpha += usize::from(a != b);
                } else {
                    assert_eq!(a, b, "only lower colour alpha may change");
                }
            }
        }
    }
    assert!(changed_alpha > 0);
    // Independent frame calls give each payload a fresh renderer cache key.
    let mut base = frame(&raw, None, 0.).unwrap();
    for image in Arc::get_mut(&mut base.scene)
        .unwrap()
        .explicit_images
        .values_mut()
    {
        let first = &image.levels()[0];
        *image = viewer3d::ExplicitImage::new(
            vec![viewer3d::Image {
                width: first.width,
                height: first.height,
                rgba: first.rgba.clone(),
            }],
            image.encoding(),
        )
        .unwrap();
    }
    let mut poison = frame(&raw, None, 0.).unwrap();
    let scene = Arc::get_mut(&mut poison.scene).unwrap();
    let image = &scene.explicit_images[&0];
    let levels = image
        .levels()
        .iter()
        .enumerate()
        .map(|(level, image)| {
            let mut rgba = image.rgba.clone();
            if level > 0 {
                for pixel in rgba.chunks_exact_mut(4) {
                    pixel[3] = 0;
                }
            }
            viewer3d::Image {
                width: image.width,
                height: image.height,
                rgba,
            }
        })
        .collect();
    scene.explicit_images.insert(
        0,
        viewer3d::ExplicitImage::new(levels, viewer3d::ImageEncoding::Srgb).unwrap(),
    );
    let mut empty = frame(&raw, None, 0.).unwrap();
    let scene = Arc::get_mut(&mut empty.scene).unwrap();
    scene.surfaces.clear();
    scene.lines.clear();
    let frames = [base, b, a, poison, empty];
    let mut cases = 0;
    let mut improvements = 0;
    let mut regressions = 0;
    let mut correction_changes = 0;
    let mut lower_changes = 0;
    let mut errors = [0usize; 2];
    let mut disagreements = [0usize; 2];
    let mut super_errors = [[0f64; 3]; 2];
    let mut super_spatial = [[0f64; 3]; 2];
    let mut reference_drift = [0f64; 2];
    let mut super_outcomes = [[0usize; 3]; 2];
    for yaw in [0.6, -0.6, 2.4] {
        for zoom in [1., 1.7] {
            for size in [128, 64, 32, 16] {
                let images: Vec<_> = frames
                    .iter()
                    .map(|f| {
                        Viewer::new(
                            f.scene.clone(),
                            Camera {
                                yaw,
                                zoom,
                                ..Default::default()
                            },
                        )
                        .looks(f.looks.clone())
                        .edges(false)
                        .style(viewer3d::Style {
                            background: egui::Color32::BLACK,
                            ..Default::default()
                        })
                        .render_gpu(size, size)
                        .expect("mask characterization requires GPU")
                    })
                    .collect();
                assert!(images[4].pixels.iter().all(|p| *p == egui::Color32::BLACK));
                // Readback discards alpha. This contrast test counts occupied pixels,
                // including partial MSAA coverage, not fractional surface area.
                let masks: Vec<Vec<bool>> = images[..4]
                    .iter()
                    .map(|image| {
                        image
                            .pixels
                            .iter()
                            .map(|p| p.r().max(p.g()).max(p.b()) > 3)
                            .collect()
                    })
                    .collect();
                let counts: Vec<_> = masks
                    .iter()
                    .map(|m| m.iter().filter(|&&p| p).count())
                    .collect();
                assert!(counts[0] > 0, "empty base-only reference");
                let error = [counts[1].abs_diff(counts[0]), counts[2].abs_diff(counts[0])];
                let mismatch: Vec<_> = masks[1..3]
                    .iter()
                    .map(|m| m.iter().zip(&masks[0]).filter(|(a, b)| a != b).count())
                    .collect();
                improvements += usize::from(error[1] < error[0]);
                regressions += usize::from(error[1] > error[0]);
                for i in 0..2 {
                    errors[i] += error[i];
                    disagreements[i] += mismatch[i];
                }
                correction_changes += masks[1]
                    .iter()
                    .zip(&masks[2])
                    .filter(|(a, b)| a != b)
                    .count();
                lower_changes += masks[1]
                    .iter()
                    .zip(&masks[3])
                    .filter(|(a, b)| a != b)
                    .count();
                cases += 1;
                println!(
                    "Mask occupancy size={size} yaw={yaw} zoom={zoom}: base/raw/corrected/poison {counts:?}, absolute errors {error:?}, disagreements {mismatch:?}"
                );
                let references: Vec<_> = [4, 8]
                    .into_iter()
                    .map(|scale| {
                        let f = &frames[0];
                        let image = Viewer::new(
                            f.scene.clone(),
                            Camera {
                                yaw,
                                zoom,
                                ..Default::default()
                            },
                        )
                        .looks(f.looks.clone())
                        .edges(false)
                        .style(viewer3d::Style {
                            background: black,
                            ..Default::default()
                        })
                        .render_gpu(size * scale, size * scale)
                        .expect("supersampled base-only reference requires GPU");
                        projected_mask_average(&image, size, scale)
                    })
                    .collect();
                let totals: Vec<f64> = references.iter().map(|r| r.iter().sum()).collect();
                assert!(
                    totals
                        .iter()
                        .all(|&total| total > 0. && total < (size * size) as f64)
                );
                let drift = [
                    (totals[0] - totals[1]).abs(),
                    references[0]
                        .iter()
                        .zip(&references[1])
                        .map(|(a, b)| (a - b).abs())
                        .sum(),
                ];
                for i in 0..2 {
                    reference_drift[i] += drift[i];
                }
                for (r, reference) in references.iter().enumerate() {
                    let area_errors: [f64; 3] =
                        std::array::from_fn(|i| (counts[i] as f64 - totals[r]).abs());
                    let spatial: [f64; 3] = std::array::from_fn(|i| {
                        masks[i]
                            .iter()
                            .zip(reference)
                            .map(|(&occupied, fraction)| {
                                (f64::from(u8::from(occupied)) - fraction).abs()
                            })
                            .sum()
                    });
                    let outcome = if area_errors[2] < area_errors[1] {
                        0
                    } else if area_errors[2] > area_errors[1] {
                        1
                    } else {
                        2
                    };
                    super_outcomes[r][outcome] += 1;
                    for i in 0..3 {
                        super_errors[r][i] += area_errors[i];
                        super_spatial[r][i] += spatial[i];
                    }
                    println!(
                        "Mask supersample scale={} size={size} yaw={yaw} zoom={zoom}: reference area {}, base/raw/corrected errors {area_errors:?}, spatial L1 {spatial:?}, 4x/8x drift {drift:?}",
                        [4, 8][r],
                        totals[r]
                    );
                }
            }
        }
    }
    assert!(lower_changes > 0, "lower mip control was not sampled");
    assert!(
        correction_changes > 0,
        "corrected alpha never affected occupancy"
    );
    // Characterization deliberately permits regressions: UV-area correction is
    // not a guarantee for projected area or a base-only aliasing reference.
    println!(
        "Mask occupancy summary: {cases} cases, improvements {improvements}, regressions {regressions}, ties {}, errors {errors:?}, disagreements {disagreements:?}, corrected changes {correction_changes}, lower-mip control {lower_changes}; backend {:?}",
        cases - improvements - regressions,
        state.adapter.get_info().backend
    );
    println!(
        "Mask supersample summary: {cases} cases, scales [4, 8], base/raw/corrected area errors {super_errors:?}, spatial L1 {super_spatial:?}, 4x/8x area and spatial drift {reference_drift:?}, improved/regressed/tied {super_outcomes:?}; backend {:?}",
        state.adapter.get_info().backend
    );
}

#[test]
fn gcad_preview_alpha_authored_modes_and_factors_reach_gpu() {
    let state = super::explicit_tests::requested_render_state();
    viewer3d::install(&state);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/game-assets/atlas-alpha.gasset");
    for explicit in [false, true] {
        let mut asset = crate::asset::author::load(&path, &[]).unwrap();
        if !explicit {
            for material in &mut asset.scene.materials {
                material.mip_chain = None;
            }
        }
        let actual = frame(&asset, None, 0.).unwrap();
        assert_eq!(actual.scene.surfaces.len(), 3);
        for (surface, mode) in actual.scene.surfaces.iter().zip([
            viewer3d::AlphaMode::Opaque,
            viewer3d::AlphaMode::Mask { cutoff: 0.5 },
            viewer3d::AlphaMode::Blend,
        ]) {
            assert_eq!(surface.material.pbr().alpha, mode);
            assert_eq!(surface.material.pbr().base_alpha, 0.75);
        }
        assert!(
            actual.looks.iter().all(|look| look.opacity == 1.),
            "alpha factor must not be applied twice"
        );
        let mut opaque = frame(&asset, None, 0.).unwrap();
        for surface in &mut Arc::get_mut(&mut opaque.scene).unwrap().surfaces {
            surface.material.0[6] = (-1f32).to_bits();
        }
        let mut factor = frame(&asset, None, 0.).unwrap();
        for surface in &mut Arc::get_mut(&mut factor.scene).unwrap().surfaces {
            surface.material.0[7] = 1f32.to_bits();
        }
        let render = |frame: Frame| {
            Viewer::new(frame.scene, Camera::default())
                .looks(frame.looks)
                .edges(false)
                .render_gpu(128, 128)
                .expect("authored alpha requires GPU")
        };
        let a = render(actual);
        let b = render(opaque);
        let c = render(factor);
        let mode_changes = a
            .pixels
            .iter()
            .zip(&b.pixels)
            .filter(|(a, b)| a != b)
            .count();
        let factor_changes = a
            .pixels
            .iter()
            .zip(&c.pixels)
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            mode_changes > 100 && factor_changes > 100,
            "alpha controls are ineffective"
        );
        let output =
            std::env::temp_dir().join(format!("gcad-alpha-refusal-{}.png", std::process::id()));
        assert!(!output.exists());
        assert!(snapshot(&asset, None, 0., &output).is_err());
        assert!(!output.exists());
        println!(
            "Authored alpha explicit {explicit}: mode changes {mode_changes}, factor changes {factor_changes}, software refusal; backend {:?}",
            state.adapter.get_info().backend
        );
    }
}
