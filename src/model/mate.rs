use super::Model;
use super::args::{Args, point3};
use super::assembly::{Condition, Feature, angle_between, distance_between};
use crate::geometry;
use crate::parse::Line;
use crate::select;
use anyhow::{Result, anyhow, bail};
use monstertruck::meshing::prelude::*;
use monstertruck::modeling::*;

#[derive(Clone, Copy, Debug)]
pub struct Cylinder {
    pub point: Point3,
    pub axis: Vector3,
    pub radius: f64,
}

impl Cylinder {
    fn offset_from(&self, other: &Cylinder) -> f64 {
        let d = other.point - self.point;
        (d - self.axis * d.dot(self.axis)).magnitude()
    }

    fn parallel(&self, other: &Cylinder) -> bool {
        self.axis.cross(other.axis).magnitude() < 1.0e-6
    }
}

pub(crate) fn fit_circle(points: &[(f64, f64)]) -> Option<((f64, f64), f64, f64)> {
    let count = points.len().max(1) as f64;
    let mean = points.iter().fold((0.0, 0.0), |(sx, sy), &(x, y)| {
        (sx + x / count, sy + y / count)
    });
    let mut m = [[0.0f64; 3]; 3];
    let mut rhs = [0.0f64; 3];
    for &(x, y) in points {
        let (x, y) = (x - mean.0, y - mean.1);
        let row = [x, y, 1.0];
        let target = -(x * x + y * y);
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] += row[i] * row[j];
            }
            rhs[i] += row[i] * target;
        }
    }
    let det = |m: &[[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let whole = det(&m);
    if whole.abs() < 1.0e-300 {
        return None;
    }
    let solve = |k: usize| {
        let mut c = m;
        (0..3).for_each(|i| c[i][k] = rhs[i]);
        det(&c) / whole
    };
    let (d, e, f) = (solve(0), solve(1), solve(2));
    let local = (-d / 2.0, -e / 2.0);
    let r2 = local.0 * local.0 + local.1 * local.1 - f;
    if r2 <= 0.0 {
        return None;
    }
    let centre = (local.0 + mean.0, local.1 + mean.1);
    let radius = r2.sqrt();
    let worst = points
        .iter()
        .map(|&(x, y)| ((x - centre.0).hypot(y - centre.1) - radius).abs())
        .fold(0.0, f64::max);
    Some((centre, radius, worst))
}

fn face_cylinder(points: &[Point3], normals: &[Vector3]) -> Option<Cylinder> {
    let first = *normals.first()?;
    let other = normals.iter().max_by(|a, b| {
        first
            .cross(**a)
            .magnitude()
            .total_cmp(&first.cross(**b).magnitude())
    })?;
    let axis = first.cross(*other);
    if axis.magnitude() < 1.0e-3 {
        return None;
    }
    let axis = axis.normalize();
    if normals.iter().any(|n| n.dot(axis).abs() > 2.0e-2) {
        return None;
    }
    let u = axis
        .cross(if axis.x.abs() < 0.9 {
            Vector3::unit_x()
        } else {
            Vector3::unit_y()
        })
        .normalize();
    let v = axis.cross(u);
    let flat: Vec<(f64, f64)> = points
        .iter()
        .map(|p| (p.to_vec().dot(u), p.to_vec().dot(v)))
        .collect();
    let ((cu, cv), radius, worst) = fit_circle(&flat)?;
    if worst > radius * 1.0e-2 {
        return None;
    }
    let along = points.iter().map(|p| p.to_vec().dot(axis)).sum::<f64>() / points.len() as f64;
    Some(Cylinder {
        point: Point3::from_vec(u * cu + v * cv + axis * along),
        axis,
        radius,
    })
}

pub(crate) fn fit_face(
    face: &Face,
    tolerance: f64,
) -> Option<(Cylinder, Vec<Point3>, Vec<Vector3>)> {
    let single: Shell = vec![face.clone()].into();
    let meshed = single.robust_triangulation(tolerance);
    let meshed_face = meshed.face_iter().next()?;
    let outward = if meshed_face.orientation() { 1.0 } else { -1.0 };
    let mesh = meshed_face.surface()?;
    let positions = mesh.positions();
    let surface_normals = mesh.normals();
    let mut points = Vec::new();
    let mut normals = Vec::new();
    for triangle in mesh.faces().triangle_iter() {
        for vertex in triangle {
            if let Some(n) = vertex.nor.and_then(|i| surface_normals.get(i)) {
                points.push(positions[vertex.pos]);
                normals.push(n.normalize() * outward);
            }
        }
    }
    let cylinder = face_cylinder(&points, &normals)?;
    Some((cylinder, points, normals))
}

pub fn cylinders(solid: &Solid, picked: &[usize]) -> Vec<Cylinder> {
    let all = select::faces(solid);
    let tolerance = geometry::mesh_tolerance(solid) * 0.5;
    let mut found: Vec<Cylinder> = Vec::new();
    for &index in picked {
        let Some(face) = all.get(index) else { continue };
        let Some((cylinder, _, _)) = fit_face(face, tolerance) else {
            continue;
        };
        let size = cylinder.radius.max(1.0e-9);
        let same = found.iter().position(|known| {
            known.parallel(&cylinder)
                && known.offset_from(&cylinder) < size * 1.0e-3
                && (known.radius - cylinder.radius).abs() < size * 1.0e-3
        });
        if same.is_none() {
            found.push(cylinder);
        }
    }
    found
}

pub fn holes_in(solid: &Solid) -> Vec<Cylinder> {
    let tolerance = geometry::mesh_tolerance(solid) * 2.0;
    let mut found: Vec<(Cylinder, Vec<Point3>)> = Vec::new();
    for face in select::faces(solid) {
        let Some((cylinder, points, normals)) = fit_face(&face, tolerance) else {
            continue;
        };
        let inward: f64 = points
            .iter()
            .zip(&normals)
            .map(|(p, n)| {
                let d = *p - cylinder.point;
                n.dot(d - cylinder.axis * d.dot(cylinder.axis))
            })
            .sum();
        if inward >= 0.0 {
            continue;
        }
        let size = cylinder.radius.max(1.0e-9);
        match found.iter_mut().find(|(known, _)| {
            known.parallel(&cylinder)
                && known.offset_from(&cylinder) < size * 1.0e-3
                && (known.radius - cylinder.radius).abs() < size * 1.0e-3
        }) {
            Some((_, known)) => known.extend(points),
            None => found.push((cylinder, points)),
        }
    }
    found
        .into_iter()
        .filter(|(cylinder, points)| {
            let axis = cylinder.axis;
            let u = axis
                .cross(if axis.x.abs() < 0.9 {
                    Vector3::unit_x()
                } else {
                    Vector3::unit_y()
                })
                .normalize();
            let v = axis.cross(u);
            let mut angles: Vec<f64> = points
                .iter()
                .map(|p| {
                    let d = *p - cylinder.point;
                    d.dot(v).atan2(d.dot(u))
                })
                .collect();
            angles.sort_by(f64::total_cmp);
            let widest = angles
                .windows(2)
                .map(|w| w[1] - w[0])
                .chain(
                    angles
                        .first()
                        .zip(angles.last())
                        .map(|(a, b)| a + std::f64::consts::TAU - b),
                )
                .fold(0.0, f64::max);
            widest < 0.5
        })
        .map(|(cylinder, _)| cylinder)
        .collect()
}

fn fixed_part(text: &str) -> String {
    text.split_once(':')
        .map_or(text, |(part, _)| part)
        .to_string()
}

fn orient(mine: Feature, theirs: Feature, condition: Condition) -> Result<Matrix4> {
    let (from, to) = (mine.direction(), theirs.direction());
    let mixed = matches!(
        (mine, theirs),
        (Feature::Axis(..), Feature::Plane(..)) | (Feature::Plane(..), Feature::Axis(..))
    );
    let goal = match condition {
        Condition::Angle(degrees) => {
            let planes = matches!((mine, theirs), (Feature::Plane(..), Feature::Plane(..)));
            let to = if !planes && from.dot(to) < 0.0 {
                -to
            } else {
                to
            };
            let mut side = to.cross(from);
            if side.magnitude() < 1.0e-9 {
                let helper = if to.x.abs() < 0.9 {
                    Vector3::unit_x()
                } else {
                    Vector3::unit_y()
                };
                side = to.cross(helper);
            }
            let side = side.normalize();
            Matrix3::from_axis_angle(side, Rad(degrees.to_radians())) * to
        }
        _ if mixed => {
            let flat = from - to * from.dot(to);
            if flat.magnitude() < 1.0e-9 {
                let helper = if to.x.abs() < 0.9 {
                    Vector3::unit_x()
                } else {
                    Vector3::unit_y()
                };
                to.cross(helper).normalize()
            } else {
                flat.normalize()
            }
        }
        _ if from.dot(to) < 0.0 => -to,
        _ => to,
    };
    let turn = about(mine.point(), turn_onto(from, goal));
    let turned = mine.moved(turn);
    let Condition::Distance(want) = condition else {
        return Ok(turn);
    };
    let (normal, current) = match (turned, theirs) {
        (Feature::Plane(p, n), other) => (n, (p - other.point()).dot(n)),
        (Feature::Axis(p, _), Feature::Plane(q, n)) => (n, (p - q).dot(n)),
        (Feature::Axis(p, d), Feature::Axis(q, _)) => {
            let offset = (p - q) - d * (p - q).dot(d);
            if offset.magnitude() < 1.0e-9 {
                bail!(
                    "the two axes are the same line, so there is no side to set them apart on; move the part off first"
                );
            }
            (offset.normalize(), offset.magnitude())
        }
    };
    let side = if current.abs() < 1.0e-9 {
        1.0
    } else {
        current.signum()
    };
    Ok(Matrix4::from_translation(normal * (side * want - current)) * turn)
}

fn turn_onto(from: Vector3, to: Vector3) -> Matrix3 {
    let axis = from.cross(to);
    let cos = from.dot(to).clamp(-1.0, 1.0);
    if axis.magnitude() < 1.0e-12 {
        if cos > 0.0 {
            return Matrix3::identity();
        }
        let helper = if from.x.abs() < 0.9 {
            Vector3::unit_x()
        } else {
            Vector3::unit_y()
        };
        return Matrix3::from_axis_angle(from.cross(helper).normalize(), Rad(std::f64::consts::PI));
    }
    Matrix3::from_axis_angle(axis.normalize(), Rad(cos.acos()))
}

fn about(point: Point3, linear: Matrix3) -> Matrix4 {
    Matrix4::from_translation(point.to_vec())
        * Matrix4::from(linear)
        * Matrix4::from_translation(-point.to_vec())
}

impl Model {
    pub(crate) fn reference(&self, text: &str) -> Result<(String, Solid, Vec<usize>)> {
        let (part, selector) = text.split_once(':').ok_or_else(|| {
            anyhow!("write `part:faces`, like `case.main:pilot.side`; `{text}` has no `:`")
        })?;
        let solid = self.named_body(part)?;
        let groups = self
            .part_groups
            .get(part)
            .map(super::PartGroups::placed)
            .unwrap_or_default();
        let tolerance = (geometry::bounds(&solid).diameter() * 1.0e-6).max(1.0e-7);
        let faces = select::select_faces(selector, &solid, &groups, tolerance)?;
        if faces.is_empty() {
            bail!("`{text}` matched no faces");
        }
        Ok((part.to_string(), solid, faces))
    }

    fn fit_key(&self, part: &str, text: &str, faces: &[usize]) -> String {
        let selector = text.split_once(':').map_or(text, |(_, s)| s);
        let placed = self.placements.get(part).copied();
        match self.sources.get(part) {
            Some(source) => format!("{source:?}|{selector}|{placed:?}|{faces:?}"),
            None => format!(
                "{}{part}|{selector}|{placed:?}|{faces:?}",
                super::INLINE_KEY
            ),
        }
    }

    pub(crate) fn holes(&self, text: &str) -> Result<(String, Vec<Cylinder>)> {
        let (part, solid, faces) = self.reference(text)?;
        let key = self.fit_key(&part, text, &faces);
        let found = self.cache.cylinders(key, || cylinders(&solid, &faces));
        if found.is_empty() {
            bail!("`{text}` has no round faces to line up");
        }
        Ok((part, found))
    }

    pub(crate) fn op_concentric(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["moving", "fixed"], &["near", "flip"], true)?;
        let flip = match args.rest.as_slice() {
            [] => false,
            ["flip"] => true,
            _ => bail!("`concentric` takes `flip` to point the moving part the other way"),
        };
        let (part, mine) = self.holes(args.text("moving")?)?;
        let (_, theirs) = self.holes(args.text("fixed")?)?;
        let [own] = mine.as_slice() else {
            bail!(
                "`{}` has {} round faces; pick one hole on the moving part",
                args.text("moving")?,
                mine.len()
            );
        };
        let near = args
            .values
            .get("near")
            .map(|text| point3(text, &self.scope))
            .transpose()?
            .unwrap_or(own.point);
        let target = theirs
            .iter()
            .min_by(|a, b| {
                let gap = |c: &Cylinder| {
                    let d = near - c.point;
                    (d - c.axis * d.dot(c.axis)).magnitude()
                };
                gap(a).total_cmp(&gap(b))
            })
            .copied()
            .ok_or_else(|| anyhow!("no hole to line up with"))?;
        let mut goal = if own.axis.dot(target.axis) < 0.0 {
            -target.axis
        } else {
            target.axis
        };
        if flip {
            goal = -goal;
        }
        let turn = about(own.point, turn_onto(own.axis, goal));
        let moved = turn.transform_point(own.point);
        let d = target.point - moved;
        let shift = d - goal * d.dot(goal);
        let transform = Matrix4::from_translation(shift) * turn;
        let fixed = fixed_part(args.text("fixed")?);
        let features = [
            Feature::Axis(own.point, own.axis),
            Feature::Axis(target.point, target.axis),
        ];
        let said = self.mate_parts(
            &line.text,
            &part,
            &fixed,
            features,
            Condition::Concentric,
            transform,
        )?;
        Ok(format!(
            "{said}; radius {:.3} in {:.3}",
            own.radius, target.radius
        ))
    }

    pub(crate) fn op_flush(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["moving", "fixed"], &["offset"], false)?;
        let plane = |text: &str| -> Result<(String, Plane)> {
            let (part, solid, faces) = self.reference(text)?;
            let all = select::faces(&solid);
            let planes: Vec<Plane> = faces
                .iter()
                .filter_map(|&i| match all[i].oriented_surface() {
                    Surface::Plane(p) => Some(p),
                    _ => None,
                })
                .collect();
            let first = *planes
                .first()
                .ok_or_else(|| anyhow!("`{text}` has no flat faces"))?;
            if planes.iter().any(|p| {
                p.normal().dot(first.normal()) < 1.0 - 1.0e-9
                    || (p.origin() - first.origin()).dot(first.normal()).abs() > 1.0e-6
            }) {
                bail!("`{text}` matched faces that are not on one plane");
            }
            Ok((part, first))
        };
        let (part, mine) = plane(args.text("moving")?)?;
        let (_, theirs) = plane(args.text("fixed")?)?;
        let gap = args.optional_number("offset", &self.scope)?.unwrap_or(0.0);
        let turn = about(mine.origin(), turn_onto(mine.normal(), -theirs.normal()));
        let origin = turn.transform_point(mine.origin());
        let distance = (theirs.origin() - origin).dot(theirs.normal()) + gap;
        let transform = Matrix4::from_translation(theirs.normal() * distance) * turn;
        let fixed = fixed_part(args.text("fixed")?);
        let features = [
            Feature::Plane(mine.origin(), mine.normal()),
            Feature::Plane(theirs.origin(), theirs.normal()),
        ];
        self.mate_parts(
            &line.text,
            &part,
            &fixed,
            features,
            Condition::Flush(gap),
            transform,
        )
    }

    fn feature(&self, text: &str) -> Result<(String, Feature, Option<f64>)> {
        let (part, solid, faces) = self.reference(text)?;
        let all = select::faces(&solid);
        let planes: Vec<Plane> = faces
            .iter()
            .filter_map(|&i| match all[i].oriented_surface() {
                Surface::Plane(p) => Some(p),
                _ => None,
            })
            .collect();
        if planes.len() == faces.len() {
            let first = planes[0];
            if planes.iter().any(|p| {
                p.normal().dot(first.normal()) < 1.0 - 1.0e-9
                    || (p.origin() - first.origin()).dot(first.normal()).abs() > 1.0e-6
            }) {
                bail!("`{text}` matched flat faces that are not on one plane");
            }
            return Ok((part, Feature::Plane(first.origin(), first.normal()), None));
        }
        let key = self.fit_key(&part, text, &faces);
        match self
            .cache
            .cylinders(key, || cylinders(&solid, &faces))
            .as_slice()
        {
            [one] => Ok((part, Feature::Axis(one.point, one.axis), Some(one.radius))),
            [] => bail!("`{text}` is neither one flat face nor one round face"),
            many => bail!(
                "`{text}` has {} round faces on different axes; pick one",
                many.len()
            ),
        }
    }

    pub(crate) fn op_orient(&mut self, line: &Line) -> Result<String> {
        let op = line.op.as_str();
        let needs: &[&str] = match op {
            "angle" => &["moving", "fixed", "degrees"],
            "distance" => &["moving", "fixed", "length"],
            _ => &["moving", "fixed"],
        };
        let args = Args::new(line, needs, &[], false)?;
        let (part, mine, radius) = self.feature(args.text("moving")?)?;
        let (_, theirs, their_radius) = self.feature(args.text("fixed")?)?;
        let fixed = fixed_part(args.text("fixed")?);
        let condition = match op {
            "parallel" => Condition::Parallel,
            "angle" => {
                let degrees = args.number("degrees", &self.scope)?;
                if !(0.0..=180.0).contains(&degrees) {
                    bail!("an angle mate takes 0 to 180 degrees, not {degrees}");
                }
                Condition::Angle(degrees)
            }
            "distance" => {
                let length = args.number("length", &self.scope)?;
                if length < 0.0 {
                    bail!("a distance is not negative; got {length}");
                }
                Condition::Distance(length)
            }
            _ => match (mine, theirs, radius, their_radius) {
                (Feature::Axis(..), Feature::Plane(..), Some(r), _)
                | (Feature::Plane(..), Feature::Axis(..), _, Some(r)) => Condition::Distance(r),
                (Feature::Axis(..), Feature::Axis(..), Some(a), Some(b)) => {
                    Condition::Distance(a + b)
                }
                _ => bail!("`tangent` puts a round face against a flat one or another round face"),
            },
        };
        let transform = orient(mine, theirs, condition)?;
        let said = self.mate_parts(
            &line.text,
            &part,
            &fixed,
            [mine, theirs],
            condition,
            transform,
        )?;
        let mates = self
            .mates
            .last()
            .map(|m| m.features)
            .unwrap_or([mine, theirs]);
        Ok(format!(
            "{said}; {:.3}° apart{}",
            angle_between(mates[0], mates[1]),
            distance_between(mates[0], mates[1])
                .map(|d| format!(", {d:.3} away"))
                .unwrap_or_default()
        ))
    }

    pub(crate) fn op_aligned(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["a", "b"], &["tol"], false)?;
        let tolerance = args.optional_number("tol", &self.scope)?.unwrap_or(0.05);
        let (_, mine) = self.holes(args.text("a")?)?;
        let (_, theirs) = self.holes(args.text("b")?)?;
        let mut worst: f64 = 0.0;
        let mut problems = Vec::new();
        for (i, own) in mine.iter().enumerate() {
            let partner = theirs
                .iter()
                .filter(|c| c.parallel(own))
                .map(|c| (own.offset_from(c), c))
                .min_by(|a, b| a.0.total_cmp(&b.0));
            match partner {
                None => problems.push(format!(
                    "hole {} at {:.3},{:.3},{:.3} has no parallel partner",
                    i + 1,
                    own.point.x,
                    own.point.y,
                    own.point.z
                )),
                Some((offset, _)) if offset > tolerance => problems.push(format!(
                    "hole {} at {:.3},{:.3},{:.3} is {offset:.3} off its partner",
                    i + 1,
                    own.point.x,
                    own.point.y,
                    own.point.z
                )),
                Some((offset, _)) => worst = worst.max(offset),
            }
        }
        if !problems.is_empty() {
            bail!("{}", problems.join("; "));
        }
        Ok(format!(
            "{} hole(s) line up, worst {worst:.3} off",
            mine.len()
        ))
    }
}
