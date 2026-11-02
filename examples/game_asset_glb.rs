//! Static export fixture; intentionally separate from public CAD CLI routes.
use anyhow::{Result, ensure};
use gcad::asset::material::{AlphaMode, Filter, NormalConvention, Sampler, Wrap};
use gcad::asset::scene::{Node, Primitive, Scene, SceneMesh, Transform};
use gcad::asset::{Mesh, glb};
use gcad::model::texture::Map;

fn normal_bake_fixture(path: &std::path::Path) -> Result<()> {
    let low = Mesh {
        positions: vec![
            [0., 0., 0.],
            [100., 0., 0.],
            [0., 100., 0.],
            [100., 100., 0.],
            [200., 0., 0.],
            [300., 0., 0.],
            [200., 100., 0.],
            [300., 100., 0.],
        ],
        normals: vec![[0., 0., 1.]; 8],
        texcoords: Some(vec![
            [0.0625, 0.125],
            [0.4375, 0.125],
            [0.0625, 0.875],
            [0.4375, 0.875],
            [0.9375, 0.125],
            [0.5625, 0.125],
            [0.9375, 0.875],
            [0.5625, 0.875],
        ]),
        tangents: None,
        triangles: vec![[0, 1, 2], [1, 3, 2], [4, 5, 6], [5, 7, 6]],
    }
    .generate_tangents()?;
    let length = 1.13_f64.sqrt();
    let high = Mesh {
        positions: vec![
            [-10., -10., 0.],
            [310., -10., 0.],
            [-10., 110., 0.],
            [310., 110., 0.],
        ]
        .into_iter()
        .map(|p| [p[0], p[1], 1. + 0.2 * p[0] + 0.3 * p[1]])
        .collect(),
        normals: vec![[-0.2 / length, -0.3 / length, 1. / length]; 4],
        texcoords: None,
        tangents: None,
        triangles: vec![[0, 1, 2], [1, 3, 2]],
    };
    let scene = |geometry| Scene {
        meshes: vec![SceneMesh {
            name: "surface".into(),
            primitives: vec![Primitive {
                geometry,
                material: 0,
            }],
        }],
        materials: vec![Default::default()],
        nodes: vec![Node {
            name: "surface".into(),
            mesh: Some(0),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut target = scene(low);
    let source = scene(high);
    let report = gcad::asset::projection::apply_to_scene(
        &mut target,
        0,
        0,
        &source,
        0,
        0,
        gcad::asset::projection::Options {
            width: 64,
            height: 32,
            max_distance_mm: 100.,
            min_normal_dot: 0.9,
            max_missed_fraction: 0.,
        },
        2,
    )?;
    ensure!(
        report.covered_pixels == 1152 && report.hit_pixels == 1152,
        "unexpected normal fixture coverage"
    );
    let bytes = glb::encode_scene(&target)?;
    std::fs::write(path, &bytes)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    println!("normal detail fixture: {} GLB bytes", bytes.len());
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        args.len() == 2
            || (args.len() == 3
                && (args[2] == "--rich"
                    || args[2] == "--scene"
                    || args[2] == "--baked"
                    || args[2] == "--atlas"
                    || args[2] == "--normal-bake"
                    || args[2] == "--skin")),
        "usage: game_asset_glb OUTPUT.glb [--rich|--scene|--baked|--skin|--atlas|--normal-bake]"
    );
    if args.len() == 3 && args[2] == "--skin" {
        return skin_fixture(std::path::Path::new(&args[1]));
    }
    if args.len() == 3 && args[2] == "--normal-bake" {
        return normal_bake_fixture(std::path::Path::new(&args[1]));
    }
    let program = gcad::parse::parse_program("rect 1000 500\nextrude 250\n")?;
    let run = gcad::model::run(&program);
    for (_, result) in &run.steps {
        if let Err(error) = result {
            anyhow::bail!("CAD fixture failed: {error:#}");
        }
    }
    let solid = run
        .model
        .solid
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("fixture has no solid"))?;
    let mut mesh = Mesh::from_solid(solid, 0.1)?.project_box(250.0)?;
    let mut material = glb::Material {
        metallic: 0.75,
        roughness: 0.8,
        colour_map: Some(Map {
            width: 2,
            height: 2,
            rgba: vec![
                220, 40, 20, 255, 30, 160, 220, 255, 30, 160, 220, 255, 220, 40, 20, 255,
            ],
        }),
        roughness_map: Some(Map {
            width: 2,
            height: 2,
            rgba: [128, 0, 0, 255].repeat(4),
        }),
        metallic_map: Some(Map {
            width: 2,
            height: 2,
            rgba: [64, 0, 0, 255].repeat(4),
        }),
        ..glb::Material::default()
    };
    if args.len() == 3 && (args[2] == "--rich" || args[2] == "--atlas") {
        let map = |rgba: [u8; 4]| Map {
            width: 2,
            height: 2,
            rgba: rgba.repeat(4),
        };
        material.normal_map = Some(map([128, 200, 240, 255]));
        material.normal_convention = NormalConvention::NegativeY;
        material.normal_scale = 0.6;
        material.occlusion_map = Some(map([180, 0, 0, 255]));
        material.occlusion_strength = 0.4;
        material.emissive_map = Some(map([30, 60, 90, 255]));
        material.emissive = [0.2, 0.4, 0.6];
        material.alpha = AlphaMode::Mask { cutoff: 0.35 };
        material.base_colour[3] = 0.65;
        material.colour_map.as_mut().unwrap().rgba[3] = 80;
        material.double_sided = true;
        material.sampler = Sampler {
            wrap_u: Wrap::Clamp,
            wrap_v: Wrap::Mirror,
            magnify: Filter::Nearest,
            minify: Filter::Nearest,
            mipmap: None,
        };
    }
    if args.len() == 3 && args[2] == "--baked" {
        let finish = gcad::model::materials::Finish {
            colour: Some([0.5, 0.25, 0.75]),
            metallic: Some(0.75),
            roughness: Some(0.8),
            texture: Some(gcad::model::texture::Texture {
                pattern: Some("wood".into()),
                tile: Some(250.0),
                turn: Some(90.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        (mesh, material) = gcad::asset::bake::finish(&mesh, &finish, [1.0; 3], 1.0)?;
    }
    if args.len() == 3 && args[2] == "--atlas" {
        let atlas = gcad::asset::atlas::pack(&mesh, 256, 128, 2, 16.0)?;
        material = atlas.rebake(&material, Filter::Nearest)?;
        mesh = atlas.mesh;
    }
    let bytes = if args.len() == 3 && args[2] == "--scene" {
        let mut first = mesh.clone();
        let mut second = mesh.clone();
        first.triangles.truncate(6);
        second.triangles.drain(..6);
        let mut alternate = material.clone();
        alternate.base_colour = [0.2, 0.4, 0.8, 1.0];
        let scene = Scene {
            consumer:None,
            nodes: vec![
                Node {
                    name: "mirror".into(),
                    parent: Some(2),
                    mesh: Some(0),
                    transform: Transform {
                        translation: [1000.0, 0.0, 0.0],
                        scale: [-2.0, 3.0, 4.0],
                        ..Default::default()
                    },
                },
                Node {
                    name: "copy".into(),
                    parent: Some(2),
                    mesh: Some(0),
                    transform: Transform {
                        translation: [-1000.0, 0.0, 0.0],
                        ..Default::default()
                    },
                },
                Node {
                    name: "assembly".into(),
                    transform: Transform {
                        translation: [0.0, 2000.0, 0.0],
                        rotation: [
                            0.0,
                            0.0,
                            std::f64::consts::FRAC_1_SQRT_2,
                            std::f64::consts::FRAC_1_SQRT_2,
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            meshes: vec![SceneMesh {
                name: "two-material-box".into(),
                primitives: vec![
                    Primitive {
                        geometry: first,
                        material: 0,
                    },
                    Primitive {
                        geometry: second,
                        material: 1,
                    },
                ],
            }],
            materials: vec![material, alternate],
            clips: Vec::new(),
        };
        glb::encode_scene(&scene)?
    } else {
        glb::encode(&mesh, &material)?
    };
    std::fs::write(&args[1], &bytes)?;
    println!(
        "source mesh: {} vertices, {} triangles; {} GLB bytes",
        mesh.positions.len(),
        mesh.triangles.len(),
        bytes.len()
    );
    Ok(())
}

fn skin_fixture(path: &std::path::Path) -> Result<()> {
    use gcad::asset::{
        animation::{Clip, Interpolation, Track, Values},
        skin::{Skin, Weights},
    };
    let mesh = Mesh {
        positions: vec![
            [-50.0, 0.0, 0.0],
            [50.0, 0.0, 0.0],
            [-50.0, 500.0, 0.0],
            [50.0, 500.0, 0.0],
            [-50.0, 1000.0, 0.0],
            [50.0, 1000.0, 0.0],
        ],
        normals: vec![[0.0, 0.0, 1.0]; 6],
        texcoords: Some(vec![
            [0.0, 0.0],
            [1.0, 0.0],
            [0.0, 0.5],
            [1.0, 0.5],
            [0.0, 1.0],
            [1.0, 1.0],
        ]),
        tangents: None,
        triangles: vec![[0, 1, 2], [1, 3, 2], [2, 3, 4], [3, 5, 4]],
    }
    .generate_tangents()?;
    let transform = Transform {
        translation: [100.0, 200.0, 300.0],
        ..Default::default()
    };
    let q = std::f64::consts::FRAC_1_SQRT_2;
    let mut scene = Scene {
        consumer:None,
        nodes: vec![
            Node {
                name: "ribbon".into(),
                mesh: Some(0),
                transform,
                ..Default::default()
            },
            Node {
                name: "base".into(),
                transform,
                ..Default::default()
            },
            Node {
                name: "tip".into(),
                parent: Some(1),
                transform: Transform {
                    translation: [0.0, 500.0, 0.0],
                    ..Default::default()
                },
                ..Default::default()
            },
            Node {
                name: "unskinned".into(),
                mesh: Some(0),
                transform: Transform {
                    translation: [-100.0, 200.0, 300.0],
                    ..Default::default()
                },
                ..Default::default()
            },
        ],
        meshes: vec![SceneMesh {
            name: "ribbon".into(),
            primitives: vec![Primitive {
                geometry: mesh,
                material: 0,
            }],
        }],
        materials: vec![glb::Material {
            colour_map: Some(Map {
                width: 2,
                height: 2,
                rgba: vec![
                    220, 40, 20, 255, 30, 160, 220, 255, 30, 160, 220, 255, 220, 40, 20, 255,
                ],
            }),
            ..Default::default()
        }],
        clips: Vec::new(),
    };
    for (name, rotation) in [("bend", [0.0, 0.0, q, q]), ("twist", [0.0, q, 0.0, q])] {
        scene.clips.push(Clip {
            name: name.into(),
            tracks: vec![Track {
                node: 2,
                times: vec![0.0, 1.0],
                interpolation: Interpolation::Linear,
                values: Values::Rotation(vec![[0.0, 0.0, 0.0, 1.0], rotation]),
            }],
        });
    }
    let weights = (0..6)
        .map(|i| {
            let tip = if i < 2 {
                0.0
            } else if i < 4 {
                0.5
            } else {
                1.0
            };
            Weights {
                joints: [0, 1, 0, 0],
                weights: [1.0 - tip, tip, 0.0, 0.0],
            }
        })
        .collect();
    let skin = Skin::bind(&scene, 0, vec![1, 2], vec![weights])?;
    let bytes = glb::encode_skinned_scene(&scene, &[skin])?;
    std::fs::write(path, &bytes)?;
    println!(
        "two-bone ribbon: 6 vertices, 4 triangles, 2 clips, {} GLB bytes",
        bytes.len()
    );
    Ok(())
}
