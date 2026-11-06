//! Engine-neutral collision declarations. A declaration names a mesh node and the shape a
//! physics engine should use for it. Box, sphere, capsule and hull shapes are fitted to the
//! node's own mesh in node space, so the shape follows the node's transform and any clip
//! that moves it. Mesh shapes use the node's triangles as they are.
//!
//! Nothing is inferred from node names. A declared node is collision only (left out of
//! rendering) unless it says `render=yes`.
use super::scene::Scene;
use anyhow::{Context, Result, bail, ensure};
use monstertruck::modeling::{InnerSpace, Matrix4, Point3, Transform as _};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

/// Hull vertices kept when `max_vertices` is not given. A triangulated hull of 64 vertices has
/// at most 124 faces, which stays inside the 128-vertex and 128-face limits common in physics
/// engines.
pub const DEFAULT_HULL_VERTICES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Box,
    Sphere,
    Capsule,
    Hull,
    Mesh,
}

impl Shape {
    pub fn name(self) -> &'static str {
        match self {
            Shape::Box => "box",
            Shape::Sphere => "sphere",
            Shape::Capsule => "capsule",
            Shape::Hull => "hull",
            Shape::Mesh => "mesh",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Collision {
    pub node: String,
    pub shape: Shape,
    /// Whether consumers still draw the node. Collision-only nodes are hidden.
    pub render: bool,
    /// Hull vertex cap; only meaningful for `Shape::Hull`.
    pub max_vertices: usize,
    /// A free-form surface name for footsteps, decals or physics materials.
    pub surface: Option<String>,
    pub friction: Option<f64>,
    pub restitution: Option<f64>,
}

impl Collision {
    /// Read `collision NODE shape=... [render=yes|no] [max_vertices=N] [surface=NAME]
    /// [friction=F] [restitution=R]`.
    pub fn parse(node: &str, named: &BTreeMap<String, String>) -> Result<Self> {
        for key in named.keys() {
            ensure!(
                matches!(
                    key.as_str(),
                    "shape" | "render" | "max_vertices" | "surface" | "friction" | "restitution"
                ),
                "collision does not take `{key}`"
            );
        }
        let shape = match named.get("shape").map(String::as_str) {
            Some("box") => Shape::Box,
            Some("sphere") => Shape::Sphere,
            Some("capsule") => Shape::Capsule,
            Some("hull") => Shape::Hull,
            Some("mesh") => Shape::Mesh,
            Some(other) => bail!("unknown collision shape `{other}`"),
            None => bail!("collision expects NODE shape=box|sphere|capsule|hull|mesh"),
        };
        let render = match named.get("render").map(String::as_str) {
            None | Some("no") => false,
            Some("yes") => true,
            Some(other) => bail!("collision render expects yes or no, not `{other}`"),
        };
        let max_vertices = match named.get("max_vertices") {
            Some(v) => {
                ensure!(
                    shape == Shape::Hull,
                    "max_vertices applies to hull shapes only"
                );
                v.parse().context("max_vertices expects a whole number")?
            }
            None => DEFAULT_HULL_VERTICES,
        };
        let number = |key: &str| -> Result<Option<f64>> {
            named
                .get(key)
                .map(|v| {
                    v.parse::<f64>()
                        .with_context(|| format!("{key} expects a number"))
                })
                .transpose()
        };
        let collision = Self {
            node: node.to_owned(),
            shape,
            render,
            max_vertices,
            surface: named.get("surface").cloned(),
            friction: number("friction")?,
            restitution: number("restitution")?,
        };
        collision.check_values()?;
        Ok(collision)
    }

    fn check_values(&self) -> Result<()> {
        ensure!(
            !self.node.trim().is_empty() && !self.node.chars().any(char::is_control),
            "collision node name is invalid"
        );
        ensure!(
            (4..=255).contains(&self.max_vertices),
            "collision max_vertices must be 4 to 255"
        );
        if let Some(surface) = &self.surface {
            ensure!(
                !surface.trim().is_empty()
                    && surface.len() <= 256
                    && !surface.chars().any(char::is_control),
                "collision surface name is invalid"
            );
        }
        if let Some(f) = self.friction {
            ensure!(
                f.is_finite() && f >= 0.,
                "collision friction must be zero or more"
            );
        }
        if let Some(r) = self.restitution {
            ensure!(
                r.is_finite() && (0. ..=1.).contains(&r),
                "collision restitution must be 0 to 1"
            );
        }
        Ok(())
    }
}

/// A shape fitted to a node's mesh, in the node's own space (millimetres, Z up).
#[derive(Clone, Debug, PartialEq)]
pub enum Fitted {
    Box {
        centre: [f64; 3],
        half: [f64; 3],
    },
    Sphere {
        centre: [f64; 3],
        radius: f64,
    },
    /// The segment `a`-`b` thickened by `radius`.
    Capsule {
        a: [f64; 3],
        b: [f64; 3],
        radius: f64,
    },
    /// `error` is how far the farthest mesh vertex lies outside the capped hull.
    Hull {
        points: Vec<[f64; 3]>,
        faces: Vec<[usize; 3]>,
        error: f64,
    },
    Mesh {
        triangles: usize,
    },
}

fn node_index(scene: &Scene, name: &str) -> Result<usize> {
    scene
        .nodes
        .iter()
        .position(|n| n.name == name)
        .with_context(|| format!("collision node `{name}` is absent"))
}

fn positions(scene: &Scene, node: usize) -> Result<Vec<[f64; 3]>> {
    let mesh = scene.nodes[node]
        .mesh
        .with_context(|| format!("collision node `{}` has no mesh", scene.nodes[node].name))?;
    Ok(scene.meshes[mesh]
        .primitives
        .iter()
        .flat_map(|p| p.geometry.positions.iter().copied())
        .collect())
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

fn bounds(points: &[[f64; 3]]) -> ([f64; 3], [f64; 3]) {
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for p in points {
        for k in 0..3 {
            low[k] = low[k].min(p[k]);
            high[k] = high[k].max(p[k]);
        }
    }
    (low, high)
}

/// Fit the declared shape to its node's mesh.
pub fn fit(scene: &Scene, collision: &Collision) -> Result<Fitted> {
    let node = node_index(scene, &collision.node)?;
    let points = positions(scene, node)?;
    ensure!(
        !points.is_empty(),
        "collision node `{}` has no vertices",
        collision.node
    );
    let (low, high) = bounds(&points);
    let centre: [f64; 3] = std::array::from_fn(|k| (low[k] + high[k]) * 0.5);
    let flat = |what: &str| {
        format!(
            "collision node `{}` is too flat for a {what}",
            collision.node
        )
    };
    Ok(match collision.shape {
        Shape::Box => {
            let half: [f64; 3] = std::array::from_fn(|k| (high[k] - low[k]) * 0.5);
            ensure!(
                half.iter().all(|&h| h > 1e-6),
                "{}; use shape=mesh",
                flat("box")
            );
            Fitted::Box { centre, half }
        }
        Shape::Sphere => {
            let radius = points
                .iter()
                .map(|&p| length(sub(p, centre)))
                .fold(0., f64::max);
            ensure!(radius > 1e-6, "{}", flat("sphere"));
            Fitted::Sphere { centre, radius }
        }
        Shape::Capsule => {
            let axis = (0..3)
                .max_by(|&i, &j| (high[i] - low[i]).total_cmp(&(high[j] - low[j])))
                .unwrap();
            let along = |p: [f64; 3]| p[axis] - centre[axis];
            let across = |p: [f64; 3]| {
                (0..3)
                    .filter(|&k| k != axis)
                    .map(|k| (p[k] - centre[k]).powi(2))
                    .sum::<f64>()
            };
            let radius = points.iter().map(|&p| across(p)).fold(0., f64::max).sqrt();
            ensure!(radius > 1e-6, "{}", flat("capsule"));
            // Each point needs the segment to reach far enough that its end cap covers it.
            let reach = points
                .iter()
                .map(|&p| along(p).abs() - (radius * radius - across(p)).max(0.).sqrt())
                .fold(0., f64::max);
            let mut a = centre;
            let mut b = centre;
            a[axis] -= reach;
            b[axis] += reach;
            Fitted::Capsule { a, b, radius }
        }
        Shape::Hull => {
            let (points, faces, error) = hull(&points, collision.max_vertices)
                .with_context(|| format!("collision node `{}`", collision.node))?;
            Fitted::Hull {
                points,
                faces,
                error,
            }
        }
        Shape::Mesh => {
            let mesh = scene.nodes[node].mesh.unwrap();
            let triangles = scene.meshes[mesh]
                .primitives
                .iter()
                .map(|p| p.geometry.triangles.len())
                .sum();
            ensure!(
                triangles > 0,
                "collision node `{}` has no triangles",
                collision.node
            );
            Fitted::Mesh { triangles }
        }
    })
}

struct Face {
    v: [usize; 3],
    normal: [f64; 3],
    offset: f64,
}

impl Face {
    fn new(points: &[[f64; 3]], v: [usize; 3]) -> Self {
        let n = cross(
            sub(points[v[1]], points[v[0]]),
            sub(points[v[2]], points[v[0]]),
        );
        let l = length(n).max(f64::MIN_POSITIVE);
        let normal = n.map(|x| x / l);
        Self {
            v,
            normal,
            offset: dot(normal, points[v[0]]),
        }
    }
    fn distance(&self, p: [f64; 3]) -> f64 {
        dot(self.normal, p) - self.offset
    }
}

/// The convex hull of `input`, built by adding the farthest outside point until none is left
/// or `max_vertices` is reached. Returns the hull's points, its outward triangles and how far
/// the farthest left-out point lies outside it.
pub fn hull(
    input: &[[f64; 3]],
    max_vertices: usize,
) -> Result<(Vec<[f64; 3]>, Vec<[usize; 3]>, f64)> {
    ensure!(max_vertices >= 4, "a hull needs at least four vertices");
    let (low, high) = bounds(input);
    let size = length(sub(high, low));
    ensure!(
        size.is_finite() && size > 0.,
        "a hull needs points that are not all the same"
    );
    let eps = size * 1e-9;
    let farthest = |score: &dyn Fn([f64; 3]) -> f64| {
        (0..input.len())
            .max_by(|&i, &j| score(input[i]).total_cmp(&score(input[j])))
            .unwrap()
    };
    let i0 = farthest(&|p| -p[0]);
    let i1 = farthest(&|p| length(sub(p, input[i0])));
    let line = sub(input[i1], input[i0]);
    let i2 = farthest(&|p| length(cross(line, sub(p, input[i0]))));
    let plane = cross(line, sub(input[i2], input[i0]));
    let i3 = farthest(&|p| dot(plane, sub(p, input[i0])).abs());
    ensure!(
        length(line) > eps
            && length(plane) > eps * size
            && dot(plane, sub(input[i3], input[i0])).abs() > eps * size * size,
        "a hull needs a solid with volume; the points are flat. Use shape=mesh or shape=box"
    );
    let centroid: [f64; 3] =
        std::array::from_fn(|k| (input[i0][k] + input[i1][k] + input[i2][k] + input[i3][k]) / 4.);
    let mut faces: Vec<Face> = [[i0, i1, i2], [i0, i3, i1], [i1, i3, i2], [i0, i2, i3]]
        .into_iter()
        .map(|v| {
            let face = Face::new(input, v);
            if face.distance(centroid) > 0. {
                Face::new(input, [v[0], v[2], v[1]])
            } else {
                face
            }
        })
        .collect();
    let mut used = vec![i0, i1, i2, i3];
    let mut outside: Vec<usize> = (0..input.len()).collect();
    let outside_by = |faces: &[Face], p: [f64; 3]| {
        faces
            .iter()
            .map(|f| f.distance(p))
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let mut error = 0.;
    loop {
        // Points inside the hull stay inside as it grows, so they are dropped for good.
        let mut best = None;
        outside.retain(|&i| {
            let d = outside_by(&faces, input[i]);
            if d > eps && best.is_none_or(|(_, b)| d > b) {
                best = Some((i, d));
            }
            d > eps
        });
        let Some((p, d)) = best else {
            break;
        };
        if used.len() >= max_vertices {
            error = d;
            break;
        }
        let visible: Vec<bool> = faces.iter().map(|f| f.distance(input[p]) > eps).collect();
        let edges: HashSet<(usize, usize)> = faces
            .iter()
            .zip(&visible)
            .filter(|(_, v)| **v)
            .flat_map(|(f, _)| [(f.v[0], f.v[1]), (f.v[1], f.v[2]), (f.v[2], f.v[0])])
            .collect();
        let horizon: Vec<(usize, usize)> = edges
            .iter()
            .filter(|(a, b)| !edges.contains(&(*b, *a)))
            .copied()
            .collect();
        let mut keep = visible.iter().map(|v| !v);
        faces.retain(|_| keep.next().unwrap());
        faces.extend(
            horizon
                .into_iter()
                .map(|(a, b)| Face::new(input, [a, b, p])),
        );
        used.push(p);
    }
    // Keep only the points the faces use, renumbered in the order they were added.
    let mut index = vec![usize::MAX; input.len()];
    let mut points = Vec::new();
    for &u in &used {
        if faces.iter().any(|f| f.v.contains(&u)) {
            index[u] = points.len();
            points.push(input[u]);
        }
    }
    let mut out: Vec<[usize; 3]> = faces.iter().map(|f| f.v.map(|v| index[v])).collect();
    out.sort();
    Ok((points, out, error))
}

/// Check every declaration against the scene: one per node, mesh nodes only, and transforms
/// a physics engine can take without rebuilding the shape while the asset moves.
pub fn validate(scene: &Scene) -> Result<()> {
    if scene.collision.is_empty() {
        return Ok(());
    }
    ensure!(
        scene.collision.len() <= 4096,
        "too many collision declarations"
    );
    let world = scene.world_transforms()?;
    let mut seen = HashSet::new();
    for collision in &scene.collision {
        collision.check_values()?;
        let node = node_index(scene, &collision.node)?;
        ensure!(
            seen.insert(node),
            "node `{}` has more than one collision declaration",
            collision.node
        );
        ensure!(
            scene.nodes[node].mesh.is_some(),
            "collision node `{}` has no mesh",
            collision.node
        );
        let m = world[node];
        let columns = [m.x.truncate(), m.y.truncate(), m.z.truncate()];
        let scales = columns.map(|c| c.magnitude());
        if matches!(collision.shape, Shape::Box | Shape::Sphere | Shape::Capsule) {
            for (i, j) in [(0, 1), (1, 2), (0, 2)] {
                ensure!(
                    columns[i].dot(columns[j]).abs() <= 1e-6 * scales[i] * scales[j],
                    "collision node `{}` is sheared by a rotated parent with uneven scale",
                    collision.node
                );
            }
        }
        if matches!(collision.shape, Shape::Sphere | Shape::Capsule) {
            let (lo, hi) = (
                scales.iter().copied().fold(f64::INFINITY, f64::min),
                scales.iter().copied().fold(0., f64::max),
            );
            ensure!(
                hi - lo <= 1e-6 * hi,
                "collision node `{}` has uneven scale, which a {} cannot follow; use a hull",
                collision.node,
                collision.shape.name()
            );
        }
        let mut chain = Some(node);
        let mut steps = 0;
        while let Some(n) = chain {
            for clip in &scene.clips {
                ensure!(
                    !clip
                        .tracks
                        .iter()
                        .any(|t| t.node == n
                            && matches!(t.values, super::animation::Values::Scale(_))),
                    "clip `{}` scales `{}`, which moves collision node `{}`; physics shapes \
                     cannot change size while they move",
                    clip.name,
                    scene.nodes[n].name,
                    collision.node
                );
            }
            chain = scene.nodes[n].parent;
            steps += 1;
            ensure!(steps <= scene.nodes.len(), "node parents form a cycle");
        }
        fit(scene, collision)?;
    }
    Ok(())
}

/// Refuse collision on skinned meshes: their triangles move with bones, not with the node.
/// Put primitive shapes on child nodes of the joints instead.
pub fn validate_skins(scene: &Scene, skins: &[super::skin::Skin]) -> Result<()> {
    for skin in skins {
        let name = &scene.nodes[skin.mesh_node].name;
        ensure!(
            !scene.collision.iter().any(|c| &c.node == name),
            "collision node `{name}` is skinned; give its joints their own collision nodes"
        );
    }
    Ok(())
}

// The same millimetre, Z-up to metre, Y-up change the GLB applies to positions.
fn gltf(p: [f64; 3]) -> [f64; 3] {
    [p[0] / 1000., p[2] / 1000., -p[1] / 1000.]
}

/// The node extras that describe a declaration in the exported GLB, in the node's glTF space.
pub fn extras(scene: &Scene, collision: &Collision) -> Result<Value> {
    let mut value = json!({
        "version": 1,
        "shape": collision.shape.name(),
        "render": collision.render,
        "length_unit": "metre",
        "up_axis": "Y",
    });
    match fit(scene, collision)? {
        Fitted::Box { centre, half } => {
            value["centre"] = json!(gltf(centre));
            value["half_extents"] = json!([half[0] / 1000., half[2] / 1000., half[1] / 1000.]);
        }
        Fitted::Sphere { centre, radius } => {
            value["centre"] = json!(gltf(centre));
            value["radius"] = json!(radius / 1000.);
        }
        Fitted::Capsule { a, b, radius } => {
            value["a"] = json!(gltf(a));
            value["b"] = json!(gltf(b));
            value["radius"] = json!(radius / 1000.);
        }
        Fitted::Hull { points, error, .. } => {
            value["points"] = json!(points.iter().map(|&p| gltf(p)).collect::<Vec<_>>());
            value["error"] = json!(error / 1000.);
        }
        Fitted::Mesh { triangles } => {
            value["triangles"] = json!(triangles);
        }
    }
    if let Some(surface) = &collision.surface {
        value["surface"] = json!(surface);
    }
    if let Some(f) = collision.friction {
        value["friction"] = json!(f);
    }
    if let Some(r) = collision.restitution {
        value["restitution"] = json!(r);
    }
    Ok(value)
}

/// Triangles that draw a fitted shape, in node space. Mesh shapes return none: the node's own
/// mesh is the shape.
pub fn display_triangles(fitted: &Fitted) -> Vec<[[f64; 3]; 3]> {
    let ring: usize = 16;
    let rows: usize = 8;
    let sphere_point =
        |centre: [f64; 3], radius: f64, row: usize, col: usize, axis: usize, lift: f64| {
            let polar = std::f64::consts::PI * row as f64 / rows as f64;
            let around = std::f64::consts::TAU * col as f64 / ring as f64;
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            let mut p = centre;
            p[axis] += radius * polar.cos() + if polar.cos() >= 0. { lift } else { -lift };
            p[u] += radius * polar.sin() * around.cos();
            p[v] += radius * polar.sin() * around.sin();
            p
        };
    let round = |centre: [f64; 3], radius: f64, axis: usize, lift: f64| {
        let mut out = Vec::new();
        // The equator row is lifted with the top half, so for a capsule the band below it
        // stretches into the cylinder between the caps.
        for row in 0..rows {
            for col in 0..ring {
                let p = |r, c| sphere_point(centre, radius, r, c % ring, axis, lift);
                let (a, b, c, d) = (
                    p(row, col),
                    p(row, col + 1),
                    p(row + 1, col),
                    p(row + 1, col + 1),
                );
                out.push([a, c, b]);
                out.push([b, c, d]);
            }
        }
        out
    };
    match fitted {
        Fitted::Box { centre, half } => {
            let corner = |i: usize| -> [f64; 3] {
                std::array::from_fn(|k| {
                    centre[k] + if i & (1 << k) == 0 { -half[k] } else { half[k] }
                })
            };
            let quads = [
                [0, 4, 6, 2],
                [1, 3, 7, 5],
                [0, 1, 5, 4],
                [2, 6, 7, 3],
                [0, 2, 3, 1],
                [4, 5, 7, 6],
            ];
            quads
                .iter()
                .flat_map(|q| {
                    [
                        [corner(q[0]), corner(q[1]), corner(q[2])],
                        [corner(q[0]), corner(q[2]), corner(q[3])],
                    ]
                })
                .collect()
        }
        Fitted::Sphere { centre, radius } => round(*centre, *radius, 2, 0.),
        Fitted::Capsule { a, b, radius } => {
            let axis = (0..3)
                .max_by(|&i, &j| (b[i] - a[i]).abs().total_cmp(&(b[j] - a[j]).abs()))
                .unwrap();
            let centre: [f64; 3] = std::array::from_fn(|k| (a[k] + b[k]) * 0.5);
            round(centre, *radius, axis, (b[axis] - a[axis]).abs() * 0.5)
        }
        Fitted::Hull { points, faces, .. } => faces
            .iter()
            .map(|f| [points[f[0]], points[f[1]], points[f[2]]])
            .collect(),
        Fitted::Mesh { .. } => Vec::new(),
    }
}

/// Place node-space triangles in the world with a node's matrix.
pub fn place(matrix: Matrix4, triangles: &[[[f64; 3]; 3]]) -> Vec<[[f64; 3]; 3]> {
    triangles
        .iter()
        .map(|t| {
            t.map(|p| {
                let q = matrix.transform_point(Point3::new(p[0], p[1], p[2]));
                [q.x, q.y, q.z]
            })
        })
        .collect()
}

/// The outward normal of a triangle, or up for a degenerate one.
pub fn triangle_normal(t: &[[f64; 3]; 3]) -> [f64; 3] {
    let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
    let l = length(n);
    if l > 0. {
        n.map(|x| x / l)
    } else {
        [0., 0., 1.]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> super::super::author::Asset {
        super::super::author::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples/game-assets/plate-collision.gasset"),
            &[],
        )
        .unwrap()
    }

    #[test]
    fn the_fixture_exports_each_shape_on_its_node() {
        let asset = fixture();
        assert_eq!(asset.scene.collision.len(), 5);
        let bytes = asset.encode_glb().unwrap();
        let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let glb: Value = serde_json::from_slice(&bytes[20..20 + length]).unwrap();
        let node = |name: &str| {
            glb["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|n| n["name"] == name)
                .unwrap()
                .clone()
        };
        assert!(node("model").get("extras").is_none());
        let plate = &node("plate")["extras"]["gcad_collision"];
        assert_eq!(plate["shape"], "box");
        assert_eq!(plate["render"], true);
        assert_eq!(plate["surface"], "metal");
        assert_eq!(plate["friction"], 0.6);
        // 40 by 30 by 3 mm, Z up, becomes 0.04 by 0.003 by 0.03 m, Y up.
        let half: Vec<f64> = serde_json::from_value(plate["half_extents"].clone()).unwrap();
        for (h, want) in half.iter().zip([0.02, 0.0015, 0.015]) {
            assert!((h - want).abs() < 1e-9, "{half:?}");
        }
        let ball = &node("ball")["extras"]["gcad_collision"];
        assert_eq!(
            (ball["shape"].as_str(), ball["render"].as_bool()),
            (Some("sphere"), Some(false))
        );
        assert!(ball["radius"].as_f64().unwrap() > 0.02);
        let rough = &node("rough")["extras"]["gcad_collision"];
        assert_eq!(rough["points"].as_array().unwrap().len(), 16);
        assert!(rough["error"].as_f64().unwrap() > 0.);
        assert!(
            node("exact")["extras"]["gcad_collision"]["triangles"]
                .as_u64()
                .unwrap()
                > 12
        );
        assert_eq!(node("lid")["extras"]["gcad_collision"]["shape"], "capsule");
        // Collision-only nodes keep their meshes, so any glTF tool still sees them.
        assert!(node("exact").get("mesh").is_some());
    }

    #[test]
    fn declarations_refuse_what_physics_cannot_follow() {
        let asset = fixture();
        let refuse = |change: &dyn Fn(&mut Scene)| {
            let mut scene = asset.scene.clone();
            change(&mut scene);
            validate(&scene).unwrap_err().to_string()
        };
        assert!(refuse(&|s| s.collision[0].node = "model".into()).contains("no mesh"));
        assert!(refuse(&|s| s.collision[0].node = "absent".into()).contains("absent"));
        assert!(refuse(&|s| s.collision[1].node = "plate".into()).contains("more than one"));
        let lid = asset
            .scene
            .nodes
            .iter()
            .position(|n| n.name == "lid")
            .unwrap();
        let model = asset
            .scene
            .nodes
            .iter()
            .position(|n| n.name == "model")
            .unwrap();
        let message = refuse(&|s| {
            s.clips[0].tracks.push(super::super::animation::Track {
                node: model,
                times: vec![0., 1.],
                interpolation: super::super::animation::Interpolation::Linear,
                values: super::super::animation::Values::Scale(vec![[1.; 3], [2.; 3]]),
            })
        });
        assert!(message.contains("cannot change size"), "{message}");
        assert!(refuse(&|s| s.nodes[lid].transform.scale = [1., 2., 1.]).contains("uneven scale"));
        let mut rotated = asset.scene.clone();
        rotated.nodes[model].transform.scale = [1., 3., 1.];
        rotated.nodes[lid].transform.rotation = [0., 0., 0.38268343236508984, 0.9238795325112867];
        rotated.collision.retain(|c| c.node == "lid");
        rotated.collision[0].shape = Shape::Box;
        assert!(
            validate(&rotated)
                .unwrap_err()
                .to_string()
                .contains("sheared")
        );
        rotated.collision[0].shape = Shape::Hull;
        validate(&rotated).unwrap();
    }

    fn cube(size: f64) -> Vec<[f64; 3]> {
        (0..8)
            .map(|i| std::array::from_fn(|k| if i & (1 << k) == 0 { 0. } else { size }))
            .collect()
    }

    #[test]
    fn hull_of_a_cube_with_inside_points_is_the_cube() {
        let mut points = cube(10.);
        points.extend([[5., 5., 5.], [2., 3., 4.], [9., 9., 1.]]);
        let (hull, faces, error) = hull(&points, 64).unwrap();
        assert_eq!(hull.len(), 8);
        assert_eq!(faces.len(), 12);
        assert_eq!(error, 0.);
        // Every face points away from the middle.
        for f in &faces {
            let face = Face::new(&hull, *f);
            assert!(face.distance([5., 5., 5.]) < 0.);
        }
    }

    #[test]
    fn a_capped_hull_reports_how_much_it_leaves_out() {
        let points: Vec<[f64; 3]> = (0..400)
            .map(|i| {
                let a = i as f64 * 2.399_963;
                let z = 1. - 2. * (i as f64 + 0.5) / 400.;
                let r = (1. - z * z).sqrt();
                [100. * r * a.cos(), 100. * r * a.sin(), 100. * z]
            })
            .collect();
        let (all, _, none) = hull(&points, 255).unwrap();
        assert_eq!(all.len(), 255);
        assert!(none > 0.);
        let (capped, faces, error) = hull(&points, 16).unwrap();
        assert_eq!(capped.len(), 16);
        assert_eq!(faces.len(), 2 * 16 - 4);
        assert!(error > none && error < 50., "error {error}");
        assert!(hull(&[[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [1., 1., 0.]], 8).is_err());
    }

    #[test]
    fn declarations_parse_and_refuse_bad_values() {
        let named = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        };
        let c = Collision::parse("crate", &named(&[("shape", "box"), ("render", "yes")])).unwrap();
        assert_eq!((c.shape, c.render), (Shape::Box, true));
        let c = Collision::parse(
            "rock",
            &named(&[
                ("shape", "hull"),
                ("max_vertices", "32"),
                ("surface", "stone"),
                ("friction", "0.8"),
            ]),
        )
        .unwrap();
        assert_eq!((c.max_vertices, c.render), (32, false));
        assert_eq!(c.surface.as_deref(), Some("stone"));
        for bad in [
            vec![("shape", "cone")],
            vec![("render", "yes")],
            vec![("shape", "box"), ("max_vertices", "8")],
            vec![("shape", "hull"), ("max_vertices", "3")],
            vec![("shape", "box"), ("restitution", "2")],
            vec![("shape", "box"), ("friction", "-1")],
            vec![("shape", "box"), ("mass", "1")],
        ] {
            assert!(Collision::parse("n", &named(&bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn capsule_covers_every_point_and_box_triangles_close() {
        let mut points = cube(10.);
        for p in &mut points {
            p[2] *= 4.;
        }
        let mut scene = Scene::default();
        scene.meshes.push(super::super::scene::SceneMesh {
            name: "m".into(),
            primitives: vec![super::super::scene::Primitive {
                geometry: super::super::Mesh {
                    positions: points.clone(),
                    normals: vec![[0., 0., 1.]; 8],
                    texcoords: None,
                    tangents: None,
                    triangles: vec![[0, 1, 2]],
                },
                material: 0,
            }],
        });
        scene.nodes.push(super::super::scene::Node {
            name: "post".into(),
            mesh: Some(0),
            ..Default::default()
        });
        let mut c = Collision::parse(
            "post",
            &[("shape".to_string(), "capsule".to_string())].into(),
        )
        .unwrap();
        let Fitted::Capsule { a, b, radius } = fit(&scene, &c).unwrap() else {
            panic!("a capsule")
        };
        assert!((radius - 50f64.sqrt()).abs() < 1e-9);
        for p in &points {
            let t = ((p[2] - a[2]) / (b[2] - a[2])).clamp(0., 1.);
            let q = [a[0], a[1], a[2] + t * (b[2] - a[2])];
            assert!(length(sub(*p, q)) <= radius + 1e-9);
        }
        c.shape = Shape::Box;
        let fitted = fit(&scene, &c).unwrap();
        assert_eq!(
            fitted,
            Fitted::Box {
                centre: [5., 5., 20.],
                half: [5., 5., 20.]
            }
        );
        let triangles = display_triangles(&fitted);
        assert_eq!(triangles.len(), 12);
        for t in &triangles {
            let n = triangle_normal(t);
            let middle: [f64; 3] = std::array::from_fn(|k| (t[0][k] + t[1][k] + t[2][k]) / 3.);
            assert!(
                dot(n, sub(middle, [5., 5., 20.])) > 0.,
                "box faces point out"
            );
        }
    }
}
