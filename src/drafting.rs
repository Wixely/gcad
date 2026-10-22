mod annotate;

use crate::drawing::{polylines, section_of, silhouettes};
use crate::geometry;
use crate::model::Model;
use crate::parse::{Line, Scope, eval, split_top_level};
use anyhow::{Result, anyhow, bail};
use monstertruck::mesh::PolygonMesh;
use monstertruck::modeling::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

pub fn is_drawing(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("gdwg"))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

#[derive(Clone, Debug)]
pub enum Mark {
    Shape {
        loops: Vec<Vec<(f64, f64)>>,
        closed: bool,
        fill: Option<[f64; 3]>,
        stroke: Option<[f64; 3]>,
        width: f64,
        dashed: bool,
    },
    Text {
        x: f64,
        y: f64,
        size: f64,
        text: String,
        anchor: Anchor,
        angle: f64,
    },
}

#[derive(Clone, Debug)]
pub struct Sheet {
    pub name: String,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
    pub marks: Vec<Mark>,
}

#[derive(Default)]
pub struct Drawing {
    pub title: String,
    pub sheets: Vec<Sheet>,
    pub steps: Vec<(Line, Result<String>)>,
}

impl Drawing {
    pub fn failed(&self) -> Option<(&Line, &anyhow::Error)> {
        self.steps
            .iter()
            .find_map(|(line, result)| result.as_ref().err().map(|e| (line, e)))
    }
}

#[derive(Clone)]
struct Frame {
    right: Vector3,
    up: Vector3,
    scale: f64,
    origin: (f64, f64),
    window: (f64, f64, f64, f64),
    up_is_z: bool,
}

impl Frame {
    fn uv(&self, p: Point3) -> (f64, f64) {
        (p.to_vec().dot(self.right), p.to_vec().dot(self.up))
    }

    fn paper(&self, (u, v): (f64, f64)) -> (f64, f64) {
        (
            self.origin.0 + (u - self.window.0) / self.scale,
            self.origin.1 - (v - self.window.1) / self.scale,
        )
    }

    fn box_paper(&self) -> (f64, f64, f64, f64) {
        let (x0, y0) = self.paper((self.window.0, self.window.1));
        let (x1, y1) = self.paper((self.window.2, self.window.3));
        (x0, y1, x1, y0)
    }
}

struct PendingView {
    view: (f64, f64, f64, f64),
    own: std::ops::Range<usize>,
    labels: Vec<annotate::Label>,
}

struct State {
    dir: PathBuf,
    scope: Scope,
    model: Option<Model>,
    model_file: PathBuf,
    drawing: Drawing,
    frame: Option<Frame>,
    cursor: (f64, f64, f64),
    pending: Vec<PendingView>,
    label_line: Option<Line>,
    view_geometry: Option<(usize, usize, usize)>,
    view_box: Option<(f64, f64, f64, f64)>,
}

pub(crate) const MARGIN: f64 = 10.0;
const TITLE_H: f64 = 28.0;
const TEXT: f64 = 2.5;
const BLACK: [f64; 3] = [0.0, 0.0, 0.0];
const BODY: [f64; 3] = [0.72, 0.72, 0.74];
const MEMBER_KINDS: [&str; 6] = ["bar ", "tube ", "pipe ", "rod ", "angle ", "channel "];

pub fn run(path: &Path, vars: &[(String, f64)]) -> Result<Drawing> {
    let source = crate::host::read_to_string(path)
        .map_err(|e| anyhow!("reading {}: {e}", path.display()))?;
    let lines = crate::parse::parse_program(&source)?;
    let mut state = State {
        dir: path.parent().map(Path::to_path_buf).unwrap_or_default(),
        scope: vars.iter().cloned().collect(),
        model: None,
        model_file: PathBuf::new(),
        drawing: Drawing::default(),
        frame: None,
        cursor: (0.0, 0.0, 0.0),
        pending: Vec::new(),
        label_line: None,
        view_geometry: None,
        view_box: None,
    };
    let fixed: Scope = vars.iter().cloned().collect();
    for line in lines {
        let result = state.apply(&line, &fixed);
        let failed = result.is_err();
        state.drawing.steps.push((line, result));
        if failed {
            break;
        }
    }
    if let Err(error) = state.flush_labels() {
        if let Some(line) = state.label_line.clone() {
            state.drawing.steps.push((line, Err(error)));
        }
    }
    let count = state.drawing.sheets.len();
    let title = state.drawing.title.clone();
    let file = state
        .model_file
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    for (index, sheet) in state.drawing.sheets.iter_mut().enumerate() {
        title_block(sheet, &title, &file, index + 1, count);
    }
    Ok(state.drawing)
}

fn text_of(line: &Line, index: usize) -> Option<String> {
    line.positional
        .get(index)
        .map(|t| t.trim_matches('"').to_string())
}

fn named<'a>(line: &'a Line, key: &str) -> Option<&'a str> {
    line.named
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn pair(text: &str, scope: &Scope) -> Result<(f64, f64)> {
    match split_top_level(text, ',').as_slice() {
        [a, b] => Ok((eval(a, scope)?, eval(b, scope)?)),
        _ => bail!("`{text}` is not a pair, write it as a,b"),
    }
}

fn point3(text: &str, scope: &Scope) -> Result<Point3> {
    match split_top_level(text, ',').as_slice() {
        [a, b, c] => Ok(Point3::new(
            eval(a, scope)?,
            eval(b, scope)?,
            eval(c, scope)?,
        )),
        _ => bail!("`{text}` is not a point, write it as x,y,z"),
    }
}

fn quad(text: &str, scope: &Scope) -> Result<(f64, f64, f64, f64)> {
    match split_top_level(text, ',').as_slice() {
        [a, b, c, d] => Ok((
            eval(a, scope)?,
            eval(b, scope)?,
            eval(c, scope)?,
            eval(d, scope)?,
        )),
        _ => bail!("`{text}` is not four numbers, write it as u0,v0,u1,v1"),
    }
}

fn scale_of(text: &str) -> Result<f64> {
    let parts: Vec<&str> = text.split(':').collect();
    match parts.as_slice() {
        [a, b] => {
            let (a, b): (f64, f64) = (a.parse()?, b.parse()?);
            if a <= 0.0 || b <= 0.0 {
                bail!("a scale is two positive numbers, like 1:25");
            }
            Ok(b / a)
        }
        _ => bail!("write a scale as 1:25, not `{text}`"),
    }
}

fn sheet_size(text: &str) -> Result<(f64, f64)> {
    Ok(match text.to_ascii_uppercase().as_str() {
        "A4" => (297.0, 210.0),
        "A3" => (420.0, 297.0),
        "A2" => (594.0, 420.0),
        "A1" => (841.0, 594.0),
        "A0" => (1189.0, 841.0),
        other => bail!("a sheet is A0 to A4, not `{other}`"),
    })
}

fn direction(text: &str) -> Result<(Vector3, Vector3)> {
    let toward = match text {
        "-y" | "front" => Vector3::unit_y(),
        "+y" | "back" => -Vector3::unit_y(),
        "-x" | "left" => Vector3::unit_x(),
        "+x" | "right" => -Vector3::unit_x(),
        "+z" | "top" => -Vector3::unit_z(),
        "-z" | "bottom" => Vector3::unit_z(),
        other => bail!("look from -y, +y, -x, +x, +z or -z, not `{other}`"),
    };
    let up = if toward.z.abs() > 0.5 {
        Vector3::unit_y() * toward.z.signum() * -1.0
    } else {
        Vector3::unit_z()
    };
    Ok((toward, up))
}

fn lighten(colour: [f64; 3], by: f64) -> [f64; 3] {
    colour.map(|c| c + (1.0 - c) * by)
}

impl State {
    fn model(&self) -> Result<&Model> {
        self.model
            .as_ref()
            .ok_or_else(|| anyhow!("start with a `part` line naming the .gcad or .gasm to draw"))
    }

    fn sheet(&mut self) -> Result<&mut Sheet> {
        self.drawing
            .sheets
            .last_mut()
            .ok_or_else(|| anyhow!("start a `sheet` before drawing on it"))
    }

    fn frame(&self) -> Result<&Frame> {
        self.frame
            .as_ref()
            .ok_or_else(|| anyhow!("draw a `view` or `section` first"))
    }

    fn picked_by(&self, line: &Line) -> Result<Vec<String>> {
        let patterns: Vec<String> = if line.positional.is_empty() {
            vec!["*".to_string()]
        } else {
            line.positional.clone()
        };
        let names = self.picked(&patterns)?;
        let Some(profiles) = named(line, "profile") else {
            return Ok(names);
        };
        let model = self.model()?;
        let wanted: Vec<&str> = profiles
            .trim_matches('"')
            .split(',')
            .map(str::trim)
            .collect();
        let kept: Vec<String> = names
            .into_iter()
            .filter(|name| {
                model.sources.get(name).is_some_and(|source| {
                    let profile = source.file.display().to_string();
                    wanted.iter().any(|w| crate::sim::matches(w, &profile))
                })
            })
            .collect();
        if kept.is_empty() {
            bail!("no member of {} has profile {profiles}", patterns.join(" "));
        }
        let mut kept = kept;
        if let Some(also) = named(line, "also") {
            let extra: Vec<String> = also.split(',').map(str::to_string).collect();
            for name in self.picked(&extra)? {
                if !kept.contains(&name) {
                    kept.push(name);
                }
            }
        }
        Ok(kept)
    }

    fn picked(&self, patterns: &[String]) -> Result<Vec<String>> {
        let model = self.model()?;
        let names: Vec<String> = model
            .named_solids()
            .into_iter()
            .map(|(name, _)| name)
            .filter(|name| {
                let mut wanted = patterns.iter().filter(|p| !p.starts_with('!')).peekable();
                let keep = wanted.peek().is_none() || wanted.any(|p| crate::sim::matches(p, name));
                let drop = patterns
                    .iter()
                    .filter_map(|p| p.strip_prefix('!'))
                    .any(|p| crate::sim::matches(p, name));
                keep && !drop
            })
            .collect();
        if names.is_empty() {
            bail!("no part matches {}", patterns.join(" "));
        }
        Ok(names)
    }

    fn apply(&mut self, line: &Line, fixed: &Scope) -> Result<String> {
        if line.op == "sheet" {
            self.flush_labels()?;
        }
        match line.op.as_str() {
            "let" => {
                for (key, value) in &line.named {
                    if fixed.contains_key(key) {
                        continue;
                    }
                    let value = eval(value, &self.scope)?;
                    self.scope.insert(key.clone(), value);
                }
                Ok(line
                    .named
                    .iter()
                    .map(|(k, _)| format!("{k}={}", self.scope[k]))
                    .collect::<Vec<_>>()
                    .join(" "))
            }
            "if" => {
                let condition = line
                    .positional
                    .first()
                    .ok_or_else(|| anyhow!("write `if condition operation ...`"))?;
                if eval(condition, &self.scope)? == 0.0 {
                    return Ok(format!("skipped, `{condition}` is false"));
                }
                let rest = line
                    .text
                    .splitn(3, char::is_whitespace)
                    .nth(2)
                    .unwrap_or("");
                let inner = crate::parse::parse_line(line.number, rest)?;
                self.apply(&inner, fixed)
            }
            "part" => self.op_part(line),
            "title" => {
                self.drawing.title = text_of(line, 0).unwrap_or_default();
                Ok(format!("title {}", self.drawing.title))
            }
            "sheet" => self.op_sheet(line),
            "view" => self.op_view(line, false),
            "section" => self.op_view(line, true),
            "dims" => self.op_dims(line),
            "dim" => self.op_dim(line),
            "level" => self.op_level(line),
            "note" => self.op_note(line),
            "callout" => self.op_callout(line),
            "label" => self.op_label(line),
            "cutlist" => self.op_cutlist(line),
            "nest" => self.op_nest(line),
            other => bail!("`{other}` is not a drawing operation; see `gcad docs`"),
        }
    }

    fn op_part(&mut self, line: &Line) -> Result<String> {
        let file = text_of(line, 0).ok_or_else(|| anyhow!("write `part file.gasm`"))?;
        let path = self.dir.join(&file);
        let vars = line
            .named
            .iter()
            .map(|(k, v)| Ok((k.clone(), eval(v, &self.scope)?)))
            .collect::<Result<Vec<_>>>()?;
        let run = crate::model::run_path(&path, &vars, None)?;
        if let Some((line, Err(error))) = run.steps.last() {
            bail!("{file} line {}: {}: {error:#}", line.number, line.text);
        }
        let count = run.model.named_solids().len();
        self.model = Some(run.model);
        self.model_file = path;
        Ok(format!("{count} parts from {file}"))
    }

    fn op_sheet(&mut self, line: &Line) -> Result<String> {
        let name =
            text_of(line, 0).unwrap_or_else(|| format!("sheet {}", self.drawing.sheets.len() + 1));
        let (width, height) = sheet_size(named(line, "size").unwrap_or("A3"))?;
        let scale = scale_of(named(line, "scale").unwrap_or("1:1"))?;
        let mut sheet = Sheet {
            name: name.clone(),
            width,
            height,
            scale,
            marks: Vec::new(),
        };
        rectangle(
            &mut sheet,
            MARGIN,
            MARGIN,
            width - MARGIN,
            height - MARGIN,
            0.5,
        );
        self.drawing.sheets.push(sheet);
        self.frame = None;
        self.cursor = (MARGIN + 22.0, MARGIN + 14.0, 0.0);
        Ok(format!("{name}: {width} x {height} mm at 1:{scale}"))
    }

    fn place(&mut self, w: f64, h: f64, at: Option<(f64, f64)>) -> Result<(f64, f64)> {
        let sheet = self
            .drawing
            .sheets
            .last()
            .ok_or_else(|| anyhow!("start a `sheet` first"))?;
        let (width, height) = (sheet.width, sheet.height);
        let (x, top) = match at {
            Some((x, y)) => (x, height - y - h),
            None => {
                let (mut x, mut y, mut row) = self.cursor;
                if x + w > width - MARGIN - 6.0 && x > MARGIN + 23.0 {
                    x = MARGIN + 22.0;
                    y += row + 36.0;
                    row = 0.0;
                }
                self.cursor = (x + w + 42.0, y, row.max(h));
                (x, y)
            }
        };
        let bottom = top + h;
        let floor = if x + w > width - MARGIN - 170.0 {
            height - MARGIN - TITLE_H
        } else {
            height - MARGIN
        };
        if x < MARGIN || top < MARGIN || x + w > width - MARGIN || bottom > floor {
            bail!(
                "it does not fit on the sheet ({} x {} mm at {},{} from the top left); give it `place=x,y`, a smaller `scale`, or a bigger sheet",
                number(w),
                number(h),
                number(x),
                number(top)
            );
        }
        Ok((x, bottom))
    }

    fn op_view(&mut self, line: &Line, cut: bool) -> Result<String> {
        let names = self.picked_by(line)?;
        let from = named(line, "from").unwrap_or(if cut { "+x" } else { "-y" });
        let (toward, up) = direction(from)?;
        let right = toward.cross(up);
        let scale = match named(line, "scale") {
            Some(s) => scale_of(s)?,
            None => self.sheet()?.scale,
        };
        let plane = if cut {
            let text = named(line, "at")
                .ok_or_else(|| anyhow!("a section needs `at=` along the direction it looks"))?;
            Some(eval(text, &self.scope)?)
        } else {
            None
        };
        let model = self.model()?;
        let solids: Vec<(String, Solid, [f64; 3])> = model
            .named_solids()
            .into_iter()
            .filter(|(n, _)| names.contains(n))
            .map(|(n, s)| {
                let colour = model.colours.get(&n).copied().unwrap_or(BODY);
                (n, s.clone(), colour)
            })
            .collect();
        let meshes: Vec<PolygonMesh> = crate::host::each(solids.iter().collect(), |(_, s, _)| {
            geometry::mesh(s, geometry::mesh_tolerance(s))
        })
        .into_iter()
        .map(|m| m.ok_or_else(|| anyhow!("meshing a part failed")))
        .collect::<Result<_>>()?;
        let frame0 = Frame {
            right,
            up,
            scale,
            origin: (0.0, 0.0),
            window: (0.0, 0.0, 0.0, 0.0),
            up_is_z: up.z.abs() > 0.5,
        };
        let mut sections: Vec<Vec<Vec<(f64, f64)>>> = Vec::new();
        let mut extent = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        let mut grow = |u: f64, v: f64| {
            extent = (
                extent.0.min(u),
                extent.1.min(v),
                extent.2.max(u),
                extent.3.max(v),
            );
        };
        for ((_, solid, _), mesh) in solids.iter().zip(&meshes) {
            if let Some(at) = plane {
                let normal = -toward;
                let loops: Vec<Vec<(f64, f64)>> =
                    section_of(mesh, solid, normal, at * (normal.x + normal.y + normal.z))
                        .into_iter()
                        .map(|l| l.into_iter().map(|p| frame0.uv(p)).collect())
                        .collect();
                for l in &loops {
                    for &(u, v) in l {
                        grow(u, v);
                    }
                }
                sections.push(loops);
            } else {
                for p in mesh.positions() {
                    let (u, v) = frame0.uv(*p);
                    grow(u, v);
                }
                sections.push(Vec::new());
            }
        }
        if !extent.0.is_finite() {
            bail!("the section misses every part");
        }
        let window = match named(line, "window") {
            Some(text) => {
                let (a, b, c, d) = quad(text, &self.scope)?;
                (a.min(c), b.min(d), a.max(c), b.max(d))
            }
            None => extent,
        };
        let (w, h) = ((window.2 - window.0) / scale, (window.3 - window.1) / scale);
        let at = named(line, "place")
            .map(|t| pair(t, &self.scope))
            .transpose()?;
        let (x, y) = self.place(w, h, at)?;
        let frame = Frame {
            origin: (x, y),
            window,
            ..frame0
        };
        let clip = (
            frame.box_paper().0,
            frame.box_paper().1,
            frame.box_paper().2,
            frame.box_paper().3,
        );
        let mut marks = Vec::new();
        if plane.is_some() {
            for ((_, _, colour), loops) in solids.iter().zip(&sections) {
                let paper: Vec<Vec<(f64, f64)>> = loops
                    .iter()
                    .map(|l| {
                        clip_polygon(&l.iter().map(|p| frame.paper(*p)).collect::<Vec<_>>(), clip)
                    })
                    .filter(|l| l.len() > 2)
                    .collect();
                if paper.is_empty() {
                    continue;
                }
                marks.push(Mark::Shape {
                    loops: paper,
                    closed: true,
                    fill: Some(*colour),
                    stroke: Some(BLACK),
                    width: 0.2,
                    dashed: false,
                });
            }
        } else {
            let mut order: Vec<usize> = (0..solids.len()).collect();
            let depth = |i: usize| {
                let b = geometry::bounds(&solids[i].1);
                let c = (b.min().to_vec() + b.max().to_vec()) / 2.0;
                c.dot(toward)
            };
            order.sort_by(|&a, &b| depth(b).total_cmp(&depth(a)));
            for i in order {
                let (_, solid, colour) = &solids[i];
                let mesh = &meshes[i];
                let positions = mesh.positions();
                let fill = lighten(*colour, 0.25);
                let mut faces = Vec::new();
                for triangle in mesh.faces().triangle_iter() {
                    let [a, b, c] = triangle.map(|v| positions[v.pos]);
                    let normal = (b - a).cross(c - a);
                    if normal.dot(toward) >= 0.0 {
                        continue;
                    }
                    let loop_ =
                        clip_polygon(&[a, b, c].map(|p| frame.paper(frame.uv(p))), clip);
                    if loop_.len() > 2 {
                        faces.push(loop_);
                    }
                }
                if !faces.is_empty() {
                    marks.push(Mark::Shape {
                        loops: faces,
                        closed: true,
                        fill: Some(fill),
                        stroke: Some(fill),
                        width: 0.06,
                        dashed: false,
                    });
                }
                let edges: Vec<Vec<(f64, f64)>> = polylines(solid)
                    .into_iter()
                    .chain(silhouettes(mesh, solid, toward))
                    .flat_map(|l| {
                        clip_line(
                            &l.iter()
                                .map(|p| frame.paper(frame.uv(*p)))
                                .collect::<Vec<_>>(),
                            clip,
                        )
                    })
                    .collect();
                if !edges.is_empty() {
                    marks.push(Mark::Shape {
                        loops: edges,
                        closed: false,
                        fill: None,
                        stroke: Some(BLACK),
                        width: 0.13,
                        dashed: false,
                    });
                }
            }
        }
        if named(line, "lengths").is_some_and(|t| t != "0") {
            let model = self.model()?;
            let mut taken: Vec<(f64, f64, f64, f64)> = Vec::new();
            for (name, solid, _) in &solids {
                let Some(source) = model.sources.get(name) else {
                    continue;
                };
                let profile = source.file.display().to_string();
                if !MEMBER_KINDS.iter().any(|k| profile.starts_with(k)) {
                    continue;
                }
                let Some(length) = source
                    .body
                    .split_whitespace()
                    .next()
                    .and_then(|w| w.parse::<f64>().ok())
                else {
                    continue;
                };
                let b = geometry::bounds(solid);
                let mut e = (
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                );
                for i in 0..8 {
                    let pick = |bit: usize, lo: f64, hi: f64| if i & bit == 0 { lo } else { hi };
                    let p = frame.paper(frame.uv(Point3::new(
                        pick(1, b.min().x, b.max().x),
                        pick(2, b.min().y, b.max().y),
                        pick(4, b.min().z, b.max().z),
                    )));
                    e = (e.0.min(p.0), e.1.min(p.1), e.2.max(p.0), e.3.max(p.1));
                }
                let (w, h) = (e.2 - e.0, e.3 - e.1);
                let words = number(length.round());
                if w.max(h) < text_width(&words, 1.8) + 2.0 {
                    continue;
                }
                let vertical = h > w;
                let centre = ((e.0 + e.2) / 2.0, (e.1 + e.3) / 2.0);
                let long = text_width(&words, 1.8);
                let room = if vertical { h } else { w };
                let free = [0.0, 0.3, -0.3, 0.15, -0.15, 0.4, -0.4]
                    .iter()
                    .map(|f| f * (room - long).max(0.0))
                    .map(|shift| {
                        if vertical {
                            (centre.0, centre.1 + shift)
                        } else {
                            (centre.0 + shift, centre.1)
                        }
                    })
                    .find(|&(x, y)| {
                        let r = if vertical {
                            (x - 1.0, y - long / 2.0, x + 1.0, y + long / 2.0)
                        } else {
                            (x - long / 2.0, y - 1.0, x + long / 2.0, y + 1.0)
                        };
                        taken.iter().all(|t: &(f64, f64, f64, f64)| {
                            r.2 < t.0 || r.0 > t.2 || r.3 < t.1 || r.1 > t.3
                        })
                    });
                let Some((x, y)) = free else {
                    continue;
                };
                taken.push(if vertical {
                    (x - 1.0, y - long / 2.0, x + 1.0, y + long / 2.0)
                } else {
                    (x - long / 2.0, y - 1.0, x + long / 2.0, y + 1.0)
                });
                marks.push(Mark::Text {
                    x: x + if vertical { 0.6 } else { 0.0 },
                    y: y + if vertical { 0.0 } else { 0.6 },
                    size: 1.8,
                    text: words,
                    anchor: Anchor::Middle,
                    angle: if vertical { -90.0 } else { 0.0 },
                });
            }
        }
        let geometry_marks = marks.len();
        let label = named(line, "label")
            .map(|t| t.trim_matches('"').to_string())
            .or_else(|| line.label.clone())
            .unwrap_or_else(|| {
                if cut {
                    "section".into()
                } else {
                    format!("view from {from}")
                }
            });
        let (bx0, by0, _, _) = frame.box_paper();
        marks.push(Mark::Text {
            x: bx0,
            y: by0 - 4.0,
            size: 3.5,
            text: format!("{label}  {}", scale_text(scale)),
            anchor: Anchor::Start,
            angle: 0.0,
        });
        let sheet_index = self.drawing.sheets.len().saturating_sub(1);
        let start = self.sheet()?.marks.len();
        self.view_geometry = Some((sheet_index, start, start + geometry_marks));
        self.view_box = Some(frame.box_paper());
        self.sheet()?.marks.extend(marks);
        self.frame = Some(frame);
        Ok(format!(
            "{} parts, {} x {} mm on the sheet",
            names.len(),
            number(w),
            number(h)
        ))
    }

    fn body_extents(&self, names: &[String]) -> Result<Vec<(f64, f64, f64, f64)>> {
        let frame = self.frame()?;
        let model = self.model()?;
        Ok(model
            .named_solids()
            .into_iter()
            .filter(|(n, _)| names.contains(n))
            .map(|(_, s)| {
                let b = geometry::bounds(s);
                let mut e = (
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                );
                for i in 0..8 {
                    let pick = |bit: usize, lo: f64, hi: f64| if i & bit == 0 { lo } else { hi };
                    let (u, v) = frame.uv(Point3::new(
                        pick(1, b.min().x, b.max().x),
                        pick(2, b.min().y, b.max().y),
                        pick(4, b.min().z, b.max().z),
                    ));
                    e = (e.0.min(u), e.1.min(v), e.2.max(u), e.3.max(v));
                }
                e
            })
            .collect())
    }

    fn op_dims(&mut self, line: &Line) -> Result<String> {
        let names = self.picked_by(line)?;
        let along = named(line, "along").unwrap_or("h");
        let horizontal = match along {
            "h" | "x" => true,
            "v" | "y" => false,
            other => bail!("`along` is h or v, not `{other}`"),
        };
        let of = named(line, "of").unwrap_or("centres");
        let row = named(line, "row")
            .map(|t| eval(t, &self.scope))
            .transpose()?
            .unwrap_or(0.0);
        let mut extents = self.body_extents(&names)?;
        if named(line, "across").is_some_and(|t| t != "0") {
            extents.retain(|e| {
                let (along_size, other) = if horizontal {
                    (e.2 - e.0, e.3 - e.1)
                } else {
                    (e.3 - e.1, e.2 - e.0)
                };
                along_size < other
            });
        }
        let frame = self.frame()?.clone();
        let mut stations: Vec<f64> = Vec::new();
        for (u0, v0, u1, v1) in &extents {
            let (lo, hi) = if horizontal { (*u0, *u1) } else { (*v0, *v1) };
            let lo = lo.max(if horizontal {
                frame.window.0
            } else {
                frame.window.1
            });
            let hi = hi.min(if horizontal {
                frame.window.2
            } else {
                frame.window.3
            });
            if hi < lo {
                continue;
            }
            match of {
                "centres" | "centers" => stations.push((lo + hi) / 2.0),
                "edges" => stations.extend([lo, hi]),
                "ends" => stations.extend([lo, hi]),
                other => bail!("`of` is centres, edges or ends, not `{other}`"),
            }
        }
        if named(line, "ends").is_some_and(|t| t != "0") {
            let lo = extents
                .iter()
                .map(|e| if horizontal { e.0 } else { e.1 })
                .fold(f64::INFINITY, f64::min);
            let hi = extents
                .iter()
                .map(|e| if horizontal { e.2 } else { e.3 })
                .fold(f64::NEG_INFINITY, f64::max);
            stations.extend([lo, hi]);
        }
        if of == "ends" && !stations.is_empty() {
            let lo = stations.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = stations.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            stations = vec![lo, hi];
        }
        if let Some(extra) = named(line, "from") {
            for word in extra.split(',') {
                stations.push(eval(word, &self.scope)?);
            }
        }
        stations.sort_by(f64::total_cmp);
        stations.dedup_by(|a, b| (*a - *b).abs() < 0.5);
        if stations.len() < 2 {
            bail!("the parts give fewer than two places to dimension between");
        }
        let style = named(line, "style").unwrap_or("chain");
        if style == "ordinate" {
            let (bx0, _, _, by1) = frame.box_paper();
            let gap = 6.0 + row * 7.0;
            let first = stations[0];
            let mut marks = Vec::new();
            let mut lines = Vec::new();
            let ends: Vec<(f64, f64)> = stations
                .iter()
                .map(|&st| {
                    if horizontal {
                        (frame.paper((st, frame.window.1)).0, by1)
                    } else {
                        (bx0, frame.paper((frame.window.0, st)).1)
                    }
                })
                .collect();
            let pitch = TEXT * 1.15;
            let mut slots: Vec<f64> = ends
                .iter()
                .map(|&(x, y)| if horizontal { x } else { y })
                .collect();
            for _ in 0..50 {
                let mut moved = false;
                for i in 1..slots.len() {
                    let (lo, hi) = if horizontal { (i - 1, i) } else { (i, i - 1) };
                    let overlap = pitch - (slots[hi] - slots[lo]);
                    if overlap > 1.0e-6 {
                        slots[lo] -= overlap / 2.0;
                        slots[hi] += overlap / 2.0;
                        moved = true;
                    }
                }
                if !moved {
                    break;
                }
            }
            for ((&st, &(x, y)), &slot) in stations.iter().zip(&ends).zip(&slots) {
                let words = number((st - first).round());
                if horizontal {
                    lines.push(vec![
                        (x, y + 1.0),
                        (x, y + gap),
                        (slot, y + gap + 2.0),
                        (slot, y + gap + 2.5),
                    ]);
                    marks.push(Mark::Text {
                        x: slot + 0.85,
                        y: y + gap + 3.0,
                        size: TEXT,
                        text: words,
                        anchor: Anchor::End,
                        angle: -90.0,
                    });
                } else {
                    lines.push(vec![
                        (x - 1.0, y),
                        (x - gap, y),
                        (x - gap - 2.0, slot),
                        (x - gap - 2.5, slot),
                    ]);
                    marks.push(Mark::Text {
                        x: x - gap - 3.0,
                        y: slot + 0.85,
                        size: TEXT,
                        text: words,
                        anchor: Anchor::End,
                        angle: 0.0,
                    });
                }
            }
            let (a, b) = (ends[0], ends[ends.len() - 1]);
            lines.push(if horizontal {
                vec![(a.0, a.1 + gap), (b.0, b.1 + gap)]
            } else {
                vec![(a.0 - gap, a.1), (b.0 - gap, b.1)]
            });
            marks.push(Mark::Shape {
                loops: lines,
                closed: false,
                fill: None,
                stroke: Some(BLACK),
                width: 0.13,
                dashed: false,
            });
            let count = stations.len();
            self.sheet()?.marks.extend(marks);
            return Ok(format!("{count} setting-out marks"));
        }
        let running = style == "running";
        let (bx0, by0, bx1, by1) = frame.box_paper();
        let gap = 8.0 + row * 7.0;
        let mut marks = Vec::new();
        let first = stations[0];
        let mut stagger = false;
        let mut last_text_end = f64::NEG_INFINITY;
        for pair in stations.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let value = if running { b - first } else { b - a };
            let (pa, pb, offset) = if horizontal {
                let pa = frame.paper((a, frame.window.1));
                let pb = frame.paper((b, frame.window.1));
                ((pa.0, by1), (pb.0, by1), (0.0, gap))
            } else {
                let pa = frame.paper((frame.window.0, a));
                let pb = frame.paper((frame.window.0, b));
                ((bx0, pa.1), (bx0, pb.1), (-gap, 0.0))
            };
            let words = number(value.round());
            let width = words.len() as f64 * TEXT * 0.55;
            let span = if horizontal { pb.0 - pa.0 } else { pa.1 - pb.1 };
            let middle = if horizontal {
                (pa.0 + pb.0) / 2.0
            } else {
                (pa.1 + pb.1) / 2.0
            };
            stagger = if span < width + 1.0 && middle - width / 2.0 < last_text_end {
                !stagger
            } else {
                false
            };
            last_text_end = middle + width / 2.0;
            dimension(&mut marks, pa, pb, offset, &words, stagger);
        }
        if named(line, "total").is_some_and(|t| t != "0") {
            let (a, b) = (stations[0], stations[stations.len() - 1]);
            let (pa, pb, offset) = if horizontal {
                (
                    (frame.paper((a, 0.0)).0, by1),
                    (frame.paper((b, 0.0)).0, by1),
                    (0.0, gap + 7.0),
                )
            } else {
                (
                    (bx0, frame.paper((0.0, a)).1),
                    (bx0, frame.paper((0.0, b)).1),
                    (-gap - 7.0, 0.0),
                )
            };
            dimension(&mut marks, pa, pb, offset, &number((b - a).round()), false);
        }
        let _ = (by0, bx1);
        let count = stations.len();
        self.sheet()?.marks.extend(marks);
        Ok(format!("{} dimensions", count - 1))
    }

    fn op_dim(&mut self, line: &Line) -> Result<String> {
        let [a, b] = [0, 1].map(|i| line.positional.get(i).cloned());
        let (Some(a), Some(b)) = (a, b) else {
            bail!("write `dim x,y,z x,y,z offset=8`");
        };
        let (a, b) = (point3(&a, &self.scope)?, point3(&b, &self.scope)?);
        let frame = self.frame()?.clone();
        let (pa, pb) = (frame.paper(frame.uv(a)), frame.paper(frame.uv(b)));
        let offset = named(line, "offset")
            .map(|t| eval(t, &self.scope))
            .transpose()?
            .unwrap_or(8.0);
        let horizontal = (pa.0 - pb.0).abs() >= (pa.1 - pb.1).abs();
        let (pa2, pb2, off, value) = if horizontal {
            let y = pa.1.max(pb.1);
            (
                (pa.0, y),
                (pb.0, y),
                (0.0, offset),
                (pb.0 - pa.0).abs() * frame.scale,
            )
        } else {
            let x = pa.0.min(pb.0);
            (
                (x, pa.1),
                (x, pb.1),
                (-offset, 0.0),
                (pb.1 - pa.1).abs() * frame.scale,
            )
        };
        let words = named(line, "text")
            .map(|t| t.trim_matches('"').to_string())
            .unwrap_or_else(|| number((value * 10.0).round() / 10.0));
        let mut marks = Vec::new();
        dimension(&mut marks, pa2, pb2, off, &words, false);
        self.sheet()?.marks.extend(marks);
        Ok(format!("{words} mm"))
    }

    fn op_level(&mut self, line: &Line) -> Result<String> {
        let z = eval(
            line.positional
                .first()
                .ok_or_else(|| anyhow!("write `level height \"name\"`"))?,
            &self.scope,
        )?;
        let name = text_of(line, 1).unwrap_or_default();
        let frame = self.frame()?.clone();
        if !frame.up_is_z {
            bail!("levels go on an elevation or section, which looks along x or y");
        }
        let (_, _, bx1, _) = frame.box_paper();
        let y = frame.paper((frame.window.0, z)).1;
        let x = bx1 + 4.0;
        let mut marks = vec![
            Mark::Shape {
                loops: vec![vec![(bx1 + 1.0, y), (x + 16.0, y)]],
                closed: false,
                fill: None,
                stroke: Some(BLACK),
                width: 0.18,
                dashed: true,
            },
            Mark::Shape {
                loops: vec![vec![(x, y), (x - 1.6, y - 2.4), (x + 1.6, y - 2.4)]],
                closed: true,
                fill: Some(BLACK),
                stroke: None,
                width: 0.0,
                dashed: false,
            },
        ];
        marks.push(Mark::Text {
            x: x + 3.0,
            y: y - 1.0,
            size: TEXT,
            text: format!(
                "{}{} {name}",
                if z >= 0.0 { "+" } else { "" },
                number(z.round())
            ),
            anchor: Anchor::Start,
            angle: 0.0,
        });
        self.sheet()?.marks.extend(marks);
        Ok(format!("level {}", number(z)))
    }

    fn op_callout(&mut self, line: &Line) -> Result<String> {
        if named(line, "to").is_none() {
            return self.op_label(line);
        }
        let at = line
            .positional
            .first()
            .ok_or_else(|| anyhow!("write `callout x,y,z \"text\" to=dx,dy`"))?;
        let point = point3(at, &self.scope)?;
        let words = text_of(line, 1).ok_or_else(|| anyhow!("a callout needs its text"))?;
        let (dx, dy) = named(line, "to")
            .map(|t| pair(t, &self.scope))
            .transpose()?
            .unwrap_or((20.0, 10.0));
        let frame = self.frame()?.clone();
        let from = frame.paper(frame.uv(point));
        let to = (from.0 + dx, from.1 - dy);
        let width = text_width(&words, TEXT);
        let (shelf, anchor) = if dx >= 0.0 {
            (to.0 + width + 1.0, Anchor::Start)
        } else {
            (to.0 - width - 1.0, Anchor::End)
        };
        let sheet = self
            .drawing
            .sheets
            .last()
            .ok_or_else(|| anyhow!("start a `sheet` first"))?;
        let (lo, hi) = (to.0.min(shelf), to.0.max(shelf));
        if lo < MARGIN
            || hi > sheet.width - MARGIN
            || to.1 - TEXT < MARGIN
            || to.1 > sheet.height - MARGIN
        {
            bail!("the callout text runs off the sheet; change `to=`");
        }
        let marks = vec![
            Mark::Shape {
                loops: vec![vec![from, to, (shelf, to.1)]],
                closed: false,
                fill: None,
                stroke: Some(BLACK),
                width: 0.13,
                dashed: false,
            },
            Mark::Shape {
                loops: vec![
                    (0..8)
                        .map(|i| {
                            let a = i as f64 * std::f64::consts::PI / 4.0;
                            (from.0 + 0.5 * a.cos(), from.1 + 0.5 * a.sin())
                        })
                        .collect(),
                ],
                closed: true,
                fill: Some(BLACK),
                stroke: None,
                width: 0.0,
                dashed: false,
            },
            Mark::Text {
                x: to.0 + if dx >= 0.0 { 0.5 } else { -0.5 },
                y: to.1 - 0.8,
                size: TEXT,
                text: words.clone(),
                anchor,
                angle: 0.0,
            },
        ];
        self.sheet()?.marks.extend(marks);
        Ok(words)
    }

    fn groups_of(&self, part: &str) -> Result<crate::select::Groups> {
        let model = self.model()?;
        Ok(match model.part_groups.get(part) {
            Some(groups) => groups.placed(),
            None => crate::select::Groups(
                model
                    .groups
                    .0
                    .iter()
                    .filter(|g| g.body == part)
                    .cloned()
                    .collect(),
            ),
        })
    }

    fn anchors_of(&self, target: &str) -> Result<(Vec<annotate::Anchor2>, Vec<(f64, String)>)> {
        let frame = self.frame()?.clone();
        let spot = |p: Point3, radius: f64| annotate::Anchor2 {
            at: frame.paper(frame.uv(p)),
            radius,
        };
        if !target.contains(':') {
            return Ok((vec![spot(point3(target, &self.scope)?, 0.0)], Vec::new()));
        }
        let model = self.model()?;
        let (part, selector) = target.split_once(':').unwrap_or((target, ""));
        let solid = model.named_body(part)?;
        let groups = self.groups_of(part)?;
        let tolerance = (geometry::bounds(&solid).diameter() * 1.0e-6).max(1.0e-7);
        if selector.contains('&') || selector.contains('|') {
            let edges = crate::select::select_edges(selector, &solid, &groups, tolerance)?;
            if edges.is_empty() {
                bail!("`{target}` matched no edges");
            }
            let anchors = edges
                .iter()
                .map(|edge| {
                    let curve = edge.curve();
                    let (t0, t1) = curve.range_tuple();
                    spot(curve.subs((t0 + t1) / 2.0), 0.0)
                })
                .collect();
            return Ok((anchors, Vec::new()));
        }
        let picked = crate::select::select_faces(selector, &solid, &groups, tolerance)?;
        if picked.is_empty() {
            bail!("`{target}` matched no faces");
        }
        let round = crate::model::mate::cylinders(&solid, &picked);
        let faces = crate::select::faces(&solid);
        let face_spots: Vec<annotate::Anchor2> = picked
            .iter()
            .filter_map(|&i| faces.get(i))
            .map(|face| {
                let samples = crate::select::sample_points(face);
                let n = samples.len().max(1) as f64;
                spot(
                    Point3::from_vec(samples.iter().map(|p| p.to_vec()).sum::<Vector3>() / n),
                    0.0,
                )
            })
            .collect();
        if !round.is_empty() {
            let toward = -frame.right.cross(frame.up);
            let (bx0, by0, bx1, by1) = frame.box_paper();
            let inside = |a: &annotate::Anchor2| {
                a.at.0 > bx0 && a.at.0 < bx1 && a.at.1 > by0 && a.at.1 < by1
            };
            let rims: Vec<annotate::Anchor2> = round
                .iter()
                .filter(|c| c.axis.dot(toward).abs() > 0.999)
                .map(|c| spot(c.point, c.radius / frame.scale))
                .collect();
            let anchors = if rims.len() == round.len() && rims.iter().all(inside) {
                rims
            } else {
                face_spots
            };
            let sizes = round
                .iter()
                .map(|c| (c.radius * 2.0, self.hole_note(c.radius * 2.0)))
                .collect();
            return Ok((anchors, sizes));
        }
        let anchors = face_spots;
        Ok((anchors, Vec::new()))
    }

    fn hole_note(&self, diameter: f64) -> String {
        self.model
            .as_ref()
            .and_then(|m| {
                m.hole_notes
                    .iter()
                    .find(|n| (n.diameter - diameter).abs() < 1.0e-3)
            })
            .map(|n| n.note.clone())
            .unwrap_or_default()
    }

    fn op_label(&mut self, line: &Line) -> Result<String> {
        let frame = self.frame()?.clone();
        let is_target = |t: &String| match t.split_once(':') {
            Some((part, _)) => self
                .model
                .as_ref()
                .is_some_and(|m| m.named_solids().iter().any(|(n, _)| n == part)),
            None => point3(t, &self.scope).is_ok(),
        };
        let quoted: Vec<&String> = line.positional.iter().filter(|t| !is_target(t)).collect();
        let targets: Vec<&String> = line.positional.iter().filter(|t| is_target(t)).collect();
        if targets.is_empty() {
            bail!("write `label \"text\" part:faces`, `label \"text\" a:b&c` for an edge or `label \"text\" x,y,z`");
        }
        let mut found_anchors: Vec<annotate::Anchor2> = Vec::new();
        let mut diameters: Vec<(f64, String)> = Vec::new();
        for target in &targets {
            let (found, round) = self.anchors_of(target)?;
            found_anchors.extend(found);
            diameters.extend(round);
        }
        let text = match quoted.first() {
            Some(t) => t.trim_matches('"').to_string(),
            None => {
                if diameters.is_empty() {
                    bail!("give the label its text, `label \"text\" {}`", targets[0]);
                }
                let mut groups: Vec<(f64, String, usize)> = Vec::new();
                for (d, note) in &diameters {
                    match groups
                        .iter_mut()
                        .find(|(known, k, _)| (known - d).abs() < 1.0e-3 && k == note)
                    {
                        Some((_, _, n)) => *n += 1,
                        None => groups.push((*d, note.clone(), 1)),
                    }
                }
                groups
                    .iter()
                    .map(|(d, note, n)| {
                        let size = number((d * 100.0).round() / 100.0);
                        let body = if note.ends_with("tapped") {
                            format!("{note} (⌀{size} drill)")
                        } else if note.is_empty() {
                            format!("⌀{size}")
                        } else {
                            format!("⌀{size} {note}")
                        };
                        if *n > 1 {
                            format!("{n}× {body}")
                        } else {
                            body
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        };
        let (bx0, by0, bx1, by1) = frame.box_paper();
        let mut anchors: Vec<annotate::Anchor2> = Vec::new();
        for a in found_anchors {
            if a.at.0 < bx0 - 0.5 || a.at.0 > bx1 + 0.5 || a.at.1 < by0 - 0.5 || a.at.1 > by1 + 0.5
            {
                continue;
            }
            if anchors
                .iter()
                .all(|k| (k.at.0 - a.at.0).hypot(k.at.1 - a.at.1) > 1.0)
            {
                anchors.push(a);
            }
        }
        if anchors.is_empty() {
            bail!("`{}` is outside the last view", targets[0]);
        }
        let side = named(line, "side").map(annotate::parse_side).transpose()?;
        let label = annotate::Label {
            text: text.clone(),
            anchors,
            side,
            single: targets.len() == 1,
        };
        let view = self
            .view_box
            .ok_or_else(|| anyhow!("draw a `view` or `section` first"))?;
        let own = self
            .view_geometry
            .map(|(_, a, b)| a..b)
            .unwrap_or(0..0);
        match self.pending.last_mut() {
            Some(last) if last.view == view && last.own == own => last.labels.push(label),
            _ => self.pending.push(PendingView {
                view,
                own,
                labels: vec![label],
            }),
        }
        self.label_line = Some(line.clone());
        Ok(text)
    }

    fn flush_labels(&mut self) -> Result<()> {
        let pending = std::mem::take(&mut self.pending);
        if pending.is_empty() {
            return Ok(());
        }
        let sheet = self
            .drawing
            .sheets
            .last()
            .ok_or_else(|| anyhow!("start a `sheet` first"))?;
        let mut added: Vec<Mark> = Vec::new();
        for group in &pending {
            let rects = |marks: &[Mark], skip: Option<&std::ops::Range<usize>>| {
                marks
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| skip.is_none_or(|r| !r.contains(i)))
                    .map(|(_, m)| m.clone())
                    .collect::<Vec<Mark>>()
            };
            let mut all = rects(&sheet.marks, Some(&group.own));
            all.extend(added.iter().cloned());
            let texts: Vec<annotate::Rect> = all
                .iter()
                .filter(|m| matches!(m, Mark::Text { .. }))
                .flat_map(annotate::mark_rects)
                .collect();
            let blocks: Vec<annotate::Rect> = all
                .iter()
                .filter(|m| matches!(m, Mark::Shape { fill: Some(_), .. }))
                .flat_map(annotate::mark_rects)
                .collect();
            let mut obstacles: Vec<annotate::Rect> =
                all.iter().flat_map(annotate::mark_rects).collect();
            obstacles.extend(
                sheet
                    .marks
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| group.own.contains(i))
                    .filter(|(_, m)| matches!(m, Mark::Text { .. }))
                    .flat_map(|(_, m)| annotate::mark_rects(m)),
            );
            obstacles.push((
                sheet.width - MARGIN - 170.0,
                sheet.height - MARGIN - TITLE_H,
                sheet.width - MARGIN,
                sheet.height - MARGIN,
            ));
            let marks = annotate::layout(
                group.view,
                (sheet.width, sheet.height, MARGIN),
                &obstacles,
                &texts,
                &blocks,
                &group.labels,
            )?;
            added.extend(marks);
        }
        self.sheet()?.marks.extend(added);
        Ok(())
    }

    fn op_note(&mut self, line: &Line) -> Result<String> {
        let words = text_of(line, 0).ok_or_else(|| anyhow!("write `note \"text\" place=x,y`"))?;
        let size = named(line, "size")
            .map(|t| eval(t, &self.scope))
            .transpose()?
            .unwrap_or(TEXT);
        let lines: Vec<&str> = words.split('|').collect();
        let height = lines.len() as f64 * size * 1.5;
        let width = lines.iter().map(|l| l.len()).max().unwrap_or(0) as f64 * size * 0.52;
        let at = named(line, "place")
            .map(|t| pair(t, &self.scope))
            .transpose()?;
        let (x, y) = self.place(width, height, at)?;
        let top = y - height;
        let spot = (x - 1.0, top - 1.0, x + width + 1.0, y + 1.0);
        let clash = self.sheet()?.marks.iter().any(|m| match m {
            Mark::Shape { fill: Some(_), .. } => annotate::mark_rects(m)
                .iter()
                .any(|r| annotate::overlaps(*r, spot)),
            _ => false,
        });
        if clash {
            bail!(
                "the note would sit on a view; move it with `place=x,y` (it is {} x {} mm)",
                number(width),
                number(height)
            );
        }
        let marks: Vec<Mark> = lines
            .iter()
            .enumerate()
            .map(|(i, l)| Mark::Text {
                x,
                y: top + (i as f64 + 1.0) * size * 1.5,
                size,
                text: l.trim().to_string(),
                anchor: Anchor::Start,
                angle: 0.0,
            })
            .collect();
        self.sheet()?.marks.extend(marks);
        Ok(format!("{} lines", lines.len()))
    }

    fn op_nest(&mut self, line: &Line) -> Result<String> {
        let names = self.picked_by(line)?;
        let model = self.model()?;
        let thickness = named(line, "thickness")
            .map(|t| eval(t, &self.scope))
            .transpose()?;
        let pieces = crate::nest::panels(model, &names, thickness)?;
        let (length, width) = crate::nest::stock_size(
            named(line, "stock")
                .ok_or_else(|| anyhow!("give the stock size, like `stock=2400x600`"))?,
        )?;
        let num = |key: &str, default: f64| -> Result<f64> {
            named(line, key).map_or(Ok(default), |t| eval(t, &self.scope))
        };
        let options = crate::nest::Options {
            length,
            width,
            gap: num("gap", 4.0)?,
            trim: num("trim", 0.0)?,
            grain: num("grain", 0.0)? != 0.0,
        };
        let layout = crate::nest::nest(pieces, options)?;
        let template = self
            .drawing
            .sheets
            .last()
            .cloned()
            .ok_or_else(|| anyhow!("start a `sheet` before drawing on it"))?;
        let scale = match named(line, "scale") {
            Some(t) => scale_of(t)?,
            None => {
                let room = template.width - 2.0 * MARGIN - 50.0;
                [5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 40.0, 50.0]
                    .into_iter()
                    .find(|s| length / s <= room)
                    .unwrap_or(50.0)
            }
        };
        let count = layout.sheets.len();
        self.sheet()?.scale = scale;
        for (s, stock) in layout.sheets.iter().enumerate() {
            if s > 0 {
                let mut sheet = Sheet {
                    name: format!("{} ({})", template.name, s + 1),
                    scale,
                    marks: Vec::new(),
                    ..template.clone()
                };
                rectangle(
                    &mut sheet,
                    MARGIN,
                    MARGIN,
                    template.width - MARGIN,
                    template.height - MARGIN,
                    0.5,
                );
                self.drawing.sheets.push(sheet);
                self.cursor = (MARGIN + 22.0, MARGIN + 14.0, 0.0);
            }
            self.nest_sheet(&layout, stock, s, count, scale)?;
        }
        self.frame = None;
        let used: f64 = layout.pieces.iter().map(|p| p.length * p.width).sum();
        Ok(format!(
            "{} pieces on {count} stock sheets of {} x {} at 1:{}, {:.0}% used",
            layout.pieces.len(),
            number(length),
            number(width),
            number(scale),
            100.0 * used / (length * width * count as f64)
        ))
    }

    fn nest_sheet(
        &mut self,
        layout: &crate::nest::Layout,
        stock: &crate::nest::Stock,
        index: usize,
        count: usize,
        scale: f64,
    ) -> Result<()> {
        let (w, h) = (layout.length / scale, layout.width / scale);
        let header = TEXT * 2.0;
        let (x, y) = self.place(w, h + header, None)?;
        let bottom = y;
        let to = |u: f64, v: f64| (x + u / scale, bottom - v / scale);
        let mut marks = Vec::new();
        marks.push(Mark::Text {
            x,
            y: bottom - h - TEXT * 0.8,
            size: TEXT,
            text: format!(
                "stock {} of {count}: {} x {} x {}, gap {}, 1:{}, {:.0}% used",
                index + 1,
                number(layout.length),
                number(layout.width),
                number(layout.pieces.first().map_or(0.0, |p| p.thickness)),
                number(layout.gap),
                number(scale),
                100.0 * stock.used() / (layout.length * layout.width)
            ),
            anchor: Anchor::Start,
            angle: 0.0,
        });
        let corner = |u0: f64, v0: f64, u1: f64, v1: f64| {
            vec![to(u0, v0), to(u1, v0), to(u1, v1), to(u0, v1)]
        };
        marks.push(Mark::Shape {
            loops: vec![corner(0.0, 0.0, layout.length, layout.width)],
            closed: true,
            fill: Some([0.93, 0.93, 0.93]),
            stroke: Some(BLACK),
            width: 0.35,
            dashed: false,
        });
        for p in &stock.placed {
            marks.push(Mark::Shape {
                loops: vec![corner(p.x, p.y, p.x + p.dx, p.y + p.dy)],
                closed: true,
                fill: Some([1.0, 1.0, 1.0]),
                stroke: Some(BLACK),
                width: 0.25,
                dashed: false,
            });
            let piece = &layout.pieces[p.piece];
            let (cx, cy) = to(p.x + p.dx / 2.0, p.y + p.dy / 2.0);
            let (pw, ph) = (p.dx / scale, p.dy / scale);
            let words = [
                format!("{} {}", p.piece + 1, piece.name),
                format!("{} x {}", number(p.dx), number(p.dy)),
            ];
            let size = TEXT.min(ph / 2.6);
            let fits = |t: &str| text_width(t, size) <= pw - 1.0;
            let lines: Vec<&String> = if size < 1.2 {
                Vec::new()
            } else if words.iter().all(|t| fits(t)) {
                words.iter().collect()
            } else if fits(&format!("{}", p.piece + 1)) {
                vec![&words[0]]
            } else {
                Vec::new()
            };
            let short = format!("{}", p.piece + 1);
            let lines: Vec<String> = if lines.len() == 1 && !fits(&words[0]) {
                vec![short]
            } else {
                lines.into_iter().cloned().collect()
            };
            let n = lines.len() as f64;
            for (k, text) in lines.into_iter().enumerate() {
                marks.push(Mark::Text {
                    x: cx,
                    y: cy + size * 0.35 + (k as f64 - (n - 1.0) / 2.0) * size * 1.3,
                    size,
                    text,
                    anchor: Anchor::Middle,
                    angle: 0.0,
                });
            }
        }
        for (c, cut) in stock.cuts.iter().enumerate() {
            let (a, b) = if cut.rip {
                (to(cut.from, cut.at), to(cut.to, cut.at))
            } else {
                (to(cut.at, cut.from), to(cut.at, cut.to))
            };
            marks.push(Mark::Shape {
                loops: vec![vec![a, b]],
                closed: false,
                fill: None,
                stroke: Some([0.8, 0.1, 0.1]),
                width: 0.3,
                dashed: true,
            });
            let (tx, ty) = if cut.rip {
                (a.0 - 1.2, a.1 + TEXT * 0.35)
            } else {
                (b.0, b.1 - 1.0)
            };
            marks.push(Mark::Text {
                x: tx,
                y: ty,
                size: TEXT * 0.8,
                text: format!("{}", c + 1),
                anchor: if cut.rip { Anchor::End } else { Anchor::Middle },
                angle: 0.0,
            });
        }
        self.sheet()?.marks.extend(marks);
        let rows: Vec<String> = stock
            .cuts
            .iter()
            .enumerate()
            .map(|(c, cut)| format!("{:>2}  {}", c + 1, crate::nest::cut_text(cut)))
            .collect();
        let size = TEXT * 0.85;
        let row_h = size * 1.45;
        let col_w = rows.iter().map(|r| text_width(r, size)).fold(0.0, f64::max) + 4.0;
        let sheet = self.drawing.sheets.last().expect("a sheet");
        let (sw, sh) = (sheet.width, sheet.height);
        let top = bottom + 8.0;
        let per_col = (((sh - MARGIN - TITLE_H - 4.0) - top) / row_h)
            .floor()
            .max(1.0) as usize;
        let mut marks = Vec::new();
        marks.push(Mark::Text {
            x,
            y: top,
            size,
            text: "cuts in order (rip: along the length at y; cross: across at x; mm from the stock corner)"
                .into(),
            anchor: Anchor::Start,
            angle: 0.0,
        });
        for (k, row) in rows.iter().enumerate() {
            let col = k / per_col;
            let cx = x + col as f64 * col_w;
            if cx + col_w > sw - MARGIN {
                bail!(
                    "the cut list does not fit under the stock; use a bigger sheet or smaller scale"
                );
            }
            marks.push(Mark::Text {
                x: cx,
                y: top + ((k % per_col) as f64 + 1.0) * row_h,
                size,
                text: row.clone(),
                anchor: Anchor::Start,
                angle: 0.0,
            });
        }
        self.sheet()?.marks.extend(marks);
        self.cursor = (sw, sh, 0.0);
        Ok(())
    }

    fn op_cutlist(&mut self, line: &Line) -> Result<String> {
        let patterns = line.positional.clone();
        let names = self.picked_by(line)?;
        let model = self.model()?;
        let mut rows: BTreeMap<(String, String), usize> = BTreeMap::new();
        for name in &names {
            let Some(source) = model.sources.get(name) else {
                continue;
            };
            let profile = source.file.display().to_string();
            if !MEMBER_KINDS.iter().any(|k| profile.starts_with(k)) {
                continue;
            }
            let mut words = profile.splitn(2, ' ');
            let kind = words.next().unwrap_or("");
            let mut size: Vec<String> = words
                .next()
                .unwrap_or("")
                .split('x')
                .map(str::to_string)
                .collect();
            if kind == "bar" && size.len() == 2 {
                size.sort_by(|a, b| {
                    a.parse::<f64>()
                        .unwrap_or(0.0)
                        .total_cmp(&b.parse::<f64>().unwrap_or(0.0))
                });
            }
            let section = if kind == "bar" {
                size.join("x")
            } else {
                format!("{kind} {}", size.join("x"))
            };
            let length = source
                .body
                .split_whitespace()
                .next()
                .and_then(|w| w.parse::<f64>().ok())
                .map_or(source.body.clone(), |l| number(l.round()));
            let cuts = source.body.split(", ").nth(1).unwrap_or("");
            let ends = if cuts.is_empty() || cuts == "square ends" || cuts == "cut 0.0/0.0" {
                String::new()
            } else {
                format!(", {cuts}")
            };
            *rows
                .entry((section, format!("{length}{ends}")))
                .or_default() += 1;
        }
        if rows.is_empty() {
            bail!("none of {} are structural members", patterns.join(" "));
        }
        let summary = named(line, "summary").is_some_and(|t| t != "0");
        let mut table: Vec<[String; 3]> = if summary {
            vec![[
                "pieces".into(),
                "section".into(),
                "total m, longest mm".into(),
            ]]
        } else {
            vec![["qty".into(), "section".into(), "length mm".into()]]
        };
        let mut sorted: Vec<((String, String), usize)> = rows.into_iter().collect();
        sorted.sort_by(|a, b| {
            a.0.0.cmp(&b.0.0).then_with(|| {
                let len = |t: &str| {
                    t.split_whitespace()
                        .next()
                        .and_then(|w| w.parse::<f64>().ok())
                        .unwrap_or(0.0)
                };
                len(&b.0.1).total_cmp(&len(&a.0.1))
            })
        });
        if summary {
            let mut totals: BTreeMap<String, (usize, f64, f64)> = BTreeMap::new();
            for ((profile, length), qty) in &sorted {
                let l: f64 = length
                    .split(',')
                    .next()
                    .and_then(|w| w.trim().parse().ok())
                    .unwrap_or(0.0);
                let entry = totals.entry(profile.clone()).or_default();
                entry.0 += qty;
                entry.1 += l * *qty as f64;
                entry.2 = entry.2.max(l);
            }
            for (profile, (pieces, total, longest)) in totals {
                table.push([
                    pieces.to_string(),
                    profile,
                    format!("{:.1} m, {}", total / 1000.0, number(longest)),
                ]);
            }
        } else {
            for ((profile, length), qty) in &sorted {
                table.push([qty.to_string(), profile.clone(), length.clone()]);
            }
        }
        let widths = [10.0, 20.0, 34.0];
        let row_h = TEXT * 1.7;
        let height = table.len() as f64 * row_h;
        let at = named(line, "place")
            .map(|t| pair(t, &self.scope))
            .transpose()?;
        let (x, y) = self.place(widths.iter().sum(), height, at)?;
        let top = y - height;
        let mut marks = Vec::new();
        for (r, cells) in table.iter().enumerate() {
            let yy = top + r as f64 * row_h;
            marks.push(Mark::Shape {
                loops: vec![vec![(x, yy), (x + widths.iter().sum::<f64>(), yy)]],
                closed: false,
                fill: None,
                stroke: Some(BLACK),
                width: if r <= 1 { 0.25 } else { 0.1 },
                dashed: false,
            });
            let mut cx = x;
            for (k, cell) in cells.iter().enumerate() {
                marks.push(Mark::Text {
                    x: cx + 1.2,
                    y: yy + row_h * 0.72,
                    size: TEXT,
                    text: cell.clone(),
                    anchor: Anchor::Start,
                    angle: 0.0,
                });
                cx += widths[k];
            }
        }
        marks.push(Mark::Shape {
            loops: vec![vec![
                (x, top),
                (x + widths.iter().sum::<f64>(), top),
                (x + widths.iter().sum::<f64>(), y),
                (x, y),
            ]],
            closed: true,
            fill: None,
            stroke: Some(BLACK),
            width: 0.25,
            dashed: false,
        });
        let count: usize = sorted.iter().map(|(_, q)| q).sum();
        self.sheet()?.marks.extend(marks);
        Ok(format!("{count} members in {} rows", table.len() - 1))
    }
}

fn scale_text(scale: f64) -> String {
    if scale < 1.0 {
        format!("{}:1", number(1.0 / scale))
    } else {
        format!("1:{}", number(scale))
    }
}

fn number(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn rectangle(sheet: &mut Sheet, x0: f64, y0: f64, x1: f64, y1: f64, width: f64) {
    sheet.marks.push(Mark::Shape {
        loops: vec![vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]],
        closed: true,
        fill: None,
        stroke: Some(BLACK),
        width,
        dashed: false,
    });
}

fn dimension(
    marks: &mut Vec<Mark>,
    a: (f64, f64),
    b: (f64, f64),
    offset: (f64, f64),
    words: &str,
    stagger: bool,
) {
    let (pa, pb) = (
        (a.0 + offset.0, a.1 + offset.1),
        (b.0 + offset.0, b.1 + offset.1),
    );
    let length = offset.0.hypot(offset.1).max(1.0e-9);
    let (ox, oy) = (offset.0 / length, offset.1 / length);
    let over = 1.5;
    let start = 1.0;
    marks.push(Mark::Shape {
        loops: vec![
            vec![
                (a.0 + ox * start, a.1 + oy * start),
                (pa.0 + ox * over, pa.1 + oy * over),
            ],
            vec![
                (b.0 + ox * start, b.1 + oy * start),
                (pb.0 + ox * over, pb.1 + oy * over),
            ],
            vec![pa, pb],
            vec![(pa.0 - 1.0, pa.1 + 1.0), (pa.0 + 1.0, pa.1 - 1.0)],
            vec![(pb.0 - 1.0, pb.1 + 1.0), (pb.0 + 1.0, pb.1 - 1.0)],
        ],
        closed: false,
        fill: None,
        stroke: Some(BLACK),
        width: 0.13,
        dashed: false,
    });
    let vertical = (pa.0 - pb.0).abs() < 1.0e-9;
    let middle = ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0);
    let lift = if stagger { TEXT * 1.3 } else { 0.0 };
    let (x, y, angle) = if vertical {
        (middle.0 - 0.8 - lift, middle.1, -90.0)
    } else {
        (middle.0, middle.1 - 0.8 - lift, 0.0)
    };
    marks.push(Mark::Text {
        x,
        y,
        size: TEXT,
        text: words.to_string(),
        anchor: Anchor::Middle,
        angle,
    });
}

fn title_block(sheet: &mut Sheet, title: &str, file: &str, index: usize, count: usize) {
    let (w, h) = (sheet.width, sheet.height);
    let (x0, x1, y1) = (w - MARGIN - 170.0, w - MARGIN, h - MARGIN);
    let y0 = y1 - TITLE_H;
    rectangle(sheet, x0, y0, x1, y1, 0.4);
    let lines = [
        (x0, y0 + 12.0, x1, y0 + 12.0),
        (x0 + 110.0, y0 + 12.0, x0 + 110.0, y1),
        (x0 + 140.0, y0 + 12.0, x0 + 140.0, y1),
    ];
    for (a, b, c, d) in lines {
        sheet.marks.push(Mark::Shape {
            loops: vec![vec![(a, b), (c, d)]],
            closed: false,
            fill: None,
            stroke: Some(BLACK),
            width: 0.25,
            dashed: false,
        });
    }
    let text = |x: f64, y: f64, size: f64, words: String| Mark::Text {
        x,
        y,
        size,
        text: words,
        anchor: Anchor::Start,
        angle: 0.0,
    };
    let date = today();
    sheet.marks.extend([
        text(x0 + 2.0, y0 + 5.5, 4.0, title.to_string()),
        text(x0 + 2.0, y0 + 10.5, 3.0, sheet.name.clone()),
        text(x0 + 2.0, y1 - 9.0, 2.2, format!("model {file}")),
        text(
            x0 + 2.0,
            y1 - 4.0,
            2.2,
            format!("drawn {date}, all sizes in mm"),
        ),
        text(x0 + 112.0, y1 - 9.0, 2.2, "scale".to_string()),
        text(
            x0 + 112.0,
            y1 - 4.0,
            3.0,
            scale_text(sheet.scale),
        ),
        text(x0 + 142.0, y1 - 9.0, 2.2, "sheet".to_string()),
        text(x0 + 142.0, y1 - 4.0, 3.0, format!("{index} of {count}")),
    ]);
}

fn clip_polygon(points: &[(f64, f64)], (x0, y0, x1, y1): (f64, f64, f64, f64)) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = points.to_vec();
    let edges: [(usize, f64, bool); 4] =
        [(0, x0, true), (0, x1, false), (1, y0, true), (1, y1, false)];
    for (axis, at, keep_above) in edges {
        if out.is_empty() {
            break;
        }
        let inside = |p: (f64, f64)| {
            let c = if axis == 0 { p.0 } else { p.1 };
            if keep_above { c >= at } else { c <= at }
        };
        let cross = |p: (f64, f64), q: (f64, f64)| {
            let (cp, cq) = if axis == 0 { (p.0, q.0) } else { (p.1, q.1) };
            let t = (at - cp) / (cq - cp);
            (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t)
        };
        let input = std::mem::take(&mut out);
        for i in 0..input.len() {
            let (p, q) = (input[i], input[(i + 1) % input.len()]);
            match (inside(p), inside(q)) {
                (true, true) => out.push(q),
                (true, false) => out.push(cross(p, q)),
                (false, true) => {
                    out.push(cross(p, q));
                    out.push(q);
                }
                (false, false) => {}
            }
        }
    }
    out
}

fn clip_line(
    points: &[(f64, f64)],
    (x0, y0, x1, y1): (f64, f64, f64, f64),
) -> Vec<Vec<(f64, f64)>> {
    let mut pieces = Vec::new();
    let mut current: Vec<(f64, f64)> = Vec::new();
    for pair in points.windows(2) {
        let (p, q) = (pair[0], pair[1]);
        let (dx, dy) = (q.0 - p.0, q.1 - p.1);
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        let mut visible = true;
        for (pp, qq) in [
            (-dx, p.0 - x0),
            (dx, x1 - p.0),
            (-dy, p.1 - y0),
            (dy, y1 - p.1),
        ] {
            if pp.abs() < 1.0e-12 {
                if qq < 0.0 {
                    visible = false;
                }
            } else {
                let r = qq / pp;
                if pp < 0.0 {
                    t0 = t0.max(r);
                } else {
                    t1 = t1.min(r);
                }
            }
        }
        if !visible || t0 > t1 {
            if current.len() > 1 {
                pieces.push(std::mem::take(&mut current));
            }
            current.clear();
            continue;
        }
        let a = (p.0 + dx * t0, p.1 + dy * t0);
        let b = (p.0 + dx * t1, p.1 + dy * t1);
        if current
            .last()
            .is_none_or(|l| (l.0 - a.0).abs() > 1.0e-9 || (l.1 - a.1).abs() > 1.0e-9)
        {
            if current.len() > 1 {
                pieces.push(std::mem::take(&mut current));
            }
            current = vec![a];
        }
        current.push(b);
        if t1 < 1.0 {
            pieces.push(std::mem::take(&mut current));
        }
    }
    if current.len() > 1 {
        pieces.push(current);
    }
    pieces
}

fn colour_hex(c: [f64; 3]) -> String {
    let [r, g, b] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn svg(sheet: &Sheet) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">",
        w = sheet.width,
        h = sheet.height
    );
    let _ = writeln!(out, "<rect width=\"100%\" height=\"100%\" fill=\"white\"/>");
    for mark in &sheet.marks {
        match mark {
            Mark::Shape {
                loops,
                closed,
                fill,
                stroke,
                width,
                dashed,
            } => {
                let mut d = String::new();
                for l in loops {
                    for (k, (x, y)) in l.iter().enumerate() {
                        let _ = write!(d, "{}{x:.2},{y:.2} ", if k == 0 { "M" } else { "L" });
                    }
                    if *closed {
                        d.push_str("Z ");
                    }
                }
                let _ = writeln!(
                    out,
                    "<path d=\"{}\" fill=\"{}\" fill-rule=\"evenodd\" stroke=\"{}\" stroke-width=\"{width:.2}\" stroke-linejoin=\"round\" stroke-linecap=\"round\"{}/>",
                    d.trim_end(),
                    fill.map_or("none".to_string(), colour_hex),
                    stroke.map_or("none".to_string(), colour_hex),
                    if *dashed {
                        " stroke-dasharray=\"2 1\""
                    } else {
                        ""
                    }
                );
            }
            Mark::Text {
                x,
                y,
                size,
                text,
                anchor,
                angle,
            } => {
                let anchor = match anchor {
                    Anchor::Start => "start",
                    Anchor::Middle => "middle",
                    Anchor::End => "end",
                };
                let turn = if *angle != 0.0 {
                    format!(" transform=\"rotate({angle} {x:.2} {y:.2})\"")
                } else {
                    String::new()
                };
                let _ = writeln!(
                    out,
                    "<text x=\"{x:.2}\" y=\"{y:.2}\" font-family=\"Helvetica, Arial, sans-serif\" font-size=\"{size:.2}\" text-anchor=\"{anchor}\"{turn}>{}</text>",
                    escape(text)
                );
            }
        }
    }
    out.push_str("</svg>\n");
    out
}

fn pdf_text(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| match c {
            c if (c as u32) < 128 => c as u8,
            '×' => 0xD7,
            '°' => 0xB0,
            '±' => 0xB1,
            '⌀' | 'Ø' => 0xD8,
            _ => b'?',
        })
        .collect()
}

pub(crate) fn text_width(text: &str, size: f64) -> f64 {
    text.chars()
        .map(|c| match c {
            'i' | 'j' | 'l' | '.' | ',' | ':' | ';' | '\'' | '|' | '!' | ' ' => 0.28,
            'm' | 'w' | 'M' | 'W' => 0.83,
            c if c.is_ascii_uppercase() => 0.67,
            c if c.is_ascii_digit() => 0.556,
            _ => 0.52,
        })
        .sum::<f64>()
        * size
}

pub fn pdf(sheets: &[Sheet]) -> Vec<u8> {
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, Stream};
    let pt = 72.0 / 25.4;
    let number = |v: f64| Object::Real(((v * 1000.0).round() / 1000.0) as f32);
    let operation = |name: &str, operands: Vec<Object>| Operation::new(name, operands);
    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    let font_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let mut kids = Vec::new();
    for sheet in sheets {
        let h = sheet.height;
        let y = |v: f64| (h - v) * pt;
        let mut operations: Vec<Operation> = Vec::new();
        for mark in &sheet.marks {
            match mark {
                Mark::Shape {
                    loops,
                    closed,
                    fill,
                    stroke,
                    width,
                    dashed,
                } => {
                    if let Some([r, g, b]) = fill {
                        operations.push(operation("rg", vec![number(*r), number(*g), number(*b)]));
                    }
                    if let Some([r, g, b]) = stroke {
                        operations.push(operation("RG", vec![number(*r), number(*g), number(*b)]));
                        operations.push(operation("w", vec![number(width * pt)]));
                    }
                    let pattern = if *dashed { vec![number(5.67), number(2.83)] } else { vec![] };
                    operations.push(operation("d", vec![Object::Array(pattern), number(0.0)]));
                    for l in loops {
                        for (k, (px, py)) in l.iter().enumerate() {
                            operations.push(operation(
                                if k == 0 { "m" } else { "l" },
                                vec![number(px * pt), number(y(*py))],
                            ));
                        }
                        if *closed {
                            operations.push(operation("h", vec![]));
                        }
                    }
                    operations.push(operation(
                        match (fill.is_some(), stroke.is_some()) {
                            (true, true) => "B*",
                            (true, false) => "f*",
                            (false, true) => "S",
                            (false, false) => "n",
                        },
                        vec![],
                    ));
                }
                Mark::Text {
                    x,
                    y: ty,
                    size,
                    text,
                    anchor,
                    angle,
                } => {
                    let width = text_width(text, *size);
                    let shift = match anchor {
                        Anchor::Start => 0.0,
                        Anchor::Middle => -width / 2.0,
                        Anchor::End => -width,
                    };
                    let a = angle.to_radians();
                    let (c, s) = (a.cos(), -a.sin());
                    let ox = x + shift * a.cos();
                    let oy = ty + shift * a.sin();
                    operations.push(operation(
                        "rg",
                        vec![number(0.0), number(0.0), number(0.0)],
                    ));
                    operations.push(operation("BT", vec![]));
                    operations.push(operation("Tf", vec!["F1".into(), number(size * pt)]));
                    operations.push(operation(
                        "Tm",
                        vec![
                            number(c),
                            number(s),
                            number(-s),
                            number(c),
                            number(ox * pt),
                            number(y(oy)),
                        ],
                    ));
                    operations.push(operation(
                        "Tj",
                        vec![Object::string_literal(pdf_text(text))],
                    ));
                    operations.push(operation("ET", vec![]));
                }
            }
        }
        let encoded = Content { operations }.encode().unwrap_or_default();
        let mut stream = Stream::new(dictionary! {}, encoded);
        let _ = stream.compress();
        let content_id = document.add_object(stream);
        let page_id = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![
                0.into(),
                0.into(),
                number(sheet.width * pt),
                number(sheet.height * pt),
            ],
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => font_id } },
            "Contents" => content_id,
        });
        kids.push(Object::Reference(page_id));
    }
    let count = kids.len() as i64;
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => count,
        }),
    );
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    document.trailer.set("Root", catalog_id);
    let mut out = Vec::new();
    let _ = document.save_to(&mut out);
    out
}

pub fn write(drawing: &Drawing, output: &str) -> Result<Vec<String>> {
    if drawing.sheets.is_empty() {
        bail!("the drawing has no sheets");
    }
    let lower = output.to_ascii_lowercase();
    if lower.ends_with(".pdf") {
        std::fs::write(output, pdf(&drawing.sheets))?;
        return Ok(vec![output.to_string()]);
    }
    if !lower.ends_with(".svg") {
        bail!("a drawing exports as .pdf or .svg");
    }
    if drawing.sheets.len() == 1 {
        std::fs::write(output, svg(&drawing.sheets[0]))?;
        return Ok(vec![output.to_string()]);
    }
    let stem = &output[..output.len() - 4];
    let mut written = Vec::new();
    for (i, sheet) in drawing.sheets.iter().enumerate() {
        let name = format!("{stem}-{}.svg", i + 1);
        std::fs::write(&name, svg(sheet))?;
        written.push(name);
    }
    Ok(written)
}

fn today() -> String {
    let Ok(since) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else {
        return String::new();
    };
    let days = (since.as_secs() / 86_400) as i64 + 719_468;
    let era = days.div_euclid(146_097);
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year}-{month:02}-{day:02}")
}
