use super::scene::{self, Highlight, Ink, Scene, V3};
use super::units::Units;
use crate::geometry;
use crate::host::Instant;
use crate::model::{
    Joint, JointKind, Rig, Snapshot, explode_offsets, label_of, posed, snapshots_path_kept,
};
use crate::parse::Line as SourceLine;
use crate::select;
use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use egui_bench::prelude::*;
use egui_bench::viewer3d::{
    self, BACKGROUND, Camera, Cut, Lighting, Look, Orbit, Painted, Placement, Preset, Projector,
    RenderMode, Viewer, dot, sub,
};
use monstertruck::modeling::{Matrix4, Solid, SquareMatrix};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
struct Program {
    lines: Vec<SourceLine>,
    snapshots: Vec<Snapshot>,
    error: Option<String>,
    files: Vec<PathBuf>,
}

fn evaluate(path: &std::path::Path, vars: &[(String, f64)]) -> Program {
    match snapshots_path_kept(path, vars) {
        Ok(run) => Program {
            snapshots: run.snapshots,
            lines: run.lines,
            error: None,
            files: run.files,
        },
        Err(error) => Program {
            error: Some(format!("{error:#}")),
            ..Default::default()
        },
    }
}

type Locks = (SceneKey, Vec<f64>, Vec<Option<String>>);

#[cfg(not(target_arch = "wasm32"))]
struct Watching {
    watcher: notify::RecommendedWatcher,
    files: Arc<std::sync::Mutex<std::collections::HashSet<PathBuf>>>,
    dirs: std::collections::HashSet<PathBuf>,
}

#[derive(Clone, PartialEq)]
struct SceneKey {
    generation: u64,
    line: usize,
    query: String,
}

struct Shown {
    key: SceneKey,
    scene: Arc<Scene>,
    faces: usize,
    bounds: ([f64; 3], [f64; 3]),
    query: Option<Result<(usize, usize), String>>,
    joints: Vec<Joint>,
    rig: Rig,
    explode: Vec<(String, monstertruck::modeling::Vector3)>,
    solids: Arc<Vec<(String, Solid)>>,
    assembly: bool,
    built: Duration,
    field: Option<Arc<crate::sim::statics::FieldView>>,
    sim: Option<Arc<SimState>>,
}

struct SimState {
    kind: crate::sim::Kind,
    outcome: Option<crate::sim::Outcome>,
    stale: Option<String>,
}

enum Status {
    Ok,
    Failed,
    NotRun,
}

#[derive(Clone, Copy)]
struct Pick {
    point: V3,
    face: Option<usize>,
    snap: Snap,
    edge: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Snap {
    Face,
    Edge,
    Corner,
    Middle,
    Centre,
}

impl Snap {
    fn name(self) -> &'static str {
        match self {
            Snap::Face => "on face",
            Snap::Edge => "on edge",
            Snap::Corner => "corner",
            Snap::Middle => "midpoint",
            Snap::Centre => "centre",
        }
    }
}

const POINT_SNAP: f32 = 12.0;
const EDGE_SNAP: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, Default)]
enum Mode {
    #[default]
    Solid,
    Ghost,
    Hidden,
}

#[derive(Clone, Copy, PartialEq)]
enum SliceShow {
    Off,
    Mean,
    Moment,
    Moving,
}

#[derive(Clone, Copy, PartialEq)]
enum Rank {
    All,
    Turbulence,
    Pressure,
}

fn chosen(lines: &[crate::sim::Stream], rank: Rank, most: bool, share: f32) -> Option<Vec<bool>> {
    let score = |s: &crate::sim::Stream| match rank {
        Rank::All => 0.0,
        Rank::Turbulence => s.turbulence,
        Rank::Pressure => s.pressure,
    };
    if rank == Rank::All {
        return None;
    }
    let mut order: Vec<usize> = (0..lines.len())
        .filter(|&k| lines[k].points.len() > 1)
        .collect();
    order.sort_by(|&a, &b| score(&lines[a]).total_cmp(&score(&lines[b])));
    if most {
        order.reverse();
    }
    let keep = ((order.len() as f32 * share).ceil() as usize).min(order.len());
    let mut mask = vec![false; lines.len()];
    for &k in order.iter().take(keep) {
        mask[k] = true;
    }
    Some(mask)
}

#[derive(Clone, Copy)]
struct Layer {
    on: bool,
    opacity: f32,
}

struct Flow {
    surface: Layer,
    streams: Layer,
    particles: Layer,
    slices: [(SliceShow, f32); 2],
    lines_moving: bool,
    lines_recorded: bool,
    rank: Rank,
    most: bool,
    share: f32,
    playing: bool,
    clock: f64,
    drift: f64,
    speed: f32,
}

impl Default for Flow {
    fn default() -> Flow {
        Flow {
            surface: Layer {
                on: true,
                opacity: 1.0,
            },
            streams: Layer {
                on: true,
                opacity: 0.5,
            },
            particles: Layer {
                on: true,
                opacity: 1.0,
            },
            slices: [(SliceShow::Off, 0.9); 2],
            lines_moving: true,
            lines_recorded: false,
            rank: Rank::All,
            most: true,
            share: 0.2,
            playing: true,
            clock: 0.0,
            drift: 0.0,
            speed: 1.0,
        }
    }
}

impl Flow {
    fn slice(&self, index: usize) -> (SliceShow, f32) {
        self.slices
            .get(index)
            .copied()
            .unwrap_or((SliceShow::Off, 1.0))
    }

    fn opacity(&self, role: scene::Role) -> Option<f32> {
        let layer = |l: Layer| if l.on { l.opacity } else { 0.0 };
        Some(match role {
            scene::Role::Body => return None,
            scene::Role::Field => layer(self.surface),
            scene::Role::Streams
                if (self.lines_moving && self.lines_recorded) || self.rank != Rank::All =>
            {
                0.0
            }
            scene::Role::Streams => layer(self.streams),
            scene::Role::Slice { index, moment } => match (self.slice(index), moment) {
                ((SliceShow::Mean, o), false) | ((SliceShow::Moment, o), true) => o,
                _ => 0.0,
            },
        })
    }
}

const REEL_RATE: f64 = 24.0;

fn frame_at(clock: f64, frames: usize) -> usize {
    ((clock * REEL_RATE).floor() as i64).rem_euclid(frames.max(1) as i64) as usize
}

fn frame_middle(frame: f64) -> f64 {
    (frame + 0.5) / REEL_RATE
}

struct Focus {
    part: String,
    at: V3,
}

pub struct App {
    path: Option<PathBuf>,
    vars: Vec<(String, f64)>,
    program: Arc<Program>,
    generation: u64,
    selected: usize,
    follow: bool,
    query: String,
    shown: Option<Shown>,
    building: Option<(SceneKey, Receiver<Shown>)>,
    cam: Camera,
    orbit: Orbit,
    events: Option<Receiver<()>>,
    #[cfg(not(target_arch = "wasm32"))]
    watching: Option<Watching>,
    dirty: Option<Instant>,
    loading: Option<(Receiver<Program>, Instant)>,
    loaded: Instant,
    took: Duration,
    measuring: bool,
    picks: Vec<Pick>,
    units: Units,
    joint_values: Vec<f64>,
    explode: f64,
    section: Option<(usize, f32, bool)>,
    joint_key: Vec<String>,
    modes: std::collections::HashMap<String, Mode>,
    opacities: std::collections::HashMap<String, f32>,
    lines_open: bool,
    parts_open: bool,
    flow: Flow,
    reels: Vec<Option<(usize, egui::TextureHandle)>>,
    opacity: f32,
    focus: Option<Focus>,
    clashes: Option<Result<Vec<String>, ()>>,
    clash_job: Option<Receiver<Vec<String>>>,
    locks: Option<Locks>,
    lock_job: Option<Receiver<Locks>>,
    browser: Option<Picker>,
    help_open: bool,
    render_open: bool,
    mode: RenderMode,
    preset: Preset,
    lighting: Lighting,
    trace: viewer3d::Trace,
    traced: bool,
    edges: bool,
    painted: Painted,
    frames: std::collections::VecDeque<Instant>,
    editing: bool,
    source: String,
    cursor_row: Option<usize>,
    sim_job: Option<SimJob>,
    sim_remote: bool,
    #[cfg(not(target_arch = "wasm32"))]
    remote: super::remote_panel::RemotePanel,
    sim_error: Option<String>,
    sim_every_part: bool,
    sim_notes: bool,
    activity: Arc<Mutex<SimProgress>>,
    _listening: crate::progress::Listening,
}

const WEB: bool = cfg!(target_arch = "wasm32");

type SimJob = (
    Receiver<Result<(), String>>,
    Instant,
    Arc<Mutex<SimProgress>>,
);

#[derive(Default)]
struct SimProgress {
    rows: Vec<StageRow>,
}

struct StageRow {
    report: crate::progress::Report,
    inside: Option<crate::progress::Report>,
    seen: Instant,
}

impl StageRow {
    fn elapsed(&self) -> Duration {
        if self.report.finished {
            self.report.elapsed
        } else {
            self.seen.elapsed()
        }
    }

    fn remaining(&self) -> Option<Duration> {
        let mut report = self.report.clone();
        report.elapsed = self.elapsed();
        report.remaining()
    }
}

impl SimProgress {
    fn take(&mut self, report: &crate::progress::Report) {
        if let Some(parent) = &report.parent
            && let Some(row) = self
                .rows
                .iter_mut()
                .rev()
                .find(|r| !r.report.finished && r.report.stage == *parent)
        {
            row.inside = (!report.finished).then(|| report.clone());
            return;
        }
        let name = report.name();
        match self
            .rows
            .iter_mut()
            .rev()
            .find(|r| !r.report.finished && r.report.name() == name)
        {
            Some(row) => {
                row.report = report.clone();
                if report.finished {
                    row.inside = None;
                }
            }
            None => self.rows.push(StageRow {
                report: report.clone(),
                inside: None,
                seen: Instant::now() - report.elapsed,
            }),
        }
    }

    fn show(&self, ui: &mut Ui) {
        const SHOWN: usize = 6;
        let done: Vec<&StageRow> = self.rows.iter().filter(|r| r.report.finished).collect();
        if done.len() > SHOWN {
            Line::new()
                .note(format!("{} earlier steps", done.len() - SHOWN))
                .size(11.0)
                .show(ui);
        }
        for row in done.iter().skip(done.len().saturating_sub(SHOWN)) {
            stage_row(
                ui,
                &row.report.name(),
                &crate::progress::duration(row.elapsed()),
                OK,
            );
        }
        for row in self.rows.iter().filter(|r| !r.report.finished) {
            let report = &row.report;
            let fraction = report.fraction();
            let right = match fraction {
                Some(f) => format!("{:.0}%", f * 100.0),
                None => crate::progress::duration(row.elapsed()),
            };
            stage_row(ui, &report.name(), &right, TRACE);
            progress(
                ui,
                "",
                fraction.unwrap_or(0.0) as f32,
                fraction.map(|_| 1.0),
                "",
            );
            let what = match &row.inside {
                Some(inside) => {
                    let mut text = inside.stage.clone();
                    if let Some(summary) = inside.summary() {
                        text += &format!(": {summary}");
                    }
                    Some(text)
                }
                None => report.summary(),
            };
            let when = match row.remaining() {
                Some(left) => format!("about {} left", crate::progress::duration(left)),
                None => format!("{} so far", crate::progress::duration(row.elapsed())),
            };
            let text = match what {
                Some(what) => format!("{when}; {what}"),
                None => when,
            };
            Line::new()
                .note(&text)
                .size(11.0)
                .elided(ui)
                .on_hover_text(text);
            ui.ctx().request_repaint_after(Duration::from_millis(250));
        }
    }
}

fn stage_row(ui: &mut Ui, name: &str, right: &str, mark: egui::Color32) {
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(value(right));
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 14.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 2.5, mark);
                ui.add(egui::Label::new(legend(name)).truncate());
            });
        });
    });
}

type Vector = (String, String, Vec<(String, String)>);

#[derive(Debug, PartialEq)]
struct ReadingsLayout {
    tiles: Vec<(String, String)>,
    vectors: Vec<Vector>,
    columns: Vec<String>,
    rows: Vec<(String, Vec<Option<String>>)>,
    blocks: Vec<(String, Vec<(String, String)>)>,
    total: usize,
}

fn bare_name(unit: &str) -> &'static str {
    match unit {
        "N·m" => "torque",
        "N" => "force",
        "MPa" => "stress",
        "mm" => "displacement",
        _ => "value",
    }
}

type Labelled<'a> = (String, Vec<(String, &'a crate::sim::Reading)>);

fn layout_readings(readings: &[crate::sim::Reading], few: Option<usize>) -> ReadingsLayout {
    use crate::sim::Reading;
    let short = crate::sim::short;
    let split = |r: &Reading| {
        r.name
            .rsplit_once('.')
            .map(|(h, t)| (h.to_string(), t.to_string()))
    };
    let mut groups: Vec<Labelled> = Vec::new();
    for r in readings {
        if let Some((head, tail)) = split(r) {
            match groups.iter_mut().find(|(h, _)| *h == head) {
                Some((_, list)) => list.push((tail, r)),
                None => groups.push((head, vec![(tail, r)])),
            }
        }
    }
    let vector = |list: &[(String, &Reading)]| {
        list.iter()
            .all(|(t, _)| matches!(t.as_str(), "x" | "y" | "z"))
    };
    let mut tiles = Vec::new();
    let with_unit = |r: &Reading| format!("{} {}", short(r.value), r.unit).trim().to_string();
    for r in readings {
        if split(r).is_none() && !groups.iter().any(|(h, list)| *h == r.name && !vector(list)) {
            tiles.push((r.name.clone(), with_unit(r)));
        }
    }
    let vectors = groups
        .iter()
        .filter(|(_, list)| vector(list))
        .map(|(head, list)| {
            let unit = list.first().map_or(String::new(), |(_, r)| r.unit.clone());
            let values = list
                .iter()
                .map(|(tail, r)| (tail.clone(), short(r.value)))
                .collect();
            (head.clone(), unit, values)
        })
        .collect();
    let mut parts: Vec<Labelled> = groups
        .into_iter()
        .filter(|(_, list)| !vector(list))
        .map(|(head, mut list)| {
            if let Some(bare) = readings.iter().find(|r| r.name == head) {
                list.insert(0, (bare_name(&bare.unit).to_string(), bare));
            }
            (head, list)
        })
        .collect();
    let mut columns: Vec<String> = Vec::new();
    for (_, list) in &parts {
        for (tail, _) in list {
            if !columns.contains(tail) {
                columns.push(tail.clone());
            }
        }
    }
    let unit_of = |column: &str| {
        let units: Vec<&str> = parts
            .iter()
            .flat_map(|(_, list)| list.iter().filter(|(t, _)| t == column))
            .map(|(_, r)| r.unit.as_str())
            .collect();
        units
            .first()
            .filter(|u| units.iter().all(|v| v == *u))
            .map(|u| u.to_string())
    };
    let units: Vec<Option<String>> = columns.iter().map(|c| unit_of(c)).collect();
    let total = parts.len();
    if let Some(few) = few.filter(|few| total > *few) {
        let first = |list: &[(String, &Reading)]| {
            list.iter()
                .find(|(t, _)| *t == columns[0])
                .map_or(0.0, |(_, r)| r.value.abs())
        };
        parts.sort_by(|a, b| first(&b.1).total_cmp(&first(&a.1)));
        parts.truncate(few);
    }
    let rows = parts
        .iter()
        .map(|(head, list)| {
            let cells = columns
                .iter()
                .zip(&units)
                .map(|(column, unit)| {
                    list.iter()
                        .find(|(t, _)| t == column)
                        .map(|(_, r)| match unit {
                            Some(_) => short(r.value),
                            None => with_unit(r),
                        })
                })
                .collect();
            (head.clone(), cells)
        })
        .collect();
    let blocks = parts
        .iter()
        .map(|(head, list)| {
            let tiles = list
                .iter()
                .map(|(tail, r)| (tail.clone(), with_unit(r)))
                .collect();
            (head.clone(), tiles)
        })
        .collect();
    let columns = columns
        .iter()
        .zip(&units)
        .map(|(c, u)| match u {
            Some(u) if !u.is_empty() => format!("{c} {u}"),
            _ => c.clone(),
        })
        .collect();
    ReadingsLayout {
        tiles,
        vectors,
        columns,
        rows,
        blocks,
        total,
    }
}

fn tile_grid(ui: &mut Ui, tiles: &[(String, String)]) {
    const PER_ROW: usize = 3;
    let width = ui.available_width() / PER_ROW as f32;
    for row in tiles.chunks(PER_ROW) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (label, value) in row {
                ui.allocate_ui_with_layout(
                    egui::vec2(width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(width);
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.add(egui::Label::new(legend(label)).truncate());
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(value)
                                    .font(egui_bench::theme::figure(14.0))
                                    .color(VALUE),
                            )
                            .truncate(),
                        );
                    },
                );
            }
        });
        ui.add_space(4.0);
    }
}

fn readings_list(ui: &mut Ui, readings: &[crate::sim::Reading], every_part: &mut bool) {
    const FEW: usize = 8;
    let layout = layout_readings(readings, (!*every_part).then_some(FEW));
    tile_grid(ui, &layout.tiles);
    for (head, unit, values) in &layout.vectors {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(legend(format!("{head} {unit}")));
            for (axis, value) in values {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(axis)
                        .font(egui_bench::theme::figure(11.0))
                        .color(LEGEND),
                );
                ui.label(
                    egui::RichText::new(value)
                        .font(egui_bench::theme::figure(12.0))
                        .color(VALUE),
                );
            }
        });
    }
    if layout.rows.is_empty() {
        return;
    }
    ui.add_space(4.0);
    if layout.total > FEW {
        let what = layout.columns[0]
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
        Line::new()
            .legend(&format!("{} parts", layout.total))
            .note(if *every_part {
                format!("by {what}")
            } else {
                format!("the {FEW} with the most {what}")
            })
            .size(11.0)
            .show(ui);
    }
    if layout.columns.len() > 4 {
        for (head, tiles) in &layout.blocks {
            ui.add_space(4.0);
            ui.label(legend(head).color(VALUE));
            tile_grid(ui, tiles);
        }
    } else {
        parts_table(ui, &layout);
    }
    if layout.total > FEW {
        ui.add_space(4.0);
        let label = if *every_part {
            "fewer parts".to_string()
        } else {
            format!("all {} parts", layout.total)
        };
        if toggle(ui, &label, *every_part).clicked() {
            *every_part = !*every_part;
        }
    }
}

fn parts_table(ui: &mut Ui, layout: &ReadingsLayout) {
    let width = ui.available_width();
    let first = (width * 0.34).max(90.0);
    let rest = (width - first) / layout.columns.len().max(1) as f32;
    let mut header: Vec<(&str, f32)> = vec![("", first)];
    header.extend(layout.columns.iter().map(|c| (c.as_str(), rest)));
    Table::new(&header, layout.rows.len()).show(ui, |i, p, row, at| {
        let (head, cells) = &layout.rows[i];
        cell(p, row, at(0), first, head, LEGEND);
        for (k, value) in cells.iter().enumerate() {
            if let Some(value) = value {
                cell(p, row, at(k + 1), rest, value, VALUE);
            }
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn changes_content(kind: &notify::EventKind) -> bool {
    use notify::EventKind;
    use notify::event::{AccessKind, AccessMode, ModifyKind};
    match kind {
        EventKind::Create(_) | EventKind::Remove(_) => true,
        EventKind::Modify(ModifyKind::Metadata(_)) => false,
        EventKind::Modify(_) => true,
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        _ => false,
    }
}

fn flat(m: Matrix4) -> Placement {
    let c = [m.x, m.y, m.z, m.w];
    let mut out = [0.0f32; 16];
    for (i, column) in c.iter().enumerate() {
        out[i * 4] = column.x as f32;
        out[i * 4 + 1] = column.y as f32;
        out[i * 4 + 2] = column.z as f32;
        out[i * 4 + 3] = column.w as f32;
    }
    out
}

const VIEWS: [Option<(&str, f32, f32)>; 8] = [
    Some(("iso", std::f32::consts::FRAC_PI_4, 0.6155)),
    Some(("top", 0.0, 1.5699)),
    Some(("front", 0.0, 0.0)),
    Some(("right", std::f32::consts::FRAC_PI_2, 0.0)),
    None,
    Some(("bottom", 0.0, -1.5699)),
    Some(("back", std::f32::consts::PI, 0.0)),
    Some(("left", -std::f32::consts::FRAC_PI_2, 0.0)),
];

const CUBE_FACES: [(V3, &str); 6] = [
    ([0.0, 0.0, 1.0], "TOP"),
    ([0.0, 0.0, -1.0], "BOTTOM"),
    ([0.0, -1.0, 0.0], "FRONT"),
    ([0.0, 1.0, 0.0], "BACK"),
    ([1.0, 0.0, 0.0], "RIGHT"),
    ([-1.0, 0.0, 0.0], "LEFT"),
];

fn ghost_button(ui: &mut Ui, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
    let lit = on || resp.hovered();
    let colour = if lit { READOUT } else { LEGEND };
    let p = ui.painter();
    let c = rect.center();
    p.circle_stroke(c, 5.5, Stroke::new(1.0, colour));
    if on {
        p.circle_filled(c, 5.5, READOUT.gamma_multiply(0.35));
    }
    p.line_segment(
        [c + Vec2::new(-4.0, 4.0), c + Vec2::new(4.0, -4.0)],
        Stroke::new(1.0, colour),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text(if on { "solid" } else { "see-through" })
}

fn unit_picker(ui: &mut Ui, current: Units) -> Option<Units> {
    let font = figure(12.0);
    let height = ui.fonts_mut(|f| f.row_height(&legend_font(11.5))) + 7.0;
    let galleys: Vec<_> = Units::ALL
        .iter()
        .map(|u| {
            let tint = if *u == current { READOUT } else { LEGEND };
            ui.painter()
                .layout_no_wrap(u.name().to_string(), font.clone(), tint)
        })
        .collect();
    let widths: Vec<f32> = galleys
        .iter()
        .map(|g| g.size().x.max(14.0) + 14.0)
        .collect();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(widths.iter().sum(), height), Sense::hover());
    let mut x = rect.left();
    let mut chosen = None;
    for ((unit, galley), width) in Units::ALL.into_iter().zip(galleys).zip(widths) {
        let cell = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(width, height));
        x += width;
        let resp = ui
            .interact(cell, ui.id().with(("units", unit.name())), Sense::click())
            .on_hover_text("units for lengths, areas, volumes and mass");
        let on = unit == current;
        let p = ui.painter();
        let fill = if on {
            READOUT.gamma_multiply(0.14)
        } else if resp.hovered() {
            BAND
        } else {
            WELL
        };
        p.rect_filled(cell, 0.0, fill);
        p.rect_stroke(cell, 0.0, Stroke::new(1.0, ETCH), egui::StrokeKind::Inside);
        if on {
            let mark = Rect::from_min_max(
                Pos2::new(cell.left(), cell.bottom() - 2.0),
                cell.right_bottom(),
            );
            p.rect_filled(mark, 0.0, READOUT);
        }
        p.galley(cell.center() - galley.size() / 2.0, galley, LEGEND);
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() && !on {
            chosen = Some(unit);
        }
    }
    chosen
}

fn closest_along(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    let ab = b - a;
    let span = ab.length_sq();
    if span < 1.0e-6 {
        return 0.0;
    }
    ((p - a).dot(ab) / span).clamp(0.0, 1.0)
}

fn lerp3(a: V3, b: V3, t: f32) -> V3 {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn snap_mark(p: &egui::Painter, at: Pos2, snap: Snap, colour: Color32) {
    let stroke = Stroke::new(1.5, colour);
    let corner = |x: f32, y: f32| at + Vec2::new(x, y);
    match snap {
        Snap::Corner => {
            p.rect_stroke(
                Rect::from_center_size(at, Vec2::splat(11.0)),
                0.0,
                stroke,
                egui::StrokeKind::Middle,
            );
        }
        Snap::Middle => {
            p.add(egui::Shape::closed_line(
                vec![corner(0.0, -6.5), corner(6.0, 4.5), corner(-6.0, 4.5)],
                stroke,
            ));
        }
        Snap::Centre => {
            p.circle_stroke(at, 6.0, stroke);
            p.line_segment([corner(-9.0, 0.0), corner(9.0, 0.0)], stroke);
            p.line_segment([corner(0.0, -9.0), corner(0.0, 9.0)], stroke);
        }
        Snap::Edge => {
            p.add(egui::Shape::closed_line(
                vec![
                    corner(-5.0, -5.0),
                    corner(5.0, -5.0),
                    corner(-5.0, 5.0),
                    corner(5.0, 5.0),
                ],
                stroke,
            ));
        }
        Snap::Face => {
            p.circle_stroke(at, 3.5, stroke);
        }
    }
}

fn inside(polygon: &[Pos2], p: Pos2) -> bool {
    (0..polygon.len()).fold(false, |inside, i| {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            !inside
        } else {
            inside
        }
    })
}

impl App {
    pub fn web(cc: &eframe::CreationContext, path: Option<PathBuf>) -> Self {
        Self::new(cc, path, String::new(), None, Vec::new())
    }

    fn new(
        cc: &eframe::CreationContext,
        path: Option<PathBuf>,
        query: String,
        line: Option<usize>,
        vars: Vec<(String, f64)>,
    ) -> Self {
        egui_bench::install(&cc.egui_ctx);
        if let Some(state) = &cc.wgpu_render_state {
            egui_bench::viewer3d::install(state);
        }
        let activity = Arc::new(Mutex::new(SimProgress::default()));
        let listening = {
            let into = activity.clone();
            let painter = cc.egui_ctx.clone();
            crate::progress::listen(move |report| {
                into.lock().unwrap_or_else(|e| e.into_inner()).take(report);
                painter.request_repaint();
            })
        };
        let mut app = App {
            path: None,
            vars,
            program: Arc::new(Program::default()),
            generation: 0,
            selected: line.map_or(0, |n| n.saturating_sub(1)),
            follow: line.is_none(),
            query,
            shown: None,
            building: None,
            cam: Camera::default(),
            orbit: Orbit::default(),
            events: None,
            #[cfg(not(target_arch = "wasm32"))]
            watching: None,
            dirty: None,
            loading: None,
            loaded: Instant::now(),
            took: Duration::ZERO,
            measuring: false,
            picks: Vec::new(),
            units: crate::host::setting("units")
                .and_then(|u| Units::parse(&u))
                .unwrap_or_default(),
            joint_values: Vec::new(),
            explode: 0.0,
            section: None,
            joint_key: Vec::new(),
            modes: Default::default(),
            opacities: Default::default(),
            lines_open: true,
            parts_open: true,
            flow: Flow::default(),
            reels: Vec::new(),
            opacity: 0.25,
            focus: None,
            clashes: None,
            clash_job: None,
            locks: None,
            lock_job: None,
            browser: None,
            help_open: false,
            render_open: false,
            mode: RenderMode::Shaded,
            preset: Preset::Studio,
            lighting: Lighting::STUDIO,
            trace: viewer3d::Trace::default(),
            traced: false,
            edges: true,
            painted: Painted::default(),
            frames: std::collections::VecDeque::new(),
            editing: WEB,
            source: String::new(),
            cursor_row: None,
            sim_job: None,
            sim_remote: false,
            #[cfg(not(target_arch = "wasm32"))]
            remote: super::remote_panel::RemotePanel::new(),
            sim_every_part: false,
            sim_notes: false,
            activity,
            _listening: listening,
            sim_error: None,
        };
        match path {
            Some(path) => app.open(path, &cc.egui_ctx),
            None if crate::host::status().is_some() => {}
            None => app.help_open = true,
        }
        app
    }

    fn open(&mut self, path: PathBuf, ctx: &egui::Context) {
        let path = crate::host::canonicalize(&path).unwrap_or(path);
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "gcad - {}",
            path.display()
        )));
        crate::host::opened(&path);
        self.source = crate::host::read_to_string(&path).unwrap_or_default();
        self.cursor_row = None;
        self.path = Some(path);
        self.program = Arc::new(Program::default());
        self.shown = None;
        self.building = None;
        self.loading = None;
        self.clash_job = None;
        self.lock_job = None;
        self.dirty = None;
        self.picks.clear();
        self.modes.clear();
        self.focus = None;
        self.clashes = None;
        self.follow = true;
        self.cam = Camera::default();
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.watching = None;
        }
        self.events = None;
        self.reload(ctx);
        self.watch(ctx.clone());
    }

    #[cfg(target_arch = "wasm32")]
    fn watch(&mut self, _ctx: egui::Context) {}

    #[cfg(not(target_arch = "wasm32"))]
    fn watch(&mut self, ctx: egui::Context) {
        use notify::{RecursiveMode, Watcher};
        let Some(file) = self.path.clone() else {
            return;
        };
        let (tx, rx) = channel();
        let watched = file.clone();
        let files: Arc<std::sync::Mutex<std::collections::HashSet<PathBuf>>> = Arc::default();
        let read = files.clone();
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let known = |p: &PathBuf| {
                let read = read.lock().unwrap_or_else(|e| e.into_inner());
                read.contains(p) || std::fs::canonicalize(p).is_ok_and(|c| read.contains(&c))
            };
            if let Ok(event) = res
                && changes_content(&event.kind)
                && event.paths.iter().any(|p| {
                    p == &watched
                        || known(p)
                        || p.parent() == watched.parent()
                            && p.extension().is_some_and(|e| {
                                e == "gcad" || e == "gasm" || e == "gsim" || e == "json"
                            })
                })
            {
                let _ = tx.send(());
                ctx.request_repaint();
            }
        });
        if let Ok(mut watcher) = watcher
            && let Some(dir) = file.parent()
            && watcher.watch(dir, RecursiveMode::NonRecursive).is_ok()
        {
            let dirs = std::fs::canonicalize(dir).into_iter().collect();
            self.watching = Some(Watching {
                watcher,
                files,
                dirs,
            });
            self.events = Some(rx);
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn follow_reads(&mut self, _read: &[PathBuf]) {}

    #[cfg(not(target_arch = "wasm32"))]
    fn follow_reads(&mut self, read: &[PathBuf]) {
        use notify::{RecursiveMode, Watcher};
        let Some(watching) = self.watching.as_mut() else {
            return;
        };
        for file in read {
            if let Some(dir) = file.parent()
                && watching.dirs.insert(dir.to_path_buf())
            {
                let _ = watching.watcher.watch(dir, RecursiveMode::NonRecursive);
            }
        }
        *watching.files.lock().unwrap_or_else(|e| e.into_inner()) = read.iter().cloned().collect();
    }

    fn reload(&mut self, ctx: &egui::Context) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let (tx, rx) = channel();
        let (ctx, vars) = (ctx.clone(), self.vars.clone());
        self.activity
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .rows
            .clear();
        crate::host::spawn(move || {
            let program = {
                let _stage = crate::progress::stage("building the model", None, "");
                evaluate(&path, &vars)
            };
            let _ = tx.send(program);
            crate::host::wake(&ctx);
        });
        self.loading = Some((rx, Instant::now()));
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.events {
            while rx.try_recv().is_ok() {
                self.dirty = Some(Instant::now());
            }
        }
        if let Some(rx) = &self.clash_job {
            match rx.try_recv() {
                Ok(found) => {
                    self.clashes = Some(Ok(found));
                    self.clash_job = None;
                }
                Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(100)),
                Err(TryRecvError::Disconnected) => {
                    self.clashes = Some(Err(()));
                    self.clash_job = None;
                }
            }
        }
        if let Some(rx) = &self.lock_job {
            match rx.try_recv() {
                Ok(locks) => {
                    self.locks = Some(locks);
                    self.lock_job = None;
                }
                Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(50)),
                Err(TryRecvError::Disconnected) => self.lock_job = None,
            }
        }
        if let Some((rx, ..)) = &self.sim_job {
            match rx.try_recv() {
                Ok(result) => {
                    self.sim_error = result.err();
                    self.sim_job = None;
                    self.generation += 1;
                    #[cfg(not(target_arch = "wasm32"))]
                    if self.sim_remote {
                        self.remote.refresh();
                    }
                    self.sim_remote = false;
                }
                Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(500)),
                Err(TryRecvError::Disconnected) => {
                    self.sim_error = Some("the simulation stopped without a result".to_string());
                    self.sim_job = None;
                    self.sim_remote = false;
                }
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self.shown.as_ref().is_some_and(|s| s.sim.is_some()) {
            self.remote.tick(ctx, self.sim_remote);
        }
        if let Some((key, rx)) = &self.building {
            match rx.try_recv() {
                Ok(shown) => {
                    if shown.key == *key {
                        self.adopt(shown);
                    }
                    self.building = None;
                }
                Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(50)),
                Err(TryRecvError::Disconnected) => self.building = None,
            }
        }
        if let Some((rx, started)) = &self.loading {
            match rx.try_recv() {
                Ok(program) => {
                    self.took = started.elapsed();
                    self.loading = None;
                    self.follow_reads(&program.files);
                    self.install(program);
                }
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(100));
                    if self.dirty.is_none() {
                        return;
                    }
                }
                Err(TryRecvError::Disconnected) => self.loading = None,
            }
        }
        if let Some(t) = self.dirty {
            let wait = Duration::from_millis(250);
            if t.elapsed() >= wait {
                self.dirty = None;
                self.reload(ctx);
            } else {
                ctx.request_repaint_after(wait - t.elapsed());
            }
        }
    }

    fn adopt(&mut self, shown: Shown) {
        let names: Vec<String> = shown.joints.iter().map(|j| j.name.clone()).collect();
        if names != self.joint_key {
            self.joint_values = shown.joints.iter().map(|j| j.value).collect();
            self.joint_key = names;
        }
        self.shown = Some(shown);
    }

    fn install(&mut self, program: Program) {
        crate::host::built(self.took.as_secs_f64());
        self.generation += 1;
        self.loaded = Instant::now();
        let last = program.snapshots.len().saturating_sub(1);
        if self.follow || self.selected >= program.lines.len() {
            self.selected = last;
        }
        self.program = Arc::new(program);
        self.clashes = None;
    }

    fn status(&self, index: usize) -> Status {
        match self.program.snapshots.get(index).map(|s| s.result.is_ok()) {
            Some(true) => Status::Ok,
            Some(false) => Status::Failed,
            None => Status::NotRun,
        }
    }

    fn refresh_scene(&mut self, ctx: &egui::Context) {
        let key = SceneKey {
            generation: self.generation,
            line: self.selected,
            query: self.query.trim().to_string(),
        };
        if self.shown.as_ref().is_some_and(|s| s.key == key)
            || self.building.as_ref().is_some_and(|(k, _)| *k == key)
        {
            return;
        }
        let program = self.program.clone();
        let index = self.selected.min(program.snapshots.len().saturating_sub(1));
        if program.snapshots.is_empty() {
            self.shown = None;
            return;
        }
        let (tx, rx) = channel();
        let job_key = key.clone();
        let ctx = ctx.clone();
        let vars = self.vars.clone();
        crate::host::spawn(move || {
            let _stage =
                crate::progress::stage_after("drawing the scene", None, "", Duration::from_secs(1));
            let snapshot = &program.snapshots[index];
            let started = Instant::now();
            if let Some(mut shown) = build_shown(snapshot, job_key, &vars) {
                shown.built = started.elapsed();
                let _ = tx.send(shown);
            }
            crate::host::wake(&ctx);
            crate::host::release_memory();
        });
        self.building = Some((key, rx));
    }

    fn mode(&self, part: &str) -> Mode {
        if let Some(mode) = self.modes.get(part) {
            return *mode;
        }
        let hidden = self
            .shown
            .as_ref()
            .and_then(|s| s.scene.parts.iter().find(|p| p.name == part))
            .is_some_and(|p| p.hidden);
        if hidden {
            Mode::Hidden
        } else {
            Mode::default()
        }
    }

    fn set_mode(&mut self, part: &str, mode: Mode) {
        self.modes.insert(part.to_string(), mode);
    }

    fn looks(&self) -> Vec<Look> {
        let Some(shown) = &self.shown else {
            return Vec::new();
        };
        let values = if self.joint_values.len() == shown.joints.len() {
            self.joint_values.clone()
        } else {
            shown.joints.iter().map(|j| j.value).collect()
        };
        let flowing = shown.field.as_ref().is_some_and(|f| is_flow(f));
        let moved = posed(&shown.joints, &values);
        let offsets = explode_offsets(&shown.joints, &shown.explode, &moved);
        shown
            .scene
            .parts
            .iter()
            .map(|part| {
                let m = moved
                    .get(&part.name)
                    .copied()
                    .unwrap_or_else(Matrix4::identity);
                let m = exploded(m, offsets.get(&part.name), self.explode);
                let flow = if flowing {
                    self.flow.opacity(part.role)
                } else {
                    None
                };
                Look {
                    placement: flat(m),
                    opacity: flow.unwrap_or_else(|| match self.mode(&part.name) {
                        Mode::Solid => self
                            .opacities
                            .get(&part.name)
                            .copied()
                            .unwrap_or(part.opacity),
                        Mode::Ghost => self.opacity,
                        Mode::Hidden => 0.0,
                    }),
                    selected: self.focus.as_ref().is_some_and(|f| f.part == part.name),
                }
            })
            .collect()
    }

    fn header(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            Line::new()
                .legend("gcad")
                .value(
                    self.path
                        .as_ref()
                        .map_or_else(|| "no file".to_string(), |p| p.display().to_string()),
                )
                .show(ui);
            if toggle(ui, "open", self.browser.is_some()).clicked() {
                self.browser = match self.browser {
                    Some(_) => None,
                    None => Some(browser_at(
                        self.path
                            .as_ref()
                            .and_then(|p| p.parent())
                            .map(std::path::Path::to_path_buf)
                            .or_else(|| std::env::current_dir().ok())
                            .unwrap_or_default(),
                    )),
                };
            }
            if toggle(ui, "?", self.help_open).clicked() {
                self.help_open = !self.help_open;
            }
            if WEB && toggle(ui, "edit", self.editing).clicked() {
                self.editing = !self.editing;
            }
            if toggle(ui, "measure", self.measuring).clicked() {
                self.measuring = !self.measuring;
                self.picks.clear();
            }
            if toggle(ui, "section", self.section.is_some()).clicked() {
                self.section = match self.section {
                    Some(_) => None,
                    None => Some((1, 0.5, false)),
                };
            }
            if toggle(ui, "lines", self.lines_open).clicked() {
                self.lines_open = !self.lines_open;
            }
            if toggle(ui, "parts", self.parts_open).clicked() {
                self.parts_open = !self.parts_open;
            }
            if toggle(ui, "render", self.render_open).clicked() {
                self.render_open = !self.render_open;
                self.parts_open |= self.render_open;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.path.is_none() {
                    return;
                }
                let failed = self.program.error.is_some()
                    || self.program.snapshots.iter().any(|s| s.result.is_err());
                lamp(ui, if failed { "error" } else { "ok" }, !failed, failed);
                let busy = self.loading.is_some();
                lamp(
                    ui,
                    if busy { "building" } else { "live" },
                    self.events.is_some() && !busy,
                    false,
                )
                .on_hover_text(format!(
                    "rebuilds when the file is saved; last build took {:.2}s, {:.0}s ago",
                    self.took.as_secs_f32(),
                    self.loaded.elapsed().as_secs_f32()
                ));
                lamp(
                    ui,
                    &format!("{} lines", self.program.lines.len()),
                    false,
                    false,
                );
                if self.path.is_some() && toggle(ui, "rebuild", false).clicked() {
                    self.reload(ctx);
                }
                ui.add_space(10.0);
                if let Some(units) = unit_picker(ui, self.units) {
                    self.units = units;
                    crate::host::set_setting("units", units.name());
                }
            });
        });
        ui.add_space(6.0);
    }

    fn parse_error(&self, ui: &mut Ui) {
        if let Some(error) = &self.program.error {
            card(
                ui,
                Some(FAULT),
                |ui| {
                    Line::new().legend("file did not parse").show(ui);
                },
                |ui| {
                    note(ui, error, FAULT);
                },
            );
            ui.add_space(6.0);
        }
    }

    fn editor(&mut self, ui: &mut Ui) {
        self.parse_error(ui);
        let Some(path) = self.path.clone() else {
            note(ui, "open a file to edit it", LEGEND);
            return;
        };
        if crate::host::edited(&path) {
            ui.horizontal(|ui| {
                note(ui, "edited, kept in this browser", LEGEND);
                if toggle(ui, "revert", false).clicked() {
                    crate::host::revert(&path);
                    self.source = crate::host::read_to_string(&path).unwrap_or_default();
                    self.dirty = Some(Instant::now());
                }
            });
            ui.add_space(4.0);
        }
        let failed = self
            .program
            .snapshots
            .iter()
            .zip(&self.program.lines)
            .find(|(s, _)| s.result.is_err())
            .map(|(_, l)| l.number);
        let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, _wrap: f32| {
            let mut job = egui::text::LayoutJob::default();
            for (row, line) in text.as_str().split_inclusive('\n').enumerate() {
                let colour = if Some(row + 1) == failed {
                    FAULT
                } else {
                    VALUE
                };
                job.append(line, 0.0, egui::TextFormat::simple(mono(13.0), colour));
            }
            job.wrap.max_width = f32::INFINITY;
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let output = egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::TextEdit::multiline(&mut self.source)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(40)
                    .lock_focus(true)
                    .layouter(&mut layouter)
                    .show(ui)
            })
            .inner;
        if output.response.changed() {
            crate::host::save(&path, &self.source);
            self.dirty = Some(Instant::now());
        }
        if let Some(range) = output.cursor_range {
            let row = self
                .source
                .chars()
                .take(range.primary.index.0)
                .filter(|c| *c == '\n')
                .count();
            if self.cursor_row != Some(row) {
                self.cursor_row = Some(row);
                let at = self
                    .program
                    .lines
                    .iter()
                    .rposition(|l| l.number <= row + 1)
                    .unwrap_or(0);
                if at < self.program.lines.len() {
                    self.select(at);
                }
            }
        }
    }

    fn lines(&mut self, ui: &mut Ui) {
        self.parse_error(ui);
        let program = self.program.clone();
        let mut clicked = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, 20.0, program.lines.len(), |ui, range| {
                for index in range {
                    let line = &program.lines[index];
                    let (rect, resp) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::click());
                    let on = index == self.selected;
                    let p = ui.painter();
                    if on {
                        p.rect_filled(rect, 0.0, PANEL);
                        p.rect_filled(
                            Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())),
                            0.0,
                            READOUT,
                        );
                    } else if resp.hovered() {
                        p.rect_filled(rect, 0.0, BAND);
                    }
                    let (dot, text_colour) = match self.status(index) {
                        Status::Ok => (OK, VALUE),
                        Status::Failed => (FAULT, FAULT),
                        Status::NotRun => (ETCH, LEGEND),
                    };
                    p.circle_filled(Pos2::new(rect.left() + 11.0, rect.center().y), 3.0, dot);
                    p.text(
                        Pos2::new(rect.left() + 40.0, rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        line.number.to_string(),
                        figure(11.0),
                        LEGEND,
                    );
                    p.with_clip_rect(rect).text(
                        Pos2::new(rect.left() + 48.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &line.text,
                        mono(12.0),
                        if on {
                            text_colour
                        } else {
                            text_colour.gamma_multiply(0.85)
                        },
                    );
                    if resp.clicked() {
                        clicked = Some(index);
                    }
                }
            });
        if let Some(index) = clicked {
            self.select(index);
        }
    }

    fn select(&mut self, index: usize) {
        self.selected = index;
        self.follow = index + 1 >= self.program.snapshots.len();
    }

    fn line_card(&mut self, ui: &mut Ui) {
        let program = self.program.clone();
        let Some(line) = program.lines.get(self.selected) else {
            return;
        };
        let snapshot = program.snapshots.get(self.selected);
        let rail = match snapshot.map(|s| s.result.is_ok()) {
            Some(true) => OK,
            Some(false) => FAULT,
            None => ETCH,
        };
        card(
            ui,
            Some(rail),
            |ui| {
                Line::new()
                    .legend("line")
                    .value(line.number.to_string())
                    .legend(&line.op)
                    .show(ui);
            },
            |ui| {
                Line::new().value(&line.text).size(12.0).wrapped(ui);
                if line.label.is_some()
                    || matches!(
                        line.op.as_str(),
                        "extrude" | "cut" | "hole" | "revolve" | "sweep" | "fillet" | "chamfer"
                    )
                {
                    Line::new()
                        .legend("faces")
                        .set(label_of(line))
                        .note("highlighted")
                        .show(ui);
                }
                match snapshot.map(|s| &s.result) {
                    Some(Ok(summary)) => note(ui, self.units.summary(summary), LEGEND),
                    Some(Err(error)) => note(ui, error, FAULT),
                    None => note(ui, "not run, an earlier line failed", LEGEND),
                }
            },
        );
        ui.add_space(8.0);
    }

    fn sim_card(&mut self, ui: &mut Ui) {
        let Some(sim) = self.shown.as_ref().and_then(|s| s.sim.clone()) else {
            return;
        };
        let short = crate::sim::short;
        let failed = sim
            .outcome
            .as_ref()
            .is_some_and(|o| o.checks.iter().any(|(_, _, pass)| !pass));
        let rail = match (&sim.outcome, &sim.stale) {
            (None, _) => ETCH,
            (Some(_), _) if failed => FAULT,
            (Some(_), Some(_)) => WARN,
            (Some(_), None) => OK,
        };
        let mut run = false;
        let mut run_remote = false;
        card(
            ui,
            Some(rail),
            |ui| {
                Line::new()
                    .legend("simulation")
                    .value(sim.kind.name())
                    .show(ui);
            },
            |ui| {
                let running = self
                    .sim_job
                    .as_ref()
                    .map(|(_, started, watched)| (*started, watched.clone()));
                if running.is_none()
                    && let Some(why) = &sim.stale
                {
                    let brief = why
                        .split_once(" is stale (")
                        .and_then(|(_, rest)| rest.rsplit_once(')'))
                        .map(|(changed, _)| format!("stale: {changed}"))
                        .unwrap_or_else(|| why.clone());
                    let tint = if sim.outcome.is_some() { WARN } else { LEGEND };
                    Line::new()
                        .note(&brief)
                        .tint(tint)
                        .size(11.0)
                        .elided(ui)
                        .on_hover_text(why);
                }
                if !WEB {
                    match &running {
                        Some((started, watched)) => {
                            lamp(
                                ui,
                                &format!(
                                    "running {} {}",
                                    if self.sim_remote { "on the server" } else { "" },
                                    crate::progress::duration(started.elapsed())
                                )
                                .replace("  ", " "),
                                true,
                                false,
                            );
                            if !self.sim_remote {
                                watched.lock().unwrap_or_else(|e| e.into_inner()).show(ui);
                            }
                        }
                        None => {
                            let label = if sim.outcome.is_some() {
                                "rerun"
                            } else {
                                "run"
                            };
                            run = toggle(ui, label, false).clicked();
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                run_remote = toggle(ui, "run on server", false)
                                    .on_hover_text("build here, solve on a paid GPU on the sims server")
                                    .clicked();
                            }
                        }
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let label = self.remote.summary();
                        if toggle(ui, &label, self.remote.open)
                            .on_hover_text("account, balance, runners and recent runs on the sims server")
                            .clicked()
                        {
                            self.remote.open = !self.remote.open;
                            self.remote.refresh();
                        }
                    }
                }
                if let Some(error) = &self.sim_error {
                    note(ui, error, FAULT);
                }
                if running.is_some() {
                    return;
                }
                let Some(outcome) = &sim.outcome else {
                    return;
                };
                readings_list(ui, &outcome.readings, &mut self.sim_every_part);
                for (text, value, pass) in &outcome.checks {
                    status(ui, *pass, &format!("{text}, reads {}", short(*value)));
                }
                for series in &outcome.series {
                    series_traces(ui, series);
                }
                if !outcome.notes.is_empty() {
                    ui.add_space(4.0);
                    let label = format!("notes ({})", outcome.notes.len());
                    if toggle(ui, &label, self.sim_notes).clicked() {
                        self.sim_notes = !self.sim_notes;
                    }
                    if self.sim_notes {
                        for line in &outcome.notes {
                            note(ui, line, LEGEND);
                        }
                    }
                }
            },
        );
        ui.add_space(8.0);
        if run {
            self.run_sim(ui.ctx());
        }
        #[cfg(not(target_arch = "wasm32"))]
        if run_remote {
            self.run_remote(ui.ctx());
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn remote_card(&mut self, ui: &mut Ui) {
        if !self.remote.open || !self.shown.as_ref().is_some_and(|s| s.sim.is_some()) {
            return;
        }
        let rail = if self.sim_remote { READOUT } else { TRACE };
        card(
            ui,
            Some(rail),
            |ui| {
                Line::new()
                    .legend("sims server")
                    .value(if self.sim_remote { "running" } else { "" })
                    .show(ui);
            },
            |ui| self.remote.show(ui),
        );
        ui.add_space(8.0);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn run_remote(&mut self, ctx: &egui::Context) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let vars = self.vars.clone();
        let limits = self.remote.limits();
        let (tx, rx) = channel();
        let wake = ctx.clone();
        crate::host::spawn(move || {
            let result = crate::remote::simulate(&path, &vars, &limits, || {
                let run = {
                    let _stage = crate::progress::stage("building the model", None, "");
                    crate::model::run_path(&path, &vars, None)?
                };
                if let Some((line, Err(error))) = run.steps.last() {
                    anyhow::bail!("line {}: {error:#}", line.number);
                }
                Ok(run)
            });
            let _ = tx.send(result.map_err(|e| format!("{e:#}")));
            crate::host::wake(&wake);
        });
        self.sim_error = None;
        self.sim_remote = true;
        self.remote.refresh();
        self.sim_job = Some((rx, Instant::now(), self.activity.clone()));
    }

    fn run_sim(&mut self, ctx: &egui::Context) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let vars = self.vars.clone();
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        let built =
            (self.dirty.is_none() && self.loading.is_none() && self.program.error.is_none())
                .then(|| self.program.snapshots.last())
                .flatten()
                .filter(|snapshot| snapshot.result.is_ok())
                .and_then(|snapshot| snapshot.model.sim.clone());
        self.activity
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .rows
            .clear();
        crate::host::spawn(move || {
            let result = match built {
                Some(spec) => simulate_spec(&path, &vars, &spec),
                None => simulate_file(&path, &vars),
            };
            let _ = tx.send(result.map_err(|e| format!("{e:#}")));
            crate::host::wake(&ctx);
        });
        self.sim_error = None;
        self.sim_job = Some((rx, Instant::now(), self.activity.clone()));
    }

    fn measure_card(&mut self, ui: &mut Ui) {
        let Some(shown) = &self.shown else { return };
        let scene = shown.scene.clone();
        let mut clear = false;
        card(
            ui,
            Some(TRACE),
            |ui| {
                Line::new().legend("measure").show(ui);
            },
            |ui| {
                if self.picks.is_empty() {
                    hint(
                        ui,
                        "click two points to measure between them. Corners, midpoints, centres and edges snap",
                    );
                }
                let units = self.units;
                let wide = |p: V3| p.map(f64::from);
                for (n, pick) in self.picks.iter().enumerate() {
                    Line::new()
                        .legend(if n == 0 { "a" } else { "b" })
                        .value(units.lengths(&wide(pick.point), 3, ", "))
                        .note(pick.snap.name())
                        .size(11.0)
                        .show(ui);
                    if let Some(edge) = pick.edge.and_then(|e| scene.edges.get(e)) {
                        let text = match edge.circle {
                            Some((_, r)) if edge.closed() => {
                                format!("circle, diameter {}", units.length(2.0 * r as f64, 3))
                            }
                            Some((_, r)) => format!(
                                "arc, radius {}, length {}",
                                units.length(r as f64, 3),
                                units.length(edge.length() as f64, 3)
                            ),
                            None => {
                                format!("edge, length {}", units.length(edge.length() as f64, 3))
                            }
                        };
                        note(ui, text, LEGEND);
                    }
                    if pick.snap == Snap::Face
                        && let Some(info) = pick.face.and_then(|f| scene.faces.get(f))
                    {
                        let part = scene.parts.get(info.body).map_or("", |p| p.name.as_str());
                        note(
                            ui,
                            format!("{part}: {}, area {}", info.kind, units.area(info.area, 3)),
                            LEGEND,
                        );
                    }
                }
                if let [a, b] = self.picks[..] {
                    let d = wide(sub(b.point, a.point));
                    let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                    readouts(
                        ui,
                        &[
                            ("distance", units.length(length, 3), TRACE),
                            ("dx", units.length(d[0].abs(), 3), VALUE),
                            ("dy", units.length(d[1].abs(), 3), VALUE),
                            ("dz", units.length(d[2].abs(), 3), VALUE),
                        ],
                    );
                }
                if !self.picks.is_empty() && toggle(ui, "clear", false).clicked() {
                    clear = true;
                }
            },
        );
        if clear {
            self.picks.clear();
        }
        ui.add_space(8.0);
    }

    fn flow_card(&mut self, ui: &mut Ui) {
        let Some(field) = self.shown.as_ref().and_then(|s| s.field.clone()) else {
            return;
        };
        let titles: Vec<String> = field
            .slices
            .iter()
            .map(|s| {
                s.title
                    .trim_end_matches(" along the wind")
                    .trim_end_matches(" along the axis")
                    .to_string()
            })
            .collect();
        let moving = field.movie.is_some();
        let recorded = field.movie.as_ref().is_some_and(|m| !m.streams.is_empty());
        let fastest = crate::plot::flow_scale(&field.slices);
        let flow = &mut self.flow;
        card(
            ui,
            None,
            |ui| {
                Line::new().legend("flow display").show(ui);
            },
            |ui| {
                let slider = |ui: &mut Ui, value: &mut f32| {
                    ui.spacing_mut().slider_width = 90.0;
                    ui.add(egui::Slider::new(value, 0.05..=1.0).show_value(false))
                        .on_hover_text("opacity");
                };
                let layer = |ui: &mut Ui, name: &str, l: &mut Layer| {
                    ui.horizontal(|ui| {
                        if toggle(ui, name, l.on).clicked() {
                            l.on = !l.on;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            slider(ui, &mut l.opacity);
                        });
                    });
                };
                layer(
                    ui,
                    field.title.trim_start_matches("mean "),
                    &mut flow.surface,
                );
                Line::new()
                    .legend(&format!(
                        "{} to {} {}",
                        crate::plot::label(field.low.round()),
                        crate::plot::label(field.high.round()),
                        field.unit
                    ))
                    .size(11.0)
                    .show(ui);
                if !field.streams.is_empty() {
                    layer(ui, "streamlines", &mut flow.streams);
                    if recorded {
                        ui.horizontal(|ui| {
                            for (label, value) in [("mean", false), ("moving", true)] {
                                if toggle(ui, label, flow.lines_moving == value).clicked() {
                                    flow.lines_moving = value;
                                }
                            }
                        });
                    }
                    layer(ui, "particles", &mut flow.particles);
                    Line::new().legend("rank lines by").size(11.0).show(ui);
                    ui.horizontal(|ui| {
                        for (label, value) in [
                            ("all", Rank::All),
                            ("turbulence", Rank::Turbulence),
                            ("pressure", Rank::Pressure),
                        ] {
                            if toggle(ui, label, flow.rank == value).clicked() {
                                flow.rank = value;
                            }
                        }
                    });
                    if flow.rank != Rank::All {
                        ui.horizontal(|ui| {
                            for (label, value) in [("most", true), ("least", false)] {
                                if toggle(ui, label, flow.most == value).clicked() {
                                    flow.most = value;
                                }
                            }
                            ui.spacing_mut().slider_width = 90.0;
                            ui.add(
                                egui::Slider::new(&mut flow.share, 0.02..=1.0).show_value(false),
                            )
                            .on_hover_text("share of the lines shown");
                            Line::new()
                                .measured(format!("{:.0}%", flow.share * 100.0))
                                .size(11.0)
                                .show(ui);
                        });
                        let scores: Vec<f64> = field
                            .streams
                            .iter()
                            .filter(|s| s.points.len() > 1)
                            .map(|s| {
                                if flow.rank == Rank::Turbulence {
                                    s.turbulence * 100.0
                                } else {
                                    s.pressure
                                }
                            })
                            .collect();
                        let low = scores.iter().copied().fold(f64::INFINITY, f64::min);
                        let high = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                        if low.is_finite() {
                            Line::new()
                                .legend(&if flow.rank == Rank::Turbulence {
                                    format!("{low:.0} to {high:.0}% of the wind, peak along a line")
                                } else {
                                    format!("{low:.0} to {high:.0} Pa, strongest along a line")
                                })
                                .size(11.0)
                                .show(ui);
                        }
                    }
                }
                for (k, title) in titles.iter().enumerate().take(flow.slices.len()) {
                    let (show, opacity) = &mut flow.slices[k];
                    ui.horizontal(|ui| {
                        Line::new().legend(title).size(11.0).show(ui);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            slider(ui, opacity);
                        });
                    });
                    ui.horizontal(|ui| {
                        let mut choices = vec![
                            ("off", SliceShow::Off),
                            ("mean", SliceShow::Mean),
                            ("moment", SliceShow::Moment),
                        ];
                        if moving {
                            choices.push(("moving", SliceShow::Moving));
                        }
                        for (label, value) in choices {
                            if toggle(ui, label, *show == value).clicked() {
                                *show = value;
                            }
                        }
                    });
                }
                Line::new()
                    .legend(&format!(
                        "air speed 0 to {} m/s",
                        crate::plot::label(fastest.round())
                    ))
                    .size(11.0)
                    .show(ui);
                ui.add_space(4.0);
                Line::new().legend("playback").show(ui);
                ui.horizontal(|ui| {
                    if toggle(ui, "frames", flow.playing)
                        .on_hover_text("play or hold the frame; particles keep moving")
                        .clicked()
                    {
                        flow.playing = !flow.playing;
                    }
                    if toggle(ui, "back", false).clicked() {
                        flow.playing = false;
                        flow.clock = frame_middle((flow.clock * REEL_RATE).floor() - 1.0);
                    }
                    if toggle(ui, "next", false).clicked() {
                        flow.playing = false;
                        flow.clock = frame_middle((flow.clock * REEL_RATE).floor() + 1.0);
                    }
                    ui.spacing_mut().slider_width = 110.0;
                    ui.add(
                        egui::Slider::new(&mut flow.speed, 0.05..=4.0)
                            .logarithmic(true)
                            .show_value(false),
                    )
                    .on_hover_text("playback speed");
                    Line::new()
                        .measured(format!("{:.2}x", flow.speed))
                        .size(11.0)
                        .show(ui);
                });
                if let Some(movie) = &field.movie {
                    let frames = movie
                        .planes
                        .iter()
                        .map(|r| r.frames.len())
                        .max()
                        .unwrap_or(1)
                        .max(1);
                    let length = frames as f64 / REEL_RATE;
                    let mut at = flow.clock.rem_euclid(length);
                    ui.spacing_mut().slider_width = ui.available_width() - 10.0;
                    if ui
                        .add(egui::Slider::new(&mut at, 0.0..=length).show_value(false))
                        .on_hover_text("time")
                        .changed()
                    {
                        flow.clock = at;
                    }
                    let frame = (at * REEL_RATE) as usize % frames;
                    Line::new()
                        .measured(format!(
                            "{:.4} s of air, frame {} of {}",
                            frame as f64 * movie.seconds,
                            frame + 1,
                            frames
                        ))
                        .size(11.0)
                        .show(ui);
                }
            },
        );
        ui.add_space(8.0);
    }

    fn draw_reels(
        &mut self,
        p: &egui::Painter,
        projector: &Projector,
        field: &crate::sim::statics::FieldView,
        ctx: &egui::Context,
        rect: Rect,
    ) {
        let Some(movie) = &field.movie else { return };
        if self.reels.len() != movie.planes.len() {
            self.reels = vec![None; movie.planes.len()];
        }
        let fastest = crate::plot::flow_scale(&field.slices);
        let palette: Vec<Color32> = (0..256)
            .map(|q| {
                if q == 255 {
                    Color32::from_gray(150)
                } else {
                    let c = crate::plot::heat(q as f64 / 254.0 * movie.top / fastest);
                    Color32::from_rgb(
                        (c[0] * 255.0) as u8,
                        (c[1] * 255.0) as u8,
                        (c[2] * 255.0) as u8,
                    )
                }
            })
            .collect();
        let mut shown_time = None;
        for (k, (reel, slice)) in movie.planes.iter().zip(&field.slices).enumerate() {
            let (show, opacity) = self.flow.slice(k);
            if show != SliceShow::Moving || reel.frames.is_empty() {
                continue;
            }
            let frame = frame_at(self.flow.clock, reel.frames.len());
            shown_time = Some(frame as f64 * movie.seconds);
            let fresh = !matches!(&self.reels[k], Some((at, _)) if *at == frame);
            if fresh {
                let pixels: Vec<Color32> = reel.frames[frame]
                    .iter()
                    .map(|q| palette[*q as usize])
                    .collect();
                let image = egui::ColorImage::new([reel.width, reel.height], pixels);
                match &mut self.reels[k] {
                    Some((at, handle)) => {
                        handle.set(image, egui::TextureOptions::LINEAR);
                        *at = frame;
                    }
                    None => {
                        let handle = ctx.load_texture(
                            format!("reel {k}"),
                            image,
                            egui::TextureOptions::LINEAR,
                        );
                        self.reels[k] = Some((frame, handle));
                    }
                }
            }
            let Some((_, handle)) = &self.reels[k] else {
                continue;
            };
            let n = 16;
            let (w, h) = (slice.width as f64, slice.height() as f64);
            let mut mesh = egui::epaint::Mesh::with_texture(handle.id());
            let tint = Color32::WHITE.gamma_multiply(opacity);
            let mut fine = true;
            for j in 0..=n {
                for i in 0..=n {
                    let (u, v) = (i as f64 / n as f64, j as f64 / n as f64);
                    let (c, r) = (-0.5 + w * u, -0.5 + h * v);
                    let at = [0, 1, 2]
                        .map(|d| (slice.origin[d] + c * slice.right[d] + r * slice.down[d]) as f32);
                    match projector.project(at) {
                        Some(screen) => mesh.vertices.push(egui::epaint::Vertex {
                            pos: screen,
                            uv: Pos2::new(u as f32, v as f32),
                            color: tint,
                        }),
                        None => fine = false,
                    }
                }
            }
            if !fine {
                continue;
            }
            let row = n as u32 + 1;
            for j in 0..n as u32 {
                for i in 0..n as u32 {
                    let a = j * row + i;
                    mesh.indices.extend_from_slice(&[
                        a,
                        a + 1,
                        a + row,
                        a + 1,
                        a + row + 1,
                        a + row,
                    ]);
                }
            }
            p.add(egui::Shape::mesh(mesh));
        }
        if let Some(t) = shown_time {
            p.text(
                rect.left_bottom() + Vec2::new(10.0, -28.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{t:.4} s"),
                egui::FontId::monospace(13.0),
                Color32::from_gray(230),
            );
        }
    }

    fn parts_card(&mut self, ui: &mut Ui) {
        let Some(shown) = &self.shown else { return };
        let scene = shown.scene.clone();
        let joints = shown.joints.clone();
        let rig = shown.rig.clone();
        let exploded = !shown.explode.is_empty();
        let solids = shown.solids.clone();
        let (min, max) = shown.bounds;
        let faces = shown.faces;
        let units = self.units;
        let mut show_all = false;
        let changed = !self.modes.is_empty();
        card(
            ui,
            None,
            |ui| {
                Line::new().legend("parts").show(ui);
                if changed {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        show_all = toggle(ui, "show all", false).clicked();
                    });
                }
            },
            |ui| {
                let mut focus = None;
                for (body, part) in scene.parts.iter().enumerate() {
                    let mode = self.mode(&part.name);
                    let focused = self.focus.as_ref().is_some_and(|f| f.part == part.name);
                    ui.horizontal(|ui| {
                        let (rect, swatch) =
                            ui.allocate_exact_size(Vec2::splat(12.0), Sense::click());
                        let [r, g, b] = part.colour.map(|c| (c.clamp(0.0, 1.0) * 255.0) as u8);
                        ui.painter()
                            .rect_filled(rect, 2.0, Color32::from_rgb(r, g, b));
                        if focused || swatch.hovered() {
                            ui.painter().rect_stroke(
                                rect.expand(2.0),
                                2.0,
                                Stroke::new(1.5, READOUT),
                                egui::StrokeKind::Outside,
                            );
                        }
                        if swatch.clicked() {
                            focus = Some((body, part.name.clone()));
                        }
                        let short = if part.name.chars().count() > 28 {
                            format!("{}…", part.name.chars().take(27).collect::<String>())
                        } else {
                            part.name.clone()
                        };
                        if toggle(ui, &short, mode != Mode::Hidden)
                            .on_hover_text(&part.name)
                            .clicked()
                        {
                            let next = if mode == Mode::Hidden {
                                Mode::Solid
                            } else {
                                Mode::Hidden
                            };
                            self.set_mode(&part.name, next);
                        }
                        if ghost_button(ui, mode == Mode::Ghost).clicked() {
                            let next = if mode == Mode::Ghost {
                                Mode::Solid
                            } else {
                                Mode::Ghost
                            };
                            self.set_mode(&part.name, next);
                        }
                        let mut opacity = self
                            .opacities
                            .get(&part.name)
                            .copied()
                            .unwrap_or(part.opacity);
                        ui.spacing_mut().slider_width = 56.0;
                        if ui
                            .add(egui::Slider::new(&mut opacity, 0.05..=1.0).show_value(false))
                            .on_hover_text("opacity")
                            .changed()
                        {
                            self.opacities.insert(part.name.clone(), opacity);
                            if mode == Mode::Ghost {
                                self.set_mode(&part.name, Mode::Solid);
                            }
                        }
                        Line::new()
                            .measured(units.volume(part.volume, 1))
                            .size(11.0)
                            .show(ui);
                    });
                }
                if let Some((body, part)) = focus {
                    self.focus_on(body, part);
                }
                ui.horizontal(|ui| {
                    Line::new().legend("ghost opacity").size(11.0).show(ui);
                    ui.add(egui::Slider::new(&mut self.opacity, 0.05..=0.95).show_value(false));
                });
                Line::new()
                    .legend("faces")
                    .measured(faces.to_string())
                    .legend("size")
                    .measured(units.lengths(
                        &[max[0] - min[0], max[1] - min[1], max[2] - min[2]],
                        1,
                        " x ",
                    ))
                    .size(11.0)
                    .show(ui);
                if exploded {
                    ui.horizontal(|ui| {
                        Line::new().legend("explode").size(11.0).show(ui);
                        ui.add(egui::Slider::new(&mut self.explode, 0.0..=1.0).show_value(false));
                    });
                }
            },
        );
        if show_all {
            self.modes.clear();
        }
        ui.add_space(8.0);
        if !joints.iter().any(Joint::movable) {
            return;
        }
        let mut check = false;
        let key = self.shown.as_ref().map(|s| s.key.clone());
        let current = self
            .locks
            .as_ref()
            .filter(|(k, values, _)| Some(k) == key.as_ref() && *values == self.joint_values);
        if current.is_none()
            && self.lock_job.is_none()
            && let Some(key) = key
        {
            let (tx, rx) = channel();
            let ctx = ui.ctx().clone();
            let rig = rig.clone();
            let values = self.joint_values.clone();
            crate::host::spawn(move || {
                let locks = locked_joints(&rig, &values);
                let _ = tx.send((key, values, locks));
                crate::host::wake(&ctx);
            });
            self.lock_job = Some(rx);
        }
        let locks: Vec<Option<String>> = self
            .locks
            .as_ref()
            .map(|(_, _, locks)| locks.clone())
            .unwrap_or_default();
        card(
            ui,
            None,
            |ui| {
                Line::new().legend("joints").show(ui);
            },
            |ui| {
                for (i, joint) in joints.iter().enumerate() {
                    if !joint.movable() || i >= self.joint_values.len() {
                        continue;
                    }
                    let (low, high) = joint.range;
                    let driver = rig.driver_of(&joint.name).map(|c| c.driver.clone());
                    let stuck = locks.get(i).cloned().flatten();
                    Line::new()
                        .legend(&joint.name)
                        .note(format!("{} on {}", joint.child, joint.parent))
                        .size(11.0)
                        .show(ui);
                    let mut value = self.joint_values[i];
                    let slider = egui::Slider::new(&mut value, low..=high)
                        .clamping(egui::SliderClamping::Always);
                    let slider = match joint.kind {
                        JointKind::Slide { .. } => slider
                            .custom_formatter(move |v, _| units.length(v, 2))
                            .custom_parser(move |text| {
                                let bare = text.trim().trim_end_matches(units.name()).trim();
                                bare.parse::<f64>().ok().map(|v| v / units.per_mm())
                            }),
                        _ => slider.suffix(joint.unit()),
                    };
                    ui.add_enabled(stuck.is_none() && driver.is_none(), slider);
                    if value != self.joint_values[i]
                        && let Ok((settled, _)) = rig.drive(&self.joint_values, i, value)
                    {
                        self.joint_values = settled;
                    }
                    if let Some(driver) = driver {
                        note(ui, format!("driven by {driver}"), LEGEND);
                    } else if let Some(why) = stuck {
                        note(ui, format!("locked: {why}"), WARN);
                    }
                }
                ui.horizontal(|ui| {
                    if toggle(ui, "as file", false).clicked() {
                        self.joint_values = joints.iter().map(|j| j.value).collect();
                    }
                    if toggle(ui, "check clashes", self.clash_job.is_some()).clicked()
                        && self.clash_job.is_none()
                    {
                        check = true;
                    }
                });
                match &self.clashes {
                    _ if self.clash_job.is_some() => note(ui, "checking", LEGEND),
                    Some(Ok(found)) if found.is_empty() => note(ui, "no parts overlap here", OK),
                    Some(Ok(found)) => found.iter().for_each(|f| note(ui, f, FAULT)),
                    Some(Err(())) => note(ui, "the check failed", FAULT),
                    None => {}
                }
            },
        );
        if check {
            let values = self.joint_values.clone();
            let (tx, rx) = channel();
            let ctx = ui.ctx().clone();
            crate::host::spawn(move || {
                let moved = posed(&joints, &values);
                let bare: Vec<&Solid> = solids.iter().map(|(_, s)| s).collect();
                let meshes: Vec<Arc<geometry::Meshed>> = geometry::meshed_all(&bare)
                    .into_iter()
                    .zip(solids.iter())
                    .map(|(mesh, (name, _))| match moved.get(name) {
                        Some(m) => Arc::new(mesh.transformed(*m)),
                        None => mesh,
                    })
                    .collect();
                let found: Vec<String> = geometry::clashes(&meshes)
                    .into_iter()
                    .map(|(i, j, shared)| {
                        format!(
                            "{} and {} share about {}",
                            solids[i].0,
                            solids[j].0,
                            units.volume(shared, 3)
                        )
                    })
                    .collect();
                let _ = tx.send(found);
                crate::host::wake(&ctx);
            });
            self.clash_job = Some(rx);
            self.clashes = None;
        }
        ui.add_space(8.0);
    }

    fn select_card(&mut self, ui: &mut Ui) {
        card(
            ui,
            None,
            |ui| {
                Line::new().legend("select").show(ui);
            },
            |ui| {
                field(ui, &mut self.query, "base.end&base.side");
                match self.shown.as_ref().and_then(|s| s.query.clone()) {
                    Some(Ok((faces, edges))) => {
                        Line::new()
                            .legend("faces")
                            .measured(faces.to_string())
                            .legend("edges")
                            .measured(edges.to_string())
                            .show(ui);
                    }
                    Some(Err(error)) => note(ui, error, FAULT),
                    None => hint(
                        ui,
                        "faces: label, label.group, >Z, +X, all, a,b. edges: faces, a&b, x|y",
                    ),
                }
            },
        );
    }

    fn inspector(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.line_card(ui);
                self.sim_card(ui);
                #[cfg(not(target_arch = "wasm32"))]
                self.remote_card(ui);
                if self.render_open {
                    self.render_card(ui);
                }
                if self.section.is_some() {
                    self.section_card(ui);
                }
                if self.measuring {
                    self.measure_card(ui);
                }
                if self
                    .shown
                    .as_ref()
                    .and_then(|s| s.field.as_ref())
                    .is_some_and(|f| is_flow(f))
                {
                    self.flow_card(ui);
                } else {
                    self.parts_card(ui);
                }
                if !self.shown.as_ref().is_some_and(|s| s.assembly) {
                    self.select_card(ui);
                }
            });
    }

    fn cut(&self) -> Option<Cut> {
        let (axis, at, flip) = self.section?;
        let scene = &self.shown.as_ref()?.scene.mesh;
        let low = scene.centre[axis] - scene.radius;
        let place = low + 2.0 * scene.radius * at;
        let mut normal = [0.0f32; 3];
        normal[axis] = if flip { -1.0 } else { 1.0 };
        Some(Cut {
            normal,
            offset: if flip { -place } else { place },
        })
    }

    fn render_card(&mut self, ui: &mut Ui) {
        card(
            ui,
            None,
            |ui| {
                Line::new().legend("render").show(ui);
            },
            |ui| {
                viewer3d::mode_controls(ui, &mut self.mode);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if toggle(ui, "ray traced", self.traced).clicked() {
                        self.traced = !self.traced;
                    }
                    if toggle(ui, "edges", self.edges).clicked() {
                        self.edges = !self.edges;
                    }
                    if self.traced {
                        note(ui, trace_state(self.painted), LEGEND);
                    }
                });
                viewer3d::trace_controls(ui, &mut self.trace);
                ui.add_space(6.0);
                if viewer3d::preset_controls(ui, &mut self.preset) {
                    self.lighting = self.preset.lighting();
                }
                egui::CollapsingHeader::new(legend("lights"))
                    .id_salt("render lights")
                    .show(ui, |ui| {
                        viewer3d::lighting_controls(ui, &mut self.lighting);
                        if toggle(ui, "reset to preset", false).clicked() {
                            self.lighting = self.preset.lighting();
                        }
                    });
            },
        );
        ui.add_space(8.0);
    }

    fn section_card(&mut self, ui: &mut Ui) {
        let Some((mut axis, mut at, mut flip)) = self.section else {
            return;
        };
        let place = self.shown.as_ref().map(|shown| {
            let scene = &shown.scene.mesh;
            scene.centre[axis] - scene.radius + 2.0 * scene.radius * at
        });
        card(
            ui,
            None,
            |ui| {
                Line::new().legend("section").show(ui);
            },
            |ui| {
                ui.horizontal(|ui| {
                    for (k, name) in ["x", "y", "z"].iter().enumerate() {
                        if toggle(ui, name, axis == k).clicked() {
                            axis = k;
                        }
                    }
                    if toggle(ui, "flip", flip).clicked() {
                        flip = !flip;
                    }
                });
                ui.add(egui::Slider::new(&mut at, 0.0..=1.0).show_value(false));
                if let Some(place) = place {
                    note(
                        ui,
                        format!(
                            "cut at {}={}, showing {}",
                            ["x", "y", "z"][axis],
                            self.units.length(place as f64, 2),
                            if flip { "above" } else { "below" }
                        ),
                        LEGEND,
                    );
                }
            },
        );
        ui.add_space(8.0);
        self.section = Some((axis, at, flip));
    }

    fn viewer(&self, shown: &Shown) -> Viewer<Ink> {
        Viewer::new(shown.scene.mesh.clone(), self.cam)
            .looks(self.looks())
            .cut(self.cut())
            .lighting(self.lighting)
            .trace(self.traced.then_some(self.trace))
            .mode(self.mode)
            .edges(self.edges)
    }

    fn pick(&self, viewer: &Viewer<Ink>, rect: Rect, at: Pos2) -> Option<Pick> {
        let scene = &self.shown.as_ref()?.scene;
        let projector = viewer.projector(rect);
        let cut = self.cut();
        let hit = viewer.pick(rect, at);
        let face = hit.as_ref().map(|h| h.surface);
        let slack = viewer.scene().radius * 0.01;
        let in_front = |p: V3, of: V3| dot(sub(p, of), projector.forward) <= slack;
        let seen = |p: V3| {
            cut.is_none_or(|c| c.keeps(p))
                && (hit.as_ref().is_some_and(|h| in_front(p, h.point))
                    || projector
                        .project(p)
                        .is_some_and(|s| viewer.pick(rect, s).is_none_or(|h| in_front(p, h.point))))
        };
        let reach = |p: V3, most: f32| {
            projector
                .project(p)
                .map(|s| s.distance(at))
                .filter(|d| *d < most)
        };
        let shown_edges = || {
            scene
                .edges
                .iter()
                .enumerate()
                .filter(|(_, e)| viewer.look(e.body).visible())
        };
        let corners = scene
            .vertices
            .iter()
            .filter(|(body, _)| viewer.look(*body).visible())
            .map(|(body, p)| (viewer.look(*body).place(*p), Snap::Corner, None));
        let marks = shown_edges().flat_map(|(i, e)| {
            let look = viewer.look(e.body);
            let centre = e
                .circle
                .map(|(c, _)| (look.place(c), Snap::Centre, Some(i)));
            let middle = (!e.closed())
                .then(|| e.middle())
                .flatten()
                .map(|m| (look.place(m), Snap::Middle, Some(i)));
            centre.into_iter().chain(middle)
        });
        let mut points: Vec<(f32, V3, Snap, Option<usize>)> = corners
            .chain(marks)
            .filter_map(|(p, snap, edge)| Some((reach(p, POINT_SNAP)?, p, snap, edge)))
            .collect();
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, point, snap, edge)) = points.into_iter().take(4).find(|c| seen(c.1)) {
            return Some(Pick {
                point,
                face,
                snap,
                edge,
            });
        }
        let mut along: Vec<(f32, V3, usize)> = shown_edges()
            .filter_map(|(i, e)| {
                let look = viewer.look(e.body);
                e.points
                    .windows(2)
                    .filter_map(|w| {
                        let (a, b) = (look.place(w[0]), look.place(w[1]));
                        let (sa, sb) = (projector.project(a)?, projector.project(b)?);
                        let t = closest_along(sa, sb, at);
                        let d = sa.lerp(sb, t).distance(at);
                        (d < EDGE_SNAP).then(|| (d, lerp3(a, b, t), i))
                    })
                    .min_by(|x, y| x.0.total_cmp(&y.0))
            })
            .collect();
        along.sort_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, point, edge)) = along.into_iter().take(4).find(|c| seen(c.1)) {
            return Some(Pick {
                point,
                face,
                snap: Snap::Edge,
                edge: Some(edge),
            });
        }
        let hit = hit?;
        Some(Pick {
            point: hit.point,
            face: Some(hit.surface),
            snap: Snap::Face,
            edge: None,
        })
    }

    fn focus_on(&mut self, body: usize, part: String) {
        if self.focus.as_ref().is_some_and(|f| f.part == part) {
            self.focus = None;
            return;
        }
        let Some(shown) = &self.shown else { return };
        let looks = self.looks();
        let points: Vec<V3> = shown
            .scene
            .vertices
            .iter()
            .filter(|(b, _)| *b == body)
            .map(|(_, p)| looks.get(body).map_or(*p, |l| l.place(*p)))
            .collect();
        let Some(&first) = points.first() else { return };
        let (low, high) = points.iter().fold((first, first), |(lo, hi), p| {
            (
                [lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])],
                [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])],
            )
        });
        self.focus = Some(Focus {
            part,
            at: [
                (low[0] + high[0]) / 2.0,
                (low[1] + high[1]) / 2.0,
                (low[2] + high[2]) / 2.0,
            ],
        });
    }

    fn focus_popup(&mut self, ui: &Ui, rect: Rect, projector: &Projector, scene: &Scene) {
        let Some(focus) = &self.focus else { return };
        let Some(part) = scene.parts.iter().find(|p| p.name == focus.part) else {
            self.focus = None;
            return;
        };
        let Some(anchor) = projector.project(focus.at).filter(|p| rect.contains(*p)) else {
            return;
        };
        ui.painter_at(rect)
            .circle_stroke(anchor, 4.0, Stroke::new(1.5, READOUT));
        let mode = self.mode(&part.name);
        let units = self.units;
        let mut chosen = None;
        let mut close = false;
        egui::Area::new(ui.id().with("focus"))
            .order(egui::Order::Foreground)
            .fixed_pos(anchor + Vec2::new(14.0, 14.0))
            .constrain_to(rect)
            .show(ui.ctx(), |ui| {
                egui::Frame::NONE
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, ETCH))
                    .corner_radius(theme::RADIUS)
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.set_max_width(300.0);
                        ui.spacing_mut().item_spacing.y = 4.0;
                        ui.horizontal(|ui| {
                            let (swatch, _) =
                                ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                            let [r, g, b] = part.colour.map(|c| (c.clamp(0.0, 1.0) * 255.0) as u8);
                            ui.painter()
                                .rect_filled(swatch, 2.0, Color32::from_rgb(r, g, b));
                            Line::new().value(&part.name).show(ui);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add(
                                            egui::Label::new(
                                                egui::RichText::new("x").color(LEGEND),
                                            )
                                            .sense(Sense::click()),
                                        )
                                        .clicked()
                                    {
                                        close = true;
                                    }
                                },
                            );
                        });
                        if let Some(source) = &part.source {
                            note(ui, source, LEGEND);
                        }
                        let [x, y, z] = part.size;
                        Line::new()
                            .legend("volume")
                            .measured(units.volume(part.volume, 1))
                            .size(11.0)
                            .show(ui);
                        Line::new()
                            .legend("size")
                            .measured(units.lengths(&[x, y, z], 1, " x "))
                            .size(11.0)
                            .show(ui);
                        if let Some((name, density)) = &part.material {
                            Line::new()
                                .legend("material")
                                .value(name)
                                .legend("mass")
                                .measured(units.mass(part.volume * density / 1000.0))
                                .size(11.0)
                                .show(ui);
                        }
                        ui.horizontal(|ui| {
                            for (label, option) in [
                                ("show", Mode::Solid),
                                ("ghost", Mode::Ghost),
                                ("hide", Mode::Hidden),
                            ] {
                                if toggle(ui, label, mode == option).clicked() {
                                    chosen = Some(option);
                                }
                            }
                        });
                        if mode == Mode::Ghost {
                            ui.horizontal(|ui| {
                                Line::new().legend("opacity").size(11.0).show(ui);
                                ui.add(
                                    egui::Slider::new(&mut self.opacity, 0.05..=0.95)
                                        .show_value(false),
                                );
                            });
                        }
                    });
            });
        let name = part.name.clone();
        if let Some(mode) = chosen {
            self.set_mode(&name, mode);
        }
        if close {
            self.focus = None;
        }
    }

    fn view_cube(&mut self, ui: &Ui, rect: Rect, projector: &Projector) -> bool {
        let size = 34.0;
        let (cell, gap) = (Vec2::new(40.0, 18.0), 2.0);
        let grid = 4.0 * cell.x + 3.0 * gap;
        let centre = Pos2::new(rect.right() - 12.0 - grid / 2.0, rect.top() + 70.0);
        let screen = |v: V3| {
            Pos2::new(
                centre.x + dot(v, projector.right) * size,
                centre.y - dot(v, projector.up) * size,
            )
        };
        let toward = [
            -projector.forward[0],
            -projector.forward[1],
            -projector.forward[2],
        ];
        let mut visible: Vec<(f32, Vec<Pos2>, V3, &str)> = CUBE_FACES
            .iter()
            .filter(|(n, _)| dot(*n, toward) > 0.05)
            .map(|(n, label)| {
                let (u, w) = if n[2].abs() > 0.5 {
                    ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
                } else if n[0].abs() > 0.5 {
                    ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0])
                } else {
                    ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0])
                };
                let corner = |a: f32, b: f32| {
                    screen([
                        n[0] + u[0] * a + w[0] * b,
                        n[1] + u[1] * a + w[1] * b,
                        n[2] + u[2] * a + w[2] * b,
                    ])
                };
                let polygon = vec![
                    corner(-1.0, -1.0),
                    corner(1.0, -1.0),
                    corner(1.0, 1.0),
                    corner(-1.0, 1.0),
                ];
                (dot(*n, toward), polygon, *n, *label)
            })
            .collect();
        visible.sort_by(|a, b| a.0.total_cmp(&b.0));
        let area = Rect::from_center_size(centre, Vec2::splat(size * 3.6));
        let resp = ui.interact(area, ui.id().with("view cube"), Sense::click());
        let hover = resp.hover_pos();
        let p = ui.painter_at(rect);
        let mut chosen = None;
        for (facing, polygon, normal, label) in &visible {
            let lit = hover.is_some_and(|h| inside(polygon, h));
            let shade = (60.0 + 90.0 * facing) as u8;
            let fill = if lit {
                READOUT.gamma_multiply(0.55)
            } else {
                Color32::from_rgb(shade / 2 + 20, shade / 2 + 24, shade / 2 + 30)
            };
            p.add(egui::Shape::convex_polygon(
                polygon.clone(),
                fill,
                Stroke::new(1.0, ETCH),
            ));
            if *facing > 0.35 {
                let mid = polygon.iter().fold(Vec2::ZERO, |acc, q| acc + q.to_vec2()) / 4.0;
                p.text(
                    mid.to_pos2(),
                    egui::Align2::CENTER_CENTER,
                    *label,
                    egui::FontId::proportional(9.0),
                    VALUE,
                );
            }
            if resp.clicked() && lit {
                chosen = Some(*normal);
            }
        }
        if let Some(n) = chosen {
            self.cam.yaw = if n[2].abs() > 0.5 {
                0.0
            } else {
                n[0].atan2(-n[1])
            };
            self.cam.pitch = (n[2] * 1.5699).clamp(-1.5699, 1.5699);
        }
        let top_left = Pos2::new(centre.x - grid / 2.0, centre.y + size * 1.9 + 4.0);
        let mut over_buttons = false;
        for (slot, view) in VIEWS.into_iter().enumerate() {
            let (column, row) = ((slot % 4) as f32, (slot / 4) as f32);
            let button = Rect::from_min_size(
                top_left + Vec2::new(column * (cell.x + gap), row * (cell.y + gap)),
                cell,
            );
            let name = view.map_or("ortho", |(name, ..)| name);
            let mut r = ui.interact(button, ui.id().with(("view", name)), Sense::click());
            over_buttons |= r.hovered();
            let on = view.is_none() && self.cam.ortho;
            let (fill, ink) = match (on, r.hovered()) {
                (true, _) => (READOUT.gamma_multiply(0.18), READOUT),
                (false, true) => (BAND, READOUT),
                (false, false) => (PANEL, LEGEND),
            };
            p.rect_filled(button, 2.0, fill);
            if on {
                p.rect_stroke(
                    button,
                    2.0,
                    Stroke::new(1.0, READOUT),
                    egui::StrokeKind::Inside,
                );
            }
            p.text(
                button.center(),
                egui::Align2::CENTER_CENTER,
                name,
                egui::FontId::proportional(10.0),
                ink,
            );
            if view.is_none() {
                r = r.on_hover_text(if self.cam.ortho {
                    "orthographic: parallel lines stay parallel. Click for perspective"
                } else {
                    "perspective. Click for orthographic, where parallel lines stay parallel"
                });
            }
            if !r.clicked() {
                continue;
            }
            match view {
                None => self.cam.ortho = !self.cam.ortho,
                Some((name, yaw, pitch)) => {
                    self.cam.yaw = yaw;
                    self.cam.pitch = pitch;
                    if name == "iso" {
                        self.cam.zoom = 1.0;
                        self.cam.shift = [0.0; 3];
                    }
                }
            }
        }
        resp.hovered() || resp.clicked() || over_buttons
    }

    fn snap_preview(
        &self,
        p: &egui::Painter,
        projector: &Projector,
        viewer: &Viewer<Ink>,
        scene: &Scene,
        pick: Pick,
    ) {
        if let Some(edge) = pick.edge.and_then(|e| scene.edges.get(e)) {
            let look = viewer.look(edge.body);
            let line: Vec<Pos2> = edge
                .points
                .iter()
                .filter_map(|q| projector.project(look.place(*q)))
                .collect();
            p.add(egui::Shape::line(
                line,
                Stroke::new(2.0, TRACE.gamma_multiply(0.8)),
            ));
        }
        let Some(at) = projector.project(pick.point) else {
            return;
        };
        snap_mark(p, at, pick.snap, TRACE);
        let label = p.layout_no_wrap(
            pick.snap.name().to_string(),
            egui::FontId::proportional(11.0),
            TRACE,
        );
        let corner = at + Vec2::new(12.0, 10.0);
        p.rect_filled(
            Rect::from_min_size(corner, label.size()).expand2(Vec2::new(4.0, 2.0)),
            2.0,
            BACKGROUND.gamma_multiply(0.9),
        );
        p.galley(corner, label, TRACE);
    }

    fn fps(&self) -> Option<f32> {
        let last = *self.frames.back()?;
        let recent: Vec<Instant> = self
            .frames
            .iter()
            .copied()
            .filter(|t| last.duration_since(*t) < Duration::from_secs(1))
            .collect();
        let span = last.duration_since(*recent.first()?).as_secs_f32();
        (recent.len() > 2 && span > 0.0).then(|| (recent.len() - 1) as f32 / span)
    }

    fn timings(&self, scene: &Scene, built: Duration) -> String {
        let triangles = scene.mesh.triangles();
        let segments = scene.mesh.segments();
        let fps = self
            .fps()
            .map_or_else(|| "idle".to_string(), |fps| format!("{fps:.0} fps"));
        format!(
            "{fps}  draw {:.1} ms  scene {:.0} ms  build {:.2} s  {:.1}k tris  {:.1}k edges",
            viewer3d::draw_time().as_secs_f32() * 1000.0,
            built.as_secs_f32() * 1000.0,
            self.took.as_secs_f32(),
            triangles as f32 / 1000.0,
            segments as f32 / 1000.0,
        )
    }

    fn activity_panel(&self, ctx: &egui::Context, at: Pos2, pivot: egui::Align2) -> bool {
        let activity = self.activity.lock().unwrap_or_else(|e| e.into_inner());
        if activity.rows.is_empty() {
            return false;
        }
        egui::Area::new(egui::Id::new("building progress"))
            .fixed_pos(at)
            .pivot(pivot)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(BACKGROUND.gamma_multiply(0.92))
                    .stroke(egui::Stroke::new(1.0, ETCH))
                    .corner_radius(theme::RADIUS)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.set_width(340.0);
                        activity.show(ui);
                    });
            });
        true
    }

    fn viewport(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let p = ui.painter_at(rect);
        let Some(shown) = &self.shown else {
            p.rect_filled(rect, 0.0, BACKGROUND);
            let status = crate::host::status();
            if status.is_none()
                && (self.loading.is_some() || self.building.is_some())
                && self.activity_panel(ui.ctx(), rect.center(), egui::Align2::CENTER_CENTER)
            {
                return;
            }
            let message = match (
                &self.path,
                self.loading.is_some() || self.building.is_some(),
            ) {
                _ if status.is_some() => status.as_deref().unwrap_or_default(),
                (None, _) => "open a .gcad part or .gasm assembly",
                (_, true) => "building",
                _ => "no solid at this line",
            };
            p.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                message,
                egui::FontId::proportional(13.0),
                LEGEND,
            );
            return;
        };
        let scene = shown.scene.clone();
        let built = shown.built;
        let mut viewer = self.viewer(shown);
        self.painted = viewer.paint(ui, rect);
        let projector = viewer.projector(rect);
        if let Some(field) = self.shown.as_ref().and_then(|s| s.field.clone())
            && is_flow(&field)
        {
            self.flow.lines_recorded = field.movie.as_ref().is_some_and(|m| !m.streams.is_empty());
            let dt = ui.input(|i| i.stable_dt) as f64 * self.flow.speed as f64;
            if self.flow.playing {
                self.flow.clock += dt;
            }
            if self.flow.particles.on {
                self.flow.drift += dt;
            }
            if self.flow.playing || self.flow.particles.on {
                ui.ctx().request_repaint();
            }
            self.draw_reels(&p, &projector, &field, ui.ctx(), rect);
            let mask = chosen(
                &field.streams,
                self.flow.rank,
                self.flow.most,
                self.flow.share,
            );
            let fastest = crate::plot::flow_scale(&field.slices);
            if self.flow.lines_recorded
                && self.flow.lines_moving
                && self.flow.streams.on
                && let Some(movie) = &field.movie
            {
                let frame = frame_at(self.flow.clock, movie.streams.len());
                moving_lines(
                    &p,
                    &projector,
                    &movie.streams[frame],
                    mask.as_deref(),
                    fastest,
                    self.flow.streams.opacity,
                );
            } else if mask.is_some() && self.flow.streams.on {
                moving_lines(
                    &p,
                    &projector,
                    &field.streams,
                    mask.as_deref(),
                    fastest,
                    self.flow.streams.opacity,
                );
            }
            if self.flow.particles.on && !field.streams.is_empty() {
                let frame = field
                    .movie
                    .as_ref()
                    .filter(|m| !m.streams.is_empty() && self.flow.lines_moving)
                    .map(|m| {
                        let k = frame_at(self.flow.clock, m.streams.len());
                        m.streams[k].as_slice()
                    });
                particles(
                    &p,
                    &projector,
                    &field,
                    frame,
                    mask.as_deref(),
                    self.flow.drift,
                    self.flow.particles.opacity,
                );
            }
        }
        let on_cube = self.view_cube(ui, rect, &projector);
        if !on_cube {
            viewer.navigate(ui, &resp, &mut self.orbit);
            self.cam = viewer.camera();
            if resp.double_clicked() {
                self.cam = Camera {
                    ortho: self.cam.ortho,
                    ..Camera::default()
                };
            } else if self.measuring
                && resp.clicked()
                && let Some(at) = resp.interact_pointer_pos()
                && let Some(pick) = self.pick(&viewer, rect, at)
            {
                if self.picks.len() >= 2 {
                    self.picks.clear();
                }
                self.picks.push(pick);
            } else if !self.measuring
                && resp.clicked()
                && let Some(at) = resp.interact_pointer_pos()
            {
                let hit = viewer.pick(rect, at).and_then(|hit| {
                    let face = scene.faces.get(hit.surface)?;
                    Some((face.body, hit.point))
                });
                self.focus = hit.and_then(|(body, point)| {
                    let part = scene.parts.get(body)?.name.clone();
                    Some(Focus { part, at: point })
                });
            }
        }
        let hover = if self.measuring && !resp.dragged() && !on_cube {
            resp.hover_pos().and_then(|at| self.pick(&viewer, rect, at))
        } else {
            None
        };
        let projector = Projector::new(&scene.mesh, &self.cam, rect);
        if let Some(pick) = hover {
            self.snap_preview(&p, &projector, &viewer, &scene, pick);
        }
        let points: Vec<Pos2> = self
            .picks
            .iter()
            .filter_map(|pick| projector.project(pick.point))
            .collect();
        if let [a, b] = points[..] {
            p.line_segment([a, b], Stroke::new(1.5, TRACE));
            let d = sub(self.picks[1].point, self.picks[0].point);
            let label = p.layout_no_wrap(
                self.units.length(dot(d, d).sqrt() as f64, 3),
                figure(13.0),
                TRACE,
            );
            let at = a.lerp(b, 0.5) + Vec2::new(8.0, -8.0 - label.size().y);
            p.rect_filled(
                Rect::from_min_size(at, label.size()).expand2(Vec2::new(5.0, 3.0)),
                2.0,
                BACKGROUND.gamma_multiply(0.9),
            );
            p.galley(at, label, TRACE);
        }
        for (n, at) in points.iter().enumerate() {
            p.circle_filled(*at, 4.0, if n == 0 { READOUT } else { TRACE });
            p.circle_stroke(*at, 4.0, Stroke::new(1.0, Color32::BLACK));
        }
        self.focus_popup(ui, rect, &projector, &scene);
        if let Some(field) = self.shown.as_ref().and_then(|s| s.field.clone()) {
            field_marks(&p, rect, &projector, &field, scene.mesh.radius);
        }
        if !resp.dragged()
            && let Some(at) = resp.hover_pos()
            && let Some(field) = self.shown.as_ref().and_then(|s| s.field.clone())
            && let Some(hit) = viewer.pick(rect, at)
        {
            let point = monstertruck::modeling::Point3::new(
                hit.point[0] as f64,
                hit.point[1] as f64,
                hit.point[2] as f64,
            );
            if hit.surface > 0 {
                if let Some(probe) = field.slice_probe((hit.surface - 1) / 2, point) {
                    let round = |v: f64| crate::plot::label((v * 1000.0).round() / 1000.0);
                    readout(
                        &p,
                        rect,
                        at,
                        vec![
                            format!("{} m/s mean", crate::plot::label(probe.mean)),
                            format!("{} m/s at one moment", crate::plot::label(probe.moment)),
                            probe.title,
                            format!(
                                "at {}, {}, {}",
                                round(probe.at.x),
                                round(probe.at.y),
                                round(probe.at.z)
                            ),
                        ],
                    );
                }
            } else if let Some(probe) = field.probe(point) {
                field_readout(&p, rect, at, &field, &probe);
            }
        }
        let help = if self.measuring {
            "click two points to measure, corners, midpoints, centres and edges snap; esc clears; drag to orbit, shift-drag to pan, scroll to zoom"
        } else {
            "click a part to select it, drag to orbit, shift-drag to pan, scroll to zoom, double-click to reset, up/down to step lines"
        };
        p.text(
            rect.left_bottom() + Vec2::new(10.0, -10.0),
            egui::Align2::LEFT_BOTTOM,
            help,
            egui::FontId::proportional(11.0),
            Color32::from_gray(130),
        );
        let stats = p.layout_no_wrap(
            self.timings(&scene, built),
            egui::FontId::monospace(11.0),
            Color32::from_gray(170),
        );
        let corner = rect.left_top() + Vec2::new(10.0, 10.0);
        p.rect_filled(
            Rect::from_min_size(corner, stats.size()).expand(4.0),
            3.0,
            BACKGROUND.gamma_multiply(0.85),
        );
        p.galley(corner, stats, Color32::from_gray(170));
        let status = crate::host::status();
        let live = self
            .activity
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .rows
            .iter()
            .any(|r| !r.report.finished);
        let panel = status.is_none()
            && self.sim_job.is_none()
            && (self.loading.is_some() || self.building.is_some() && live)
            && self.activity_panel(
                ui.ctx(),
                rect.left_top() + Vec2::new(10.0, 28.0),
                egui::Align2::LEFT_TOP,
            );
        if !panel && (status.is_some() || self.loading.is_some() || self.building.is_some()) {
            p.text(
                rect.left_top() + Vec2::new(10.0, 30.0),
                egui::Align2::LEFT_TOP,
                status.as_deref().unwrap_or("rebuilding"),
                egui::FontId::proportional(11.0),
                READOUT,
            );
        }
    }

    fn browser(&mut self, ctx: &egui::Context) {
        let Some(browser) = &mut self.browser else {
            return;
        };
        match browser.show(ctx) {
            Picked::Browsing => {}
            Picked::Cancelled => self.browser = None,
            Picked::File(path) | Picked::Folder(path) => {
                self.browser = None;
                self.open(path, ctx);
            }
        }
    }

    fn help(&mut self, ctx: &egui::Context) {
        if !self.help_open {
            return;
        }
        let modal = egui::Modal::new(egui::Id::new("gcad_help"))
            .backdrop_color(Color32::from_black_alpha(150))
            .show(ctx, |ui| {
            ui.set_width(560.0);
            modal_title(ui, "gcad is built for coding agents");
            note(
                ui,
                "it models mechanical parts as one operation per line, like G-code. an agent writes the file, runs `gcad check`, and fixes errors until it builds. this viewer shows the result and keeps it live.",
                LEGEND,
            );
            ui.add_space(8.0);
            Line::new().legend("open a file").show(ui);
            hint(ui, "cmd+o or the open button above. drop .gcad, .gasm and .gsim files anywhere on the view. `gcad file.gcad` on the command line opens it too");
            ui.add_space(4.0);
            Line::new().legend("follow the agent").show(ui);
            hint(ui, "save to the file while it is open and the view rebuilds itself. the line panel shows every operation, and a failing one is marked");
            ui.add_space(4.0);
            Line::new().legend("work with the model").show(ui);
            hint(ui, "orbit by dragging, pan with the middle button, zoom with the wheel. the cube sets the view. for assemblies, drag the joint sliders, use explode to pull the parts apart, and section to cut a view through them");
            ui.add_space(4.0);
            Line::new().legend("measure and inspect").show(ui);
            hint(ui, "press the measure button, then click two points to measure between them. the parts panel shows materials, mass and geometry, with show, ghost and opacity controls per part");
            ui.add_space(8.0);
            if WEB {
                hint(ui, "the examples ship with the page, and the address bar opens any file from github");
            } else {
                hint(ui, "open a file to start working");
            }
            let label = if WEB { "open the examples" } else { "browse" };
            footer(ui, |ui| {
                if ui.add(Key::new(label).solid(READOUT).min_width(100.0)).clicked() {
                    self.help_open = false;
                    self.browser = Some(browser_at(
                        if WEB {
                            PathBuf::from("examples")
                        } else {
                            std::env::current_dir().unwrap_or_default()
                        },
                    ));
                }
                if ui.add(Key::new("close").outline(VALUE).min_width(100.0)).clicked() {
                    self.help_open = false;
                }
            });
        });
        if modal.should_close() {
            self.help_open = false;
        }
    }

    fn dropped(&mut self, ctx: &egui::Context) {
        let files = ctx.input(|i| i.raw.dropped_files.clone());
        let mut found: Vec<PathBuf> = Vec::new();
        for file in files {
            let path = match (file.path, &file.bytes) {
                (_, Some(bytes)) if WEB => {
                    let path = PathBuf::from("dropped").join(&file.name);
                    match std::str::from_utf8(bytes) {
                        Ok(text) => crate::host::save(&path, text),
                        Err(_) => {
                            let _ = crate::host::write(&path, bytes);
                        }
                    }
                    path
                }
                (Some(path), _) => path,
                _ => continue,
            };
            found.push(path);
        }
        let rank = |p: &PathBuf| match p.extension().and_then(|e| e.to_str()) {
            Some("gasm") => 0,
            Some("gcad") => 1,
            Some("gsim") => 2,
            _ => 3,
        };
        if let Some(path) = found.into_iter().filter(|p| rank(p) < 3).min_by_key(rank) {
            self.browser = None;
            self.open(path, ctx);
        }
    }

    fn keys(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.help_open {
                self.help_open = false;
                return;
            }
            self.picks.clear();
            self.focus = None;
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O)) {
            self.browser = Some(browser_at(std::env::current_dir().unwrap_or_default()));
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Questionmark)) {
            self.help_open = true;
        }
        let count = self.program.lines.len();
        if count == 0 {
            return;
        }
        let (up, down, home, end) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::Home),
                i.key_pressed(egui::Key::End),
            )
        });
        if up && self.selected > 0 {
            self.select(self.selected - 1);
        }
        if down && self.selected + 1 < count {
            self.select(self.selected + 1);
        }
        if home {
            self.select(0);
        }
        if end {
            self.select(count - 1);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
type Picker = Browser;

#[cfg(not(target_arch = "wasm32"))]
fn browser_at(dir: PathBuf) -> Browser {
    Browser::new("open a part or assembly", dir).suffixes(&[".gcad", ".gasm", ".gsim"])
}

fn moving_lines(
    p: &egui::Painter,
    projector: &Projector,
    lines: &[crate::sim::Stream],
    mask: Option<&[bool]>,
    fastest: f64,
    opacity: f32,
) {
    let mut mesh = egui::epaint::Mesh::default();
    let half = 0.8;
    for (k, line) in lines.iter().enumerate() {
        if mask.is_some_and(|m| !m.get(k).copied().unwrap_or(false)) {
            continue;
        }
        let screen: Vec<Option<Pos2>> = line
            .points
            .iter()
            .map(|q| projector.project(q.map(|v| v as f32)))
            .collect();
        for k in 1..screen.len() {
            let (Some(a), Some(b)) = (screen[k - 1], screen[k]) else {
                continue;
            };
            let along = b - a;
            let length = along.length();
            if length < 1.0e-3 {
                continue;
            }
            let side = Vec2::new(-along.y, along.x) / length * half;
            let speed = 0.5 * (line.speed[k - 1] + line.speed[k]);
            let c = crate::plot::heat(speed / fastest);
            let colour = Color32::from_rgb(
                (c[0] * 255.0) as u8,
                (c[1] * 255.0) as u8,
                (c[2] * 255.0) as u8,
            )
            .gamma_multiply(opacity);
            let base = mesh.vertices.len() as u32;
            for corner in [a + side, a - side, b - side, b + side] {
                mesh.colored_vertex(corner, colour);
            }
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base, base + 2, base + 3);
        }
    }
    p.add(egui::Shape::mesh(mesh));
}

fn is_flow(field: &crate::sim::statics::FieldView) -> bool {
    !field.streams.is_empty() || !field.slices.is_empty()
}

fn particles(
    p: &egui::Painter,
    projector: &Projector,
    field: &crate::sim::statics::FieldView,
    frame: Option<&[crate::sim::Stream]>,
    mask: Option<&[bool]>,
    time: f64,
    opacity: f32,
) {
    use crate::sim::statics::{position_at, travel_times};
    let mut longest: Vec<f64> = field
        .travel()
        .iter()
        .filter_map(|t| t.last().copied())
        .collect();
    if longest.is_empty() {
        return;
    }
    longest.sort_by(f64::total_cmp);
    let crossing = longest[longest.len() / 2].max(1.0e-6);
    let pace = crossing / 4.0;
    let gap = crossing / 8.0;
    let tail = gap * 0.3;
    let fastest = crate::plot::flow_scale(&field.slices);
    let project = |x: [f64; 3]| projector.project(x.map(|v| v as f32));
    let lines = frame.unwrap_or(&field.streams);
    let travel: Vec<Vec<f64>> = match frame {
        Some(lines) => lines.iter().map(travel_times).collect(),
        None => field.travel().to_vec(),
    };
    for (k, (line, t)) in lines.iter().zip(&travel).enumerate() {
        if mask.is_some_and(|m| !m.get(k).copied().unwrap_or(false)) {
            continue;
        }
        let Some(&end) = t.last() else { continue };
        let offset = (k as f64 * 0.618_034).fract() * gap;
        let mut release = (time * pace + offset) % gap;
        while release < end {
            if let (Some((head, speed)), Some((back, _))) = (
                position_at(line, t, release),
                position_at(line, t, (release - tail).max(0.0)),
            ) && let (Some(a), Some(b)) = (project(back), project(head))
            {
                let c = crate::plot::heat(speed / fastest).map(|v| (v * 255.0) as u8);
                let colour = Color32::from_rgb(c[0], c[1], c[2]).gamma_multiply(opacity);
                p.line_segment([a, b], Stroke::new(2.5, colour));
                p.circle_filled(b, 1.6, Color32::WHITE.gamma_multiply(0.7 * opacity));
            }
            release += gap;
        }
    }
}

#[cfg(target_arch = "wasm32")]
type Picker = Library;

#[cfg(target_arch = "wasm32")]
fn browser_at(_dir: PathBuf) -> Library {
    Library
}

#[cfg(target_arch = "wasm32")]
const OPENS: [&str; 3] = ["gcad", "gasm", "gsim"];

#[cfg(target_arch = "wasm32")]
struct Library;

#[cfg(target_arch = "wasm32")]
impl Library {
    fn show(&mut self, ctx: &egui::Context) -> Picked {
        let mut picked = Picked::Browsing;
        let mut open = true;
        let files: Vec<PathBuf> = crate::host::list()
            .into_iter()
            .filter(|p| {
                p.extension()
                    .is_some_and(|e| OPENS.contains(&&*e.to_string_lossy()))
            })
            .collect();
        let modal = egui::Modal::new(egui::Id::new("open a part or assembly"))
            .backdrop_color(Color32::from_black_alpha(150))
            .show(ctx, |ui| {
                ui.set_width(420.0);
                modal_title(ui, "open a part or assembly");
                note(ui, "or drop .gcad and .gasm files on the page", LEGEND);
                let w = ui.available_width();
                egui::ScrollArea::vertical()
                    .max_height(560.0)
                    .show(ui, |ui| {
                        ui.set_max_width(w);
                        let mut folder = None;
                        for path in files {
                            let parent = path.parent().map(std::path::Path::to_path_buf);
                            if folder != parent {
                                ui.add_space(6.0);
                                Line::new()
                                    .legend(
                                        &parent
                                            .as_ref()
                                            .map_or_else(String::new, |p| p.display().to_string()),
                                    )
                                    .show(ui);
                                folder = parent;
                            }
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            if ui.selectable_label(false, name).clicked() {
                                picked = Picked::File(path.clone());
                            }
                        }
                    });
                footer(ui, |ui| {
                    if ui.add(Key::new("cancel").outline(VALUE).min_width(100.0)).clicked() {
                        open = false;
                    }
                });
            });
        if modal.should_close() {
            open = false;
        }
        if open { picked } else { Picked::Cancelled }
    }
}

fn field_marks(
    p: &egui::Painter,
    rect: Rect,
    projector: &Projector,
    field: &crate::sim::statics::FieldView,
    radius: f32,
) {
    use crate::sim::statics::Stimulus;
    let v = |q: monstertruck::modeling::Point3| [q.x as f32, q.y as f32, q.z as f32];
    let font = egui::FontId::proportional(12.0);
    let label = |at: Pos2, text: &str, colour: Color32| {
        let galley = p.layout_no_wrap(text.to_string(), font.clone(), colour);
        let size = galley.size();
        let wanted = at + Vec2::new(6.0, -size.y - 4.0);
        let corner = Pos2::new(
            wanted
                .x
                .min(rect.right() - size.x - 6.0)
                .max(rect.left() + 6.0),
            wanted
                .y
                .min(rect.bottom() - size.y - 6.0)
                .max(rect.top() + 6.0),
        );
        p.rect_filled(
            Rect::from_min_size(corner, galley.size()).expand(3.0),
            3.0,
            Color32::from_black_alpha(190),
        );
        p.galley(corner, galley, colour);
    };
    for mark in &field.marks {
        let colour = match mark.kind {
            Stimulus::Push => Color32::from_rgb(255, 80, 200),
            Stimulus::Hold => Color32::from_rgb(235, 235, 235),
            Stimulus::Heat => Color32::from_rgb(255, 170, 60),
            Stimulus::Wind => Color32::from_rgb(115, 190, 255),
        };
        if mark.nodes.is_empty() && mark.kind != Stimulus::Wind {
            continue;
        }
        let Some(head) = projector.project(v(mark.at)) else {
            continue;
        };
        let anchor = match mark.toward {
            Some(toward) => {
                let length = radius as f64 * 0.45;
                let tail = mark.at - toward * length;
                let Some(tail) = projector.project(v(tail)) else {
                    continue;
                };
                let along = (head - tail).normalized();
                let side = Vec2::new(-along.y, along.x);
                for (width, ink) in [(5.0, Color32::BLACK), (2.5, colour)] {
                    let stroke = Stroke::new(width, ink);
                    p.line_segment([tail, head], stroke);
                    p.line_segment([head, head - along * 12.0 + side * 6.0], stroke);
                    p.line_segment([head, head - along * 12.0 - side * 6.0], stroke);
                }
                tail
            }
            None => {
                let Some(spot) = mark
                    .spot
                    .and_then(|n| projector.project(v(field.positions[n as usize])))
                else {
                    continue;
                };
                spot
            }
        };
        label(anchor, &mark.text, colour);
    }
    let lines: Vec<&str> = field.marks.iter().map(|m| m.text.as_str()).collect();
    if lines.is_empty() {
        return;
    }
    let galley = p.layout_no_wrap(
        lines.join("\n"),
        egui::FontId::monospace(11.0),
        Color32::from_gray(225),
    );
    let corner = rect.right_bottom() - galley.size() - Vec2::new(12.0, 30.0);
    p.rect_filled(
        Rect::from_min_size(corner, galley.size()).expand(5.0),
        3.0,
        Color32::from_black_alpha(190),
    );
    p.galley(corner, galley, Color32::from_gray(225));
}

fn field_readout(
    p: &egui::Painter,
    rect: Rect,
    at: Pos2,
    field: &crate::sim::statics::FieldView,
    probe: &crate::sim::statics::Probe,
) {
    use monstertruck::modeling::InnerSpace;
    let label = crate::plot::label;
    let round = |v: f64| label((v * 1000.0).round() / 1000.0);
    let mut lines = vec![format!(
        "{} {}",
        label((probe.value * 1.0e4).round() / 1.0e4),
        field.unit
    )];
    lines.push(field.title.clone());
    if field.unit == "MPa" {
        lines.push(format!("moved {} mm", label(probe.moved.magnitude())));
    }
    lines.push(format!(
        "at {}, {}, {}",
        round(probe.at.x),
        round(probe.at.y),
        round(probe.at.z)
    ));
    readout(p, rect, at, lines);
}

fn readout(p: &egui::Painter, rect: Rect, at: Pos2, lines: Vec<String>) {
    let text = p.layout_no_wrap(
        lines.join("\n"),
        egui::FontId::monospace(12.0),
        Color32::from_gray(230),
    );
    let mut corner = at + Vec2::new(16.0, 16.0);
    if corner.x + text.size().x > rect.right() - 8.0 {
        corner.x = at.x - 16.0 - text.size().x;
    }
    if corner.y + text.size().y > rect.bottom() - 8.0 {
        corner.y = at.y - 16.0 - text.size().y;
    }
    p.circle_stroke(at, 4.0, Stroke::new(1.5, READOUT));
    p.rect_filled(
        Rect::from_min_size(corner, text.size()).expand(5.0),
        3.0,
        Color32::from_black_alpha(200),
    );
    p.galley(corner, text, Color32::from_gray(230));
}

fn simulate_file(path: &std::path::Path, vars: &[(String, f64)]) -> anyhow::Result<()> {
    let run = {
        let _stage = crate::progress::stage("building the model", None, "");
        crate::model::run_path(path, vars, None)?
    };
    if let Some((line, Err(error))) = run.steps.last() {
        anyhow::bail!("line {}: {error:#}", line.number);
    }
    let Some(spec) = run.model.sim.as_deref() else {
        anyhow::bail!("{} is not a simulation", path.display());
    };
    simulate_spec(path, vars, spec)
}

fn simulate_spec(
    path: &std::path::Path,
    vars: &[(String, f64)],
    spec: &crate::sim::Spec,
) -> anyhow::Result<()> {
    let outcome = crate::sim::simulate(spec)?;
    let _stage = crate::progress::stage("writing the result", None, "");
    crate::sim::save(path, vars, &outcome)?;
    Ok(())
}

fn series_traces(ui: &mut Ui, series: &crate::sim::Series) {
    Line::new()
        .legend(&series.title)
        .note(&series.axis)
        .size(11.0)
        .wrapped(ui);
    for curve in &series.curves {
        let points: Vec<f32> = curve.values.iter().map(|&v| v as f32).collect();
        let finite = points.iter().copied().filter(|v| v.is_finite());
        let low = finite.clone().fold(f32::INFINITY, f32::min);
        let high = finite.fold(f32::NEG_INFINITY, f32::max);
        if !low.is_finite() || high - low <= 0.0 {
            continue;
        }
        Line::new()
            .legend(&curve.name)
            .value(format!(
                "{} to {}",
                crate::sim::short(low as f64),
                crate::sim::short(high as f64)
            ))
            .note(&curve.unit)
            .size(11.0)
            .show(ui);
        ui.add(
            Trace::new(&points)
                .size(Vec2::new(ui.available_width(), 36.0))
                .range(low - 0.1 * (high - low), high + 0.1 * (high - low))
                .filled(true),
        );
    }
}

fn field_shown(view: &crate::sim::statics::FieldView, key: SceneKey) -> Shown {
    let bounds: monstertruck::modeling::BoundingBox<monstertruck::modeling::Point3> =
        view.positions.iter().copied().collect();
    let (min, max) = (bounds.min(), bounds.max());
    Shown {
        key,
        scene: Arc::new(scene::build_field(view)),
        faces: 0,
        bounds: ([min.x, min.y, min.z], [max.x, max.y, max.z]),
        query: None,
        joints: Vec::new(),
        rig: Rig::default(),
        explode: Vec::new(),
        solids: Arc::default(),
        assembly: false,
        built: Duration::ZERO,
        field: Some(Arc::new(view.clone())),
        sim: None,
    }
}

fn build_shown(snapshot: &Snapshot, key: SceneKey, vars: &[(String, f64)]) -> Option<Shown> {
    let picture = snapshot
        .model
        .sim
        .as_deref()
        .and_then(|spec| crate::sim::picture(spec, vars));
    let (field, sim) = match picture {
        Some(p) => (
            p.field,
            Some(Arc::new(SimState {
                kind: p.kind,
                outcome: p.outcome,
                stale: p.stale,
            })),
        ),
        None => (None, None),
    };
    if let Some(view) = field {
        return Some(Shown {
            sim,
            ..field_shown(&view, key)
        });
    }
    let model = match snapshot
        .model
        .sim
        .as_deref()
        .and_then(|s| s.target.as_deref())
    {
        Some(target) => target,
        None => &snapshot.model,
    };
    if model.solids().is_empty() {
        return None;
    }
    let tolerance = model.tolerance();
    let own: Vec<usize> = model
        .solid
        .as_ref()
        .map(|solid| {
            select::select_faces(&label_of(&snapshot.line), solid, &model.groups, tolerance)
                .unwrap_or_default()
        })
        .unwrap_or_default();
    let (query_faces, query_edges, query) = match (key.query.as_str(), model.solid.as_ref()) {
        ("", _) | (_, None) => (Vec::new(), Vec::new(), None),
        (text, Some(solid)) => {
            let faces = if text.contains('&') || text.contains('|') {
                Ok(Vec::new())
            } else {
                select::select_faces(text, solid, &model.groups, tolerance)
            };
            let edges = select::select_edges(text, solid, &model.groups, tolerance);
            match (faces, edges) {
                (Ok(faces), Ok(edges)) => {
                    let counts = (faces.len(), edges.len());
                    (faces, edges, Some(Ok(counts)))
                }
                (Err(error), _) | (_, Err(error)) => {
                    (Vec::new(), Vec::new(), Some(Err(format!("{error:#}"))))
                }
            }
        }
    };
    let rgb = |c: Color32| [c.r(), c.g(), c.b()].map(|v| v as f32 / 255.0);
    let highlights = [
        Highlight {
            faces: &own,
            colour: rgb(READOUT),
        },
        Highlight {
            faces: &query_faces,
            colour: rgb(TRACE),
        },
    ];
    let scene = scene::build(model, &highlights, &query_edges, rgb(TRACE));
    let bounds = model
        .solids()
        .iter()
        .map(|s| geometry::bounds(s))
        .reduce(|a, b| a + b)?;
    let (min, max) = (bounds.min(), bounds.max());
    let solids: Vec<(String, Solid)> = model
        .bodies
        .iter()
        .cloned()
        .chain(model.solid.clone().map(|s| (model.current_body(), s)))
        .collect();
    Some(Shown {
        key,
        scene: Arc::new(scene),
        faces: model.solids().iter().map(|s| select::faces(s).len()).sum(),
        bounds: ([min.x, min.y, min.z], [max.x, max.y, max.z]),
        query,
        joints: model.joints.clone(),
        rig: model.rig(),
        explode: model.explode.clone(),
        solids: Arc::new(solids),
        assembly: model.assembly,
        built: Duration::ZERO,
        field: None,
        sim,
    })
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.dropped(&ctx);
        if let Some(path) = crate::host::take_open() {
            self.browser = None;
            self.open(path, &ctx);
        }
        let now = Instant::now();
        self.frames.push_back(now);
        while self
            .frames
            .front()
            .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(2))
        {
            self.frames.pop_front();
        }
        self.poll(&ctx);
        self.keys(&ctx);
        self.refresh_scene(&ctx);
        egui::Panel::top("header")
            .frame(egui::Frame::NONE.fill(CHASSIS).inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: 8,
                bottom: 2,
            }))
            .show(ui, |ui| self.header(ui, &ctx));
        let mut lines_open = self.lines_open;
        egui::Panel::left("lines")
            .resizable(true)
            .default_size(360.0)
            .min_size(if self.editing { 320.0 } else { 160.0 })
            .frame(
                egui::Frame::NONE
                    .fill(CHASSIS)
                    .inner_margin(egui::Margin::symmetric(6, 8)),
            )
            .show_collapsible(ui, &mut lines_open, |ui| {
                if self.editing {
                    self.editor(ui)
                } else {
                    self.lines(ui)
                }
            });
        self.lines_open = lines_open;
        let mut parts_open = self.parts_open;
        egui::Panel::right("inspector")
            .resizable(true)
            .default_size(320.0)
            .min_size(200.0)
            .frame(
                egui::Frame::NONE
                    .fill(CHASSIS)
                    .inner_margin(egui::Margin::symmetric(10, 8)),
            )
            .show_collapsible(ui, &mut parts_open, |ui| self.inspector(ui));
        self.parts_open = parts_open;
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let r = ui.max_rect();
            ui.painter()
                .line_segment([r.left_top(), r.left_bottom()], Stroke::new(1.0, ETCH));
            self.viewport(ui);
        });
        self.browser(&ctx);
        self.help(&ctx);
        if self.loading.is_some() || self.building.is_some() || self.clash_job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run(
    path: Option<PathBuf>,
    query: String,
    line: Option<usize>,
    vars: Vec<(String, f64)>,
) -> eframe::Result<()> {
    let title = match &path {
        Some(path) => format!("gcad - {}", path.display()),
        None => "gcad".to_string(),
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1500.0, 950.0])
            .with_title(title.clone())
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icon-256.png"))
                    .unwrap_or_default(),
            ),
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: viewer3d::wgpu_setup(),
            ..Default::default()
        },
        ..Default::default()
    };
    eframe::run_native(
        &title,
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, path, query, line, vars)))),
    )
}

fn trace_state(painted: Painted) -> String {
    let engine = if painted.hardware { "ray query" } else { "compute" };
    if painted.traced() {
        format!("{engine}: {} / {} samples", painted.samples, painted.target)
    } else {
        format!("{engine}: settling")
    }
}

fn exploded(
    moved: Matrix4,
    offset: Option<&monstertruck::modeling::Vector3>,
    scale: f64,
) -> Matrix4 {
    match offset {
        Some(by) => Matrix4::from_translation(by * scale) * moved,
        None => moved,
    }
}

fn locked_joints(rig: &Rig, values: &[f64]) -> Vec<Option<String>> {
    rig.joints
        .iter()
        .enumerate()
        .map(|(i, joint)| {
            let (low, high) = joint.range;
            if !joint.movable()
                || low >= high
                || i >= values.len()
                || rig.driver_of(&joint.name).is_some()
            {
                return None;
            }
            let step = match joint.kind {
                JointKind::Turn { .. } => 0.5,
                _ => 0.05,
            };
            let nudged = |delta: f64| {
                let to = (values[i] + delta).clamp(low, high);
                (to != values[i]).then(|| rig.drive(values, i, to).err())
            };
            match (nudged(step), nudged(-step)) {
                (Some(Some(why)), Some(Some(_)))
                | (Some(Some(why)), None)
                | (None, Some(Some(why))) => Some(why),
                _ => None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use monstertruck::modeling::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn run_writes_a_fresh_result() {
        let dir = std::env::temp_dir().join("gcad-tests/viewer_run");
        std::fs::create_dir_all(&dir).expect("scratch dir");
        std::fs::write(
            dir.join("bar.gcad"),
            "material steel\nplane XY\nrect 100 10\nextrude 10\n",
        )
        .expect("part");
        let spec = dir.join("bar.gsim");
        std::fs::write(
            &spec,
            "sim static\npart bar.gcad\nfix <X\nload >X force=0,0,-10\nmesh 5",
        )
        .expect("spec");
        let _ = std::fs::remove_file(crate::sim::result_path(&spec));
        let _serial = crate::progress::SERIAL
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let watched = Arc::new(Mutex::new(super::SimProgress::default()));
        let into = watched.clone();
        let listening = crate::progress::listen(move |report| {
            into.lock().unwrap().take(report);
        });
        super::simulate_file(&spec, &[]).expect("runs");
        drop(listening);
        assert_eq!(crate::sim::stale(&spec, &[]), None);
        let watched = watched.lock().unwrap();
        assert!(
            watched
                .rows
                .iter()
                .all(|r| r.report.finished && r.inside.is_none())
        );
        let names: Vec<String> = watched.rows.iter().map(|r| r.report.name()).collect();
        assert_eq!(
            names.first().map(String::as_str),
            Some("building the model")
        );
        assert!(
            names.contains(&"taking readings".to_string())
                && names.last().map(String::as_str) == Some("writing the result"),
            "{names:?}"
        );
        assert!(
            !names.contains(&"running `bar.gsim`".to_string()),
            "{names:?}"
        );
    }

    #[test]
    fn readings_lay_out_as_tiles_and_a_part_table() {
        let reading = |name: &str, value: f64, unit: &str| crate::sim::Reading {
            name: name.into(),
            value,
            unit: unit.into(),
            at: String::new(),
        };
        let readings = vec![
            reading("drag", 10.0, "N"),
            reading("moment.x", 1.0, "N·m"),
            reading("moment.y", 2.0, "N·m"),
            reading("moment.z", 3.0, "N·m"),
            reading("mast", 2.0, "N"),
            reading("mast.moment", 0.5, "N·m"),
            reading("boom.moment", 7.0, "N·m"),
            reading("tip.stress", 4.0, "MPa"),
            reading("clamp.main.moment", 1.0, "N·m"),
        ];
        let layout = super::layout_readings(&readings, None);
        let tiles: Vec<&str> = layout.tiles.iter().map(|t| t.0.as_str()).collect();
        assert_eq!(tiles, ["drag"]);
        assert_eq!(layout.vectors.len(), 1);
        assert_eq!(
            (layout.vectors[0].0.as_str(), layout.vectors[0].2.len()),
            ("moment", 3)
        );
        assert_eq!(layout.columns, ["force N", "moment N·m", "stress MPa"]);
        assert_eq!(
            layout.blocks[0].1,
            [
                ("force".to_string(), "2.000 N".to_string()),
                ("moment".to_string(), "0.5000 N·m".to_string())
            ]
        );
        let heads: Vec<&str> = layout.rows.iter().map(|r| r.0.as_str()).collect();
        assert_eq!(heads, ["mast", "boom", "tip", "clamp.main"]);
        assert_eq!(
            layout.rows[0].1,
            [Some("2.000".to_string()), Some("0.5000".to_string()), None]
        );
        let few = super::layout_readings(&readings, Some(2));
        assert_eq!(few.total, 4);
        let heads: Vec<&str> = few.rows.iter().map(|r| r.0.as_str()).collect();
        assert_eq!(heads, ["mast", "boom"]);
    }

    #[test]
    fn frames_step_and_wrap() {
        use super::{frame_at, frame_middle};
        assert_eq!(frame_at(frame_middle(0.0), 10), 0);
        assert_eq!(frame_at(frame_middle(-1.0), 10), 9);
        assert_eq!(frame_at(frame_middle(23.0), 10), 3);
        let mut clock = frame_middle(4.0);
        for want in [5, 6, 7] {
            clock = frame_middle((clock * super::REEL_RATE).floor() + 1.0);
            assert_eq!(frame_at(clock, 10), want);
        }
    }

    #[test]
    fn explode_offsets_do_not_turn_with_the_part() {
        let turned = Matrix4::from_angle_z(Deg(90.0));
        let at = super::exploded(turned, Some(&Vector3::new(10.0, 0.0, 0.0)), 1.0)
            .transform_point(Point3::origin());
        assert!(
            (at - Point3::new(10.0, 0.0, 0.0)).magnitude() < 1.0e-9,
            "{at:?}"
        );
    }
}
