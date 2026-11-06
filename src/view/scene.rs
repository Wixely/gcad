use crate::geometry;
use crate::model::Model;
use egui_bench::viewer3d::{self, Material, Pbr, Shading};
use monstertruck::meshing::prelude::*;
use monstertruck::modeling::*;
use std::sync::Arc;

pub use egui_bench::viewer3d::V3;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Ink {
    #[default]
    Body,
    Finished(Finish),
    Edge,
    Marked,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Finish([u32; 10]);

impl Finish {
    fn new(finish: &crate::model::materials::Finish, colour: V3) -> Finish {
        let base = Ink::Body.shading().pbr();
        let or = |v: Option<f64>, default: f32| v.map_or(default, |v| v as f32);
        let glow = or(finish.emission, 0.0);
        let linear = |c: f32| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let values = [
            or(finish.roughness, base.roughness),
            or(finish.metallic, base.metallic),
            or(finish.specular, base.specular),
            or(finish.clearcoat, base.clearcoat),
            or(finish.clearcoat_roughness, base.clearcoat_roughness),
            or(finish.transmission, base.transmission),
            or(finish.ior, base.ior),
            linear(colour[0]) * glow,
            linear(colour[1]) * glow,
            linear(colour[2]) * glow,
        ];
        Finish(values.map(f32::to_bits))
    }

    fn pbr(&self) -> Pbr {
        let [
            roughness,
            metallic,
            specular,
            clearcoat,
            clearcoat_roughness,
            transmission,
            ior,
            r,
            g,
            b,
        ] = self.0.map(f32::from_bits);
        Pbr {
            roughness,
            metallic,
            specular,
            clearcoat,
            clearcoat_roughness,
            transmission,
            ior,
            emission: [r, g, b],
            ..Pbr::default()
        }
    }
}

impl Material for Ink {
    fn pbr(&self) -> Pbr {
        match self {
            Ink::Finished(finish) => finish.pbr(),
            _ => self.shading().pbr(),
        }
    }

    fn shading(&self) -> Shading {
        match self {
            Ink::Body | Ink::Finished(_) => Shading {
                capped: true,
                ..Shading::default()
            },
            Ink::Edge => Shading::default(),
            Ink::Marked => Shading {
                tinted: false,
                ..Shading::default()
            },
        }
    }
}

pub type Surface = viewer3d::Surface<Ink>;
pub type Lines = viewer3d::Lines<Ink>;

#[derive(Clone)]
pub struct Part {
    pub name: String,
    pub colour: V3,
    pub opacity: f32,
    pub volume: f64,
    pub current: bool,
    pub source: Option<String>,
    pub material: Option<(String, f64)>,
    pub size: [f64; 3],
    pub hidden: bool,
    pub role: Role,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Role {
    Body,
    Field,
    Streams,
    Slice { index: usize, moment: bool },
}

#[derive(Clone)]
pub struct FaceInfo {
    pub body: usize,
    pub kind: String,
    pub area: f64,
}

pub struct Scene {
    pub mesh: Arc<viewer3d::Scene<Ink>>,
    pub parts: Vec<Part>,
    pub vertices: Vec<(usize, V3)>,
    pub edges: Vec<SnapEdge>,
    pub faces: Vec<FaceInfo>,
}

#[derive(Clone, Debug)]
pub struct SnapEdge {
    pub body: usize,
    pub points: Vec<V3>,
    pub circle: Option<(V3, f32)>,
}

impl SnapEdge {
    fn new(body: usize, points: Vec<V3>) -> SnapEdge {
        let circle = circle_through(&points);
        SnapEdge {
            body,
            points,
            circle,
        }
    }

    pub fn closed(&self) -> bool {
        closed(&self.points)
    }

    pub fn length(&self) -> f32 {
        self.points
            .windows(2)
            .map(|w| length(sub(w[1], w[0])))
            .sum()
    }

    pub fn middle(&self) -> Option<V3> {
        let half = self.length() / 2.0;
        let mut walked = 0.0;
        for w in self.points.windows(2) {
            let step = length(sub(w[1], w[0]));
            if walked + step >= half && step > 0.0 {
                let t = (half - walked) / step;
                return Some(std::array::from_fn(|k| w[0][k] + (w[1][k] - w[0][k]) * t));
            }
            walked += step;
        }
        None
    }
}

fn length(a: V3) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn closed(points: &[V3]) -> bool {
    match (points.first(), points.last()) {
        (Some(a), Some(b)) if points.len() > 2 => length(sub(*a, *b)) < 1.0e-6,
        _ => false,
    }
}

fn circle_through(points: &[V3]) -> Option<(V3, f32)> {
    let closed = closed(points);
    let open = points.len() - usize::from(closed);
    if open < 4 {
        return None;
    }
    let spread = if closed {
        [0, open / 3, 2 * open / 3]
    } else {
        [0, open / 2, open - 1]
    };
    let wide = |p: V3| Vector3::new(p[0] as f64, p[1] as f64, p[2] as f64);
    let [a, b, c] = spread.map(|i| wide(points[i]));
    let (ab, ac) = (b - a, c - a);
    let n = ab.cross(ac);
    if n.magnitude2() <= 1.0e-12 * ab.magnitude2() * ac.magnitude2() {
        return None;
    }
    let offset =
        (ac.cross(n) * ab.magnitude2() + n.cross(ab) * ac.magnitude2()) / (2.0 * n.magnitude2());
    let (centre, radius, axis) = (a + offset, offset.magnitude(), n.normalize());
    let tolerance = radius * 2.0e-3 + 1.0e-5;
    points
        .iter()
        .all(|p| {
            let gap = wide(*p) - centre;
            (gap.magnitude() - radius).abs() < tolerance && gap.dot(axis).abs() < tolerance
        })
        .then_some((
            [centre.x as f32, centre.y as f32, centre.z as f32],
            radius as f32,
        ))
}

pub struct Highlight<'a> {
    pub faces: &'a [usize],
    pub colour: V3,
}

pub const BODY: V3 = [0.62, 0.66, 0.72];
pub const EDGE: V3 = [0.05, 0.06, 0.07];
pub const EDGE_WIDTH: f32 = 1.5;
pub const MARKED_WIDTH: f32 = 3.5;
pub const WIND: V3 = [0.45, 0.75, 1.0];

fn v3(p: Point3) -> V3 {
    [p.x as f32, p.y as f32, p.z as f32]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(a: V3) -> V3 {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2])
        .sqrt()
        .max(1.0e-12);
    [a[0] / l, a[1] / l, a[2] / l]
}

fn face_colour(index: usize, base: V3, highlights: &[Highlight<'_>]) -> V3 {
    highlights
        .iter()
        .rev()
        .find(|h| h.faces.contains(&index))
        .map_or(base, |h| h.colour)
}

fn describe(surface: &monstertruck::modeling::Surface) -> String {
    match surface {
        monstertruck::modeling::Surface::Plane(plane) => {
            let n = plane.normal();
            let tidy = |v: f64| if v.abs() < 5.0e-4 { 0.0 } else { v };
            format!(
                "flat, normal {:.3}, {:.3}, {:.3}",
                tidy(n.x),
                tidy(n.y),
                tidy(n.z)
            )
        }
        _ => "curved".to_string(),
    }
}

type Meshed = monstertruck::topology::Solid<Point3, PolylineCurve<Point3>, Option<PolygonMesh>>;

fn faces(
    solid: &Solid,
    meshed: &Meshed,
    body: usize,
    base: V3,
    ink: Ink,
    highlights: &[Highlight<'_>],
    out: &mut Vec<Surface>,
    info: &mut Vec<FaceInfo>,
) -> f64 {
    let originals: Vec<Face> = crate::select::faces(solid);
    let faces = meshed
        .boundaries()
        .iter()
        .flat_map(|shell| shell.face_iter().cloned())
        .collect::<Vec<_>>();
    let mut volume = 0.0;
    for (index, face) in faces.iter().enumerate() {
        let Some(mesh) = face.surface() else { continue };
        let mut surface = Surface {
            colour: face_colour(index, base, highlights),
            material: ink,
            group: body,
            ..Default::default()
        };
        let positions = mesh.positions();
        let normals = mesh.normals();
        let mut area = 0.0;
        let outward = face.orientation();
        for triangle in mesh.faces().triangle_iter() {
            let triangle = if outward {
                triangle
            } else {
                [triangle[0], triangle[2], triangle[1]]
            };
            let corners = triangle.map(|v| v3(positions[v.pos]));
            let [a, b, c] = triangle.map(|v| positions[v.pos]);
            area += (b - a).cross(c - a).magnitude() / 2.0;
            volume += a.to_vec().dot(b.to_vec().cross(c.to_vec())) / 6.0;
            let flat = norm(cross(
                sub(corners[1], corners[0]),
                sub(corners[2], corners[0]),
            ));
            for (vertex, corner) in triangle.iter().zip(corners) {
                surface.positions.push(corner);
                let n = vertex.nor.and_then(|i| normals.get(i)).map_or(flat, |n| {
                    let n = norm([n.x as f32, n.y as f32, n.z as f32]);
                    if outward { n } else { [-n[0], -n[1], -n[2]] }
                });
                surface.normals.push(n);
            }
        }
        info.push(FaceInfo {
            body,
            kind: originals
                .get(index)
                .map_or_else(|| "face".to_string(), |f| describe(&f.oriented_surface())),
            area,
        });
        out.push(surface);
    }
    volume
}

fn polyline(edge: &Edge) -> Vec<V3> {
    geometry::curve_samples(&edge.curve())
        .into_iter()
        .map(v3)
        .collect()
}

fn segments(points: &[V3], out: &mut Vec<[V3; 2]>) {
    out.extend(
        points
            .windows(2)
            .filter(|pair| sub(pair[1], pair[0]).iter().any(|c| c.abs() > 1.0e-9))
            .map(|pair| [pair[0], pair[1]]),
    );
}

fn bend(
    surface: &monstertruck::modeling::Surface,
    at: Point3,
    across: Vector3,
) -> Option<(Vector3, f64)> {
    let (u, v) = surface.search_nearest_parameter(at, None, 100)?;
    let (su, sv) = (surface.uder(u, v), surface.vder(u, v));
    let n = surface.normal(u, v);
    let (e, f, g) = (su.dot(su), su.dot(sv), sv.dot(sv));
    let det = e * g - f * f;
    if det.abs() < 1.0e-18 {
        return None;
    }
    let (p, q) = (across.dot(su), across.dot(sv));
    let (a, b) = ((g * p - f * q) / det, (e * q - f * p) / det);
    let first = su * a + sv * b;
    let second = surface.uuder(u, v) * (a * a)
        + surface.uvder(u, v) * (2.0 * a * b)
        + surface.vvder(u, v) * (b * b);
    Some((n, second.dot(n) / first.magnitude2().max(1.0e-18)))
}

fn smooth(edge: &Edge, a: &Face, b: &Face) -> bool {
    let (sa, sb) = (a.oriented_surface(), b.oriented_surface());
    let curve = edge.curve();
    let (t0, t1) = curve.range_tuple();
    let h = (t1 - t0) * 1.0e-4;
    [0.25, 0.5, 0.75].iter().all(|&s| {
        let t = t0 + (t1 - t0) * s;
        let at = curve.subs(t);
        let tangent = curve.subs(t + h) - curve.subs(t - h);
        if tangent.magnitude2() < 1.0e-30 {
            return false;
        }
        let Some((na, _)) = bend(&sa, at, tangent) else {
            return false;
        };
        let across = na.cross(tangent).normalize();
        match (bend(&sa, at, across), bend(&sb, at, across)) {
            (Some((na, ka)), Some((nb, kb))) => {
                na.dot(nb) > 1.0 - 1.0e-6
                    && (ka - kb).abs() <= 1.0e-3 * (ka.abs() + kb.abs()) + 1.0e-9
            }
            _ => false,
        }
    })
}

fn edge_lines(
    solid: &Solid,
    meshed: &Meshed,
    body: usize,
    skip: &std::collections::HashSet<EdgeId>,
    out: &mut Vec<[V3; 2]>,
    snaps: &mut Vec<SnapEdge>,
) {
    for (shell, drawn) in solid.boundaries().iter().zip(meshed.boundaries()) {
        let mut owners: std::collections::HashMap<EdgeId, Vec<Face>> = Default::default();
        for face in shell.face_iter() {
            for edge in face.edge_iter() {
                owners.entry(edge.id()).or_default().push(face.clone());
            }
        }
        let mut seen = std::collections::HashSet::new();
        for (edge, polyline) in shell.edge_iter().zip(drawn.edge_iter()) {
            if !seen.insert(edge.id()) {
                continue;
            }
            let marked = skip.contains(&edge.id());
            if !marked
                && let Some([a, b]) = owners.get(&edge.id()).map(Vec::as_slice)
                && a.id() != b.id()
                && smooth(&edge, a, b)
            {
                continue;
            }
            let points: Vec<V3> = polyline.curve().iter().map(|p| v3(*p)).collect();
            if !marked {
                segments(&points, out);
            }
            snaps.push(SnapEdge::new(body, points));
        }
    }
}

fn describe_source(source: &crate::model::Source) -> String {
    let file = source.file.file_name().map_or_else(
        || source.file.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let mut text = file;
    if !source.body.is_empty() {
        text.push_str(&format!(" body={}", source.body));
    }
    for (name, value) in &source.vars {
        text.push_str(&format!(" {name}={value}"));
    }
    text
}

fn mesh_surfaces(
    mesh: &crate::mesh::Mesh,
    body: usize,
    tints: &[V3],
    selected: Option<(&[f64], V3)>,
    out: &mut Vec<Surface>,
    info: &mut Vec<FaceInfo>,
) {
    let normals = mesh.vertex_normals();
    let point = |p: crate::mesh::P3| [p[0] as f32, p[1] as f32, p[2] as f32];
    let mut by_part: Vec<Vec<[u32; 3]>> = vec![Vec::new(); mesh.parts.len().max(1)];
    let last = by_part.len() - 1;
    for &t in &mesh.triangles {
        let label = mesh.labels[t[0] as usize] as usize;
        by_part[label.min(last)].push(t);
    }
    let picked = |t: &[u32; 3]| {
        selected.is_some_and(|(weights, _)| {
            t.iter().map(|&i| weights[i as usize]).sum::<f64>() / 3.0 >= 0.5
        })
    };
    for (part, triangles) in by_part.iter().enumerate() {
        if triangles.is_empty() {
            continue;
        }
        let area: f64 = triangles
            .iter()
            .map(|&t| {
                let [a, b, c] = mesh.corners(t);
                crate::mesh::length(crate::mesh::cross(
                    crate::mesh::sub(b, a),
                    crate::mesh::sub(c, a),
                )) / 2.0
            })
            .sum();
        let kind = format!(
            "mesh part `{}`, {} triangles",
            mesh.parts.get(part).map_or("mesh", |p| p.as_str()),
            triangles.len()
        );
        let colour = tints.get(part).copied().unwrap_or(BODY);
        for (highlit, tint) in [(false, colour), (true, selected.map_or(colour, |(_, c)| c))] {
            let mut surface = Surface {
                colour: tint,
                material: Ink::Body,
                group: body,
                ..Default::default()
            };
            for t in triangles.iter().filter(|t| picked(t) == highlit) {
                for &i in t {
                    surface.positions.push(point(mesh.positions[i as usize]));
                    surface.normals.push(point(normals[i as usize]));
                }
            }
            if surface.positions.is_empty() {
                continue;
            }
            out.push(surface);
            info.push(FaceInfo {
                body,
                kind: kind.clone(),
                area,
            });
        }
    }
}

pub fn build(
    model: &Model,
    highlights: &[Highlight<'_>],
    marked: &[Edge],
    marked_colour: V3,
) -> Scene {
    let mut scene = build_solids(model, highlights, marked, marked_colour);
    let meshes = model.mesh_tints();
    if meshes.is_empty() {
        return scene;
    }
    let current = model.current_body();
    let selection = model.mesh_body.selection.as_ref();
    let solid_mesh = std::mem::take(&mut scene.mesh);
    let solid_mesh = Arc::try_unwrap(solid_mesh).unwrap_or_default();
    let had_solids = !model.solids().is_empty();
    let (solid_centre, solid_radius) = (solid_mesh.centre, solid_mesh.radius);
    let mut surfaces = solid_mesh.surfaces;
    let mut bounds: Option<(crate::mesh::P3, crate::mesh::P3)> = None;
    for (name, mesh, tints) in &meshes {
        let body = scene.parts.len();
        let tints: Vec<V3> = tints.iter().map(|c| c.map(|v| v as f32)).collect();
        let colour = model
            .colours
            .get(name)
            .map_or(BODY, |c| c.map(|v| v as f32));
        let is_current = *name == current;
        let selected = selection
            .filter(|s| is_current && s.weights.len() == mesh.positions.len())
            .map(|s| {
                (
                    s.weights.as_slice(),
                    highlights.last().map_or(BODY, |h| h.colour),
                )
            });
        mesh_surfaces(
            mesh,
            body,
            &tints,
            selected,
            &mut surfaces,
            &mut scene.faces,
        );
        if let Some((lo, hi)) = mesh.bounds() {
            bounds = Some(match bounds {
                None => (lo, hi),
                Some((a, b)) => (
                    std::array::from_fn(|k| a[k].min(lo[k])),
                    std::array::from_fn(|k| b[k].max(hi[k])),
                ),
            });
        }
        let (lo, hi) = mesh.bounds().unwrap_or_default();
        scene.parts.push(Part {
            name: name.clone(),
            colour,
            opacity: model.opacities.get(name).map_or(1.0, |o| *o as f32),
            volume: mesh.volume(),
            current: is_current,
            source: None,
            material: model
                .materials
                .get(name)
                .map(|m| (m.name.clone(), m.density)),
            size: std::array::from_fn(|k| hi[k] - lo[k]),
            hidden: false,
            role: Role::Body,
        });
    }
    let (mut lo, mut hi) = bounds.unwrap_or_default();
    if had_solids {
        let r = solid_radius as f64;
        let c = solid_centre.map(|v| v as f64);
        lo = std::array::from_fn(|k| lo[k].min(c[k] - r));
        hi = std::array::from_fn(|k| hi[k].max(c[k] + r));
    }
    scene.mesh = Arc::new(viewer3d::Scene {
        surfaces,
        lines: solid_mesh.lines,
        images: solid_mesh.images,
        centre: std::array::from_fn(|k| ((lo[k] + hi[k]) / 2.0) as f32),
        radius: (crate::mesh::length(crate::mesh::sub(hi, lo)) / 2.0).max(1.0e-3) as f32,
        ..Default::default()
    });
    scene
}

fn build_solids(
    model: &Model,
    highlights: &[Highlight<'_>],
    marked: &[Edge],
    marked_colour: V3,
) -> Scene {
    let current = model.current_body();
    let bodies: Vec<(String, &Solid)> = model
        .bodies
        .iter()
        .map(|(name, solid)| (name.clone(), solid))
        .chain(model.solid.as_ref().map(|solid| (current.clone(), solid)))
        .collect();
    let bounds: BoundingBox<Point3> = bodies
        .iter()
        .flat_map(|(_, s)| {
            let b = geometry::bounds(s);
            [b.min(), b.max()]
        })
        .collect();
    let radius = (bounds.diameter() / 2.0).max(1.0e-3) as f32;
    let mut surfaces = Vec::new();
    let mut lines = Vec::new();
    let mut info = Vec::new();
    let mut parts = Vec::new();
    let mut vertices = Vec::new();
    let skip: std::collections::HashSet<_> = marked.iter().map(|m| m.id()).collect();
    let mut textures = super::textures::Library::new();
    let bindings: Vec<_> = bodies
        .iter()
        .map(|(name, _)| {
            let tint = model
                .colours
                .get(name)
                .map_or(BODY, |c| c.map(|v| v as f32));
            model
                .materials
                .get(name)
                .and_then(|m| m.finish.as_ref())
                .and_then(|f| textures.binding(f, tint))
        })
        .collect();
    let built: Vec<_> = {
        use rayon::prelude::*;
        bodies
            .par_iter()
            .enumerate()
            .map(|(body, (name, solid))| {
                let meshed = solid.robust_triangulation(geometry::mesh_tolerance(solid) * 0.5);
                let is_current = *name == current && model.solid.is_some();
                let colour = model
                    .colours
                    .get(name)
                    .map_or(BODY, |c| c.map(|v| v as f32));
                let own: &[Highlight<'_>] = if is_current { highlights } else { &[] };
                let mut surfaces = Vec::new();
                let mut info = Vec::new();
                let ink = model
                    .materials
                    .get(name)
                    .and_then(|m| m.finish.as_ref())
                    .map_or(Ink::Body, |f| Ink::Finished(Finish::new(f, colour)));
                let volume = faces(
                    solid,
                    &meshed,
                    body,
                    colour,
                    ink,
                    own,
                    &mut surfaces,
                    &mut info,
                );
                if let Some(binding) = bindings[body] {
                    let placement = model
                        .placements
                        .get(name)
                        .copied()
                        .unwrap_or_else(Matrix4::identity);
                    super::textures::apply(&mut surfaces, binding, placement);
                }
                let mut edges = Lines {
                    colour: EDGE,
                    material: Ink::Edge,
                    group: body,
                    width: EDGE_WIDTH,
                    ..Default::default()
                };
                let mut snaps = Vec::new();
                edge_lines(solid, &meshed, body, &skip, &mut edges.segments, &mut snaps);
                let mut seen = std::collections::HashSet::new();
                let vertices: Vec<(usize, V3)> = solid
                    .boundaries()
                    .iter()
                    .flat_map(|shell| shell.vertex_iter())
                    .filter(|v| seen.insert(v.id()))
                    .map(|v| (body, v3(v.point())))
                    .collect();
                let extent = geometry::bounds(solid);
                let (low, high) = (extent.min(), extent.max());
                let part = Part {
                    name: name.clone(),
                    colour,
                    opacity: model.opacities.get(name).map_or(1.0, |o| *o as f32),
                    volume,
                    current: is_current,
                    source: model.sources.get(name).map(describe_source),
                    material: model
                        .materials
                        .get(name)
                        .map(|m| (m.name.clone(), m.density)),
                    size: [high.x - low.x, high.y - low.y, high.z - low.z],
                    hidden: false,
                    role: Role::Body,
                };
                (surfaces, info, edges, vertices, snaps, part)
            })
            .collect()
    };
    let mut snap_edges = Vec::new();
    for (body_surfaces, body_info, edges, body_vertices, snaps, part) in built {
        snap_edges.extend(snaps);
        surfaces.extend(body_surfaces);
        info.extend(body_info);
        lines.push(edges);
        vertices.extend(body_vertices);
        parts.push(part);
    }
    let current_index = parts.iter().position(|p| p.current).unwrap_or(0);
    let mut highlighted = Lines {
        colour: marked_colour,
        material: Ink::Marked,
        group: current_index,
        width: MARKED_WIDTH,
        ..Default::default()
    };
    marked
        .iter()
        .for_each(|edge| segments(&polyline(edge), &mut highlighted.segments));
    lines.push(highlighted);
    Scene {
        mesh: Arc::new(viewer3d::Scene {
            surfaces,
            lines,
            images: textures.images,
            centre: v3(bounds.center()),
            radius,
            ..Default::default()
        }),
        parts,
        vertices,
        edges: snap_edges,
        faces: info,
    }
}

pub fn build_field(view: &crate::sim::statics::FieldView) -> Scene {
    let plain = view.values.is_empty();
    let mut surface = Surface {
        colour: if plain { BODY } else { [1.0, 1.0, 1.0] },
        texture: (!plain).then_some(0),
        group: 0,
        ..Default::default()
    };
    for t in &view.triangles {
        let corners = t.map(|v| v3(view.positions[v as usize]));
        let flat = norm(cross(
            sub(corners[1], corners[0]),
            sub(corners[2], corners[0]),
        ));
        for (k, corner) in corners.iter().enumerate() {
            surface.positions.push(*corner);
            surface.normals.push(flat);
            surface
                .uvs
                .push([view.shade[t[k] as usize].clamp(0.002, 0.998) as f32, 0.5]);
        }
    }
    let mut rgba = Vec::with_capacity(256 * 4);
    for k in 0..256 {
        let c = crate::plot::heat(k as f64 / 255.0);
        rgba.extend(c.map(|v| (v * 255.0) as u8));
        rgba.push(255);
    }
    let bounds: BoundingBox<Point3> = view.positions.iter().copied().collect();
    let low = bounds.min();
    let high = bounds.max();
    let short = |v: f64| {
        if v.abs() >= 100.0 {
            crate::plot::label(v.round())
        } else {
            crate::plot::label((v * 100.0).round() / 100.0)
        }
    };
    let name = if plain {
        view.title.clone()
    } else {
        format!(
            "{} ({} to {} {})",
            view.title.trim_start_matches("mean "),
            short(view.low),
            short(view.high),
            view.unit
        )
    };
    let point = |v: u32| v3(view.positions[v as usize]);
    let curve = |edges: &[[u32; 3]], colour: V3, width: f32| {
        let mut out = Lines {
            colour,
            material: Ink::Edge,
            group: 0,
            width,
            ..Default::default()
        };
        for [a, m, b] in edges {
            out.segments.push([point(*a), point(*m)]);
            out.segments.push([point(*m), point(*b)]);
        }
        out
    };
    let mut lines = vec![curve(&view.edges, EDGE, EDGE_WIDTH)];
    for mark in &view.marks {
        let colour = match mark.kind {
            crate::sim::statics::Stimulus::Push => [1.0, 0.31, 0.78],
            crate::sim::statics::Stimulus::Hold => [0.95, 0.95, 0.95],
            crate::sim::statics::Stimulus::Heat => [1.0, 0.67, 0.24],
            crate::sim::statics::Stimulus::Wind => WIND,
        };
        lines.push(curve(&mark.outline, colour, MARKED_WIDTH));
    }
    let mut surfaces = vec![surface];
    let mut images = vec![viewer3d::Image {
        width: 256,
        height: 1,
        rgba,
    }];
    let mut parts = vec![Part {
        name,
        colour: if plain {
            BODY
        } else {
            crate::plot::heat(1.0).map(|v| v as f32)
        },
        opacity: 1.0,
        volume: 0.0,
        current: true,
        source: None,
        material: None,
        size: [high.x - low.x, high.y - low.y, high.z - low.z],
        hidden: false,
        role: Role::Field,
    }];
    let fastest = crate::plot::flow_scale(&view.slices);
    if !view.streams.is_empty() {
        let group = parts.len();
        let bins = 24;
        let mut bands: Vec<Lines> = (0..bins)
            .map(|b| Lines {
                colour: crate::plot::heat((b as f64 + 0.5) / bins as f64).map(|v| v as f32),
                material: Ink::Edge,
                group,
                width: 1.6,
                ..Default::default()
            })
            .collect();
        for stream in &view.streams {
            for (pair, speeds) in stream.points.windows(2).zip(stream.speed.windows(2)) {
                let t = 0.5 * (speeds[0] + speeds[1]) / fastest;
                let band = ((t * bins as f64) as usize).min(bins - 1);
                bands[band]
                    .segments
                    .push([pair[0].map(|v| v as f32), pair[1].map(|v| v as f32)]);
            }
        }
        lines.extend(bands);
        parts.push(Part {
            name: format!(
                "streamlines (0 to {} m/s)",
                crate::plot::label(fastest.round())
            ),
            colour: crate::plot::heat(0.8).map(|v| v as f32),
            opacity: 1.0,
            volume: 0.0,
            current: false,
            source: None,
            material: None,
            size: [0.0; 3],
            hidden: false,
            role: Role::Streams,
        });
    }
    for (index, slice) in view.slices.iter().enumerate() {
        for (moment, values) in [(false, &slice.speed), (true, &slice.snapshot)] {
            let mut rgba = Vec::with_capacity(values.len() * 4);
            for v in values.iter() {
                let c = if *v < 0.0 {
                    [0.59, 0.59, 0.59]
                } else {
                    crate::plot::heat(v / fastest)
                };
                rgba.extend(c.map(|x| (x * 255.0) as u8));
                rgba.push(255);
            }
            images.push(viewer3d::Image {
                width: slice.width,
                height: slice.height(),
                rgba,
            });
            let group = parts.len();
            surfaces.push(slice_quad(slice, images.len() - 1, group));
            parts.push(Part {
                name: format!(
                    "{}, {}",
                    slice
                        .title
                        .trim_end_matches(" along the wind")
                        .trim_end_matches(" along the axis"),
                    if moment { "moment" } else { "mean" }
                ),
                colour: crate::plot::heat(0.5).map(|v| v as f32),
                opacity: 1.0,
                volume: 0.0,
                current: false,
                source: None,
                material: None,
                size: [0.0; 3],
                hidden: true,
                role: Role::Slice { index, moment },
            });
        }
    }
    Scene {
        mesh: Arc::new(viewer3d::Scene {
            surfaces,
            lines,
            images,
            centre: v3(bounds.center()),
            radius: (bounds.diameter() / 2.0).max(1.0e-3) as f32,
            ..Default::default()
        }),
        parts,
        vertices: Vec::new(),
        edges: Vec::new(),
        faces: Vec::new(),
    }
}

fn slice_quad(slice: &crate::sim::Slice, texture: usize, group: usize) -> Surface {
    let o = slice.origin;
    let (right, down) = (slice.right, slice.down);
    let (w, h) = (slice.width as f64, slice.height() as f64);
    let at =
        |c: f64, r: f64| -> V3 { [0, 1, 2].map(|k| (o[k] + c * right[k] + r * down[k]) as f32) };
    let corners = [
        (at(-0.5, -0.5), [0.0, 0.0]),
        (at(w - 0.5, -0.5), [1.0, 0.0]),
        (at(w - 0.5, h - 0.5), [1.0, 1.0]),
        (at(-0.5, h - 0.5), [0.0, 1.0]),
    ];
    let normal = norm(cross(
        sub(corners[1].0, corners[0].0),
        sub(corners[3].0, corners[0].0),
    ));
    let back = normal.map(|v| -v);
    let mut surface = Surface {
        colour: [1.0, 1.0, 1.0],
        texture: Some(texture),
        group,
        ..Default::default()
    };
    for (triangle, n) in [
        ([0, 1, 2], normal),
        ([0, 2, 3], normal),
        ([0, 2, 1], back),
        ([0, 3, 2], back),
    ] {
        for k in triangle {
            surface.positions.push(corners[k].0);
            surface.normals.push(n);
            surface.uvs.push(corners[k].1);
        }
    }
    surface
}

#[cfg(test)]
mod tests {
    use super::{Finish, Ink, SnapEdge, V3};
    use egui_bench::viewer3d::Material;

    fn arc(centre: V3, radius: f32, from: f32, to: f32, n: usize) -> Vec<V3> {
        (0..=n)
            .map(|i| {
                let a = from + (to - from) * i as f32 / n as f32;
                [
                    centre[0] + radius * a.cos(),
                    centre[1],
                    centre[2] + radius * a.sin(),
                ]
            })
            .collect()
    }

    #[test]
    fn a_hole_rim_snaps_to_its_centre() {
        let rim = SnapEdge::new(
            0,
            arc([18.0, 8.0, 6.0], 2.0, 0.0, std::f32::consts::TAU, 24),
        );
        assert!(rim.closed());
        let (centre, radius) = rim.circle.expect("a circle");
        assert!((radius - 2.0).abs() < 1.0e-4, "{radius}");
        assert!(
            centre
                .iter()
                .zip([18.0, 8.0, 6.0])
                .all(|(a, b)| (a - b).abs() < 1.0e-4)
        );
    }

    #[test]
    fn a_short_arc_finds_its_centre_closely() {
        let sliver = SnapEdge::new(0, arc([18.0, 8.0, 16.0], 5.0, 0.3, 0.7, 6));
        let (centre, radius) = sliver.circle.expect("a circle");
        assert!((radius - 5.0).abs() < 1.0e-3, "{radius}");
        assert!(
            centre
                .iter()
                .zip([18.0, 8.0, 16.0])
                .all(|(a, b)| (a - b).abs() < 1.0e-3),
            "{centre:?}"
        );
    }

    #[test]
    fn a_fillet_arc_snaps_to_its_centre_and_middle() {
        let quarter = SnapEdge::new(0, arc([0.0; 3], 5.0, 0.0, std::f32::consts::FRAC_PI_2, 8));
        assert!(!quarter.closed());
        assert!(quarter.circle.is_some());
        let middle = quarter.middle().expect("a middle");
        let expected = 5.0 * std::f32::consts::FRAC_PI_4.cos();
        assert!((middle[0] - expected).abs() < 0.05 && (middle[2] - expected).abs() < 0.05);
    }

    #[test]
    fn straight_and_wavy_edges_have_no_centre() {
        let line = SnapEdge::new(0, (0..6).map(|i| [i as f32, 0.0, 0.0]).collect());
        assert!(line.circle.is_none());
        assert_eq!(line.middle(), Some([2.5, 0.0, 0.0]));
        let wave = SnapEdge::new(
            0,
            (0..12).map(|i| [i as f32, (i as f32).sin(), 0.0]).collect(),
        );
        assert!(wave.circle.is_none());
    }

    #[test]
    fn a_finish_overrides_only_what_it_names() {
        let plain = Ink::Body.pbr();
        let glass = crate::model::materials::Finish {
            transmission: Some(1.0),
            ior: Some(1.52),
            roughness: Some(0.01),
            ..Default::default()
        };
        let pbr = Ink::Finished(Finish::new(&glass, [0.9, 0.95, 0.93])).pbr();
        assert_eq!(
            (pbr.transmission, pbr.ior, pbr.roughness),
            (1.0, 1.52, 0.01)
        );
        assert_eq!(
            (pbr.metallic, pbr.specular),
            (plain.metallic, plain.specular)
        );
        assert_eq!(pbr.emission, [0.0; 3]);
        let lamp = crate::model::materials::Finish {
            emission: Some(4.0),
            ..Default::default()
        };
        let glow = Ink::Finished(Finish::new(&lamp, [1.0, 1.0, 1.0]))
            .pbr()
            .emission;
        assert!(glow.iter().all(|c| (c - 4.0).abs() < 1.0e-5), "{glow:?}");
        assert!(Ink::Finished(Finish::new(&lamp, [1.0; 3])).shading().capped);
    }
}
