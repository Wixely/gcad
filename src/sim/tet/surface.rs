use super::sizing::SizeField;
use crate::geometry;
use crate::select;
use anyhow::{Result, bail};
use monstertruck::meshing::prelude::*;
use monstertruck::modeling::*;
use sims_tet::{HashMap, HashSet, Welder};
use spade::handles::FixedVertexHandle;
use spade::{
    AngleLimit, ConstrainedDelaunayTriangulation, Point2, RefinementParameters, Triangulation,
};

pub type SurfaceMesh = sims_tet::SurfaceMesh<super::Exact>;

fn metric(points: &[Point3], uvs: &[Vector2], triangles: &[[usize; 3]]) -> Matrix2 {
    let mut sum = Matrix2::zero();
    let mut weight = 0.0;
    for &[a, b, c] in triangles {
        let (d1, d2) = (uvs[b] - uvs[a], uvs[c] - uvs[a]);
        let det = d1.x * d2.y - d1.y * d2.x;
        if det.abs() < 1.0e-18 {
            continue;
        }
        let (e1, e2) = (points[b] - points[a], points[c] - points[a]);
        let gram = Matrix2::new(e1.dot(e1), e1.dot(e2), e1.dot(e2), e2.dot(e2));
        let d = Matrix2::new(d1.x, d1.y, d2.x, d2.y);
        let Some(inverse) = d.invert() else { continue };
        let g = inverse.transpose() * gram * inverse;
        let w = det.abs();
        sum += g * w;
        weight += w;
    }
    if weight == 0.0 {
        return Matrix2::identity();
    }
    let g = sum / weight;
    let (a, b, d) = (g[0][0], g[0][1], g[1][1]);
    let mean = (a + d) / 2.0;
    let spread = (((a - d) / 2.0).powi(2) + b * b).sqrt();
    let (l1, l2) = ((mean + spread).max(1.0e-18), (mean - spread).max(1.0e-18));
    let (v1, v2) = if b.abs() > 1.0e-15 * (a.abs() + d.abs()) {
        let v = Vector2::new(l1 - d, b).normalize();
        (v, Vector2::new(-v.y, v.x))
    } else if a >= d {
        (Vector2::unit_x(), Vector2::unit_y())
    } else {
        (Vector2::unit_y(), Vector2::unit_x())
    };
    let outer = |v: Vector2| Matrix2::new(v.x * v.x, v.x * v.y, v.x * v.y, v.y * v.y);
    outer(v1) * l1.sqrt() + outer(v2) * l2.sqrt()
}

type KernelFace = (Option<PolygonMesh>, Vec<Vec<Point3>>);

pub fn surface_mesh(solid: &Solid, size: f64) -> Result<SurfaceMesh> {
    let diameter = geometry::bounds(solid).diameter();
    let finest = diameter * 1.0e-5;
    let mut tolerance = (size * 0.02).min(diameter * 2.0e-3).max(finest);
    let mut tries = 0;
    loop {
        match surface_at(solid, size, diameter, tolerance) {
            Err(_) if tries < 6 && tolerance > finest => {
                tries += 1;
                tolerance = (tolerance / 2.0).max(finest);
            }
            result => return result,
        }
    }
}

fn surface_at(solid: &Solid, size: f64, diameter: f64, tolerance: f64) -> Result<SurfaceMesh> {
    let faces = select::faces(solid);
    let kernel: Vec<KernelFace> = solid
        .boundaries()
        .iter()
        .flat_map(|shell| {
            shell
                .robust_triangulation(tolerance)
                .face_iter()
                .map(|f| {
                    let polylines = f
                        .boundaries()
                        .iter()
                        .flat_map(|wire| {
                            wire.edge_iter()
                                .map(|e| e.curve().0.clone())
                                .collect::<Vec<_>>()
                        })
                        .collect();
                    (f.surface(), polylines)
                })
                .collect::<Vec<_>>()
        })
        .collect();
    if kernel.len() != faces.len() {
        bail!(
            "the kernel meshed {} of {} faces",
            kernel.len(),
            faces.len()
        );
    }
    let least = (size / 16.0).max(diameter * 1.0e-4);
    let mut seen = HashSet::default();
    let mut curves: Vec<(Vec<Point3>, [Point3; 2])> = Vec::new();
    for shell in solid.boundaries() {
        for edge in shell.edge_iter() {
            if seen.insert(edge.id()) {
                let polyline = geometry::curve_samples(&edge.curve());
                curves.push((polyline, [edge.front().point(), edge.back().point()]));
            }
        }
    }
    let first = build(&faces, &kernel, diameter, &SizeField::uniform(size), None)?;
    let bends = curvature(&first, &faces, least);
    let field = SizeField::from_gaps(
        size,
        least,
        first
            .triangles
            .iter()
            .map(|t| t.map(|v| first.points[v as usize]))
            .collect(),
        &curves,
        bends,
    );
    let out = if field.is_uniform() {
        first
    } else {
        build(&faces, &kernel, diameter, &field, Some(&first))?
    };
    let mut out = out;
    out.delaunay_flips();
    let open = out.open_edges();
    if open > 0 {
        bail!("the surface mesh has {open} open edge(s); the faces do not meet cleanly");
    }
    Ok(out)
}

const TURN: f64 = 1.0;

fn curvature(mesh: &SurfaceMesh, faces: &[Face], least: f64) -> Vec<(Point3, f64)> {
    use rayon::prelude::*;
    mesh.triangles
        .par_iter()
        .zip(&mesh.tags)
        .filter_map(|(t, &tag)| {
            let surface = faces[tag].surface();
            let normals: Vec<Vector3> = t
                .iter()
                .map(|&v| {
                    let uv = mesh.uv_on(v, tag)?;
                    let n = surface.normal(uv.x, uv.y);
                    (n.magnitude() > 0.5).then_some(n)
                })
                .collect::<Option<_>>()?;
            let p = t.map(|v| mesh.points[v as usize]);
            let mut size = f64::INFINITY;
            for k in 0..3 {
                let j = (k + 1) % 3;
                let angle = normals[k].angle(normals[j]).0;
                if angle > 1.0e-3 {
                    size = size.min((p[j] - p[k]).magnitude() / angle * TURN);
                }
            }
            size.is_finite().then(|| {
                let centre =
                    Point3::from_vec((p[0].to_vec() + p[1].to_vec() + p[2].to_vec()) / 3.0);
                (centre, size.max(least))
            })
        })
        .collect()
}

fn build(
    faces: &[Face],
    kernel: &[KernelFace],
    diameter: f64,
    field: &SizeField,
    seeds: Option<&SurfaceMesh>,
) -> Result<SurfaceMesh> {
    let mut out = SurfaceMesh {
        points: Vec::new(),
        triangles: Vec::new(),
        tags: Vec::new(),
        uvs: Vec::new(),
        faces: faces.iter().map(|f| super::Exact(f.surface())).collect(),
    };
    let mut welder = Welder {
        quantum: diameter * 1.0e-8,
        index: HashMap::default(),
    };
    for (index, (face, (piece, polylines))) in faces.iter().zip(kernel).enumerate() {
        let Some(mesh) = piece else {
            bail!("face {index} could not be meshed");
        };
        if area_of(mesh) <= diameter * diameter * 1.0e-12 {
            continue;
        }
        let seeded = seeds.map(|first| seed_points(first, index, face, field));
        mesh_face(
            &mut out,
            &mut welder,
            index,
            face,
            mesh,
            polylines,
            field,
            seeded.unwrap_or_default(),
        )?;
    }
    out.triangles
        .retain(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2]);
    let mut seen = HashSet::default();
    let mut keep = Vec::new();
    for (t, tri) in out.triangles.iter().enumerate() {
        let mut key = *tri;
        key.sort_unstable();
        if seen.insert(key) {
            keep.push(t);
        }
    }
    out.tags = keep.iter().map(|&t| out.tags[t]).collect();
    out.triangles = keep.iter().map(|&t| out.triangles[t]).collect();
    Ok(out)
}

fn area_of(mesh: &PolygonMesh) -> f64 {
    let p = mesh.positions();
    mesh.faces()
        .triangle_iter()
        .map(|t| {
            let [a, b, c] = t.map(|v| p[v.pos]);
            (b - a).cross(c - a).magnitude() / 2.0
        })
        .sum()
}

fn seed_points(first: &SurfaceMesh, index: usize, face: &Face, field: &SizeField) -> Vec<Vector2> {
    let surface = face.surface();
    let own: Vec<[u32; 3]> = first
        .triangles
        .iter()
        .zip(&first.tags)
        .filter(|(_, tag)| **tag == index)
        .map(|(t, _)| *t)
        .collect();
    let mut count: HashMap<[u32; 2], usize> = HashMap::default();
    for t in &own {
        for k in 0..3 {
            let mut e = [t[k], t[(k + 1) % 3]];
            e.sort_unstable();
            *count.entry(e).or_default() += 1;
        }
    }
    let mut points = Vec::new();
    for t in &own {
        let Some(uv) = t
            .iter()
            .map(|&v| first.uv_on(v, index))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let rim = [0, 1, 2].map(|k| {
            let mut e = [t[k], t[(k + 1) % 3]];
            e.sort_unstable();
            count[&e] == 1
        });
        split_seed(&surface, field, [uv[0], uv[1], uv[2]], rim, 0, &mut points);
    }
    points
}

fn split_seed(
    surface: &Surface,
    field: &SizeField,
    uv: [Vector2; 3],
    rim: [bool; 3],
    depth: usize,
    out: &mut Vec<Vector2>,
) {
    if depth > 16 {
        return;
    }
    let p = uv.map(|q| surface.subs(q.x, q.y));
    let (k, longest) = (0..3)
        .map(|k| (k, (p[k] - p[(k + 1) % 3]).magnitude()))
        .fold((0, 0.0), |a, b| if b.1 > a.1 { b } else { a });
    let centre = (uv[0] + uv[1] + uv[2]) / 3.0;
    if longest <= field.at(surface.subs(centre.x, centre.y)) * 1.2 {
        return;
    }
    let (a, b, c) = (k, (k + 1) % 3, (k + 2) % 3);
    let m = (uv[a] + uv[b]) / 2.0;
    if !rim[a] {
        out.push(m);
    }
    split_seed(
        surface,
        field,
        [uv[a], m, uv[c]],
        [rim[a], false, rim[c]],
        depth + 1,
        out,
    );
    split_seed(
        surface,
        field,
        [m, uv[b], uv[c]],
        [rim[a], rim[b], false],
        depth + 1,
        out,
    );
}

fn subdivide(
    field: &SizeField,
    a: Point3,
    b: Point3,
    out: &mut Vec<f64>,
    (t0, t1): (f64, f64),
    depth: usize,
) {
    let (pa, pb) = (a + (b - a) * t0, a + (b - a) * t1);
    let mid = pa.midpoint(pb);
    if depth < 12 && (pb - pa).magnitude() > field.at(mid) {
        let tm = (t0 + t1) / 2.0;
        subdivide(field, a, b, out, (t0, tm), depth + 1);
        subdivide(field, a, b, out, (tm, t1), depth + 1);
    } else {
        out.push(t1);
    }
}

#[allow(clippy::too_many_arguments)]
fn mesh_face(
    out: &mut SurfaceMesh,
    welder: &mut Welder,
    index: usize,
    face: &Face,
    mesh: &PolygonMesh,
    polylines: &[Vec<Point3>],
    field: &SizeField,
    seeds: Vec<Vector2>,
) -> Result<()> {
    let size = field.top;
    let positions = mesh.positions();
    let uv_coords = mesh.uv_coords();
    let triangles: Vec<[usize; 3]> = mesh
        .faces()
        .triangle_iter()
        .map(|t| t.map(|v| v.pos))
        .collect();
    let uv_of: HashMap<usize, Vector2> = mesh
        .faces()
        .triangle_iter()
        .flat_map(|t| t.into_iter())
        .filter_map(|v| Some((v.pos, *uv_coords.as_slice().get(v.uv?)?)))
        .collect();
    let uvs: Vec<Vector2> = (0..positions.len())
        .map(|i| uv_of.get(&i).copied().unwrap_or_else(Vector2::zero))
        .collect();
    let scale = metric(positions, &uvs, &triangles);
    let unscale = scale.invert().unwrap_or_else(Matrix2::identity);
    let surface = face.surface();
    let mut cdt: ConstrainedDelaunayTriangulation<Point2<f64>> = Default::default();
    let mut ids: HashMap<FixedVertexHandle, u32> = HashMap::default();
    let mut handles: HashMap<u32, FixedVertexHandle> = HashMap::default();
    let quantum = welder.quantum;
    let mut add = |out: &mut SurfaceMesh,
                   cdt: &mut ConstrainedDelaunayTriangulation<Point2<f64>>,
                   p: Point3,
                   uv: Vector2|
     -> Option<FixedVertexHandle> {
        let id = out.welded(welder, p);
        out.note_uv(id, index, uv);
        if let Some(&handle) = handles.get(&id) {
            return Some(handle);
        }
        let s = scale * uv;
        let tidy = |x: f64| if x.abs() < 1.0e-30 { 0.0 } else { x };
        let handle = cdt.insert(Point2::new(tidy(s.x), tidy(s.y))).ok()?;
        ids.entry(handle).or_insert(id);
        handles.insert(id, handle);
        Some(handle)
    };
    let mut rim_count: HashMap<[usize; 2], usize> = HashMap::default();
    for t in &triangles {
        for k in 0..3 {
            let mut e = [t[k], t[(k + 1) % 3]];
            e.sort_unstable();
            *rim_count.entry(e).or_default() += 1;
        }
    }
    let rim: Vec<[usize; 2]> = rim_count
        .into_iter()
        .filter(|(_, n)| *n == 1)
        .map(|(e, _)| e)
        .collect();
    let uv_span = {
        let (lo, hi) = uvs.iter().fold(
            (
                Vector2::new(f64::INFINITY, f64::INFINITY),
                Vector2::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
            ),
            |(lo, hi), q| {
                (
                    Vector2::new(lo.x.min(q.x), lo.y.min(q.y)),
                    Vector2::new(hi.x.max(q.x), hi.y.max(q.y)),
                )
            },
        );
        (hi - lo).magnitude().max(1.0e-12)
    };
    let uv_near = |p: Point3| -> Vector2 {
        let hint = rim
            .iter()
            .map(|&[a, b]| {
                let (pa, pb) = (positions[a], positions[b]);
                let d = pb - pa;
                let t = ((p - pa).dot(d) / d.magnitude2().max(1.0e-300)).clamp(0.0, 1.0);
                (
                    (pa + d * t - p).magnitude2(),
                    uvs[a] + (uvs[b] - uvs[a]) * t,
                )
            })
            .min_by(|x, y| x.0.total_cmp(&y.0))
            .map_or(Vector2::zero(), |(_, uv)| uv);
        match surface.search_nearest_parameter(p, Some((hint.x, hint.y)), 30) {
            Some((u, v))
                if (surface.subs(u, v) - p).magnitude() < size * 0.05
                    && (Vector2::new(u, v) - hint).magnitude() < uv_span * 0.05 =>
            {
                Vector2::new(u, v)
            }
            _ => hint,
        }
    };
    let quantised = |p: Point3| [p.x, p.y, p.z].map(|v| (v / quantum).round() as i64);
    let mut chains: Vec<Vec<FixedVertexHandle>> = Vec::new();
    let mut seen: HashSet<[[i64; 3]; 2]> = HashSet::default();
    for line in polylines {
        let line_uv: Vec<Vector2> = line.iter().map(|p| uv_near(*p)).collect();
        for k in 0..line.len().saturating_sub(1) {
            let (a, b) = (k, k + 1);
            let (pa, pb) = (line[a], line[b]);
            let mut key = [quantised(pa), quantised(pb)];
            key.sort_unstable();
            if key[0] == key[1] || !seen.insert(key) {
                continue;
            }
            let (lo, hi) = if [pa.x, pa.y, pa.z] <= [pb.x, pb.y, pb.z] {
                (a, b)
            } else {
                (b, a)
            };
            let (plo, phi) = (line[lo], line[hi]);
            let mut steps = vec![0.0];
            subdivide(field, plo, phi, &mut steps, (0.0, 1.0), 0);
            let mut chain = Vec::new();
            for t in steps {
                let p = plo + (phi - plo) * t;
                let uv = line_uv[lo] + (line_uv[hi] - line_uv[lo]) * t;
                if let Some(h) = add(out, &mut cdt, p, uv) {
                    chain.push(h);
                }
            }
            chains.push(chain);
        }
    }
    for chain in &chains {
        for pair in chain.windows(2) {
            if pair[0] != pair[1] && cdt.can_add_constraint(pair[0], pair[1]) {
                cdt.add_constraint(pair[0], pair[1]);
            }
        }
    }
    for uv in seeds {
        let s = scale * uv;
        let _ = cdt.insert(Point2::new(s.x, s.y));
    }
    let known = cdt.num_vertices();
    let area = 3.0_f64.sqrt() / 4.0 * size * size;
    let result = cdt.refine(
        RefinementParameters::<f64>::new()
            .exclude_outer_faces(true)
            .keep_constraint_edges()
            .with_max_allowed_area(area)
            .with_min_required_area(area * 1.0e-4)
            .with_angle_limit(AngleLimit::from_deg(25.0))
            .with_max_additional_vertices(known * 50 + 200_000),
    );
    let excluded: HashSet<_> = result.excluded_faces.into_iter().collect();
    for vertex in cdt.vertices() {
        let handle = vertex.fix();
        if ids.contains_key(&handle) {
            continue;
        }
        let p = vertex.position();
        let uv = unscale * Vector2::new(p.x, p.y);
        let at = surface.subs(uv.x, uv.y);
        let id = out.fresh(at);
        out.note_uv(id, index, uv);
        ids.insert(handle, id);
    }
    for f in cdt.inner_faces() {
        if excluded.contains(&f.fix()) {
            continue;
        }
        let tri = f.vertices().map(|v| ids[&v.fix()]);
        out.triangles.push(tri);
        out.tags.push(index);
    }
    Ok(())
}
