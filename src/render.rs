use crate::geometry;
use anyhow::Result;
use image::{Rgb, RgbImage};
use monstertruck::modeling::*;
use rayon::prelude::*;

const CELL: u32 = 600;
const SAMPLES: usize = 3;
const SIDE: usize = CELL as usize * SAMPLES;
const BACKGROUND: Rgb<u8> = Rgb([250, 250, 248]);
const EDGE: [f64; 3] = [25.0, 25.0, 30.0];
const SILHOUETTE: [f64; 3] = [40.0, 44.0, 52.0];
const OVERLAY: f64 = 1.0e300;
const EDGE_WIDTH: f64 = 1.1;
const SMOOTH_ANGLE_COS: f64 = 0.8;

struct View {
    right: Vector3,
    up: Vector3,
    toward_eye: Vector3,
}

impl View {
    fn looking_from(eye: Vector3, up_hint: Vector3) -> View {
        let toward_eye = eye.normalize();
        let right = up_hint.cross(toward_eye).normalize();
        View {
            right,
            up: toward_eye.cross(right),
            toward_eye,
        }
    }
}

fn views() -> [View; 4] {
    [
        View::looking_from(Vector3::new(1.0, -1.0, 0.8), Vector3::unit_z()),
        View::looking_from(Vector3::unit_z(), Vector3::unit_y()),
        View::looking_from(-Vector3::unit_y(), Vector3::unit_z()),
        View::looking_from(Vector3::unit_x(), Vector3::unit_z()),
    ]
}

struct Light {
    toward_eye: Vector3,
    key: Vector3,
    half: Vector3,
    floor: f64,
}

impl Light {
    fn new(view: &View, floor: f64) -> Light {
        let key = (view.toward_eye * 0.6 + view.up * 0.5 - view.right * 0.35).normalize();
        Light {
            toward_eye: view.toward_eye,
            key,
            half: (key + view.toward_eye).normalize(),
            floor,
        }
    }

    fn shade(&self, normal: Vector3, tint: [f64; 3]) -> [f64; 3] {
        let facing = normal.dot(self.toward_eye).max(0.0);
        let key = normal.dot(self.key).max(0.0);
        let diffuse = self.floor + (1.0 - self.floor) * (0.5 * facing + 0.5 * key);
        let gloss = 0.2 * (1.0 - self.floor) * normal.dot(self.half).max(0.0).powi(30);
        tint.map(|c| 255.0 * (c * diffuse + gloss))
    }
}

struct Canvas {
    colour: Vec<[f64; 3]>,
    depth: Vec<f64>,
    slope: Vec<[f64; 2]>,
}

impl Canvas {
    fn new() -> Canvas {
        let background = BACKGROUND.0.map(f64::from);
        Canvas {
            colour: vec![background; SIDE * SIDE],
            depth: vec![f64::NEG_INFINITY; SIDE * SIDE],
            slope: vec![[0.0; 2]; SIDE * SIDE],
        }
    }

    fn index(x: i64, y: i64) -> Option<usize> {
        (x >= 0 && y >= 0 && x < SIDE as i64 && y < SIDE as i64)
            .then(|| y as usize * SIDE + x as usize)
    }

    fn triangle(
        &mut self,
        p: [(f64, f64, f64); 3],
        tints: [[f64; 3]; 3],
        normals: [Vector3; 3],
        light: &Light,
    ) {
        let low = |f: fn(&(f64, f64, f64)) -> f64| {
            p.iter()
                .map(f)
                .fold(f64::INFINITY, f64::min)
                .floor()
                .max(0.0) as i64
        };
        let high = |f: fn(&(f64, f64, f64)) -> f64| {
            p.iter()
                .map(f)
                .fold(f64::NEG_INFINITY, f64::max)
                .ceil()
                .min(SIDE as f64 - 1.0) as i64
        };
        let (min_x, max_x) = (low(|q| q.0), high(|q| q.0));
        let (min_y, max_y) = (low(|q| q.1), high(|q| q.1));
        let area = (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[2].0 - p[0].0) * (p[1].1 - p[0].1);
        if area.abs() < 1.0e-12 {
            return;
        }
        let slope = [
            ((p[1].2 - p[0].2) * (p[2].1 - p[0].1) - (p[2].2 - p[0].2) * (p[1].1 - p[0].1)) / area,
            ((p[2].2 - p[0].2) * (p[1].0 - p[0].0) - (p[1].2 - p[0].2) * (p[2].0 - p[0].0)) / area,
        ];
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                let w0 = ((p[1].0 - px) * (p[2].1 - py) - (p[2].0 - px) * (p[1].1 - py)) / area;
                let w1 = ((p[2].0 - px) * (p[0].1 - py) - (p[0].0 - px) * (p[2].1 - py)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let z = w0 * p[0].2 + w1 * p[1].2 + w2 * p[2].2;
                let index = y as usize * SIDE + x as usize;
                if z < self.depth[index] {
                    continue;
                }
                self.depth[index] = z;
                self.slope[index] = slope;
                let normal = normals[0] * w0 + normals[1] * w1 + normals[2] * w2;
                let normal = if normal.magnitude2() > 1.0e-24 {
                    normal.normalize()
                } else {
                    normals[0]
                };
                let tint =
                    [0, 1, 2].map(|k| w0 * tints[0][k] + w1 * tints[1][k] + w2 * tints[2][k]);
                self.colour[index] = light.shade(normal, tint);
            }
        }
    }

    fn silhouettes(&mut self, jump_per_sample: f64) {
        let reach = SAMPLES as i64;
        let threshold = jump_per_sample * reach as f64;
        let outline: Vec<usize> = (0..SIDE * SIDE)
            .into_par_iter()
            .filter(|&index| {
                let z = self.depth[index];
                if z == f64::NEG_INFINITY {
                    return false;
                }
                let (x, y) = ((index % SIDE) as i64, (index / SIDE) as i64);
                let [gx, gy] = self.slope[index];
                [(reach, 0), (-reach, 0), (0, reach), (0, -reach)]
                    .into_iter()
                    .any(|(dx, dy)| {
                        let expected = z + gx * dx as f64 + gy * dy as f64;
                        Canvas::index(x + dx, y + dy).is_none_or(|n| {
                            self.depth[n] < z - threshold && self.depth[n] < expected - threshold
                        })
                    })
            })
            .collect();
        outline
            .into_iter()
            .for_each(|index| self.colour[index] = SILHOUETTE);
    }

    fn stamp(&mut self, x: f64, y: f64, radius: f64, colour: [f64; 3], on_surface: bool) {
        let r = radius.ceil() as i64;
        let (cx, cy) = (x.floor() as i64, y.floor() as i64);
        for dy in -r..=r {
            for dx in -r..=r {
                let (sx, sy) = ((cx + dx) as f64 + 0.5 - x, (cy + dy) as f64 + 0.5 - y);
                if sx * sx + sy * sy > radius * radius {
                    continue;
                }
                if let Some(index) = Canvas::index(cx + dx, cy + dy)
                    .filter(|&index| !on_surface || self.depth[index] > f64::NEG_INFINITY)
                {
                    self.colour[index] = colour;
                }
            }
        }
    }

    fn line(
        &mut self,
        a: (f64, f64, f64),
        b: (f64, f64, f64),
        colour: [f64; 3],
        bias: f64,
        width: f64,
        on_surface: bool,
    ) {
        let radius = width * SAMPLES as f64 / 2.0;
        let steps = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) * 2.0).ceil() as usize;
        (0..=steps.max(1)).for_each(|i| {
            let t = i as f64 / steps.max(1) as f64;
            let (x, y, z) = (
                a.0 + (b.0 - a.0) * t,
                a.1 + (b.1 - a.1) * t,
                a.2 + (b.2 - a.2) * t,
            );
            let seen = Canvas::index(x.floor() as i64, y.floor() as i64)
                .filter(|&index| !on_surface || self.depth[index] > f64::NEG_INFINITY)
                .is_some_and(|index| z + bias >= self.depth[index]);
            if seen {
                self.stamp(x, y, radius, colour, on_surface);
            }
        });
    }

    fn downsample(&self, image: &mut RgbImage, offset: (u32, u32)) {
        let area = (SAMPLES * SAMPLES) as f64;
        for y in 0..CELL as usize {
            for x in 0..CELL as usize {
                let mut sum = [0.0; 3];
                for sy in 0..SAMPLES {
                    let row = (y * SAMPLES + sy) * SIDE + x * SAMPLES;
                    for sample in &self.colour[row..row + SAMPLES] {
                        (0..3).for_each(|k| sum[k] += sample[k]);
                    }
                }
                image.put_pixel(
                    offset.0 + x as u32,
                    offset.1 + y as u32,
                    Rgb(sum.map(|s| (s / area).round().clamp(0.0, 255.0) as u8)),
                );
            }
        }
    }
}

fn edge_polylines(solid: &Solid) -> Vec<Vec<Point3>> {
    let mut seen = Vec::new();
    solid
        .boundaries()
        .iter()
        .flat_map(|shell| shell.edge_iter())
        .filter(|edge| {
            let fresh = !seen.contains(&edge.id());
            seen.push(edge.id());
            fresh
        })
        .map(|edge| {
            let curve = edge.curve();
            let (t0, t1) = curve.range_tuple();
            (0..=96)
                .map(|i| curve.subs(t0 + (t1 - t0) * i as f64 / 96.0))
                .collect()
        })
        .collect()
}

pub fn render(solids: &[&Solid], path: &str) -> Result<()> {
    let parts: Vec<(&Solid, Option<[f64; 3]>)> = solids.iter().map(|s| (*s, None)).collect();
    render_coloured(&parts, path)
}

fn face_normal(positions: &[Point3], corners: [usize; 3]) -> Option<Vector3> {
    let [a, b, c] = corners.map(|i| positions[i]);
    let normal = (b - a).cross(c - a);
    (normal.magnitude2() > 1.0e-24).then(|| normal.normalize())
}

pub fn render_coloured(parts: &[(&Solid, Option<[f64; 3]>)], path: &str) -> Result<()> {
    render_with_meshes(parts, &[], path)
}

type ColouredMesh = (String, std::sync::Arc<crate::mesh::Mesh>, Vec<[f64; 3]>);

pub fn render_with_meshes(
    parts: &[(&Solid, Option<[f64; 3]>)],
    meshes: &[ColouredMesh],
    path: &str,
) -> Result<()> {
    let solids: Vec<&Solid> = parts.iter().map(|(s, _)| *s).collect();
    let mut positions: Vec<Point3> = Vec::new();
    let mut triangles: Vec<([usize; 3], [[f64; 3]; 3])> = Vec::new();
    let mut normals: Vec<[Vector3; 3]> = Vec::new();
    for (solid, colour) in parts {
        let mesh = geometry::mesh(solid, geometry::mesh_tolerance(solid) * 0.5);
        let base = positions.len();
        positions.extend(mesh.positions().iter().copied());
        let tint = colour.unwrap_or([150.0 / 255.0, 175.0 / 255.0, 205.0 / 255.0]);
        for t in mesh.faces().triangle_iter() {
            let corners = [t[0].pos + base, t[1].pos + base, t[2].pos + base];
            let flat = face_normal(&positions, corners).unwrap_or(Vector3::unit_z());
            normals.push(t.map(|v| {
                v.nor
                    .and_then(|i| mesh.normals().get(i))
                    .filter(|n| n.magnitude2() > 1.0e-24)
                    .map_or(flat, |n| {
                        let n = n.normalize();
                        if n.dot(flat) < 0.0 { -n } else { n }
                    })
            }));
            triangles.push((corners, [tint; 3]));
        }
    }
    for (_, mesh, tints) in meshes {
        let base = positions.len();
        let at = |p: crate::mesh::P3| Point3::new(p[0], p[1], p[2]);
        positions.extend(mesh.positions.iter().map(|&p| at(p)));
        let vertex_normals = mesh.vertex_normals();
        let fallback = [150.0 / 255.0, 175.0 / 255.0, 205.0 / 255.0];
        let tint_of = |i: u32| {
            tints
                .get(mesh.labels[i as usize] as usize)
                .copied()
                .unwrap_or(fallback)
        };
        for t in &mesh.triangles {
            let corners = t.map(|i| i as usize + base);
            normals.push(t.map(|i| {
                let n = vertex_normals[i as usize];
                Vector3::new(n[0], n[1], n[2])
            }));
            triangles.push((corners, t.map(tint_of)));
        }
    }
    let edges: Vec<Vec<Point3>> = solids
        .iter()
        .flat_map(|solid| edge_polylines(solid))
        .collect();
    let bounds: BoundingBox<Point3> = solids
        .iter()
        .flat_map(|solid| [geometry::bounds(solid).min(), geometry::bounds(solid).max()])
        .chain(
            meshes
                .iter()
                .filter_map(|(_, mesh, _)| {
                    let (lo, hi) = mesh.bounds()?;
                    Some([
                        Point3::new(lo[0], lo[1], lo[2]),
                        Point3::new(hi[0], hi[1], hi[2]),
                    ])
                })
                .flatten(),
        )
        .collect();
    let scene = Scene {
        positions: &positions,
        triangles: &triangles,
        normals: &normals,
        edges: &edges,
        lines: &[],
        floor: 0.3,
        edges_on_surface: true,
    };
    draw_lit(&scene, bounds).save(path)?;
    Ok(())
}

fn smooth_normals(
    positions: &[Point3],
    triangles: &[([usize; 3], [[f64; 3]; 3])],
) -> Vec<[Vector3; 3]> {
    let flat: Vec<Option<Vector3>> = triangles
        .iter()
        .map(|(corners, _)| face_normal(positions, *corners))
        .collect();
    let mut around: Vec<Vec<usize>> = vec![Vec::new(); positions.len()];
    triangles.iter().enumerate().for_each(|(t, (corners, _))| {
        corners.iter().for_each(|&c| around[c].push(t));
    });
    triangles
        .iter()
        .zip(&flat)
        .map(|((corners, _), own)| {
            let own = own.unwrap_or(Vector3::unit_z());
            corners.map(|c| {
                let sum = around[c]
                    .iter()
                    .filter_map(|&t| flat[t])
                    .filter(|n| n.dot(own) >= SMOOTH_ANGLE_COS)
                    .fold(Vector3::zero(), |sum, n| sum + n);
                if sum.magnitude2() > 1.0e-24 {
                    sum.normalize()
                } else {
                    own
                }
            })
        })
        .collect()
}

pub fn render_triangles(
    positions: &[Point3],
    triangles: &[([usize; 3], [[f64; 3]; 3])],
    edges: &[Vec<Point3>],
    bounds: BoundingBox<Point3>,
    path: &str,
) -> Result<()> {
    draw_triangles(positions, triangles, edges, bounds).save(path)?;
    Ok(())
}

pub fn draw_triangles(
    positions: &[Point3],
    triangles: &[([usize; 3], [[f64; 3]; 3])],
    edges: &[Vec<Point3>],
    bounds: BoundingBox<Point3>,
) -> RgbImage {
    draw_smooth(positions, triangles, edges, &[], bounds, 0.3)
}

pub fn draw_field(
    positions: &[Point3],
    triangles: &[([usize; 3], [[f64; 3]; 3])],
    edges: &[Vec<Point3>],
    bounds: BoundingBox<Point3>,
) -> RgbImage {
    draw_smooth(positions, triangles, edges, &[], bounds, 0.8)
}

pub type Line = (Vec<Point3>, Vec<Rgb<u8>>);

pub fn draw_field_lines(
    positions: &[Point3],
    triangles: &[([usize; 3], [[f64; 3]; 3])],
    edges: &[Vec<Point3>],
    lines: &[Line],
    bounds: BoundingBox<Point3>,
) -> RgbImage {
    draw_smooth(positions, triangles, edges, lines, bounds, 0.8)
}

fn draw_smooth(
    positions: &[Point3],
    triangles: &[([usize; 3], [[f64; 3]; 3])],
    edges: &[Vec<Point3>],
    lines: &[Line],
    bounds: BoundingBox<Point3>,
    floor: f64,
) -> RgbImage {
    let normals = smooth_normals(positions, triangles);
    let scene = Scene {
        positions,
        triangles,
        normals: &normals,
        edges,
        lines,
        floor,
        edges_on_surface: false,
    };
    draw_lit(&scene, bounds)
}

struct Scene<'a> {
    positions: &'a [Point3],
    triangles: &'a [([usize; 3], [[f64; 3]; 3])],
    normals: &'a [[Vector3; 3]],
    edges: &'a [Vec<Point3>],
    lines: &'a [Line],
    floor: f64,
    edges_on_surface: bool,
}

fn draw_view(scene: &Scene, view: &View, center: Vector3) -> Canvas {
    let positions = scene.positions;
    let project = |p: Point3| {
        let v = p.to_vec() - center;
        (v.dot(view.right), v.dot(view.up), v.dot(view.toward_eye))
    };
    let projected: Vec<(f64, f64, f64)> = positions.iter().map(|&p| project(p)).collect();
    let parts = projected
        .iter()
        .map(|p| p.0.abs().max(p.1.abs()))
        .fold(1.0e-9, f64::max);
    let extent = scene
        .lines
        .iter()
        .flat_map(|(points, _)| points)
        .map(|&p| {
            let q = project(p);
            q.0.abs().max(q.1.abs())
        })
        .fold(parts, f64::max)
        .min(2.5 * parts);
    let scale = (SIDE as f64 * 0.42) / extent;
    let depth_range = projected.iter().map(|p| p.2.abs()).fold(1.0e-9, f64::max);
    let to_pixel = |(x, y, z): (f64, f64, f64)| {
        (
            SIDE as f64 / 2.0 + x * scale,
            SIDE as f64 / 2.0 - y * scale,
            z,
        )
    };
    let light = Light::new(view, scene.floor);
    let mut canvas = Canvas::new();
    for ((corners, tints), normals) in scene.triangles.iter().zip(scene.normals) {
        let Some(flat) = face_normal(positions, *corners) else {
            continue;
        };
        let toward = if flat.dot(view.toward_eye) < 0.0 {
            -1.0
        } else {
            1.0
        };
        canvas.triangle(
            corners.map(|i| to_pixel(projected[i])),
            *tints,
            normals.map(|n| n * toward),
            &light,
        );
    }
    canvas.silhouettes(1.0 / scale);
    let bias = depth_range * 0.006;
    for polyline in scene.edges {
        polyline.windows(2).for_each(|pair| {
            canvas.line(
                to_pixel(project(pair[0])),
                to_pixel(project(pair[1])),
                EDGE,
                bias,
                EDGE_WIDTH,
                scene.edges_on_surface,
            );
        });
    }
    for (points, colours) in scene.lines {
        for (pair, colour) in points.windows(2).zip(colours) {
            canvas.line(
                to_pixel(project(pair[0])),
                to_pixel(project(pair[1])),
                colour.0.map(f64::from),
                bias,
                1.4,
                false,
            );
        }
    }
    let triad = extent * 0.15;
    let origin = (40.0 * SAMPLES as f64, SIDE as f64 - 40.0 * SAMPLES as f64);
    [
        (Vector3::unit_x(), [220.0, 40.0, 40.0]),
        (Vector3::unit_y(), [30.0, 160.0, 60.0]),
        (Vector3::unit_z(), [40.0, 80.0, 220.0]),
    ]
    .into_iter()
    .for_each(|(axis, colour)| {
        let tip = (
            origin.0 + axis.dot(view.right) * triad * scale,
            origin.1 - axis.dot(view.up) * triad * scale,
            OVERLAY,
        );
        canvas.line((origin.0, origin.1, OVERLAY), tip, colour, 0.0, 1.5, false);
    });
    canvas
}

fn draw_lit(scene: &Scene, bounds: BoundingBox<Point3>) -> RgbImage {
    let center = bounds.center().to_vec();
    let mut image = RgbImage::from_pixel(CELL * 2, CELL * 2, BACKGROUND);
    let canvases: Vec<Canvas> = views()
        .par_iter()
        .map(|view| draw_view(scene, view, center))
        .collect();
    for (index, canvas) in canvases.iter().enumerate() {
        canvas.downsample(
            &mut image,
            ((index as u32 % 2) * CELL, (index as u32 / 2) * CELL),
        );
    }
    (0..CELL * 2).for_each(|i| {
        image.put_pixel(CELL, i, Rgb([200, 200, 200]));
        image.put_pixel(i, CELL, Rgb([200, 200, 200]));
    });
    image
}
