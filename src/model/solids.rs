use super::Model;
use crate::geometry::{self, Frame, Profile};
use crate::select;
use anyhow::{Result, anyhow, bail};
use monstertruck::geometry::prelude::TryIntoHomogeneousBsplineCurve;
use monstertruck::meshing::prelude::PolygonMesh;
use monstertruck::modeling::*;

pub(crate) struct Loop {
    pub(crate) wire: Wire,
    pub(crate) outline: Vec<(f64, f64)>,
    pub(crate) profile: usize,
    pub(crate) whole: bool,
}

fn outline(wire: &Wire, frame: &Frame) -> Vec<(f64, f64)> {
    wire.edge_iter()
        .flat_map(|edge| {
            let curve = edge.oriented_curve();
            let (t0, t1) = curve.range_tuple();
            (0..16)
                .map(move |i| frame.local(curve.subs(t0 + (t1 - t0) * i as f64 / 16.0)))
                .collect::<Vec<_>>()
        })
        .collect()
}

pub(crate) fn loops(frame: &Frame, profiles: &[Profile]) -> Result<Vec<Loop>> {
    let mut result = Vec::new();
    for (index, profile) in profiles.iter().enumerate() {
        let wires = profile.wires(frame).map_err(|error| anyhow!(error))?;
        let whole = wires.len() == 1;
        result.extend(wires.into_iter().map(|wire| Loop {
            outline: outline(&wire, frame),
            wire,
            profile: index,
            whole,
        }));
    }
    Ok(result)
}

fn contains(polygon: &[(f64, f64)], (x, y): (f64, f64)) -> bool {
    (0..polygon.len()).fold(false, |inside, i| {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        if (a.1 > y) != (b.1 > y) && x < (b.0 - a.0) * (y - a.1) / (b.1 - a.1) + a.0 {
            !inside
        } else {
            inside
        }
    })
}

fn clip_band(polygon: Vec<(f64, f64, f64)>, low: f64, high: f64) -> Vec<(f64, f64, f64)> {
    let cut = |polygon: Vec<(f64, f64, f64)>, depth: &dyn Fn(f64) -> f64| {
        let n = polygon.len();
        (0..n)
            .flat_map(|i| {
                let (a, b) = (polygon[i], polygon[(i + 1) % n]);
                let (da, db) = (depth(a.2), depth(b.2));
                let crossing = (da >= 0.0) != (db >= 0.0);
                let t = da / (da - db);
                let between = (
                    a.0 + (b.0 - a.0) * t,
                    a.1 + (b.1 - a.1) * t,
                    a.2 + (b.2 - a.2) * t,
                );
                [(da >= 0.0).then_some(a), crossing.then_some(between)]
            })
            .flatten()
            .collect::<Vec<_>>()
    };
    let above = cut(polygon, &|h| h - low);
    cut(above, &|h| high - h)
}

fn segments_cross(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let side = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| {
        (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)
    };
    side(a, b, c) * side(a, b, d) <= 0.0 && side(c, d, a) * side(c, d, b) <= 0.0
}

fn edges(polygon: &[(f64, f64)]) -> impl Iterator<Item = ((f64, f64), (f64, f64))> + '_ {
    (0..polygon.len()).map(|i| (polygon[i], polygon[(i + 1) % polygon.len()]))
}

pub(crate) fn clear_above(solid: &Solid, frame: &Frame, shapes: &[Loop], height: f64) -> bool {
    let mesh = geometry::mesh(solid, geometry::mesh_tolerance(solid));
    let outlines: Vec<Vec<(f64, f64)>> = shapes.iter().map(|s| s.outline.clone()).collect();
    band_is_empty(&mesh, frame, &outlines, height)
}

fn band_is_empty(
    mesh: &PolygonMesh,
    frame: &Frame,
    outlines: &[Vec<(f64, f64)>],
    height: f64,
) -> bool {
    let positions = mesh.positions();
    let local = |p: Point3| {
        let (u, v) = frame.local(p);
        (u, v, (p - frame.origin).dot(frame.normal))
    };
    let covered = |point: (f64, f64)| {
        outlines
            .iter()
            .filter(|outline| contains(outline, point))
            .count()
            % 2
            == 1
    };
    let touches = |piece: &[(f64, f64)]| {
        piece.iter().any(|&p| covered(p))
            || outlines
                .iter()
                .any(|outline| outline.iter().any(|&p| contains(piece, p)))
            || edges(piece).any(|(a, b)| {
                outlines
                    .iter()
                    .any(|outline| edges(outline).any(|(c, d)| segments_cross(a, b, c, d)))
            })
    };
    let crossed = mesh.faces().triangle_iter().any(|triangle| {
        let corners: Vec<_> = triangle.iter().map(|v| local(positions[v.pos])).collect();
        let piece: Vec<(f64, f64)> = clip_band(corners, height * 1.0e-3, height)
            .into_iter()
            .map(|(u, v, _)| (u, v))
            .collect();
        !piece.is_empty() && touches(&piece)
    });
    if crossed {
        return false;
    }
    let Some(&(u, v)) = outlines.first().and_then(|outline| outline.first()) else {
        return true;
    };
    let direction = Vector3::new(0.5773, 0.5774, 0.5775).normalize();
    let probe = frame.at(u, v) + frame.normal * (height * 0.5);
    geometry::ray_hits(mesh, probe, direction)
        .iter()
        .filter(|(t, _)| *t > 0.0)
        .count()
        % 2
        == 0
}

pub(crate) fn regions(loops: &[Loop]) -> Vec<Vec<usize>> {
    let encloses = |outer: usize, inner: usize| {
        outer != inner
            && loops[inner]
                .outline
                .iter()
                .all(|&point| contains(&loops[outer].outline, point))
    };
    let containers: Vec<Vec<usize>> = (0..loops.len())
        .map(|i| (0..loops.len()).filter(|&j| encloses(j, i)).collect())
        .collect();
    let depth = |i: usize| containers[i].len();
    let parent = |i: usize| containers[i].iter().copied().max_by_key(|&j| depth(j));
    (0..loops.len())
        .filter(|&i| depth(i) % 2 == 0)
        .map(|outer| {
            std::iter::once(outer)
                .chain((0..loops.len()).filter(|&i| depth(i) % 2 == 1 && parent(i) == Some(outer)))
                .collect()
        })
        .collect()
}

fn revolved_span(
    profile: &BsplineCurve<Vector4>,
    origin: Point3,
    axis: Vector3,
    span: f64,
) -> NurbsSurface<Vector4> {
    let (half_cos, half_tan) = ((span / 2.0).cos(), (span / 2.0).tan());
    let rows = profile
        .control_points()
        .iter()
        .map(|cp| {
            let (point, weight) = (cp.to_point(), cp.weight());
            let centre = origin + axis * (point - origin).dot(axis);
            let radial = point - centre;
            let side = axis.cross(radial);
            let middle = centre + radial + side * half_tan;
            let end = centre + radial * span.cos() + side * span.sin();
            vec![
                point.to_vec().extend(1.0) * weight,
                middle.to_vec().extend(1.0) * (weight * half_cos),
                end.to_vec().extend(1.0) * weight,
            ]
        })
        .collect();
    NurbsSurface::new(BsplineSurface::new(
        (profile.knot_vector().clone(), KnotVector::bezier_knot(2)),
        rows,
    ))
}

fn span_surface(surface: &Surface, face: &Face, span: f64) -> Option<NurbsSurface<Vector4>> {
    let Surface::RevolutionSurface(processor) = surface else {
        return None;
    };
    let transform = *processor.transform();
    let revolution = processor.entity();
    let profile = revolution
        .entity_curve()
        .try_into_homogeneous_bspline_curve()?;
    let profile = BsplineCurve::new(
        profile.knot_vector().clone(),
        profile
            .control_points()
            .iter()
            .map(|p| transform * *p)
            .collect(),
    );
    let origin = transform.transform_point(revolution.origin());
    let axis = transform.transform_vector(revolution.axis()).normalize();
    let fits = |nurbs: &NurbsSurface<Vector4>| {
        let size = face
            .vertex_iter()
            .map(|v| v.point().to_vec().magnitude())
            .fold(1.0, f64::max);
        face.vertex_iter().all(|v| {
            nurbs
                .search_nearest_parameter(v.point(), None, 100)
                .is_some_and(|(u, w)| {
                    nurbs.subs(u, w).distance(v.point()) < 1.0e-7 * size
                        && (-1.0e-9..=1.0 + 1.0e-9).contains(&w)
                })
        })
    };
    [span, -span]
        .into_iter()
        .map(|s| revolved_span(&profile, origin, axis, s))
        .find(fits)
}

fn flat(surface: &Surface) -> Option<Plane> {
    let (u, v) = surface.try_range_tuple();
    let ((u0, u1), (v0, v1)) = (u?, v?);
    let samples: Vec<(Point3, Vector3)> = (0..5)
        .flat_map(|i| {
            (0..5).map(move |j| {
                (
                    u0 + (u1 - u0) * (0.1 + 0.2 * i as f64),
                    v0 + (v1 - v0) * (0.1 + 0.2 * j as f64),
                )
            })
        })
        .map(|(u, v)| (surface.subs(u, v), surface.normal(u, v)))
        .collect();
    let (origin, normal) = samples[12];
    if !normal.magnitude().is_finite() || normal.magnitude() < 0.5 {
        return None;
    }
    let size = samples
        .iter()
        .map(|(p, _)| p.distance(origin))
        .fold(0.0, f64::max)
        .max(1.0e-9);
    let level = samples.iter().all(|(p, n)| {
        n.dot(normal) > 1.0 - 1.0e-9 && (p - origin).dot(normal).abs() < size * 1.0e-9
    });
    if !level {
        return None;
    }
    let x = samples
        .iter()
        .map(|(p, _)| *p - origin)
        .find(|d| d.magnitude() > size * 0.1)?
        .normalize();
    Some(Plane::new(origin, origin + x, origin + normal.cross(x)))
}

pub(crate) fn rational(solid: Solid, span: f64) -> Solid {
    let copy = builder::clone(&solid);
    for face in copy.boundaries().iter().flat_map(|shell| shell.face_iter()) {
        let surface = face.surface();
        if !matches!(surface, Surface::RevolutionSurface(_)) {
            continue;
        }
        if let Some(plane) = flat(&surface) {
            face.set_surface(Surface::Plane(plane));
            continue;
        }
        let Some(mut nurbs) = span_surface(&surface, face, span) else {
            continue;
        };
        let Some(point) = face.vertex_iter().next().map(|v| v.point()) else {
            continue;
        };
        let agree = surface
            .search_nearest_parameter(point, None, 100)
            .zip(nurbs.search_nearest_parameter(point, None, 100))
            .is_none_or(|((u0, v0), (u1, v1))| {
                surface.normal(u0, v0).dot(nurbs.normal(u1, v1)) > 0.0
            });
        if !agree {
            nurbs.invert();
        }
        face.set_surface(Surface::NurbsSurface(nurbs));
    }
    copy
}

pub(crate) fn placed(solid: &Solid, transform: Matrix4) -> Solid {
    let moved = builder::transformed(solid, transform);
    if transform.determinant() < 0.0 {
        oriented(moved)
    } else {
        moved
    }
}

pub(crate) fn oriented(mut solid: Solid) -> Solid {
    if geometry::volume(&solid) < 0.0 {
        solid.not();
    }
    solid
}

pub(crate) fn prism(
    frame: &Frame,
    loops: &[Loop],
    region: &[usize],
    profiles: &[Profile],
    direction: Vector3,
    insets: (f64, f64),
) -> Result<Solid> {
    if insets.0 == 0.0 && insets.1 == 0.0 {
        let wires: Vec<Wire> = region.iter().map(|&i| loops[i].wire.clone()).collect();
        let solid: Solid = profile::solid_from_planar_profile(wires, direction)
            .map_err(|error| anyhow!("cannot extrude the sketch: {error}"))?;
        return Ok(oriented(solid));
    }
    let [only] = region else {
        bail!("draft works on a profile without holes; cut the holes in a separate line");
    };
    if !loops[*only].whole {
        bail!("draft works on rect, circle, poly and slot");
    }
    tapered(frame, &profiles[loops[*only].profile], direction, insets)
}

pub(crate) fn tapered(
    frame: &Frame,
    profile: &Profile,
    direction: Vector3,
    (start, end): (f64, f64),
) -> Result<Solid> {
    let top_frame = Frame {
        origin: frame.origin + direction,
        ..*frame
    };
    let bottom = profile
        .inset(start)
        .map_err(|error| anyhow!(error))?
        .wire(frame);
    let top = profile
        .inset(end)
        .map_err(|error| anyhow!(error))?
        .wire(&top_frame);
    loft_wires(&[bottom, top])
}

pub(crate) fn loft_wires(wires: &[Wire]) -> Result<Solid> {
    let mut shell: Shell =
        builder::try_skin_wires(wires).map_err(|error| anyhow!("cannot loft: {error}"))?;
    let cap = |wire: Wire| -> Result<Face> {
        builder::try_attach_plane(&[wire]).map_err(|error| anyhow!("cannot cap the loft: {error}"))
    };
    let first = wires.first().ok_or_else(|| anyhow!("nothing to loft"))?;
    let last = wires.last().ok_or_else(|| anyhow!("nothing to loft"))?;
    shell.push(cap(first.inverse())?);
    shell.push(cap(last.clone())?);
    let solid = Solid::try_new(vec![shell])
        .map_err(|error| anyhow!("lofted solid is not closed: {error}"))?;
    Ok(oriented(solid))
}

pub(crate) fn classify(solid: &Solid, direction: Vector3) -> Vec<(&'static str, Surface)> {
    let along = direction.normalize();
    select::faces(solid)
        .iter()
        .map(|face| {
            let surface = face.oriented_surface();
            let group = match &surface {
                Surface::Plane(plane) if plane.normal().dot(along) < -0.999 => "start",
                Surface::Plane(plane) if plane.normal().dot(along) > 0.999 => "end",
                _ => "side",
            };
            (group, surface)
        })
        .collect()
}

impl Model {
    pub(crate) fn record(&mut self, label: &str, groups: Vec<(&str, Surface)>) {
        let body = self.body_name();
        groups
            .into_iter()
            .for_each(|(group, surface)| self.groups.record(&body, label, group, surface));
    }

    pub(crate) fn record_for(
        &mut self,
        label: &str,
        groups: Vec<(&str, Surface)>,
        combine: super::Combine,
    ) {
        match combine {
            super::Combine::Remove => self.record_inverted(label, groups),
            _ => self.record(label, groups),
        }
    }

    pub(crate) fn record_inverted(&mut self, label: &str, groups: Vec<(&str, Surface)>) {
        let body = self.body_name();
        groups.into_iter().for_each(|(group, mut surface)| {
            surface.invert();
            self.groups.record(&body, label, group, surface);
        });
    }
}

pub(crate) fn clear_round_walls(
    solid: &Solid,
    frame: &Frame,
    profiles: Vec<Profile>,
    depth: f64,
) -> Vec<Profile> {
    let circles: Vec<usize> = (0..profiles.len())
        .filter(|&i| matches!(profiles[i], Profile::Circle { .. }))
        .collect();
    if circles.is_empty() {
        return profiles;
    }
    let Ok(shapes) = loops(frame, &profiles) else {
        return profiles;
    };
    let size = geometry::bounds(solid).diameter();
    let tolerance = geometry::mesh_tolerance(solid);
    let margin = size * 5.0e-3;
    let mesh = geometry::mesh(solid, tolerance);
    let direction = Vector3::new(0.5773, 0.5774, 0.5775).normalize();
    let inside = |p: Point3| {
        geometry::ray_hits(&mesh, p, direction)
            .iter()
            .filter(|(t, _)| *t > 0.0)
            .count()
            % 2
            == 1
    };
    let nesting = |profile: usize| {
        let Some(own) = shapes.iter().find(|l| l.profile == profile) else {
            return 0;
        };
        shapes
            .iter()
            .filter(|other| other.profile != profile)
            .filter(|other| own.outline.iter().all(|&p| contains(&other.outline, p)))
            .count()
    };
    let mut profiles = profiles;
    for index in circles {
        let Profile::Circle { center, diameter } = profiles[index] else {
            continue;
        };
        let outward = if nesting(index) % 2 == 0 { 1.0 } else { -1.0 };
        let radius = diameter / 2.0;
        let ring = |offset: f64, height: f64| {
            (0..16).map(move |k| {
                let angle = std::f64::consts::TAU * (k as f64 + 0.37) / 16.0;
                let r = radius + outward * offset;
                frame.at(center.0 + r * angle.cos(), center.1 + r * angle.sin())
                    - frame.normal * height
            })
        };
        let heights: Vec<f64> = (1..=5).map(|k| depth * k as f64 / 6.0).collect();
        let walled = || {
            heights
                .iter()
                .any(|&h| ring(-4.0 * tolerance, h).any(inside))
        };
        let open = || {
            heights.iter().all(|&h| {
                [4.0 * tolerance, margin / 2.0, margin]
                    .iter()
                    .all(|&offset| !ring(offset, h).any(inside))
            })
        };
        if radius > margin && walled() && open() {
            profiles[index] = Profile::Circle {
                center,
                diameter: diameter + outward * 2.0 * margin,
            };
        }
    }
    profiles
}

pub(crate) fn clear_flush(tool: &Solid, solid: &Solid) -> Solid {
    let size = geometry::bounds(solid).diameter();
    let margin = size * 5.0e-3;
    let close = size * 1.0e-9;
    let planes: Vec<Plane> = select::faces(solid)
        .iter()
        .filter_map(|face| match face.oriented_surface() {
            Surface::Plane(plane) => Some(plane),
            _ => None,
        })
        .collect();
    let copy = builder::clone(tool);
    let mesh = std::cell::OnceCell::new();
    let room_beyond = |face: &Face, own: &Plane| {
        let frame = Frame::from_normal(own.origin(), own.normal());
        let outlines: Vec<Vec<(f64, f64)>> = face
            .boundaries()
            .iter()
            .map(|wire| outline(wire, &frame))
            .collect();
        let mesh = mesh.get_or_init(|| geometry::mesh(solid, geometry::mesh_tolerance(solid)));
        band_is_empty(mesh, &frame, &outlines, margin)
    };
    let flush: Vec<Face> = select::faces(&copy)
        .into_iter()
        .filter(|face| match face.oriented_surface() {
            Surface::Plane(own) => {
                planes.iter().any(|p| {
                    p.normal().dot(own.normal()) > 1.0 - 1.0e-9
                        && (p.origin() - own.origin()).dot(own.normal()).abs() < close
                }) && room_beyond(face, &own)
            }
            _ => false,
        })
        .collect();
    if flush.is_empty() {
        return tool.clone();
    }
    match flush
        .iter()
        .try_for_each(|face| super::features::sink_face(&copy, face, margin, "the tool"))
    {
        Ok(()) => copy,
        Err(_) => tool.clone(),
    }
}
