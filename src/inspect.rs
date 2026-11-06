use crate::mesh::{self, P3};
use crate::model::Model;
use crate::plot::{Anchor, Canvas};
use anyhow::{Result, anyhow, bail};
use std::fmt::Write;

const SIZE: usize = 1024;
const MARGIN: f64 = 70.0;
const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub name: &'static str,
    pub right: P3,
    pub up: P3,
    pub toward: P3,
}

pub fn view(name: &str) -> Result<View> {
    let v = |name, right, up, toward| View {
        name,
        right,
        up,
        toward,
    };
    Ok(match name {
        "front" => v("front", [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]),
        "back" => v("back", [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        "right" => v("right", [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        "left" => v("left", [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [-1.0, 0.0, 0.0]),
        "top" => v("top", [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        "bottom" => v(
            "bottom",
            [1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, -1.0],
        ),
        "iso" => {
            let toward = mesh::normalized([1.0, -1.0, 0.8]);
            let right = mesh::normalized(mesh::cross([0.0, 0.0, 1.0], toward));
            v("iso", right, mesh::cross(toward, right), toward)
        }
        other => bail!("views are front, back, right, left, top, bottom and iso, not `{other}`"),
    })
}

pub struct Options {
    pub view: View,
    pub centre: Option<P3>,
    pub span: Option<f64>,
    pub part: Option<String>,
    pub grid: Option<f64>,
    pub marks: Vec<(String, P3)>,
    pub picks: Vec<(f64, f64)>,
}

struct Surface {
    positions: Vec<P3>,
    triangles: Vec<[u32; 3]>,
    owner: Vec<u32>,
    names: Vec<String>,
    weight: Vec<f64>,
}

fn gather(model: &Model) -> Surface {
    let mut s = Surface {
        positions: Vec::new(),
        triangles: Vec::new(),
        owner: Vec::new(),
        names: Vec::new(),
        weight: Vec::new(),
    };
    let current = model.current_body();
    let selection = model.mesh_body.selection.as_ref();
    for (body, mesh) in model.meshes() {
        let base = s.positions.len() as u32;
        let first = s.names.len() as u32;
        s.names
            .extend(mesh.parts.iter().map(|p| format!("{body}.{p}")));
        s.positions.extend(&mesh.positions);
        let weights = selection
            .filter(|sel| body == current && sel.weights.len() == mesh.positions.len())
            .map(|sel| sel.weights.clone())
            .unwrap_or_else(|| vec![0.0; mesh.positions.len()]);
        s.weight.extend(weights);
        for &t in &mesh.triangles {
            s.triangles.push(t.map(|i| i + base));
            s.owner.push(first + mesh.labels[t[0] as usize] as u32);
        }
    }
    for (name, solid, _) in model.named_parts() {
        let (points, triangles) = crate::geometry::welded(&crate::geometry::mesh(
            solid,
            crate::geometry::mesh_tolerance(solid),
        ));
        let base = s.positions.len() as u32;
        let id = s.names.len() as u32;
        s.names.push(name);
        s.positions.extend(points.iter().map(|p| [p.x, p.y, p.z]));
        s.weight.extend(std::iter::repeat_n(0.0, points.len()));
        for t in triangles {
            s.triangles.push(t.map(|i| i as u32 + base));
            s.owner.push(id);
        }
    }
    s
}

pub struct Frame {
    pub view: View,
    pub centre: P3,
    pub span: f64,
    pub pixels: f64,
}

impl Frame {
    pub fn to_pixel(&self, p: P3) -> (f64, f64, f64) {
        let d = mesh::sub(p, self.centre);
        let s = (SIZE as f64 - 2.0 * MARGIN) / self.span;
        (
            SIZE as f64 / 2.0 + mesh::dot(d, self.view.right) * s,
            SIZE as f64 / 2.0 - mesh::dot(d, self.view.up) * s,
            mesh::dot(d, self.view.toward),
        )
    }

    pub fn on_plane(&self, x: f64, y: f64) -> P3 {
        let s = (SIZE as f64 - 2.0 * MARGIN) / self.span;
        let (u, v) = ((x - SIZE as f64 / 2.0) / s, (SIZE as f64 / 2.0 - y) / s);
        mesh::add(
            self.centre,
            mesh::add(
                mesh::scale(self.view.right, u),
                mesh::scale(self.view.up, v),
            ),
        )
    }
}

pub struct Hit {
    pub point: P3,
    pub normal: P3,
    pub part: String,
}

pub struct Picture {
    pub canvas: Canvas,
    pub frame: Frame,
    pub hits: Vec<Option<Hit>>,
    pub marks: Vec<(String, P3, Option<P3>)>,
    pub report: String,
}

fn bounds_of(points: impl Iterator<Item = P3>) -> Option<(P3, P3)> {
    points.fold(None, |acc, p| {
        Some(match acc {
            None => (p, p),
            Some((lo, hi)) => (
                std::array::from_fn(|k| lo[k].min(p[k])),
                std::array::from_fn(|k| hi[k].max(p[k])),
            ),
        })
    })
}

fn nice_step(span: f64, about: f64) -> f64 {
    let raw = span / about;
    let power = 10f64.powf(raw.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * power)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * power)
}

fn number(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

fn axis_name(dir: P3) -> String {
    let k = (0..3)
        .max_by(|&a, &b| dir[a].abs().total_cmp(&dir[b].abs()))
        .expect("three axes");
    if dir[k].abs() < 0.999 {
        return String::new();
    }
    format!(
        "{}{}",
        if dir[k] < 0.0 { "-" } else { "+" },
        ["x", "y", "z"][k]
    )
}

pub fn inspect(model: &Model, options: &Options) -> Result<Picture> {
    let surface = gather(model);
    if surface.triangles.is_empty() {
        bail!("there is nothing to look at: the file builds no solid or mesh");
    }
    let focus: Vec<u32> = match &options.part {
        Some(name) => {
            let wanted: Vec<u32> = surface
                .names
                .iter()
                .enumerate()
                .filter(|(_, n)| *n == name || n.rsplit('.').next() == Some(name.as_str()))
                .map(|(i, _)| i as u32)
                .collect();
            if wanted.is_empty() {
                bail!(
                    "there is no part `{name}`; the parts are {}",
                    surface
                        .names
                        .iter()
                        .map(|n| n.rsplit('.').next().unwrap_or(n))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            wanted
        }
        None => (0..surface.names.len() as u32).collect(),
    };
    let view = options.view;
    let focused = bounds_of(
        surface
            .triangles
            .iter()
            .zip(&surface.owner)
            .filter(|(_, o)| focus.contains(o))
            .flat_map(|(t, _)| t.map(|i| surface.positions[i as usize])),
    )
    .ok_or_else(|| anyhow!("the part has no triangles"))?;
    let centre = options
        .centre
        .unwrap_or_else(|| mesh::scale(mesh::add(focused.0, focused.1), 0.5));
    let corners: Vec<P3> = (0..8)
        .map(|c| {
            std::array::from_fn(|k| {
                if (c >> k) & 1 == 1 {
                    focused.1[k]
                } else {
                    focused.0[k]
                }
            })
        })
        .collect();
    let extent = corners
        .iter()
        .flat_map(|p| {
            let d = mesh::sub(*p, centre);
            [mesh::dot(d, view.right).abs(), mesh::dot(d, view.up).abs()]
        })
        .fold(0.0, f64::max);
    let span = options.span.unwrap_or(2.0 * extent * 1.08).max(1.0e-6);
    let frame = Frame {
        view,
        centre,
        span,
        pixels: (SIZE as f64 - 2.0 * MARGIN) / span,
    };
    let mut depth = vec![f64::NEG_INFINITY; SIZE * SIZE];
    let mut tri_at = vec![NONE; SIZE * SIZE];
    let projected: Vec<(f64, f64, f64)> = surface
        .positions
        .iter()
        .map(|&p| frame.to_pixel(p))
        .collect();
    for (f, t) in surface.triangles.iter().enumerate() {
        let [a, b, c] = t.map(|i| projected[i as usize]);
        let area = (b.0 - a.0) * (c.1 - a.1) - (c.0 - a.0) * (b.1 - a.1);
        if area.abs() < 1.0e-12 {
            continue;
        }
        let x0 = a.0.min(b.0).min(c.0).floor().max(0.0) as i64;
        let x1 = a.0.max(b.0).max(c.0).ceil().min(SIZE as f64 - 1.0) as i64;
        let y0 = a.1.min(b.1).min(c.1).floor().max(0.0) as i64;
        let y1 = a.1.max(b.1).max(c.1).ceil().min(SIZE as f64 - 1.0) as i64;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                let w0 = ((b.0 - px) * (c.1 - py) - (c.0 - px) * (b.1 - py)) / area;
                let w1 = ((c.0 - px) * (a.1 - py) - (a.0 - px) * (c.1 - py)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let z = w0 * a.2 + w1 * b.2 + w2 * c.2;
                let k = y as usize * SIZE + x as usize;
                if z > depth[k] {
                    depth[k] = z;
                    tri_at[k] = f as u32;
                }
            }
        }
    }
    let normals = {
        let mut n = vec![[0.0; 3]; surface.positions.len()];
        for t in &surface.triangles {
            let [a, b, c] = t.map(|i| surface.positions[i as usize]);
            let face = mesh::cross(mesh::sub(b, a), mesh::sub(c, a));
            for i in t {
                n[*i as usize] = mesh::add(n[*i as usize], face);
            }
        }
        n.into_iter().map(mesh::normalized).collect::<Vec<_>>()
    };
    let surface_at = |x: f64, y: f64| -> Option<(u32, P3, P3, f64)> {
        let (xi, yi) = (x.floor() as i64, y.floor() as i64);
        if xi < 0 || yi < 0 || xi >= SIZE as i64 || yi >= SIZE as i64 {
            return None;
        }
        let f = tri_at[yi as usize * SIZE + xi as usize];
        if f == NONE {
            return None;
        }
        let t = surface.triangles[f as usize];
        let [a, b, c] = t.map(|i| projected[i as usize]);
        let area = (b.0 - a.0) * (c.1 - a.1) - (c.0 - a.0) * (b.1 - a.1);
        let w0 = ((b.0 - x) * (c.1 - y) - (c.0 - x) * (b.1 - y)) / area;
        let w1 = ((c.0 - x) * (a.1 - y) - (a.0 - x) * (c.1 - y)) / area;
        let w2 = 1.0 - w0 - w1;
        let [pa, pb, pc] = t.map(|i| surface.positions[i as usize]);
        let point = mesh::add(
            mesh::add(mesh::scale(pa, w0), mesh::scale(pb, w1)),
            mesh::scale(pc, w2),
        );
        let [na, nb, nc] = t.map(|i| normals[i as usize]);
        let normal = mesh::normalized(mesh::add(
            mesh::add(mesh::scale(na, w0), mesh::scale(nb, w1)),
            mesh::scale(nc, w2),
        ));
        let [ga, gb, gc] = t.map(|i| surface.weight[i as usize]);
        Some((
            surface.owner[f as usize],
            point,
            normal,
            w0 * ga + w1 * gb + w2 * gc,
        ))
    };
    let mut canvas = Canvas::new(SIZE as u32, SIZE as u32);
    let light = mesh::normalized(mesh::add(
        mesh::add(mesh::scale(view.toward, 0.8), mesh::scale(view.up, 0.45)),
        mesh::scale(view.right, -0.3),
    ));
    let mut owner_px = vec![NONE; SIZE * SIZE];
    let mut weight_px = vec![0.0f64; SIZE * SIZE];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let Some((owner, _, n, w)) = surface_at(x as f64 + 0.5, y as f64 + 0.5) else {
                continue;
            };
            owner_px[y * SIZE + x] = owner;
            weight_px[y * SIZE + x] = w;
            let n = if mesh::dot(n, view.toward) < 0.0 {
                mesh::scale(n, -1.0)
            } else {
                n
            };
            let lit = 0.6 + 0.4 * mesh::dot(n, light).max(0.0);
            let base = [
                0.5 + 0.5 * mesh::dot(n, view.right),
                0.5 + 0.5 * mesh::dot(n, view.up),
                0.55 + 0.45 * mesh::dot(n, view.toward),
            ];
            let dim = if focus.contains(&owner) { 1.0 } else { 0.45 };
            let rgb = base.map(|c| (c * lit * dim).clamp(0.0, 1.0));
            canvas.image.put_pixel(
                x as u32,
                y as u32,
                image::Rgb(rgb.map(|c| (c * 255.0) as u8)),
            );
        }
    }
    for y in 1..SIZE - 1 {
        for x in 1..SIZE - 1 {
            let k = y * SIZE + x;
            let o = owner_px[k];
            let crosses = |level: f64| {
                [k + 1, k + SIZE, k - 1, k - SIZE].iter().any(|&j| {
                    (weight_px[k] > level) != (weight_px[j] > level) && weight_px[k] > level
                })
            };
            if crosses(0.999) || crosses(1.0e-6) {
                let full = crosses(0.999);
                let colour = if full { [255, 220, 0] } else { [255, 150, 0] };
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    canvas
                        .image
                        .put_pixel((x + dx) as u32, (y + dy) as u32, image::Rgb(colour));
                }
                continue;
            }
            let edge = o != NONE && [k + 1, k + SIZE].iter().any(|&j| owner_px[j] != o);
            if edge {
                let p = canvas.image.get_pixel_mut(x as u32, y as u32);
                *p = image::Rgb(p.0.map(|c| (c as f64 * 0.35) as u8));
            }
        }
    }
    let step = options.grid.unwrap_or_else(|| nice_step(span, 8.0));
    let (h_axis, v_axis) = (axis_name(view.right), axis_name(view.up));
    let low = frame.on_plane(MARGIN, SIZE as f64 - MARGIN);
    let high = frame.on_plane(SIZE as f64 - MARGIN, MARGIN);
    let (u0, u1) = {
        let (a, b) = (mesh::dot(low, view.right), mesh::dot(high, view.right));
        (a.min(b), a.max(b))
    };
    let (v0, v1) = {
        let (a, b) = (mesh::dot(low, view.up), mesh::dot(high, view.up));
        (a.min(b), a.max(b))
    };
    let grid = [70, 110, 210];
    let aligned = !h_axis.is_empty() && !v_axis.is_empty();
    let mut u = if aligned {
        (u0 / step).ceil() * step
    } else {
        f64::INFINITY
    };
    while u <= u1 + 1.0e-9 {
        let x = SIZE as f64 / 2.0 + (u - mesh::dot(frame.centre, view.right)) * frame.pixels;
        canvas.line(&[(x, MARGIN), (x, SIZE as f64 - MARGIN)], 0.8, grid);
        canvas.text(
            (x, SIZE as f64 - MARGIN + 18.0),
            13.0,
            &number(u),
            Anchor::Centre,
            grid,
        );
        canvas.text((x, MARGIN - 8.0), 13.0, &number(u), Anchor::Centre, grid);
        u += step;
    }
    let mut v = if aligned {
        (v0 / step).ceil() * step
    } else {
        f64::INFINITY
    };
    while v <= v1 + 1.0e-9 {
        let y = SIZE as f64 / 2.0 - (v - mesh::dot(frame.centre, view.up)) * frame.pixels;
        canvas.line(&[(MARGIN, y), (SIZE as f64 - MARGIN, y)], 0.8, grid);
        canvas.text(
            (MARGIN - 6.0, y + 5.0),
            13.0,
            &number(v),
            Anchor::Right,
            grid,
        );
        canvas.text(
            (SIZE as f64 - MARGIN + 6.0, y + 5.0),
            13.0,
            &number(v),
            Anchor::Left,
            grid,
        );
        v += step;
    }
    let ink = [20, 20, 20];
    if surface.weight.iter().any(|w| *w > 0.0) {
        canvas.rect((MARGIN, 38.0), (MARGIN + 18.0, 42.0), [255, 220, 0]);
        canvas.text(
            (MARGIN + 24.0, 45.0),
            13.0,
            "selection at full strength",
            Anchor::Left,
            ink,
        );
        canvas.rect(
            (MARGIN + 210.0, 38.0),
            (MARGIN + 228.0, 42.0),
            [255, 150, 0],
        );
        canvas.text(
            (MARGIN + 234.0, 45.0),
            13.0,
            "where its falloff ends",
            Anchor::Left,
            ink,
        );
    }
    let depth_axis = axis_name(view.toward);
    let title = if !aligned {
        format!(
            "{} view: no grid, it is not square to an axis; use marks and picks, scale bar {}",
            view.name,
            number(step)
        )
    } else {
        format!(
            "{} view: right {h_axis}, up {v_axis}, toward you {depth_axis}; grid {}",
            view.name,
            number(step)
        )
    };
    canvas.text((MARGIN, 26.0), 16.0, &title, Anchor::Left, ink);
    let bar = step;
    let bx = SIZE as f64 - MARGIN - bar * frame.pixels;
    let by = SIZE as f64 - 22.0;
    canvas.line(&[(bx, by), (bx + bar * frame.pixels, by)], 3.0, ink);
    canvas.text(
        (bx + bar * frame.pixels / 2.0, by - 6.0),
        13.0,
        &format!("{} units", number(bar)),
        Anchor::Centre,
        ink,
    );
    let (ax, ay) = (MARGIN - 40.0, SIZE as f64 - 30.0);
    for (dir, colour, label) in [
        ([1.0, 0.0, 0.0], [210, 40, 40], "x"),
        ([0.0, 1.0, 0.0], [40, 160, 40], "y"),
        ([0.0, 0.0, 1.0], [40, 70, 220], "z"),
    ] {
        let (dx, dy) = (mesh::dot(dir, view.right), -mesh::dot(dir, view.up));
        if dx.abs() + dy.abs() < 0.05 {
            let sign = if mesh::dot(dir, view.toward) > 0.0 {
                "toward you"
            } else {
                "away"
            };
            canvas.text(
                (ax + 4.0, ay - 4.0),
                12.0,
                &format!("{label} {sign}"),
                Anchor::Left,
                colour,
            );
            continue;
        }
        canvas.line(&[(ax, ay), (ax + dx * 26.0, ay + dy * 26.0)], 2.0, colour);
        canvas.text(
            (ax + dx * 34.0, ay + dy * 34.0 + 4.0),
            12.0,
            label,
            Anchor::Centre,
            colour,
        );
    }
    let mut labelled: Vec<(u32, (f64, f64), usize)> = Vec::new();
    for y in (0..SIZE).step_by(2) {
        for x in (0..SIZE).step_by(2) {
            let o = owner_px[y * SIZE + x];
            if o == NONE {
                continue;
            }
            match labelled.iter_mut().find(|e| e.0 == o) {
                Some(e) => {
                    e.1.0 += x as f64;
                    e.1.1 += y as f64;
                    e.2 += 1;
                }
                None => labelled.push((o, (x as f64, y as f64), 1)),
            }
        }
    }
    for (o, (sx, sy), n) in &labelled {
        if *n < 60 {
            continue;
        }
        let name = surface.names[*o as usize]
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_string();
        let (cx, cy) = (sx / *n as f64, sy / *n as f64);
        let w = canvas.text_width(&name, 12.0);
        canvas.rect(
            (cx - w / 2.0 - 3.0, cy - 11.0),
            (cx + w / 2.0 + 3.0, cy + 4.0),
            [255, 255, 255],
        );
        canvas.text((cx, cy), 12.0, &name, Anchor::Centre, ink);
    }
    let mut report = String::new();
    let _ = writeln!(
        report,
        "{} view, centre {},{},{}, {} units across, {} px per unit; pixel (x,y) is the plane point centre + (x-512)/{:.3} along {} and (512-y)/{:.3} along {}",
        view.name,
        number(centre[0]),
        number(centre[1]),
        number(centre[2]),
        number(span),
        number(frame.pixels),
        frame.pixels,
        if h_axis.is_empty() {
            "right".into()
        } else {
            h_axis.clone()
        },
        frame.pixels,
        if v_axis.is_empty() {
            "up".into()
        } else {
            v_axis.clone()
        },
    );
    let mut marks = Vec::new();
    for (k, (name, p)) in options.marks.iter().enumerate() {
        let (x, y, z) = frame.to_pixel(*p);
        let seen = surface_at(x, y).map(|(_, q, _, _)| frame.to_pixel(q).2);
        let hidden = seen.is_some_and(|front| front > z + 0.01 * span);
        let colour = if hidden {
            [150, 150, 150]
        } else {
            [220, 30, 30]
        };
        canvas.line(&[(x - 7.0, y), (x + 7.0, y)], 2.0, colour);
        canvas.line(&[(x, y - 7.0), (x, y + 7.0)], 2.0, colour);
        let tag = format!("{} {name}", k + 1);
        canvas.text((x + 8.0, y - 6.0), 13.0, &tag, Anchor::Left, colour);
        let _ = writeln!(
            report,
            "mark {} {name}: {},{},{} at pixel {:.0},{:.0}{}",
            k + 1,
            number(p[0]),
            number(p[1]),
            number(p[2]),
            x,
            y,
            if hidden { ", behind the surface" } else { "" }
        );
        marks.push((name.clone(), *p, (!hidden).then_some(*p)));
    }
    let mut hits = Vec::new();
    for (k, &(x, y)) in options.picks.iter().enumerate() {
        let hit = surface_at(x, y).map(|(o, point, normal, _)| Hit {
            point,
            normal: if mesh::dot(normal, view.toward) < 0.0 {
                mesh::scale(normal, -1.0)
            } else {
                normal
            },
            part: surface.names[o as usize]
                .rsplit('.')
                .next()
                .unwrap_or("")
                .to_string(),
        });
        let colour = [255, 140, 0];
        canvas.line(&[(x - 9.0, y - 9.0), (x + 9.0, y + 9.0)], 2.0, colour);
        canvas.line(&[(x - 9.0, y + 9.0), (x + 9.0, y - 9.0)], 2.0, colour);
        canvas.text(
            (x + 10.0, y + 16.0),
            13.0,
            &format!("p{}", k + 1),
            Anchor::Left,
            colour,
        );
        match &hit {
            Some(h) => {
                let _ = writeln!(
                    report,
                    "pick p{} at pixel {},{}: {},{},{} on `{}`, normal {},{},{}",
                    k + 1,
                    number(x),
                    number(y),
                    number(h.point[0]),
                    number(h.point[1]),
                    number(h.point[2]),
                    h.part,
                    number(h.normal[0]),
                    number(h.normal[1]),
                    number(h.normal[2]),
                );
            }
            None => {
                let p = frame.on_plane(x, y);
                let _ = writeln!(
                    report,
                    "pick p{} at pixel {},{}: misses the surface; on the view plane it is {},{},{}",
                    k + 1,
                    number(x),
                    number(y),
                    number(p[0]),
                    number(p[1]),
                    number(p[2]),
                );
            }
        }
        hits.push(hit);
    }
    Ok(Picture {
        canvas,
        frame,
        hits,
        marks,
        report,
    })
}
