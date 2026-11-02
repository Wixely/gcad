//! Emit a bounded binary atlas fixture for the isolated renderer candidate.
use anyhow::{Result, ensure};
use gcad::{
    asset::{
        Mesh, atlas,
        material::{Material, NormalConvention},
    },
    model::texture::Map,
};
use std::io::Write;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 2,
        "usage: game_asset_renderer_fixture NEW_FILE.bin"
    );
    let mesh = Mesh {
        positions: vec![
            [-1., -1., 0.],
            [0., -1., 0.],
            [-0.5, 1., 0.],
            [0., -1., 0.],
            [1., -1., 0.],
            [0.5, 1., 0.],
        ],
        normals: vec![[0., 0., 1.]; 6],
        texcoords: Some(vec![
            [0.1, 0.1],
            [0.2, 0.1],
            [0.1, 0.2],
            [0.8, 0.1],
            [0.9, 0.1],
            [0.8, 0.2],
        ]),
        triangles: vec![[0, 1, 2], [3, 4, 5]],
        tangents: None,
    };
    let atlas = atlas::pack(&mesh, 128, 64, 8, 200.)?;
    let flat = |rgba| {
        Some(Map {
            width: 1,
            height: 1,
            rgba: Vec::from(rgba),
        })
    };
    let material = Material {
        base_colour: [0.25, 0.5, 1., 1.],
        colour_map: Some(Map {
            width: 64,
            height: 1,
            rgba: (0..64)
                .flat_map(|x| {
                    if x < 32 {
                        [255, 0, 0, 255]
                    } else {
                        [0, 0, 255, 255]
                    }
                })
                .collect(),
        }),
        normal_map: flat([128, 64, 255, 255]),
        normal_convention: NormalConvention::NegativeY,
        roughness_map: flat([64; 4]),
        metallic_map: flat([192; 4]),
        ..Material::default()
    };
    let chain = atlas.rebake_chain(&material, 3)?;
    let maps = gcad::view::asset::mip_images(&chain)?;
    let mut bytes = b"GCADAT01".to_vec();
    let mut u32le = |value: u32| bytes.extend(value.to_le_bytes());
    u32le(6);
    for i in 0..6 {
        for v in atlas.mesh.positions[i]
            .into_iter()
            .chain(atlas.mesh.normals[i])
            .chain(atlas.mesh.texcoords.as_ref().unwrap()[i])
        {
            bytes.extend((v as f32).to_le_bytes());
        }
    }
    for slot in maps {
        bytes.extend((slot.len() as u32).to_le_bytes());
        for map in slot {
            bytes.extend((map.width as u32).to_le_bytes());
            bytes.extend((map.height as u32).to_le_bytes());
            bytes.extend(map.rgba);
        }
    }
    ensure!(bytes.len() < 1024 * 1024, "fixture exceeds bound");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?;
    file.write_all(&bytes)?;
    println!(
        "PASS: two gcad atlas charts, three prepared map slots, four levels; {} bytes",
        bytes.len()
    );
    Ok(())
}
