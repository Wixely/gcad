use super::Report;
use super::materials::{self, Material};
use super::tables::MM_PER_INCH;
use anyhow::{Result, anyhow, bail};
use serde::Deserialize;
use std::sync::OnceLock;

const METRIC: &str = include_str!("data/thread_metric.toml");
const UNIFIED: &str = include_str!("data/thread_unified.toml");
const PIPE: &str = include_str!("data/thread_pipe.toml");
const CLEARANCE: &str = include_str!("data/thread_clearance.toml");
const HEADS: &str = include_str!("data/thread_heads.toml");
const STRENGTH: &str = include_str!("data/thread_strength.toml");
const TOLERANCE: &str = include_str!("data/thread_tolerance.toml");

pub const RUNOUT_PITCHES: f64 = 3.0;
pub const DEFAULT_NUT_FACTOR: f64 = 0.2;
pub const SOURCES: [&str; 6] = [
    "https://www.roymech.co.uk/Useful_Tables/Screws/iso-metric-thread-dimensions/ (ISO 261, ISO 724)",
    "https://www.roymech.co.uk/Useful_Tables/Screws/cap_screws.htm (ISO 4762, ISO 10642)",
    "https://www.roymech.co.uk/Useful_Tables/Screws/Screw_loads.htm (ISO 898-1 proof loads and torque)",
    "https://www.roymech.co.uk/Useful_Tables/Screws/Thread_tol.html (ISO 965-1 tolerance)",
    "https://tabelic.com/threads/thread-engagement-chart/ (minimum engagement)",
    "https://en.wikipedia.org/wiki/Unified_Thread_Standard and https://en.wikipedia.org/wiki/British_standard_pipe_thread",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Family {
    Metric,
    Unified,
    Pipe,
}

#[derive(Debug, Clone)]
pub struct Thread {
    pub name: String,
    pub family: Family,
    pub major: f64,
    pub pitch: f64,
    pub tap_drill: f64,
    pub coarse: bool,
}

impl Thread {
    pub fn minor_internal(&self) -> f64 {
        match self.family {
            Family::Pipe => self.major - 1.28065 * self.pitch,
            _ => self.major - 1.082532 * self.pitch,
        }
    }

    pub fn minor_external(&self) -> f64 {
        match self.family {
            Family::Pipe => self.major - 1.28065 * self.pitch,
            _ => self.major - 1.226869 * self.pitch,
        }
    }

    pub fn pitch_dia(&self) -> f64 {
        match self.family {
            Family::Pipe => self.major - 0.640327 * self.pitch,
            _ => self.major - 0.649519 * self.pitch,
        }
    }

    pub fn stress_area(&self) -> f64 {
        let d = self.major - 0.9382 * self.pitch;
        std::f64::consts::FRAC_PI_4 * d * d
    }
}

#[derive(Deserialize)]
struct Rows<T> {
    rows: Vec<T>,
}

fn parse<T: for<'a> Deserialize<'a>>(text: &str, name: &str) -> T {
    toml::from_str(text).unwrap_or_else(|e| panic!("calc table {name}: {e}"))
}

fn all() -> &'static [Thread] {
    static ALL: OnceLock<Vec<Thread>> = OnceLock::new();
    ALL.get_or_init(|| {
        let metric = parse::<Rows<(f64, f64, f64, bool)>>(METRIC, "thread_metric")
            .rows
            .into_iter()
            .map(|(major, pitch, tap_drill, coarse)| Thread {
                name: if coarse {
                    format!("M{major}")
                } else {
                    format!("M{major}x{pitch}")
                },
                family: Family::Metric,
                major,
                pitch,
                tap_drill,
                coarse,
            });
        let unified = parse::<Rows<(String, f64, f64, f64, bool)>>(UNIFIED, "thread_unified")
            .rows
            .into_iter()
            .map(|(size, tpi, major, drill, coarse)| Thread {
                name: format!("{size}-{tpi}"),
                family: Family::Unified,
                major: major * MM_PER_INCH,
                pitch: MM_PER_INCH / tpi,
                tap_drill: drill * MM_PER_INCH,
                coarse,
            });
        let pipe = parse::<Rows<(String, f64, f64)>>(PIPE, "thread_pipe")
            .rows
            .into_iter()
            .map(|(name, tpi, major)| Thread {
                name,
                family: Family::Pipe,
                major,
                pitch: MM_PER_INCH / tpi,
                tap_drill: major - 1.28065 * MM_PER_INCH / tpi,
                coarse: true,
            });
        metric.chain(unified).chain(pipe).collect()
    })
}

pub fn known() -> String {
    all()
        .iter()
        .map(|t| t.name.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

fn unknown(spec: &str) -> anyhow::Error {
    anyhow!("unknown thread `{spec}`; known: {}", known())
}

pub fn find(spec: &str) -> Result<Thread> {
    let text = spec.trim();
    let lower = text.to_ascii_lowercase();
    let table = all();
    if let Some(rest) = lower.strip_prefix('m') {
        let (major, pitch) = match rest.split_once(['x', '*']) {
            Some((major, pitch)) => (major, Some(pitch)),
            None => (rest, None),
        };
        let major: f64 = major.parse().map_err(|_| unknown(spec))?;
        let pitch: Option<f64> = pitch
            .map(|p| p.parse().map_err(|_| unknown(spec)))
            .transpose()?;
        return table
            .iter()
            .filter(|t| t.family == Family::Metric && (t.major - major).abs() < 1.0e-9)
            .find(|t| match pitch {
                Some(p) => (t.pitch - p).abs() < 1.0e-9,
                None => t.coarse,
            })
            .cloned()
            .ok_or_else(|| unknown(spec));
    }
    if lower.starts_with('g') {
        return table
            .iter()
            .find(|t| t.family == Family::Pipe && t.name.eq_ignore_ascii_case(text))
            .cloned()
            .ok_or_else(|| unknown(spec));
    }
    let (size, tpi) = match text.rsplit_once('-') {
        Some((size, tpi)) if tpi.parse::<f64>().is_ok() => (size, tpi.parse::<f64>().ok()),
        _ => (text, None),
    };
    let size = size.trim_end_matches(['"', ' ']);
    table
        .iter()
        .filter(|t| {
            t.family == Family::Unified && t.name.rsplit_once('-').map(|(s, _)| s) == Some(size)
        })
        .find(|t| match tpi {
            Some(n) => (MM_PER_INCH / t.pitch - n).abs() < 1.0e-6,
            None => t.coarse,
        })
        .cloned()
        .ok_or_else(|| unknown(spec))
}

pub fn clearance(thread: &Thread) -> Option<[f64; 3]> {
    if thread.family != Family::Metric {
        return None;
    }
    parse::<Rows<[f64; 4]>>(CLEARANCE, "thread_clearance")
        .rows
        .into_iter()
        .find(|row| (row[0] - thread.major).abs() < 1.0e-9)
        .map(|row| [row[1], row[2], row[3]])
}

#[derive(Debug, Clone, PartialEq)]
pub struct Head {
    pub standard: &'static str,
    pub head_dia: f64,
    pub head_height: f64,
    pub bore_dia: f64,
    pub bore_depth: f64,
}

#[derive(Deserialize)]
struct HeadFile {
    iso4762: Vec<[f64; 4]>,
    iso7380: Vec<[f64; 3]>,
    iso10642: Vec<[f64; 3]>,
}

pub fn heads(thread: &Thread) -> Vec<Head> {
    let file: HeadFile = parse(HEADS, "thread_heads");
    let at = |major: f64| (major - thread.major).abs() < 1.0e-9;
    let mut out = Vec::new();
    if thread.family != Family::Metric {
        return out;
    }
    if let Some(r) = file.iso4762.iter().find(|r| at(r[0])) {
        out.push(Head {
            standard: "ISO 4762",
            head_dia: r[1],
            head_height: r[2],
            bore_dia: r[3],
            bore_depth: r[2] + 0.4,
        });
    }
    if let Some(r) = file.iso7380.iter().find(|r| at(r[0])) {
        out.push(Head {
            standard: "ISO 7380",
            head_dia: r[1],
            head_height: r[2],
            bore_dia: r[1] + 1.0,
            bore_depth: r[2] + 0.4,
        });
    }
    if let Some(r) = file.iso10642.iter().find(|r| at(r[0])) {
        let bore = ((r[1] + 0.2) * 100.0).round() / 100.0;
        out.push(Head {
            standard: "ISO 10642",
            head_dia: r[1],
            head_height: r[2],
            bore_dia: bore,
            bore_depth: (bore - thread.major) / 2.0,
        });
    }
    out
}

#[derive(Debug, Clone)]
pub struct Torque {
    pub class: String,
    pub proof_mpa: f64,
    pub stress_area: f64,
    pub proof_load_kn: f64,
    pub newton_metres: f64,
}

#[derive(Deserialize)]
struct Strength {
    preload_fraction: f64,
    proof: std::collections::BTreeMap<String, f64>,
}

fn strength() -> &'static Strength {
    static FILE: OnceLock<Strength> = OnceLock::new();
    FILE.get_or_init(|| parse(STRENGTH, "thread_strength"))
}

pub fn classes() -> Vec<String> {
    strength().proof.keys().cloned().collect()
}

pub fn torque(thread: &Thread, class: &str, nut_factor: f64) -> Result<Torque> {
    if thread.family != Family::Metric {
        bail!(
            "strength and torque are tabulated for metric screws, not {}",
            thread.name
        );
    }
    if nut_factor <= 0.0 {
        bail!("nut factor must be above 0");
    }
    let file = strength();
    let key = class.to_ascii_uppercase();
    let proof = file
        .proof
        .iter()
        .find(|(k, _)| k.to_ascii_uppercase() == key)
        .ok_or_else(|| {
            anyhow!(
                "unknown strength class `{class}`; known: {}",
                classes().join(", ")
            )
        })?;
    let area = thread.stress_area();
    let preload_n = file.preload_fraction * proof.1 * area;
    Ok(Torque {
        class: proof.0.clone(),
        proof_mpa: *proof.1,
        stress_area: area,
        proof_load_kn: proof.1 * area / 1000.0,
        newton_metres: nut_factor * preload_n * thread.major / 1000.0,
    })
}

pub fn tolerance_g(thread: &Thread) -> Option<f64> {
    #[derive(Deserialize)]
    struct File {
        g: Vec<[f64; 2]>,
    }
    let file: File = parse(TOLERANCE, "thread_tolerance");
    file.g
        .iter()
        .find(|row| (row[0] - thread.pitch).abs() < 1.0e-9)
        .map(|row| row[1])
}

pub fn engagement(material: &Material, class: &str, diameter: f64) -> f64 {
    let strong = class.starts_with("10.9") || class.starts_with("12.9");
    material.engagement[usize::from(strong)] * diameter
}

#[derive(Debug, Clone)]
pub struct DepthCheck {
    pub ok: bool,
    pub through: bool,
    pub hole_depth: f64,
    pub usable: f64,
    pub required: f64,
    pub needed_depth: f64,
}

pub fn check_depth(
    thread: &Thread,
    material: &Material,
    class: &str,
    hole_depth: f64,
    through: bool,
) -> DepthCheck {
    let required = engagement(material, class, thread.major);
    let runout = if through {
        0.0
    } else {
        RUNOUT_PITCHES * thread.pitch
    };
    let usable = hole_depth - runout;
    DepthCheck {
        ok: usable + 1.0e-9 >= required,
        through,
        hole_depth,
        usable,
        required,
        needed_depth: required + runout,
    }
}

pub struct Input {
    pub thread: Thread,
    pub material: &'static Material,
    pub class: String,
    pub nut_factor: f64,
    pub depth: Option<f64>,
    pub through: bool,
}

pub fn report(input: &Input) -> Result<Report> {
    let t = &input.thread;
    let mut rows = vec![
        (
            "thread".into(),
            format!(
                "{} pitch {:.3} mm{}",
                t.name,
                t.pitch,
                if t.coarse { " (coarse)" } else { "" }
            ),
        ),
        ("major dia".into(), format!("{:.3} mm", t.major)),
        ("pitch dia".into(), format!("{:.3} mm", t.pitch_dia())),
        (
            "minor dia".into(),
            format!(
                "{:.3} mm nut, {:.3} mm screw",
                t.minor_internal(),
                t.minor_external()
            ),
        ),
        ("tap drill".into(), format!("{:.2} mm", t.tap_drill)),
    ];
    if let Some(c) = clearance(t) {
        rows.push((
            "clearance".into(),
            format!(
                "{} close, {} normal, {} loose mm (ISO 273)",
                c[0], c[1], c[2]
            ),
        ));
    }
    for head in heads(t) {
        rows.push((
            head.standard.into(),
            format!(
                "head {} x {} mm, counterbore {} x {:.1} mm",
                head.head_dia, head.head_height, head.bore_dia, head.bore_depth
            ),
        ));
    }
    if t.family == Family::Metric {
        if let Some(g) = tolerance_g(t) {
            rows.push((
                "tolerance".into(),
                format!("6H nut, 6g screw (es {g} um), medium class"),
            ));
        }
        let torque = torque(t, &input.class, input.nut_factor)?;
        rows.push((
            "strength".into(),
            format!(
                "class {} proof {} MPa, stress area {:.1} mm2, proof load {:.2} kN",
                torque.class, torque.proof_mpa, torque.stress_area, torque.proof_load_kn
            ),
        ));
        rows.push((
            "torque".into(),
            format!(
                "{:.1} N.m at nut factor {}",
                torque.newton_metres, input.nut_factor
            ),
        ));
    }
    let wanted = engagement(input.material, &input.class, t.major);
    rows.push((
        "engagement".into(),
        format!("{:.1} mm minimum in {}", wanted, input.material.name),
    ));
    let mut why = format!(
        "In {} the screw needs {:.1} mm of thread so it breaks before the thread strips.",
        input.material.name, wanted
    );
    if let Some(depth) = input.depth {
        let check = check_depth(t, input.material, &input.class, depth, input.through);
        let kind = if check.through { "through" } else { "blind" };
        rows.push((
            "depth".into(),
            format!(
                "{kind} {:.1} mm gives {:.1} mm of thread: {}",
                check.hole_depth,
                check.usable,
                if check.ok {
                    "ok".to_string()
                } else {
                    format!("short, needs {:.1} mm deep", check.needed_depth)
                }
            ),
        ));
        if !check.ok {
            why = format!(
                "The hole is short: {:.1} mm of thread is usable and {:.1} mm is needed, so drill it {:.1} mm deep.",
                check.usable, check.required, check.needed_depth
            );
        }
    }
    if input.material.print {
        rows.push((
            "print".into(),
            "printed threads are weak: tap a heat-set insert or print the hole 0.2 mm over".into(),
        ));
    }
    let mut sources: Vec<String> = SOURCES.iter().map(|s| s.to_string()).collect();
    sources.extend(materials::SOURCES.iter().take(1).map(|s| s.to_string()));
    Ok(Report {
        title: format!("thread {}", t.name),
        rows,
        why,
        formulas: vec![
            "D1 = D - 1.0825 P, D2 = D - 0.6495 P, d3 = D - 1.2269 P".into(),
            "As = pi/4 (D - 0.9382 P)^2".into(),
            "T = K x 0.85 x Sp x As x D  (N.mm / 1000 for N.m)".into(),
            "blind hole depth = engagement + 3 P of tap chamfer".into(),
        ],
        sources,
    })
}

pub const MIN_ENGAGEMENT_DIAMETERS: f64 = 1.0;

pub fn short_note(name: &str, length: f64) -> Option<String> {
    let thread = find(name).ok()?;
    let least = MIN_ENGAGEMENT_DIAMETERS * thread.major;
    (length + 1.0e-9 < least).then(|| {
        format!(
            "warning: {:.2} mm of {} thread is under the {:.1} mm minimum engagement even in hardened steel; run gcad calc thread {} --depth for your material",
            length, thread.name, least, thread.name
        )
    })
}
