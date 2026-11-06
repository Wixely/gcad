use super::Model;
use super::args::{Args, label_of, positive};
use super::mate;
use super::skin::{interpolate, knots, uniform};
use crate::geometry;
use crate::parse::Line;
use crate::select;
use anyhow::{Result, anyhow, bail};
use monstertruck::meshing::prelude::*;
use monstertruck::modeling::*;
use std::collections::HashSet;
use std::f64::consts::TAU;

const SPANS_PER_TURN: usize = 64;
const END_SAMPLES: usize = 33;

pub(crate) fn thread_named(name: &str) -> Result<(f64, f64, f64)> {
    super::fastener::named(name)
        .map(|m| (m.major, m.tap, m.pitch))
        .map_err(|_| {
            anyhow!(
                "unknown thread `{name}`; known: {}",
                super::fastener::METRIC
                    .iter()
                    .map(|m| m.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

pub(crate) fn basic_profile(major: f64, pitch: f64) -> Vec<(f64, f64)> {
    let crest = major / 2.0;
    let root = crest - 5.0 / 8.0 * 3.0_f64.sqrt() / 2.0 * pitch;
    vec![
        (0.0, crest),
        (pitch / 8.0, crest),
        (pitch * 7.0 / 16.0, root),
        (pitch * 11.0 / 16.0, root),
        (pitch, crest),
    ]
}

fn slope(a: (f64, f64), b: (f64, f64)) -> f64 {
    (b.1 - a.1) / (b.0 - a.0)
}

pub(crate) fn clipped(profile: &[(f64, f64)], radius: f64, external: bool) -> Vec<(f64, f64)> {
    let clamp = |r: f64| {
        if external {
            r.min(radius)
        } else {
            r.max(radius)
        }
    };
    let pitch = profile[profile.len() - 1].0 - profile[0].0;
    let mut points = vec![(profile[0].0, clamp(profile[0].1))];
    for pair in profile.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if (a.1 - radius) * (b.1 - radius) < 0.0 {
            let t = (radius - a.1) / (b.1 - a.1);
            points.push((a.0 + (b.0 - a.0) * t, radius));
        }
        points.push((b.0, clamp(b.1)));
    }
    points.dedup_by(|b, a| (b.0 - a.0).abs() < pitch * 1.0e-9);
    let same = |x: f64, y: f64| (x - y).abs() < 1.0e-9;
    let mut merged: Vec<(f64, f64)> = vec![points[0]];
    for &point in &points[1..] {
        if merged.len() >= 2 {
            let before = merged[merged.len() - 2];
            let last = merged[merged.len() - 1];
            if same(slope(before, last), slope(last, point)) {
                merged.pop();
            }
        }
        merged.push(point);
    }
    let n = merged.len() - 1;
    if n >= 2 && same(slope(merged[n - 1], merged[n]), slope(merged[0], merged[1])) {
        let next = merged[1];
        merged.remove(0);
        merged.pop();
        merged.push((next.0 + pitch, next.1));
    }
    merged
}

struct Helix {
    origin: Point3,
    along: Vector3,
    x: Vector3,
    y: Vector3,
    pitch: f64,
}

impl Helix {
    fn at(&self, theta: f64, s: f64, r: f64) -> Point3 {
        self.origin
            + (self.x * theta.cos() + self.y * theta.sin()) * r
            + self.along * (s + self.pitch * theta / TAU)
    }

    fn point(&self, angle: f64, r: f64, height: f64) -> Point3 {
        self.origin + (self.x * angle.cos() + self.y * angle.sin()) * r + self.along * height
    }

    fn radial(&self, p: Point3) -> Vector3 {
        let d = p - self.origin;
        d - self.along * self.along.dot(d)
    }
}

struct Ends {
    axis: Vector3,
    centre: Point3,
    radius: f64,
    bottom: f64,
    top: f64,
}

fn edge_points(edge: &Edge) -> Vec<Point3> {
    let curve = edge.oriented_curve();
    let (t0, t1) = curve.range_tuple();
    (0..=8)
        .map(|i| curve.subs(t0 + (t1 - t0) * i as f64 / 8.0))
        .collect()
}

fn plane_normal(points: &[Point3]) -> Vector3 {
    let first = points[0];
    let far = |from: &dyn Fn(Point3) -> f64| {
        points
            .iter()
            .copied()
            .max_by(|a, b| from(*a).total_cmp(&from(*b)))
            .expect("points")
    };
    let second = far(&|p| p.distance(first));
    let chord = (second - first).normalize();
    let third = far(&|p| (p - first).cross(chord).magnitude());
    (second - first).cross(third - first)
}

fn ends_of(edges: &[Edge], rough: &mate::Cylinder, tolerance: f64) -> Result<Ends> {
    let points: Vec<Point3> = edges.iter().flat_map(edge_points).collect();
    let height = |p: &Point3| rough.axis.dot(p.to_vec());
    let (low, high) = points
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| {
            (a.min(height(p)), b.max(height(p)))
        });
    let middle = (low + high) / 2.0;
    let bottom: Vec<Point3> = points
        .iter()
        .copied()
        .filter(|p| height(p) < middle)
        .collect();
    let axis = plane_normal(&bottom);
    let axis = if axis.dot(rough.axis) < 0.0 {
        -axis
    } else {
        axis
    }
    .normalize();
    let u = axis
        .cross(if axis.x.abs() < 0.9 {
            Vector3::unit_x()
        } else {
            Vector3::unit_y()
        })
        .normalize();
    let v = axis.cross(u);
    let flat: Vec<(f64, f64)> = bottom
        .iter()
        .map(|p| (p.to_vec().dot(u), p.to_vec().dot(v)))
        .collect();
    let ((cu, cv), radius, _) =
        mate::fit_circle(&flat).ok_or_else(|| anyhow!("the round face's ends are not circles"))?;
    let centre = Point3::from_vec(u * cu + v * cv);
    let slack = tolerance * 100.0 + radius * 1.0e-6;
    let along = |p: &Point3| axis.dot(*p - centre);
    let bottom_at = bottom.iter().map(along).sum::<f64>() / bottom.len() as f64;
    let top: Vec<f64> = points
        .iter()
        .filter(|p| height(p) >= middle)
        .map(along)
        .collect();
    let top_at = top.iter().sum::<f64>() / top.len() as f64;
    let flat_ends = points.iter().all(|p| {
        let a = along(p);
        let off = ((*p - centre) - axis * a).magnitude() - radius;
        ((a - bottom_at).abs() < slack || (a - top_at).abs() < slack) && off.abs() < slack
    });
    if !flat_ends {
        bail!("a thread goes on a plain round face that runs between two flat ends");
    }
    Ok(Ends {
        axis,
        centre,
        radius,
        bottom: bottom_at,
        top: top_at,
    })
}

fn turning(points: &[Point3], centre: Point3, axis: Vector3) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| {
            (points[i] - centre)
                .cross(points[(i + 1) % n] - centre)
                .dot(axis)
        })
        .sum()
}

fn wire_points(wire: &Wire) -> Vec<Point3> {
    wire.edge_iter()
        .flat_map(|e| {
            let mut points = edge_points(e);
            points.pop();
            points
        })
        .collect()
}

fn capped(cap: &Face, end: &HashSet<EdgeId>, new_loop: &Wire, helix: &Helix) -> Result<Face> {
    let mut replaced = false;
    let wires = cap
        .absolute_boundaries()
        .iter()
        .map(|wire| {
            if !wire.edge_iter().all(|e| end.contains(&e.id())) {
                return wire.clone();
            }
            replaced = true;
            let old = turning(&wire_points(wire), helix.origin, helix.along);
            let new = turning(&wire_points(new_loop), helix.origin, helix.along);
            if old * new > 0.0 {
                new_loop.clone()
            } else {
                new_loop.inverse()
            }
        })
        .collect::<Vec<Wire>>();
    if !replaced {
        bail!("a thread's end face must meet the thread along a whole loop");
    }
    let mut face = Face::try_new(wires, cap.surface())
        .map_err(|error| anyhow!("threaded end face: {error}"))?;
    if !cap.orientation() {
        face.invert();
    }
    Ok(face)
}

struct Rail {
    params: Vec<f64>,
    vertices: Vec<Vertex>,
    edges: Vec<Edge>,
}

impl Rail {
    fn new(curve: &BsplineCurve<Point3>, params: Vec<f64>) -> Rail {
        let vertices: Vec<Vertex> = params
            .iter()
            .map(|&t| builder::vertex(curve.subs(t)))
            .collect();
        let edges = params
            .windows(2)
            .enumerate()
            .map(|(i, pair)| {
                let mut whole = curve.clone();
                let mut piece = whole.cut(pair[0]);
                piece.cut(pair[1]);
                Edge::new(&vertices[i], &vertices[i + 1], Curve::BsplineCurve(piece))
            })
            .collect();
        Rail {
            params,
            vertices,
            edges,
        }
    }

    fn index(&self, theta: f64) -> usize {
        self.params
            .iter()
            .position(|t| (t - theta).abs() < 1.0e-9)
            .expect("a vertex on the rail")
    }

    fn vertex(&self, theta: f64) -> &Vertex {
        &self.vertices[self.index(theta)]
    }

    fn run(&self, from: f64, to: f64) -> Vec<Edge> {
        self.edges[self.index(from)..self.index(to)].to_vec()
    }
}

pub(crate) struct Threaded {
    pub(crate) solid: Solid,
    pub(crate) strips: Vec<Surface>,
    pub(crate) external: bool,
    pub(crate) length: f64,
}

enum Kind {
    Flat {
        cap: usize,
    },
    Cone {
        faces: Vec<usize>,
        cap: usize,
        far_edges: Vec<Edge>,
        far_radius: f64,
        far_height: f64,
    },
}

struct End {
    near: f64,
    kind: Kind,
}

impl End {
    fn height_at(&self, r: f64, radius: f64) -> f64 {
        match &self.kind {
            Kind::Flat { .. } => self.near,
            Kind::Cone {
                far_radius,
                far_height,
                ..
            } => self.near + (far_height - self.near) * (radius - r) / (radius - far_radius),
        }
    }
}

fn push_new<T: PartialEq>(list: &mut Vec<T>, item: T) {
    if !list.contains(&item) {
        list.push(item);
    }
}

struct Around<'a> {
    faces: &'a [Face],
    owners: &'a std::collections::HashMap<EdgeId, Vec<usize>>,
    mine: &'a HashSet<usize>,
    helix: &'a Helix,
    radius: f64,
    slack: f64,
}

fn end_of(around: &Around, edges: &[Edge], near: f64) -> Result<End> {
    let Around {
        faces,
        owners,
        mine,
        helix,
        radius,
        slack,
    } = *around;
    let mut neighbours = Vec::new();
    for edge in edges {
        for &f in &owners[&edge.id()] {
            if !mine.contains(&f) {
                push_new(&mut neighbours, f);
            }
        }
    }
    let plane = |f: usize| match faces[f].oriented_surface() {
        Surface::Plane(plane) => Some(plane.normal().cross(helix.along).magnitude() < 1.0e-6),
        _ => None,
    };
    if let [cap] = neighbours[..]
        && plane(cap) == Some(true)
    {
        return Ok(End {
            near,
            kind: Kind::Flat { cap },
        });
    }
    if neighbours.iter().any(|&f| plane(f).is_some()) {
        bail!("a thread's ends must be flat faces square to its axis, or a chamfer");
    }
    let mut far_edges: Vec<Edge> = Vec::new();
    let mut caps = Vec::new();
    for &f in &neighbours {
        for edge in faces[f].edge_iter() {
            let owned = &owners[&edge.id()];
            if owned.iter().any(|o| mine.contains(o)) {
                continue;
            }
            let others: Vec<usize> = owned
                .iter()
                .copied()
                .filter(|o| !neighbours.contains(o))
                .collect();
            if others.is_empty() {
                continue;
            }
            if !far_edges.iter().any(|e| e.is_same(&edge)) {
                far_edges.push(edge.clone());
            }
            others.into_iter().for_each(|o| push_new(&mut caps, o));
        }
    }
    let [cap] = caps[..] else {
        bail!("a chamfer at a thread's end must lead to a single face");
    };
    let height = |p: Point3| helix.along.dot(p - helix.origin);
    let corners: Vec<Point3> = far_edges
        .iter()
        .flat_map(|e| [e.front().point(), e.back().point()])
        .collect();
    let far_radius = corners
        .iter()
        .map(|p| helix.radial(*p).magnitude())
        .sum::<f64>()
        / corners.len() as f64;
    let far_height = corners.iter().map(|p| height(*p)).sum::<f64>() / corners.len() as f64;
    let rough = radius * 0.02 + (far_radius - radius).abs() * 0.05 + slack;
    let far: Vec<Point3> = far_edges.iter().flat_map(edge_points).collect();
    let on_circle = corners.iter().all(|p| {
        (helix.radial(*p).magnitude() - far_radius).abs() < slack
            && (height(*p) - far_height).abs() < slack
    }) && far.iter().all(|p| {
        (helix.radial(*p).magnitude() - far_radius).abs() < rough
            && (height(*p) - far_height).abs() < rough
    });
    if !on_circle || (far_height - near).abs() < slack || (far_radius - radius).abs() < slack {
        bail!("a thread's end must be flat or a chamfer around its axis");
    }
    let mut seams: Vec<Edge> = Vec::new();
    for &f in &neighbours {
        for edge in faces[f].edge_iter() {
            let shared = owners[&edge.id()]
                .iter()
                .filter(|o| neighbours.contains(o))
                .count()
                == 2;
            if shared && !seams.iter().any(|e| e.is_same(&edge)) {
                seams.push(edge.clone());
            }
        }
    }
    let straight = seams.iter().all(|edge| {
        let points = edge_points(edge);
        let (a, b) = (points[0], points[points.len() - 1]);
        let chord = (b - a).normalize();
        points
            .iter()
            .all(|p| (*p - a).cross(chord).magnitude() < slack * 10.0)
    });
    if !straight {
        bail!("a thread's end must be flat or a chamfer around its axis, not a round");
    }
    let mesh_tolerance = (radius * 1.0e-3).max(1.0e-4);
    for &f in &neighbours {
        let single: Shell = vec![faces[f].clone()].into();
        let mesh = single.robust_triangulation(mesh_tolerance).to_polygon();
        let conical = mesh.positions().iter().all(|p| {
            let expected =
                radius + (far_radius - radius) * (height(*p) - near) / (far_height - near);
            (helix.radial(*p).magnitude() - expected).abs() < rough
        });
        if !conical {
            bail!("a thread's end must be flat or a chamfer around its axis, not a round");
        }
    }
    Ok(End {
        near,
        kind: Kind::Cone {
            faces: neighbours,
            cap,
            far_edges,
            far_radius,
            far_height,
        },
    })
}

fn seam_angles(segments: &[(f64, f64)]) -> Result<Vec<f64>> {
    let samples = 720;
    let wrap = |a: f64| a.rem_euclid(TAU);
    let gap = |a: f64, b: f64| {
        let d = wrap(a - b);
        d.min(TAU - d)
    };
    let margin = 2.0_f64.to_radians();
    let single: Vec<bool> = (0..samples)
        .map(|i| {
            let phi = TAU * i as f64 / samples as f64;
            let count: usize = segments
                .iter()
                .map(|&(a, b)| {
                    let (lo, hi) = (a.min(b), a.max(b));
                    let first = ((lo - phi) / TAU).ceil() as i64;
                    let last = ((hi - phi) / TAU).floor() as i64;
                    (last - first + 1).max(0) as usize
                })
                .sum();
            count == 1
                && segments
                    .iter()
                    .all(|&(a, b)| gap(phi, a) > margin && gap(phi, b) > margin)
        })
        .collect();
    let at = |i: usize| TAU * i as f64 / samples as f64;
    let Some(anchor) = (0..samples).max_by_key(|&i| {
        (0..samples)
            .take_while(|&d| single[(i + d) % samples] && single[(i + samples - d) % samples])
            .count()
    }) else {
        bail!("no room for a seam");
    };
    if !single[anchor] {
        bail!("a chamfer this steep folds the thread's end over itself everywhere");
    }
    let mut seams: Vec<f64> = vec![at(anchor)];
    for k in 1..4 {
        let target = at(anchor) + TAU * k as f64 / 4.0;
        let best = (0..samples)
            .filter(|&i| single[i])
            .min_by(|&a, &b| gap(at(a), target).total_cmp(&gap(at(b), target)));
        if let Some(i) = best
            && gap(at(i), target) < 40.0_f64.to_radians()
            && seams.iter().all(|&s| gap(s, at(i)) > 30.0_f64.to_radians())
        {
            seams.push(at(i));
        }
    }
    if seams.len() < 2 {
        bail!("a chamfer this steep folds the thread's end over itself too far round");
    }
    seams.sort_by(f64::total_cmp);
    Ok(seams)
}

fn cone_patch(
    helix: &Helix,
    near: (f64, f64),
    far: (f64, f64),
    from: f64,
    to: f64,
) -> (NurbsSurface<Vector4>, NurbsCurve<Vector4>) {
    let spans = ((to - from) / (TAU / 4.0)).ceil().max(1.0) as usize;
    let span = (to - from) / spans as f64;
    let half = (span / 2.0).cos();
    let mut values = vec![0.0; 3];
    for i in 1..spans {
        values.extend([i as f64, i as f64]);
    }
    values.extend([spans as f64; 3]);
    let row = |angle: f64, scale: f64, weight: f64| -> Vec<Vector4> {
        [near, far]
            .iter()
            .map(|&(r, h)| (helix.point(angle, r * scale, h).to_vec() * weight).extend(weight))
            .collect()
    };
    let mut rows = vec![row(from, 1.0, 1.0)];
    for i in 0..spans {
        let a = from + span * i as f64;
        rows.push(row(a + span / 2.0, 1.0 / half, half));
        rows.push(row(a + span, 1.0, 1.0));
    }
    let knots = KnotVector::from(values);
    let arc = NurbsCurve::new(BsplineCurve::new(
        knots.clone(),
        rows.iter().map(|r| r[1]).collect(),
    ));
    let surface = NurbsSurface::new(BsplineSurface::new(
        (knots, KnotVector::bezier_knot(1)),
        rows,
    ));
    (surface, arc)
}

struct Crossing {
    angle: f64,
    vertex: Vertex,
}

fn chamfer_faces(
    end: &End,
    loop_edges: &[Edge],
    crossings: &[Crossing],
    helix: &Helix,
    radius: f64,
    external: bool,
) -> Result<(Vec<Face>, Wire)> {
    let Kind::Cone {
        far_radius,
        far_height,
        ..
    } = end.kind
    else {
        unreachable!("chamfered ends only")
    };
    let place = |vertex: &Vertex| {
        loop_edges
            .iter()
            .position(|e| e.front() == vertex)
            .expect("a crossing on the end loop")
    };
    let m = crossings.len();
    let far_vertices: Vec<Vertex> = crossings
        .iter()
        .map(|c| builder::vertex(helix.point(c.angle, far_radius, far_height)))
        .collect();
    let rulings: Vec<Edge> = crossings
        .iter()
        .zip(&far_vertices)
        .map(|(c, far)| {
            Edge::new(
                &c.vertex,
                far,
                Curve::Line(Line(c.vertex.point(), far.point())),
            )
        })
        .collect();
    let mut pieces = Vec::new();
    let mut arcs = Vec::new();
    for i in 0..m {
        let next = (i + 1) % m;
        let from = crossings[i].angle;
        let to = crossings[next].angle + if next == 0 { TAU } else { 0.0 };
        let (surface, arc) = cone_patch(
            helix,
            (radius, end.near),
            (far_radius, far_height),
            from,
            to,
        );
        let arc = Edge::new(
            &far_vertices[i],
            &far_vertices[next],
            Curve::NurbsCurve(arc),
        );
        let (a, b) = (place(&crossings[next].vertex), place(&crossings[i].vertex));
        let count = (b + loop_edges.len() - a) % loop_edges.len();
        let mut wire: Vec<Edge> = (0..count)
            .rev()
            .map(|d| loop_edges[(a + d) % loop_edges.len()].inverse())
            .collect();
        wire.push(rulings[next].clone());
        wire.push(arc.inverse());
        wire.push(rulings[i].inverse());
        let surface = Surface::NurbsSurface(surface);
        let (u0, u1) = surface.try_range_tuple().0.expect("bounded");
        let middle = (u0 + u1) / 2.0;
        let normal = surface.normal(middle, 0.5);
        let mut face = Face::try_new(vec![wire.into()], surface.clone())
            .map_err(|error| anyhow!("chamfered thread end: {error}"))?;
        if (normal.dot(helix.radial(surface.subs(middle, 0.5))) > 0.0) != external {
            face.invert();
        }
        pieces.push(face);
        arcs.push(arc);
    }
    Ok((pieces, arcs.into()))
}

pub(crate) fn thread_faces(
    solid: &Solid,
    picked: &[usize],
    major: f64,
    pitch: f64,
    left: bool,
    tolerance: f64,
) -> Result<Threaded> {
    let faces = select::faces(solid);
    let owners = select::edge_owners(&faces);
    let mesh_tolerance = geometry::mesh_tolerance(solid) * 0.5;
    let (rough, points, normals) = mate::fit_face(&faces[picked[0]], mesh_tolerance)
        .ok_or_else(|| anyhow!("`on=` must pick a round face"))?;
    for &i in &picked[1..] {
        let (other, ..) = mate::fit_face(&faces[i], mesh_tolerance)
            .ok_or_else(|| anyhow!("`on=` must pick a round face"))?;
        let d = other.point - rough.point;
        let offset = (d - rough.axis * d.dot(rough.axis)).magnitude();
        if rough.axis.cross(other.axis).magnitude() > 1.0e-3
            || offset > rough.radius * 1.0e-3
            || (other.radius - rough.radius).abs() > rough.radius * 1.0e-3
        {
            bail!("`on=` must pick one round face, or the pieces of one");
        }
    }
    let outward = points
        .iter()
        .zip(&normals)
        .map(|(p, n)| {
            let d = *p - rough.point;
            n.dot(d - rough.axis * d.dot(rough.axis))
        })
        .sum::<f64>();
    let external = outward > 0.0;
    let mine: HashSet<usize> = picked.iter().copied().collect();
    let mut end_edges: Vec<Edge> = Vec::new();
    for &i in picked {
        for edge in faces[i].edge_iter() {
            let outside = owners[&edge.id()].iter().any(|f| !mine.contains(f));
            if outside && !end_edges.iter().any(|e| e.is_same(&edge)) {
                end_edges.push(edge.clone());
            }
        }
    }
    let ends = ends_of(&end_edges, &rough, tolerance)?;
    let crest = major / 2.0;
    let profile = basic_profile(major, pitch);
    let root = profile[2].1;
    let slack = ends.radius * 1.0e-4;
    let radius = if (ends.radius - if external { crest } else { root }).abs() < slack {
        if external { crest } else { root }
    } else {
        ends.radius
    };
    if external && (radius > crest || radius <= root) {
        bail!(
            "an M{major} thread needs a rod between {:.3} and {major} across, this one is {:.3}",
            root * 2.0,
            radius * 2.0
        );
    }
    if !external && (radius < root || radius >= crest) {
        bail!(
            "an M{major} thread needs a hole between {:.3} and {major} across, this one is {:.3}",
            root * 2.0,
            radius * 2.0
        );
    }
    let profile = clipped(&profile, radius, external);
    let n = profile.len() - 1;
    let x = {
        let seed = if ends.axis.x.abs() < 0.9 {
            Vector3::unit_x()
        } else {
            Vector3::unit_y()
        };
        (seed - ends.axis * ends.axis.dot(seed)).normalize()
    };
    let y = if left {
        x.cross(ends.axis)
    } else {
        ends.axis.cross(x)
    };
    let helix = Helix {
        origin: ends.centre + ends.axis * ends.bottom,
        along: ends.axis,
        x,
        y,
        pitch,
    };
    let length = ends.top - ends.bottom;
    let (low_edges, high_edges): (Vec<Edge>, Vec<Edge>) =
        end_edges.iter().cloned().partition(|edge| {
            let points = edge_points(edge);
            let p = points[points.len() / 2];
            helix.along.dot(p - helix.origin) < length / 2.0
        });
    let end_slack = tolerance * 100.0 + radius * 1.0e-5;
    let around = Around {
        faces: &faces,
        owners: &owners,
        mine: &mine,
        helix: &helix,
        radius,
        slack: end_slack,
    };
    let bottom = end_of(&around, &low_edges, 0.0)?;
    let top = end_of(&around, &high_edges, length)?;
    let (lowest_r, highest_r) = profile
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| {
            (a.min(p.1), b.max(p.1))
        });
    for end in [&bottom, &top] {
        if let Kind::Cone { far_radius, .. } = end.kind {
            let reaches = if external {
                far_radius < lowest_r - end_slack
            } else {
                far_radius > highest_r + end_slack
            };
            if !reaches {
                bail!(
                    "a chamfer at a thread's end must go past the thread's root, to {} {:.3} across; this one reaches {:.3}",
                    if external { "under" } else { "over" },
                    if external { lowest_r } else { highest_r } * 2.0,
                    far_radius * 2.0
                );
            }
        }
    }
    let raw_low = |j: usize| TAU * (bottom.height_at(profile[j].1, radius) - profile[j].0) / pitch;
    let raw_high = |j: usize| TAU * (top.height_at(profile[j].1, radius) - profile[j].0) / pitch;

    let step = TAU / SPANS_PER_TURN as f64;
    let lowest = (0..=n).map(raw_low).fold(f64::INFINITY, f64::min) - TAU;
    let highest = (0..=n).map(raw_high).fold(f64::NEG_INFINITY, f64::max) + TAU;
    let start = (lowest / step).floor() * step;
    let spans = ((highest - start) / step).ceil() as usize;
    let end = start + spans as f64 * step;
    let mut knot_values = vec![start; 4];
    knot_values.extend((1..spans).map(|k| start + k as f64 * step));
    knot_values.extend(vec![end; 4]);
    let theta_knots = KnotVector::from(knot_values.clone());
    let scale = 6.0 / (4.0 + 2.0 * step.cos());
    let rails: Vec<Vec<Point3>> = profile
        .iter()
        .map(|&(s, r)| {
            (0..spans + 3)
                .map(|k| {
                    let greville = knot_values[k + 1..k + 4].iter().sum::<f64>() / 3.0;
                    helix.at(greville, s, r * scale)
                })
                .collect()
        })
        .collect();
    let surfaces: Vec<BsplineSurface<Point3>> = (0..n)
        .map(|k| {
            BsplineSurface::new(
                (theta_knots.clone(), KnotVector::bezier_knot(1)),
                (0..spans + 3)
                    .map(|m| vec![rails[k][m], rails[k + 1][m]])
                    .collect(),
            )
        })
        .collect();
    let rail_curve = |j: usize| BsplineCurve::new(theta_knots.clone(), rails[j].clone());
    let snap = |theta: f64| {
        let k = ((theta - start) / step).round();
        if (theta - (start + k * step)).abs() < step * 1.0e-9 {
            knot_values[k as usize + 3]
        } else {
            theta
        }
    };
    let low_at = |j: usize| snap(raw_low(j));
    let high_at = |j: usize| snap(raw_high(j));
    let first_cut = (0..=n).map(low_at).fold(f64::NEG_INFINITY, f64::max);
    let last_cut = (0..=n).map(high_at).fold(f64::INFINITY, f64::min) - step;
    let cuts: Vec<f64> = (1..)
        .map(|m| snap(first_cut + TAU * m as f64))
        .take_while(|&theta| theta < last_cut)
        .collect();
    let shift = |k: usize| if k + 1 == n { TAU } else { 0.0 };
    let rail_runs: Vec<Rail> = (0..n)
        .map(|j| {
            let wrapped = cuts
                .iter()
                .map(|c| snap(c + if j == 0 { TAU } else { 0.0 }));
            let mut params: Vec<f64> = cuts
                .iter()
                .copied()
                .chain(wrapped)
                .filter(|&t| t > low_at(j) + step && t < high_at(j) - step)
                .collect();
            params.sort_by(f64::total_cmp);
            params.dedup_by(|b, a| (*b - *a).abs() < 1.0e-9);
            params.insert(0, low_at(j));
            params.push(high_at(j));
            Rail::new(&rail_curve(j), params)
        })
        .collect();
    let pcurve = |surface: &BsplineSurface<Point3>, from: Point2, to: Point2| -> Result<Curve> {
        let params = uniform(END_SAMPLES);
        let points: Vec<Point3> = params
            .iter()
            .map(|&t| {
                let uv = from + (to - from) * t;
                surface.subs(uv.x, uv.y)
            })
            .collect();
        let knots = knots(&params, 3);
        let control = interpolate(&points, &params, &knots)?;
        Ok(Curve::BsplineCurve(BsplineCurve::new(knots, control)))
    };
    let crossings_of =
        |end: &End, at: &dyn Fn(usize) -> f64| -> Result<Vec<(Crossing, usize, f64, f64)>> {
            if matches!(end.kind, Kind::Flat { .. }) {
                return Ok(Vec::new());
            }
            let segments: Vec<(f64, f64)> = (0..n).map(|k| (at(k), at(k + 1))).collect();
            seam_angles(&segments)?
                .into_iter()
                .map(|angle| {
                    segments
                        .iter()
                        .enumerate()
                        .find_map(|(k, &(a, b))| {
                            let (lo, hi) = (a.min(b), a.max(b));
                            let theta = angle + TAU * ((lo - angle) / TAU).ceil();
                            (theta < hi).then(|| {
                                let t = (theta - a) / (b - a);
                                let vertex = builder::vertex(surfaces[k].subs(theta, t));
                                (Crossing { angle, vertex }, k, theta, t)
                            })
                        })
                        .ok_or_else(|| anyhow!("a seam missed the thread's end"))
                })
                .collect()
        };
    let bottom_crossings = crossings_of(&bottom, &low_at)?;
    let top_crossings = crossings_of(&top, &high_at)?;
    let chain = |piece: &BsplineSurface<Point3>,
                 k: usize,
                 at: &dyn Fn(usize) -> f64,
                 crossings: &[(Crossing, usize, f64, f64)],
                 from: &Vertex,
                 to: &Vertex|
     -> Result<Vec<Edge>> {
        let mut stops: Vec<(Point2, Vertex)> = vec![(Point2::new(at(k), 0.0), from.clone())];
        let mut inside: Vec<&(Crossing, usize, f64, f64)> =
            crossings.iter().filter(|c| c.1 == k).collect();
        inside.sort_by(|a, b| a.3.total_cmp(&b.3));
        stops.extend(
            inside
                .into_iter()
                .map(|(c, _, theta, t)| (Point2::new(*theta, *t), c.vertex.clone())),
        );
        stops.push((Point2::new(at(k + 1), 1.0), to.clone()));
        stops
            .windows(2)
            .map(|pair| {
                Ok(Edge::new(
                    &pair[0].1,
                    &pair[1].1,
                    pcurve(piece, pair[0].0, pair[1].0)?,
                ))
            })
            .collect()
    };
    let mut bottom_edges = Vec::new();
    let mut top_edges = Vec::new();
    let mut strips = Vec::new();
    for k in 0..n {
        let upper = &rail_runs[(k + 1) % n];
        let lower = &rail_runs[k];
        let lifted = shift(k);
        let mut bounds = vec![low_at(k).min(low_at(k + 1))];
        bounds.extend(cuts.iter().copied());
        bounds.push(high_at(k).max(high_at(k + 1)));
        let columns: Vec<Edge> = cuts
            .iter()
            .map(|&theta| {
                let (from, to) = (lower.vertex(theta), upper.vertex(theta + lifted));
                let curve =
                    BsplineCurve::new(KnotVector::bezier_knot(1), vec![from.point(), to.point()]);
                Edge::new(from, to, Curve::BsplineCurve(curve))
            })
            .collect();
        let last = bounds.len() - 2;
        for f in 0..=last {
            let (a, b) = (bounds[f], bounds[f + 1]);
            let mut surface = surfaces[k].clone();
            let mut piece = surface.cut_u(a);
            piece.cut_u(b);
            let mut edges = lower.run(a.max(low_at(k)), b.min(high_at(k)));
            if f == last {
                let top = chain(
                    &piece,
                    k,
                    &high_at,
                    &top_crossings,
                    lower.vertex(high_at(k)),
                    upper.vertex(high_at(k + 1) + lifted),
                )?;
                edges.extend(top.iter().cloned());
                top_edges.extend(top);
            } else {
                edges.push(columns[f].clone());
            }
            edges.extend(
                upper
                    .run(
                        a.max(low_at(k + 1)) + lifted,
                        b.min(high_at(k + 1)) + lifted,
                    )
                    .into_iter()
                    .rev()
                    .map(|e| e.inverse()),
            );
            if f == 0 {
                let bottom = chain(
                    &piece,
                    k,
                    &low_at,
                    &bottom_crossings,
                    lower.vertex(low_at(k)),
                    upper.vertex(low_at(k + 1) + lifted),
                )?;
                edges.extend(bottom.iter().rev().map(|e| e.inverse()));
                bottom_edges.extend(bottom);
            } else {
                edges.push(columns[f - 1].inverse());
            }
            let middle = (a + b) / 2.0;
            let surface = Surface::BsplineSurface(piece);
            let normal = surface.normal(middle, 0.5);
            let mut face = Face::try_new(vec![edges.into()], surface.clone())
                .map_err(|error| anyhow!("thread face: {error}"))?;
            if (normal.dot(helix.radial(surface.subs(middle, 0.5))) > 0.0) != external {
                face.invert();
            }
            strips.push(face);
        }
    }
    let mut removed: HashSet<FaceId> = picked.iter().map(|&i| faces[i].id()).collect();
    let mut replaced: Vec<(FaceId, Face)> = Vec::new();
    let mut added: Vec<Face> = strips.clone();
    for (end, loop_edges, crossings, side_edges) in [
        (&bottom, bottom_edges, bottom_crossings, &low_edges),
        (&top, top_edges, top_crossings, &high_edges),
    ] {
        match &end.kind {
            Kind::Flat { cap } => {
                let ids: HashSet<EdgeId> = side_edges.iter().map(|e| e.id()).collect();
                let wire: Wire = loop_edges.into();
                replaced.push((faces[*cap].id(), capped(&faces[*cap], &ids, &wire, &helix)?));
            }
            Kind::Cone {
                faces: cone,
                cap,
                far_edges,
                ..
            } => {
                let mut crossings: Vec<Crossing> = crossings.into_iter().map(|(c, ..)| c).collect();
                crossings.sort_by(|a, b| a.angle.total_cmp(&b.angle));
                let (pieces, far_loop) =
                    chamfer_faces(end, &loop_edges, &crossings, &helix, radius, external)?;
                cone.iter().for_each(|&f| {
                    removed.insert(faces[f].id());
                });
                let ids: HashSet<EdgeId> = far_edges.iter().map(|e| e.id()).collect();
                replaced.push((
                    faces[*cap].id(),
                    capped(&faces[*cap], &ids, &far_loop, &helix)?,
                ));
                added.extend(pieces);
            }
        }
    }
    let mut shells = Vec::new();
    for shell in solid.boundaries() {
        let mut kept: Vec<Face> = Vec::new();
        let mut touched = false;
        for face in shell.face_iter() {
            if removed.contains(&face.id()) {
                touched = true;
            } else if let Some((_, new)) = replaced.iter().find(|(id, _)| *id == face.id()) {
                kept.push(new.clone());
            } else {
                kept.push(face.clone());
            }
        }
        if touched {
            kept.extend(added.iter().cloned());
        }
        shells.push(Shell::from(kept));
    }
    let solid = Solid::try_new(shells)
        .map_err(|error| anyhow!("the threaded solid is not closed: {error}"))?;
    Ok(Threaded {
        solid,
        strips: strips.iter().map(|f| f.oriented_surface()).collect(),
        external,
        length,
    })
}

impl Model {
    pub(crate) fn op_thread(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["size"], &["on", "pitch"], true)?;
        let left = match args.rest.as_slice() {
            [] => false,
            ["left"] => true,
            _ => bail!("`thread` takes `left` for a left-hand thread"),
        };
        let name = args.text("size")?;
        let (major, _, standard) = thread_named(name)?;
        let pitch = match args.optional_number("pitch", &self.scope)? {
            Some(p) => positive(p, "pitch")?,
            None => standard,
        };
        let selector = args.text("on")?;
        let solid = self.active("thread")?.clone();
        let tolerance = self.tolerance();
        let picked = select::select_faces(selector, &solid, &self.groups, tolerance)?;
        if picked.is_empty() {
            bail!("`{selector}` matched no faces");
        }
        let threaded = thread_faces(&solid, &picked, major, pitch, left, tolerance)?;
        let label = label_of(line);
        threaded.strips.into_iter().for_each(|surface| {
            self.groups
                .record(&self.body_name(), &label, "faces", surface)
        });
        self.solid = Some(threaded.solid);
        let warning = if threaded.external {
            None
        } else {
            crate::calc::thread::short_note(name, threaded.length)
        };
        Ok(format!(
            "{} {} x {pitch} thread, {:.3} long; {}{}",
            if threaded.external {
                "outside"
            } else {
                "inside"
            },
            name.to_uppercase(),
            threaded.length,
            self.describe_solid()?,
            warning.map(|w| format!("; {w}")).unwrap_or_default()
        ))
    }
}
