//! Painting guides for prepared meshes, shared by CLI and live texture authoring.
use super::{Mesh, author::Asset};
use anyhow::{Context, Result, ensure};
use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::VecDeque, io::Write, path::Path};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    version: u32,
    node: String,
    primitive: usize,
    resolution: u32,
    padding: u32,
    mesh_uv_sha256: String,
    image_origin: String,
    regions: Vec<Region>,
}
#[derive(Serialize, Deserialize)]
struct Region {
    id: usize,
    colour: [u8; 3],
    triangles: usize,
    centre_mm: [f64; 3],
    normal: [f64; 3],
    label_uv: [f64; 2],
}
fn geometry<'a>(asset: &'a Asset, node: &str, primitive: usize) -> Result<&'a Mesh> {
    let node = asset
        .scene
        .nodes
        .iter()
        .find(|n| n.name == node)
        .context("unknown paint node")?;
    let mesh = node.mesh.context("paint node must contain a mesh")?;
    Ok(&asset.scene.meshes[mesh]
        .primitives
        .get(primitive)
        .context("unknown paint primitive")?
        .geometry)
}
fn stamp(mesh: &Mesh) -> Result<String> {
    let mut hash = Sha256::new();
    hash.update(b"gcad-paint-guide-1");
    hash.update(serde_json::to_vec(&(
        &mesh.positions,
        &mesh.normals,
        &mesh.triangles,
        &mesh.texcoords,
    ))?);
    Ok(format!("{:x}", hash.finalize()))
}
fn colour(id: usize) -> [u8; 3] {
    let hue = (id as f64 * 0.61803398875).fract() * 6.;
    let x = (1. - (hue % 2. - 1.).abs()) * 170. + 60.;
    match hue as usize {
        0 => [230, x as u8, 60],
        1 => [x as u8, 230, 60],
        2 => [60, 230, x as u8],
        3 => [60, x as u8, 230],
        4 => [x as u8, 60, 230],
        _ => [230, 60, x as u8],
    }
}
fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}
fn regions(mesh: &Mesh) -> Result<(Vec<usize>, Vec<Region>)> {
    mesh.validate()?;
    let uv = mesh
        .texcoords
        .as_ref()
        .context("paint guide requires UVs")?;
    ensure!(
        uv.iter().flatten().all(|v| (0.0..=1.0).contains(v)),
        "paint guide requires a unique atlas in UV range 0..1; use projection=unwrap"
    );
    let mut parent: Vec<_> = (0..mesh.positions.len()).collect();
    for t in &mesh.triangles {
        let a = root(&mut parent, t[0] as usize);
        for &v in &t[1..] {
            let b = root(&mut parent, v as usize);
            parent[b] = a;
        }
    }
    let mut ids = std::collections::BTreeMap::new();
    let mut assignments = Vec::new();
    let mut regions: Vec<Region> = Vec::new();
    let mut largest = Vec::new();
    for t in &mesh.triangles {
        let key = root(&mut parent, t[0] as usize);
        let next = ids.len();
        let id = *ids.entry(key).or_insert(next);
        if id == regions.len() {
            regions.push(Region {
                id: id + 1,
                colour: colour(id),
                triangles: 0,
                centre_mm: [0.; 3],
                normal: [0.; 3],
                label_uv: [0.; 2],
            });
            largest.push(0.);
        }
        assignments.push(id);
        let r = &mut regions[id];
        r.triangles += 1;
        for &v in t {
            for k in 0..3 {
                r.centre_mm[k] += mesh.positions[v as usize][k] / 3.;
                r.normal[k] += mesh.normals[v as usize][k] / 3.;
            }
        }
        let p = t.map(|v| uv[v as usize]);
        let area = edge(p[0], p[1], p[2]).abs();
        ensure!(area > 1e-16, "degenerate paint UV triangle");
        if area > largest[id] {
            largest[id] = area;
            r.label_uv = std::array::from_fn(|k| (p[0][k] + p[1][k] + p[2][k]) / 3.);
        }
    }
    for r in &mut regions {
        for k in 0..3 {
            r.centre_mm[k] /= r.triangles as f64;
            r.normal[k] /= r.triangles as f64;
        }
    }
    Ok((assignments, regions))
}
fn edge(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}
fn raster(
    p: [[f64; 2]; 3],
    size: u32,
    budget: &mut u64,
    mut pixel: impl FnMut(u32, u32) -> Result<()>,
) -> Result<()> {
    let lo: [u32; 2] = std::array::from_fn(|k| {
        p.iter()
            .map(|v| v[k])
            .fold(f64::INFINITY, f64::min)
            .floor()
            .clamp(0., size as f64) as u32
    });
    let hi: [u32; 2] = std::array::from_fn(|k| {
        p.iter()
            .map(|v| v[k])
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil()
            .clamp(0., size as f64) as u32
    });
    let cost = (hi[0] - lo[0]) as u64 * (hi[1] - lo[1]) as u64;
    ensure!(
        *budget >= cost,
        "painting raster budget exceeded; simplify mesh or reduce resolution"
    );
    *budget -= cost;
    let sign = edge(p[0], p[1], p[2]).signum();
    for y in lo[1]..hi[1] {
        for x in lo[0]..hi[0] {
            let q = [x as f64 + 0.5, y as f64 + 0.5];
            if (0..3).all(|i| edge(p[i], p[(i + 1) % 3], q) * sign >= -1e-9) {
                pixel(x, y)?;
            }
        }
    }
    Ok(())
}
fn coverage(mesh: &Mesh, size: u32, ids: &[usize]) -> Result<Vec<Option<usize>>> {
    let mut covered = vec![None; (size * size) as usize];
    let mut budget = 200_000_000;
    let mut interior = vec![false; covered.len()];
    let uv = mesh.texcoords.as_ref().unwrap();
    for (t, &id) in mesh.triangles.iter().zip(ids) {
        let p = t.map(|v| uv[v as usize].map(|x| x * size as f64));
        raster(p, size, &mut budget, |x, y| {
            let i = (y * size + x) as usize;
            let q = [x as f64 + 0.5, y as f64 + 0.5];
            let inside = (0..3)
                .all(|k| edge(p[k], p[(k + 1) % 3], q) * edge(p[0], p[1], p[2]).signum() > 1e-9);
            ensure!(
                !(inside && interior[i]),
                "UV triangle interiors overlap; generate a unique unwrap"
            );
            interior[i] |= inside;
            let old = &mut covered[(y * size + x) as usize];
            ensure!(
                old.is_none_or(|old| old == id),
                "UV islands overlap; generate a unique unwrap"
            );
            *old = Some(id);
            Ok(())
        })?;
    }
    ensure!(
        covered.iter().any(Option::is_some),
        "UV atlas covers no pixels"
    );
    Ok(covered)
}
fn png(img: &RgbaImage) -> Result<Vec<u8>> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    img.write_to(&mut bytes, image::ImageFormat::Png)?;
    Ok(bytes.into_inner())
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}
fn label(img: &mut RgbaImage, x: i32, y: i32, id: usize) {
    const DIGITS: [[u8; 5]; 10] = [
        [7, 5, 5, 5, 7],
        [2, 6, 2, 2, 7],
        [7, 1, 7, 4, 7],
        [7, 1, 7, 1, 7],
        [5, 5, 7, 1, 1],
        [7, 4, 7, 1, 7],
        [7, 4, 7, 5, 7],
        [7, 1, 1, 1, 1],
        [7, 5, 7, 5, 7],
        [7, 5, 7, 1, 7],
    ];
    for (n, d) in id.to_string().bytes().enumerate() {
        for (j, row) in DIGITS[(d - b'0') as usize].iter().enumerate() {
            for k in 0..3 {
                for sy in 0..2 {
                    for sx in 0..2 {
                        let px = x + n as i32 * 8 + k * 2 + sx;
                        let py = y + j as i32 * 2 + sy;
                        if px >= 0 && py >= 0 && px < img.width() as i32 && py < img.height() as i32
                        {
                            img.put_pixel(
                                px as u32,
                                py as u32,
                                Rgba(if row & (4 >> k) != 0 {
                                    [0, 0, 0, 255]
                                } else {
                                    [255, 255, 255, 255]
                                }),
                            );
                        }
                    }
                }
            }
        }
    }
}
/// Export a new directory only. IDs correspond across the atlas and model views.
pub fn export(
    asset: &Asset,
    node: &str,
    primitive: usize,
    resolution: u32,
    padding: u32,
    output: &Path,
) -> Result<()> {
    super::unwrap::Settings {
        resolution,
        padding,
    }
    .validate()?;
    ensure!(!output.exists(), "painting guide output already exists");
    let mesh = geometry(asset, node, primitive)?;
    ensure!(
        mesh.triangles.len() <= 250_000,
        "painting guide triangle budget exceeded"
    );
    let (ids, regions) = regions(mesh)?;
    let covered = coverage(mesh, resolution, &ids)?;
    let mut region_image = RgbaImage::new(resolution, resolution);
    let mut blank = region_image.clone();
    for (i, id) in covered.iter().enumerate() {
        if let Some(id) = id {
            let [r, g, b] = regions[*id].colour;
            let x = i as u32 % resolution;
            let y = i as u32 / resolution;
            region_image.put_pixel(x, y, Rgba([r, g, b, 255]));
            blank.put_pixel(x, y, Rgba([180, 180, 180, 255]));
        }
    }
    let mut labelled = region_image.clone();
    for r in &regions {
        label(
            &mut labelled,
            (r.label_uv[0] * resolution as f64) as i32,
            (r.label_uv[1] * resolution as f64) as i32,
            r.id,
        );
    }
    let uv = mesh.texcoords.as_ref().unwrap();
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {resolution} {resolution}\">\n<g fill=\"none\" stroke=\"black\" stroke-width=\"0.6\">\n"
    );
    for t in &mesh.triangles {
        let p = t.map(|v| uv[v as usize].map(|x| x * resolution as f64));
        svg.push_str(&format!(
            "<path d=\"M{},{} L{},{} L{},{} Z\"/>\n",
            p[0][0], p[0][1], p[1][0], p[1][1], p[2][0], p[2][1]
        ));
    }
    svg.push_str("</g></svg>\n");
    let manifest = Manifest {
        version: 1,
        node: node.into(),
        primitive,
        resolution,
        padding,
        mesh_uv_sha256: stamp(mesh)?,
        image_origin: "top-left; U right, V down; do not flip".into(),
        regions,
    };
    let mut files = vec![
        ("regions.png", png(&region_image)?),
        ("labelled.png", png(&labelled)?),
        ("paint.png", png(&blank)?),
        ("uv.svg", svg.into_bytes()),
        ("guide.json", serde_json::to_vec_pretty(&manifest)?),
    ];
    for (name, axes, sign) in [
        ("front.png", [0, 2, 1], 1.),
        ("back.png", [0, 2, 1], -1.),
        ("right.png", [1, 2, 0], 1.),
        ("left.png", [1, 2, 0], -1.),
        ("top.png", [0, 1, 2], 1.),
        ("bottom.png", [0, 1, 2], -1.),
    ] {
        files.push((
            name,
            png(&view(mesh, &ids, &manifest.regions, axes, sign)?)?,
        ));
    }
    files.push(("README.txt",b"Paint paint.png using labelled.png, regions.png, uv.svg and the six matching model views. Region numbers and colours match guide.json. Keep the exact canvas size and island positions. Use unlit base colour: no baked highlights or shadows. Do not put region numbers, UV lines or backgrounds into the painted image. Use asset-paint-apply to validate geometry and extend edge padding, then bind the finished PNG with a map declaration. Guides are per node/primitive in mesh-local space; they are not a whole-assembly atlas. Keep this guide with the paint source.\n".to_vec()));
    std::fs::create_dir(output)?;
    for (name, bytes) in files {
        write_new(&output.join(name), &bytes)?;
    }
    Ok(())
}
fn view(
    mesh: &Mesh,
    ids: &[usize],
    regions: &[Region],
    axes: [usize; 3],
    sign: f64,
) -> Result<RgbaImage> {
    let size = 768;
    let mut img = RgbaImage::from_pixel(size, size, Rgba([235, 238, 242, 255]));
    let points: Vec<[f64; 3]> = mesh
        .positions
        .iter()
        .map(|p| {
            [
                p[axes[0]] * sign,
                -p[axes[1]],
                p[axes[2]] * sign * if axes[2] == 1 { -1. } else { 1. },
            ]
        })
        .collect();
    let low: [f64; 2] =
        std::array::from_fn(|k| points.iter().map(|p| p[k]).fold(f64::INFINITY, f64::min));
    let high: [f64; 2] = std::array::from_fn(|k| {
        points
            .iter()
            .map(|p| p[k])
            .fold(f64::NEG_INFINITY, f64::max)
    });
    let scale = 700. / (high[0] - low[0]).max(high[1] - low[1]).max(0.001);
    let project = |p: [f64; 3]| {
        [
            (p[0] - (low[0] + high[0]) * 0.5) * scale + 384.,
            (p[1] - (low[1] + high[1]) * 0.5) * scale + 384.,
        ]
    };
    let mut depths = vec![f64::NEG_INFINITY; (size * size) as usize];
    let mut budget = 200_000_000;
    for i in 0..mesh.triangles.len() {
        let t = mesh.triangles[i];
        let p = t.map(|v| project(points[v as usize]));
        if edge(p[0], p[1], p[2]).abs() < 1e-8 {
            continue;
        }
        let c = regions[ids[i]].colour;
        raster(p, size, &mut budget, |x, y| {
            let q = [x as f64 + 0.5, y as f64 + 0.5];
            let depth = (0..3)
                .map(|k| {
                    edge(p[(k + 1) % 3], p[(k + 2) % 3], q) / edge(p[0], p[1], p[2])
                        * points[t[k] as usize][2]
                })
                .sum::<f64>();
            let index = (y * size + x) as usize;
            if depth > depths[index] {
                depths[index] = depth;
                img.put_pixel(x, y, Rgba([c[0], c[1], c[2], 255]));
            }
            Ok(())
        })?;
    }
    Ok(img)
}
/// Validate the saved guide against the current prepared mesh and pad its islands.
pub fn apply(asset: &Asset, guide: &Path, painted: &Path, output: &Path) -> Result<()> {
    ensure!(!output.exists(), "paint output already exists");
    let manifest: Manifest = serde_json::from_slice(&std::fs::read(guide.join("guide.json"))?)?;
    ensure!(manifest.version == 1, "unsupported painting guide version");
    super::unwrap::Settings {
        resolution: manifest.resolution,
        padding: manifest.padding,
    }
    .validate()?;
    let mesh = geometry(asset, &manifest.node, manifest.primitive)?;
    ensure!(
        stamp(mesh)? == manifest.mesh_uv_sha256,
        "painting guide is stale; regenerate it for the current mesh/UVs"
    );
    let (ids, _) = regions(mesh)?;
    let covered = coverage(mesh, manifest.resolution, &ids)?;
    let reader = image::ImageReader::open(painted)?.with_guessed_format()?;
    let (width, height) = image::ImageReader::open(painted)?
        .with_guessed_format()?
        .into_dimensions()?;
    ensure!(
        width == manifest.resolution && height == manifest.resolution,
        "painted image dimensions must match the guide exactly"
    );
    let source = reader.decode()?.to_rgba8();
    let mut result = RgbaImage::new(width, height);
    let mut distance = vec![u32::MAX; covered.len()];
    let mut queue = VecDeque::new();
    for (i, id) in covered.iter().enumerate() {
        if id.is_some() {
            let x = i as u32 % width;
            let y = i as u32 / width;
            result.put_pixel(x, y, *source.get_pixel(x, y));
            distance[i] = 0;
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        if distance[i] >= manifest.padding {
            continue;
        }
        let x = i as u32 % width;
        let y = i as u32 / width;
        for (nx, ny) in [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ] {
            if nx >= width || ny >= height {
                continue;
            }
            let j = (ny * width + nx) as usize;
            if distance[j] == u32::MAX {
                distance[j] = distance[i] + 1;
                result.put_pixel(nx, ny, *result.get_pixel(x, y));
                queue.push_back(j);
            }
        }
    }
    write_new(output, &png(&result)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unwrap_guide_and_paint_validate_layout_dimensions_padding_and_staleness() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/game-assets/plate-unwrapped.gasset");
        let mut asset = super::super::author::load(&path, &[]).unwrap();
        let root = std::env::temp_dir().join(format!(
            "gcad-paint-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let guide = root.join("guide");
        export(&asset, "plate", 0, 1024, 8, &guide).unwrap();
        assert!(export(&asset, "plate", 0, 1024, 8, &guide).is_err());
        let manifest: Manifest =
            serde_json::from_slice(&std::fs::read(guide.join("guide.json")).unwrap()).unwrap();
        assert!(manifest.regions.len() > 1);
        let output = root.join("painted.png");
        apply(&asset, &guide, &guide.join("regions.png"), &output).unwrap();
        let before = image::open(guide.join("regions.png")).unwrap().to_rgba8();
        let after = image::open(&output).unwrap().to_rgba8();
        assert!(
            after.pixels().filter(|p| p[3] > 0).count()
                > before.pixels().filter(|p| p[3] > 0).count()
        );
        for (a, b) in before.pixels().zip(after.pixels()) {
            if a[3] > 0 {
                assert_eq!(a, b);
            }
        }
        assert!(apply(&asset, &guide, &guide.join("regions.png"), &output).is_err());
        let small = root.join("small.png");
        RgbaImage::new(64, 64).save(&small).unwrap();
        assert!(apply(&asset, &guide, &small, &root.join("wrong.png")).is_err());
        asset.scene.meshes[0].primitives[0]
            .geometry
            .texcoords
            .as_mut()
            .unwrap()[0][0] += 0.001;
        assert!(
            apply(
                &asset,
                &guide,
                &guide.join("regions.png"),
                &root.join("stale.png")
            )
            .unwrap_err()
            .to_string()
            .contains("stale")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
