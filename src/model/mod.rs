mod args;
mod assembly;
mod blends;
mod bodies;
mod clearance;
mod constrain;
mod enclosure;
mod exact;
mod fastener;
mod features;
mod fuse;
mod holes;
mod import;
pub mod mate;
pub mod materials;
mod measure;
mod member;
mod path;
mod pattern;
mod rib;
mod round;
mod sheet;
mod simulate;
mod sketch;
mod skin;
mod solids;
mod standard;
mod step_raw;
pub(crate) mod stored;
pub mod texture;
mod thread;
mod weld;
mod wrap;

use crate::geometry::{self, Frame, Profile, Segment};
use crate::parse::{Line, Scope};
use crate::select::{self, Groups};
use anyhow::{Result, anyhow, bail};
use monstertruck::meshing::prelude::*;
use monstertruck::modeling::*;
use std::collections::HashMap;
use std::path::PathBuf;

pub use args::label_of;
pub use assembly::{
    ASSEMBLY_OPERATIONS, Couple, Joint, JointKind, Mate, Rig, Source, broken, explode_offsets,
    posed, subtree,
};
pub use bodies::{Material, Mechanics};
pub use measure::{MassProperties, mass_properties};
pub use path::SweepPath;
pub use sheet::{Sheet, flat_dxf};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Combine {
    Add,
    Remove,
    Common,
}

#[derive(Clone, Debug)]
pub struct Pen {
    pub start: (f64, f64),
    pub cursor: (f64, f64),
    pub segments: Vec<Segment>,
}

#[derive(Clone, Debug)]
pub struct Prism {
    pub frame: Frame,
    pub profiles: Vec<Profile>,
    pub distance: f64,
    pub alone: Option<Vec<monstertruck::topology::FaceId<Surface>>>,
}

#[derive(Clone)]
pub struct Rounded {
    pub from: Point3,
    pub to: Point3,
    pub radius: f64,
}

#[derive(Clone, Default)]
pub struct Model {
    pub scope: Scope,
    pub frame: Option<Frame>,
    pub sketch: Vec<Profile>,
    pub pen: Option<Pen>,
    pub sections: Vec<(Frame, Vec<Profile>)>,
    pub solid: Option<Solid>,
    pub bodies: Vec<(String, Solid)>,
    pub body: String,
    pub groups: Groups,
    pub path: Option<SweepPath>,
    pub tools: HashMap<String, Vec<(Combine, Solid)>>,
    pub prisms: HashMap<String, Prism>,
    pub dir: Option<PathBuf>,
    pub fixed: Scope,
    pub depth: usize,
    pub points: Vec<constrain::SketchPoint>,
    pub constraints: Vec<constrain::Constraint>,
    pub construction: Vec<(Frame, Profile)>,
    pub axes: HashMap<String, (Point3, Vector3)>,
    pub colours: HashMap<String, [f64; 3]>,
    pub opacities: HashMap<String, f64>,
    pub materials: HashMap<String, bodies::Material>,
    pub library: HashMap<String, materials::Entry>,
    pub library_loaded: bool,
    pub sources: HashMap<String, assembly::Source>,
    pub placements: HashMap<String, Matrix4>,
    pub stand_ins: HashMap<String, String>,
    pub filled: HashMap<String, usize>,
    pub explode: Vec<(String, Vector3)>,
    pub revolves: HashMap<String, features::Revolve>,
    pub joints: Vec<assembly::Joint>,
    pub assembly: bool,
    pub part_groups: HashMap<String, PartGroups>,
    pub mates: Vec<assembly::Mate>,
    pub couples: Vec<assembly::Couple>,
    pub cache: Cache,
    pub rounded: Vec<Rounded>,
    pub gear: Option<GearLead>,
    pub hole_notes: Vec<HoleNote>,
    pub sheet: Option<sheet::Sheet>,
    pub members: Vec<String>,
    pub sim: Option<Box<crate::sim::Spec>>,
}

#[derive(Clone)]
pub struct PartGroups {
    groups: std::sync::Arc<Groups>,
    placed: Matrix4,
}

impl PartGroups {
    pub(crate) fn parts(&self) -> (&Groups, Matrix4) {
        (&self.groups, self.placed)
    }

    pub(crate) fn from_parts(groups: Groups, placed: Matrix4) -> Self {
        PartGroups {
            groups: std::sync::Arc::new(groups),
            placed,
        }
    }

    pub fn new(groups: Groups) -> Self {
        PartGroups {
            groups: std::sync::Arc::new(groups),
            placed: Matrix4::identity(),
        }
    }

    pub fn moved(&self, transform: Matrix4) -> Self {
        PartGroups {
            groups: self.groups.clone(),
            placed: transform * self.placed,
        }
    }

    pub fn placed(&self) -> Groups {
        let mut groups = (*self.groups).clone();
        if self.placed != Matrix4::identity() {
            groups.0.iter_mut().for_each(|entry| {
                entry.surface = bodies::moved_surface(&entry.surface, self.placed)
            });
        }
        groups
    }
}

#[derive(Clone, Debug)]
pub struct HoleNote {
    pub diameter: f64,
    pub centers: Vec<Point3>,
    pub note: String,
}

#[derive(Clone, Copy, Debug)]
pub struct GearLead {
    pub pitch_radius: f64,
    pub helix: f64,
    pub cone: f64,
}

#[derive(Clone, Default)]
pub struct Cache(std::sync::Arc<Shared>);

#[derive(Default)]
struct Shared {
    entries: std::sync::Mutex<CacheEntries>,
    settled: std::sync::Condvar,
}

#[derive(Default)]
struct CacheEntries {
    parts: HashMap<String, Entry>,
    cylinders: HashMap<String, Vec<mate::Cylinder>>,
    pending: HashMap<String, u64>,
    clock: u64,
}

struct Entry {
    model: std::sync::Arc<Model>,
    reads: crate::host::Reads,
    uses: Vec<String>,
    used: u64,
}

pub(crate) struct Traced<T = Model> {
    result: Result<T>,
    reads: crate::host::Reads,
    uses: Vec<String>,
}

thread_local! {
    static USES: std::cell::RefCell<Vec<Vec<String>>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub(crate) fn traced<T>(load: impl FnOnce() -> Result<T>) -> Traced<T> {
    USES.with(|stack| stack.borrow_mut().push(Vec::new()));
    let (result, reads) = crate::host::recording(load);
    let uses = USES.with(|stack| {
        let mut stack = stack.borrow_mut();
        let uses = stack.pop().unwrap_or_default();
        if let Some(outer) = stack.last_mut() {
            for key in &uses {
                if !outer.contains(key) {
                    outer.push(key.clone());
                }
            }
        }
        uses
    });
    Traced {
        result,
        reads,
        uses,
    }
}

fn note_use(key: &str) {
    USES.with(|stack| {
        if let Some(top) = stack.borrow_mut().last_mut()
            && !top.iter().any(|k| k == key)
        {
            top.push(key.to_string());
        }
    });
}

pub(crate) const INLINE_KEY: &str = "inline|";

pub(crate) struct Settling<'a> {
    cache: &'a Cache,
    key: &'a str,
    done: bool,
}

impl<'a> Settling<'a> {
    pub(crate) fn new(cache: &'a Cache, key: &'a str) -> Self {
        Settling {
            cache,
            key,
            done: false,
        }
    }

    pub(crate) fn settle(mut self, traced: Traced) {
        self.done = true;
        self.cache.settle(self.key, Some(traced));
    }
}

impl Drop for Settling<'_> {
    fn drop(&mut self) {
        if !self.done {
            self.cache.settle(self.key, None);
        }
    }
}

const KEPT_PARTS: usize = 256;

static KEPT: std::sync::Mutex<Option<Cache>> = std::sync::Mutex::new(None);

impl CacheEntries {
    fn stale(&self, key: &str, memo: &mut HashMap<String, bool>) -> bool {
        if let Some(known) = memo.get(key) {
            return *known;
        }
        memo.insert(key.to_string(), true);
        let stale = match self.parts.get(key) {
            None => true,
            Some(entry) => {
                !entry.reads.unchanged() || entry.uses.iter().any(|used| self.stale(used, memo))
            }
        };
        memo.insert(key.to_string(), stale);
        stale
    }

    fn refresh(&mut self) {
        let mut memo = HashMap::default();
        let mut by_age: Vec<(u64, &String)> = self
            .parts
            .iter()
            .map(|(key, entry)| (entry.used, key))
            .collect();
        by_age.sort();
        let idle = by_age.len().saturating_sub(KEPT_PARTS);
        let stale: Vec<String> = by_age[..idle]
            .iter()
            .map(|(_, key)| (*key).clone())
            .chain(
                self.parts
                    .keys()
                    .filter(|key| self.stale(key, &mut memo))
                    .cloned()
                    .collect::<Vec<_>>(),
            )
            .collect();
        let changed: Vec<String> = stale
            .iter()
            .filter_map(|key| self.parts.get(key))
            .flat_map(|entry| {
                entry
                    .reads
                    .files()
                    .map(|f| format!("{f:?}"))
                    .collect::<Vec<_>>()
            })
            .collect();
        for key in &stale {
            self.parts.remove(key);
        }
        self.cylinders.retain(|key, _| {
            !key.starts_with(INLINE_KEY) && !changed.iter().any(|file| key.contains(file.as_str()))
        });
    }

    fn building(&self, key: &str) -> bool {
        self.pending
            .get(key)
            .is_some_and(|&run| !crate::host::run_superseded(run))
    }

    fn found(&mut self, key: &str) -> Option<std::sync::Arc<Model>> {
        self.clock += 1;
        let clock = self.clock;
        let entry = self.parts.get_mut(key)?;
        entry.used = clock;
        Some(entry.model.clone())
    }

    fn insert(&mut self, key: String, traced: Traced) -> Result<std::sync::Arc<Model>> {
        let model = std::sync::Arc::new(traced.result?);
        let entry = Entry {
            model: model.clone(),
            reads: traced.reads,
            uses: traced.uses,
            used: self.clock,
        };
        self.clock += 1;
        self.parts.insert(key, entry);
        Ok(model)
    }
}

pub(crate) fn forget_kept() {
    *KEPT.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

impl Cache {
    fn kept() -> Cache {
        let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
        let cache = kept.take().unwrap_or_default();
        cache.lock().refresh();
        *kept = Some(cache.clone());
        cache
    }

    fn files_reached<T>(&self, run: &Traced<T>) -> Vec<PathBuf> {
        let entries = self.lock();
        let mut seen = std::collections::HashSet::new();
        let mut todo: Vec<&String> = run.uses.iter().collect();
        let mut files: Vec<PathBuf> = run
            .reads
            .files()
            .map(std::path::Path::to_path_buf)
            .collect();
        while let Some(key) = todo.pop() {
            if !seen.insert(key.clone()) {
                continue;
            }
            if let Some(entry) = entries.parts.get(key) {
                files.extend(entry.reads.files().map(std::path::Path::to_path_buf));
                todo.extend(entry.uses.iter());
            }
        }
        let mut files: Vec<PathBuf> = files
            .into_iter()
            .filter_map(|file| {
                let dir = std::fs::canonicalize(file.parent()?).ok()?;
                Some(dir.join(file.file_name()?))
            })
            .collect();
        files.sort();
        files.dedup();
        files
    }

    fn reads_reached(&self, own: &crate::host::Reads, uses: &[String]) -> crate::host::Reads {
        let entries = self.lock();
        let mut reads = own.clone();
        let mut seen = std::collections::HashSet::new();
        let mut todo: Vec<&String> = uses.iter().collect();
        while let Some(key) = todo.pop() {
            if !seen.insert(key.clone()) {
                continue;
            }
            if let Some(entry) = entries.parts.get(key) {
                reads.merge(entry.reads.clone());
                todo.extend(entry.uses.iter());
            }
        }
        reads
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, CacheEntries> {
        self.0.entries.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn part(
        &self,
        key: String,
        load: impl FnOnce() -> Result<Model>,
    ) -> Result<std::sync::Arc<Model>> {
        note_use(&key);
        let mut entries = self.lock();
        while !entries.parts.contains_key(&key) && entries.building(&key) {
            entries = self
                .0
                .settled
                .wait(entries)
                .unwrap_or_else(|e| e.into_inner());
        }
        if let Some(found) = entries.found(&key) {
            return Ok(found);
        }
        drop(entries);
        let traced = traced(load);
        if crate::host::superseded() {
            return traced.result.map(std::sync::Arc::new);
        }
        self.lock().insert(key, traced)
    }

    pub(crate) fn claim(&self, key: &str) -> bool {
        let mut entries = self.lock();
        if entries.found(key).is_some() || entries.building(key) {
            return false;
        }
        entries
            .pending
            .insert(key.to_string(), crate::host::current_run());
        true
    }

    fn settle(&self, key: &str, traced: Option<Traced>) {
        let mut entries = self.lock();
        if entries.pending.get(key) == Some(&crate::host::current_run()) {
            entries.pending.remove(key);
            if let Some(traced) = traced.filter(|_| !crate::host::superseded()) {
                let _ = entries.insert(key.to_string(), traced);
            }
        }
        drop(entries);
        self.0.settled.notify_all();
    }

    pub(crate) fn cylinders(
        &self,
        key: String,
        fit: impl FnOnce() -> Vec<mate::Cylinder>,
    ) -> Vec<mate::Cylinder> {
        if let Some(found) = self.lock().cylinders.get(&key).cloned() {
            return found;
        }
        let fitted = fit();
        self.lock().cylinders.insert(key, fitted.clone());
        fitted
    }
}

pub const OPERATIONS: &[&str] = &[
    "let",
    "wrap",
    "material",
    "plane",
    "rect",
    "circle",
    "poly",
    "ngon",
    "gear",
    "rack",
    "offset",
    "slot",
    "ellipse",
    "pen",
    "line",
    "arc",
    "close",
    "spline",
    "text",
    "section",
    "loft",
    "extrude",
    "cut",
    "revolve",
    "path",
    "helix",
    "sweep",
    "hole",
    "boss",
    "lip",
    "groove",
    "vent",
    "snap",
    "thread",
    "sheet",
    "tab",
    "flange",
    "unfold",
    "member",
    "fillet",
    "chamfer",
    "shell",
    "draft",
    "push",
    "mirror",
    "repeat",
    "move",
    "rotate",
    "scale",
    "split",
    "body",
    "combine",
    "place",
    "measure",
    "import",
    "if",
    "include",
    "point",
    "dist",
    "horizontal",
    "vertical",
    "angle",
    "reflect",
    "array",
    "axis",
];

const PROFILE_OPS: &[&str] = &[
    "rect", "circle", "poly", "ngon", "gear", "rack", "offset", "slot", "ellipse", "close",
    "spline", "text",
];

impl Model {
    pub fn tolerance(&self) -> f64 {
        self.solid.as_ref().map_or(1.0e-6, |solid| {
            (geometry::bounds(solid).diameter() * 1.0e-6).max(1.0e-7)
        })
    }

    pub fn solids(&self) -> Vec<&Solid> {
        self.bodies
            .iter()
            .map(|(_, solid)| solid)
            .chain(self.solid.as_ref())
            .collect()
    }

    pub fn parts(&self) -> Vec<(&Solid, Option<[f64; 3]>)> {
        let current = if self.body.is_empty() {
            "main"
        } else {
            self.body.as_str()
        };
        self.bodies
            .iter()
            .map(|(name, solid)| (solid, self.colours.get(name).copied()))
            .chain(
                self.solid
                    .as_ref()
                    .map(|solid| (solid, self.colours.get(current).copied())),
            )
            .collect()
    }

    pub fn named_parts(&self) -> Vec<(String, &Solid, Option<[f64; 3]>)> {
        let current = if self.body.is_empty() {
            "main"
        } else {
            self.body.as_str()
        };
        self.bodies
            .iter()
            .map(|(name, solid)| (name.clone(), solid, self.colours.get(name).copied()))
            .chain(self.solid.as_ref().map(|solid| {
                (
                    current.to_string(),
                    solid,
                    self.colours.get(current).copied(),
                )
            }))
            .collect()
    }

    pub fn exploded_parts(&self, scale: f64) -> Vec<(Solid, Option<[f64; 3]>)> {
        let offsets = assembly::explode_offsets(&self.joints, &self.explode, &Default::default());
        self.named_solids()
            .into_iter()
            .map(|(name, solid)| {
                let colour = self.colours.get(&name).copied();
                match offsets.get(&name) {
                    Some(by) if scale != 0.0 => (
                        builder::transformed(solid, Matrix4::from_translation(by * scale)),
                        colour,
                    ),
                    _ => (solid.clone(), colour),
                }
            })
            .collect()
    }

    pub(crate) fn apply_shared(&mut self, line: &Line) -> Result<String> {
        match line.op.as_str() {
            "if" => self.op_if(line),
            _ => self.op_include(line),
        }
    }

    pub fn apply(&mut self, line: &Line) -> Result<String> {
        if self.sim.is_some() {
            return self.apply_sim(line);
        }
        if self.assembly {
            return self.apply_assembly(line);
        }
        if PROFILE_OPS.contains(&line.op.as_str())
            && line.positional.iter().any(|w| w == "construct")
        {
            let mut plain = line.clone();
            plain.positional.retain(|w| w != "construct");
            let count = self.sketch.len();
            self.apply_op(&plain)?;
            let frame = self.sketch_frame();
            let added: Vec<Profile> = self.sketch.drain(count..).collect();
            self.construction
                .extend(added.into_iter().map(|p| (frame, p)));
            return Ok(format!(
                "construction; sketch has {} profile(s)",
                self.sketch.len()
            ));
        }
        self.apply_op(line)
    }

    fn apply_op(&mut self, line: &Line) -> Result<String> {
        match line.op.as_str() {
            "let" => self.op_let(line),
            "plane" => self.op_plane(line),
            "rect" => self.op_rect(line),
            "circle" => self.op_circle(line),
            "poly" => self.op_poly(line),
            "ngon" => self.op_ngon(line),
            "gear" => self.op_gear(line),
            "rack" => self.op_rack(line),
            "offset" => self.op_offset(line),
            "slot" => self.op_slot(line),
            "ellipse" => self.op_ellipse(line),
            "pen" => self.op_pen(line),
            "line" => self.op_line(line),
            "arc" => self.op_arc(line),
            "close" => self.op_close(line),
            "spline" => self.op_spline(line),
            "text" => self.op_text(line),
            "section" => self.op_section(line),
            "loft" => self.op_loft(line),
            "extrude" => self.op_extrude(line),
            "cut" => self.op_cut(line),
            "revolve" => self.op_revolve(line),
            "path" => self.op_path(line),
            "helix" => self.op_helix(line),
            "sweep" => self.op_sweep(line),
            "hole" => self.op_hole(line),
            "boss" => self.op_boss(line),
            "lip" => self.op_lip(line, false),
            "groove" => self.op_lip(line, true),
            "vent" => self.op_vent(line),
            "snap" => self.op_snap(line),
            "sheet" => self.op_sheet(line),
            "tab" => self.op_tab(line),
            "flange" => self.op_flange(line),
            "bend" => self.op_bend(line),
            "dart" => self.op_dart(line),
            "unfold" => self.op_unfold(line),
            "member" => self.op_member(line),
            "pattern" => self.op_pattern(line),
            "thread" => self.op_thread(line),
            "fillet" => self.op_blend(line, FilletProfile::Round),
            "chamfer" => self.op_blend(line, FilletProfile::Chamfer),
            "shell" => self.op_shell(line),
            "draft" => self.op_draft(line),
            "push" => self.op_push(line),
            "mirror" => self.op_mirror(line),
            "repeat" => self.op_repeat(line),
            "move" => self.op_move(line),
            "rotate" => self.op_rotate(line),
            "scale" => self.op_scale(line),
            "split" => self.op_split(line),
            "body" => self.op_body(line),
            "combine" => self.op_combine(line),
            "place" => self.op_place(line),
            "measure" => self.op_measure(line),
            "if" => self.op_if(line),
            "include" => self.op_include(line),
            "point" => self.op_point(line),
            "dist" | "horizontal" | "vertical" | "angle" | "coincident" | "parallel"
            | "perpendicular" | "equal" | "midpoint" | "online" => self.op_constrain(line),
            "reflect" => self.op_reflect(line),
            "array" => self.op_array(line),
            "axis" => self.op_axis(line),
            "color" => self.op_color(line),
            "material" => self.op_material(line),
            "materials" => self.op_materials(line),
            "dxf" | "svg" => self.op_drawing_file(line),
            "thicken" => self.op_thicken(line),
            "wrap" => self.op_wrap(line),
            "rib" => self.op_rib(line),
            "joint" | "pose" | "couple" | "explode" | "interference" | "part" | "concentric"
            | "flush" | "aligned" | "distance" | "tangent" => {
                bail!(
                    "`{}` belongs in an assembly (.gasm) file, which brings parts in with `part name file.gcad`",
                    line.op
                )
            }
            "import" => self.op_import(line),
            other => bail!(
                "unknown operation `{other}`; operations are {}",
                OPERATIONS.join(", ")
            ),
        }
    }

    pub(crate) fn active(&self, op: &str) -> Result<&Solid> {
        self.solid
            .as_ref()
            .ok_or_else(|| anyhow!("`{op}` needs a solid"))
    }

    pub(crate) fn merge_all(
        &mut self,
        label: &str,
        tools: Vec<Solid>,
        combine: Combine,
    ) -> Result<()> {
        let apart = |a: &BoundingBox<Point3>, b: &BoundingBox<Point3>| {
            (0..3).any(|k| a.max()[k] < b.min()[k] || b.max()[k] < a.min()[k])
        };
        let boxes: Vec<BoundingBox<Point3>> = tools.iter().map(geometry::bounds).collect();
        let disjoint =
            (0..boxes.len()).all(|i| (i + 1..boxes.len()).all(|j| apart(&boxes[i], &boxes[j])));
        if combine == Combine::Add
            && let Some(existing) = self.solid.as_ref()
        {
            let welded = tools
                .iter()
                .try_fold(existing.clone(), |solid, tool| weld::weld(&solid, tool));
            if let Some(solid) = welded {
                self.solid = Some(fuse::fuse_coplanar(&exact::exact_edges(&solid)));
                let entry = self.tools.entry(label.to_string()).or_default();
                entry.extend(tools.into_iter().map(|tool| (combine, tool)));
                return Ok(());
            }
        }
        if tools.len() > 1 && disjoint && self.solid.is_some() {
            let faces: Vec<Face> = tools.iter().flat_map(select::faces).collect();
            let together = Solid::new_unchecked(vec![faces.into()]);
            let before = self.solid.clone();
            if self.merge(label, together, combine).is_ok() {
                return Ok(());
            }
            self.solid = before;
        }
        tools
            .into_iter()
            .try_for_each(|tool| self.merge(label, tool, combine))
    }

    pub(crate) fn merge(&mut self, label: &str, tool: Solid, combine: Combine) -> Result<()> {
        let result = match (self.solid.as_ref(), combine) {
            (None, Combine::Add) => tool.clone(),
            (None, _) => bail!("there is no solid to cut or intersect"),
            (Some(existing), Combine::Add) => match weld::weld(existing, &tool) {
                Some(solid) => solid,
                None => monstertruck::solid::or_normalized(existing, &tool)
                    .map_err(|error| anyhow!("union failed: {error}"))?,
            },
            (Some(existing), Combine::Remove) => {
                let tool = solids::clear_flush(&tool, existing);
                monstertruck::solid::difference_normalized(existing, &tool)
                    .map_err(|e| anyhow!("cut failed: {e}"))?
            }
            (Some(existing), Combine::Common) => {
                let tool = solids::clear_flush(&tool, existing);
                monstertruck::solid::and_normalized(existing, &tool)
                    .map_err(|e| anyhow!("intersection failed: {e}"))?
            }
        };
        self.solid = Some(fuse::fuse_coplanar(&exact::exact_edges(&result)));
        self.tools
            .entry(label.to_string())
            .or_default()
            .push((combine, tool));
        Ok(())
    }

    fn op_if(&mut self, line: &Line) -> Result<String> {
        match self.guarded(line)? {
            Ok(inner) => self.apply(&inner),
            Err(condition) => Ok(format!("skipped, `{condition}` is false")),
        }
    }

    pub(crate) fn guarded(&self, line: &Line) -> Result<std::result::Result<Line, String>> {
        let text = line.text.trim_start();
        let text = match &line.label {
            Some(label) => text.trim_start_matches(&format!("{label}:")).trim_start(),
            None => text,
        };
        let rest = text
            .strip_prefix("if")
            .ok_or_else(|| anyhow!("`if` must start the line"))?
            .trim_start();
        let (condition, inner) = rest
            .split_once(char::is_whitespace)
            .ok_or_else(|| anyhow!("write `if <condition> <operation ...>`"))?;
        let holds = crate::parse::eval(condition, &self.scope)?;
        if holds == 0.0 {
            return Ok(Err(condition.to_string()));
        }
        let mut inner = crate::parse::parse_line(line.number, inner.trim())?;
        if inner.label.is_none() {
            inner.label = line.label.clone();
        }
        Ok(Ok(inner))
    }

    fn op_include(&mut self, line: &Line) -> Result<String> {
        let file = line
            .positional
            .first()
            .ok_or_else(|| anyhow!("`include` needs a file"))?;
        if line.positional.len() > 1 {
            bail!("`include` takes one file and name=value pairs");
        }
        if self.depth >= 16 {
            bail!("includes are nested more than 16 deep");
        }
        let path = match &self.dir {
            Some(dir) if std::path::Path::new(file).is_relative() => dir.join(file),
            _ => PathBuf::from(file),
        };
        let source = crate::host::read_to_string(&path)
            .map_err(|e| anyhow!("reading {}: {e}", path.display()))?;
        let lines = crate::parse::parse_program(&source)?;
        let (saved_fixed, saved_scope, saved_dir) =
            (self.fixed.clone(), self.scope.clone(), self.dir.clone());
        for (name, text) in &line.named {
            let value = crate::parse::eval(text, &saved_scope)?;
            self.fixed.insert(name.clone(), value);
            self.scope.insert(name.clone(), value);
        }
        self.dir = path.parent().map(std::path::Path::to_path_buf);
        self.depth += 1;
        let result = lines.iter().try_for_each(|inner| {
            self.apply(inner)
                .map(|_| ())
                .map_err(|e| anyhow!("{}:{}: {e:#}", path.display(), inner.number))
        });
        self.depth -= 1;
        let defined: Vec<(String, f64)> = std::mem::take(&mut self.scope)
            .into_iter()
            .filter(|(name, _)| {
                !saved_scope.contains_key(name) && !line.named.iter().any(|(n, _)| n == name)
            })
            .collect();
        (self.fixed, self.scope, self.dir) = (saved_fixed, saved_scope, saved_dir);
        self.scope.extend(defined);
        result?;
        Ok(format!(
            "ran {} line(s) from {}; {}",
            lines.len(),
            path.display(),
            self.describe_solid().unwrap_or_default()
        ))
    }

    pub fn describe_solid(&self) -> Result<String> {
        let solid = self.solid.as_ref().ok_or_else(|| anyhow!("no solid"))?;
        if self.depth > 0 {
            return Ok(format!("{} faces", select::faces(solid).len()));
        }
        let mesh = geometry::mesh(solid, geometry::mesh_tolerance(solid));
        let bounds: BoundingBox<Point3> = mesh.positions().iter().copied().collect();
        let tidy = |v: f64| if v.abs() < 5.0e-4 { 0.0 } else { v };
        let (min, max) = (bounds.min().map(tidy), bounds.max().map(tidy));
        let others = match self.bodies.len() {
            0 => String::new(),
            n => format!(" ({n} other bodies)"),
        };
        Ok(format!(
            "{} faces, volume {:.3}, bbox [{:.3}, {:.3}, {:.3}]..[{:.3}, {:.3}, {:.3}]{others}",
            select::faces(solid).len(),
            mesh.volume(),
            min.x,
            min.y,
            min.z,
            max.x,
            max.y,
            max.z
        ))
    }
}

#[derive(Clone)]
pub struct Snapshot {
    pub line: Line,
    pub result: std::result::Result<String, String>,
    pub model: Model,
}

fn start(dir: Option<PathBuf>) -> Model {
    Model {
        dir,
        ..Default::default()
    }
}

pub fn is_assembly(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("gasm"))
}

pub(crate) fn start_for(path: &std::path::Path, vars: &[(String, f64)], depth: usize) -> Model {
    let mut model = start(path.parent().map(std::path::Path::to_path_buf));
    model.fixed = vars.iter().cloned().collect();
    model.scope = model.fixed.clone();
    model.assembly = is_assembly(path);
    if crate::sim::is_sim(path) {
        model.sim = Some(Box::new(crate::sim::Spec {
            origin: Some(path.to_path_buf()),
            ..Default::default()
        }));
    }
    model.depth = depth;
    model
}

pub(crate) fn read_lines(path: &std::path::Path) -> Result<Vec<Line>> {
    let source = crate::host::read_to_string(path)
        .map_err(|e| anyhow!("reading {}: {e}", path.display()))?;
    crate::parse::parse_program(&source).map_err(|e| anyhow!("{}: {e:#}", path.display()))
}

pub(crate) fn load_model(
    path: &std::path::Path,
    vars: &[(String, f64)],
    depth: usize,
    cache: &Cache,
) -> Result<Model> {
    if let Some(spec) = path.to_str().filter(|text| standard::is_standard(text)) {
        return standard_model(spec);
    }
    let lasting = !crate::sim::is_sim(path) && egui_bench::revision::covering(path).is_none();
    if lasting && let Some(model) = stored::fetch(path, vars) {
        return Ok(model);
    }
    let Traced {
        result: built,
        reads,
        uses,
    } = traced(|| {
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("step") || e.eq_ignore_ascii_case("stp"))
        {
            return bodies::step_model(path);
        }
        let mut start = start_for(path, vars, depth);
        start.cache = cache.clone();
        let run = run_model(start, &read_lines(path)?, Some(path));
        if let Some((line, Err(error))) = run.steps.last() {
            bail!("{}:{}: {error:#}", path.display(), line.number);
        }
        Ok(Model {
            cache: Cache::default(),
            ..run.model
        })
    });
    let model = built?;
    if lasting && !crate::host::superseded() {
        stored::put(path, vars, &model, &cache.reads_reached(&reads, &uses));
    }
    Ok(model)
}

fn standard_model(spec: &str) -> Result<Model> {
    let part = standard::standard(spec)?;
    let lines = crate::parse::parse_program(&part.source)?;
    let run = run_model(start(None), &lines, None);
    if let Some((line, Err(error))) = run.steps.last() {
        bail!("{spec}: line {} `{}`: {error:#}", line.number, line.text);
    }
    let mut model = run.model;
    model.sources.insert(
        "main".to_string(),
        assembly::Source {
            file: PathBuf::from(part.title),
            body: "main".to_string(),
            vars: Vec::new(),
        },
    );
    Ok(model)
}

pub fn file_title(file: &std::path::Path) -> String {
    let known = file.extension().is_some_and(|e| {
        ["gcad", "gasm", "step", "stp"]
            .iter()
            .any(|k| e.eq_ignore_ascii_case(k))
    });
    let name = if known {
        file.file_stem()
    } else {
        file.file_name()
    };
    name.map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

pub fn run_path(
    path: &std::path::Path,
    vars: &[(String, f64)],
    until: Option<usize>,
) -> Result<Run> {
    let lines: Vec<Line> = read_lines(path)?
        .into_iter()
        .filter(|line| until.is_none_or(|last| line.number <= last))
        .collect();
    Ok(run_model(start_for(path, vars, 0), &lines, Some(path)))
}

pub fn snapshots_path(
    path: &std::path::Path,
    vars: &[(String, f64)],
) -> Result<(Vec<Line>, Vec<Snapshot>)> {
    let lines = read_lines(path)?;
    let snapshots = snapshots_from(start_for(path, vars, 0), &lines, Some(path));
    Ok((lines, snapshots))
}

pub use stored::{default_dir as default_part_store, store_parts_in};

pub struct KeptRun {
    pub lines: Vec<Line>,
    pub snapshots: Vec<Snapshot>,
    pub files: Vec<PathBuf>,
}

pub fn snapshots_path_kept(path: &std::path::Path, vars: &[(String, f64)]) -> Result<KeptRun> {
    crate::host::superseding(|| {
        let cache = Cache::kept();
        let mut start = start_for(path, vars, 0);
        start.cache = cache.clone();
        let traced = traced(|| {
            let lines = read_lines(path)?;
            let snapshots = snapshots_from(start, &lines, Some(path));
            Ok((lines, snapshots))
        });
        let files = cache.files_reached(&traced);
        let (lines, snapshots) = traced.result?;
        Ok(KeptRun {
            lines,
            snapshots,
            files,
        })
    })
}

pub fn run_snapshots_in(dir: Option<PathBuf>, lines: &[Line]) -> Vec<Snapshot> {
    run_snapshots_full(dir, &[], lines)
}

pub fn run_snapshots_full(
    dir: Option<PathBuf>,
    vars: &[(String, f64)],
    lines: &[Line],
) -> Vec<Snapshot> {
    let mut model = start(dir);
    model.fixed = vars.iter().cloned().collect();
    model.scope = model.fixed.clone();
    snapshots_from(model, lines, None)
}

fn lines_stage(
    model: &Model,
    lines: &[Line],
    path: Option<&std::path::Path>,
) -> crate::progress::Stage {
    let file = path
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned());
    match file.filter(|_| model.depth <= 1) {
        Some(file) => crate::progress::stage_after(
            format!("running `{file}`"),
            Some(lines.len()),
            "lines",
            std::time::Duration::from_secs(1),
        ),
        None => crate::progress::Stage::silent(),
    }
}

fn line_text(line: &Line) -> String {
    let text: String = line.text.chars().take(60).collect();
    format!("line {}: `{text}`", line.number)
}

fn snapshots_from(
    mut model: Model,
    lines: &[Line],
    path: Option<&std::path::Path>,
) -> Vec<Snapshot> {
    if model.assembly {
        model.prefetch(lines);
    }
    let mut snapshots = Vec::new();
    let running = lines_stage(&model, lines, path);
    for (index, line) in lines.iter().enumerate() {
        if crate::host::superseded() {
            break;
        }
        running.progress(index as f64, || line_text(line));
        let before = model.clone();
        let result = model.apply(line).map_err(|error| format!("{error:#}"));
        let failed = result.is_err();
        if failed {
            model = before;
        }
        snapshots.push(Snapshot {
            line: line.clone(),
            result,
            model: model.clone(),
        });
        if failed {
            break;
        }
    }
    snapshots
}

pub fn run_snapshots(lines: &[Line]) -> Vec<Snapshot> {
    run_snapshots_in(None, lines)
}

pub struct Run {
    pub model: Model,
    pub steps: Vec<(Line, Result<String>)>,
    pub took: Vec<std::time::Duration>,
}

pub fn run_in(dir: Option<PathBuf>, lines: &[Line]) -> Run {
    run_model(start(dir), lines, None)
}

fn run_model(mut model: Model, lines: &[Line], path: Option<&std::path::Path>) -> Run {
    if model.assembly {
        model.prefetch(lines);
    }
    let mut steps = Vec::new();
    let mut took = Vec::new();
    let running = lines_stage(&model, lines, path);
    for (index, line) in lines.iter().enumerate() {
        if crate::host::superseded() {
            steps.push((line.clone(), Err(anyhow!("a newer build took over"))));
            took.push(std::time::Duration::ZERO);
            break;
        }
        running.progress(index as f64, || line_text(line));
        let before = model.clone();
        let clock = crate::host::Instant::now();
        let result = model.apply(line);
        took.push(clock.elapsed());
        let failed = result.is_err();
        if failed {
            model = before;
        }
        steps.push((line.clone(), result));
        if failed {
            break;
        }
    }
    Run { model, steps, took }
}

pub fn run(lines: &[Line]) -> Run {
    run_in(None, lines)
}

pub fn run_with(vars: &[(String, f64)], lines: &[Line]) -> Run {
    run_full(None, vars, lines)
}

pub fn run_assembly(dir: Option<PathBuf>, lines: &[Line]) -> Run {
    let mut model = start(dir);
    model.assembly = true;
    run_model(model, lines, None)
}

pub fn run_full(dir: Option<PathBuf>, vars: &[(String, f64)], lines: &[Line]) -> Run {
    let mut model = start(dir);
    model.fixed = vars.iter().cloned().collect();
    model.scope = model.fixed.clone();
    run_model(model, lines, None)
}
