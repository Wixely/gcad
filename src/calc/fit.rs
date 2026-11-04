use super::Report;
use anyhow::{Result, anyhow, bail};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const GRADES: &str = include_str!("data/iso286_it.toml");
const SHAFT: &str = include_str!("data/iso286_shaft.toml");
const DELTA: &str = include_str!("data/iso286_delta.toml");
const NOTES: &str = include_str!("data/fit_notes.toml");

pub const SOURCES: [&str; 2] = [
    "https://mechsheet.com/it-grades/ (ISO 286-1:2010 Table 1)",
    "https://mechsheet.com/tolerance/fundamental-deviations/ (ISO 286-1:2010 Tables 2 and 3)",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Clearance,
    Transition,
    Interference,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Clearance => "clearance",
            Kind::Transition => "transition",
            Kind::Interference => "interference",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Fit {
    pub pair: String,
    pub nominal: f64,
    pub hole: Limits,
    pub shaft: Limits,
    pub min_clearance: f64,
    pub max_clearance: f64,
    pub kind: Kind,
}

#[derive(Deserialize)]
struct ShaftFile {
    letters: Vec<String>,
    rows: Vec<Vec<f64>>,
}

#[derive(Deserialize)]
struct DeltaFile {
    rows: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Note {
    pub name: String,
    #[serde(rename = "use")]
    pub usage: String,
    pub cnc: String,
    pub print: String,
}

#[derive(Deserialize)]
struct NoteFile {
    fit: Vec<Note>,
}

fn parse<T: for<'a> Deserialize<'a>>(text: &str, name: &str) -> T {
    toml::from_str(text).unwrap_or_else(|e| panic!("calc table {name}: {e}"))
}

fn grades() -> &'static BTreeMap<String, Vec<f64>> {
    static FILE: OnceLock<BTreeMap<String, Vec<f64>>> = OnceLock::new();
    FILE.get_or_init(|| parse(GRADES, "iso286_it"))
}

fn shaft_file() -> &'static ShaftFile {
    static FILE: OnceLock<ShaftFile> = OnceLock::new();
    FILE.get_or_init(|| parse(SHAFT, "iso286_shaft"))
}

fn delta_file() -> &'static DeltaFile {
    static FILE: OnceLock<DeltaFile> = OnceLock::new();
    FILE.get_or_init(|| parse(DELTA, "iso286_delta"))
}

pub fn notes() -> &'static [Note] {
    static FILE: OnceLock<NoteFile> = OnceLock::new();
    &FILE.get_or_init(|| parse(NOTES, "fit_notes")).fit
}

fn tolerance(grade: u32, nominal: f64) -> Result<f64> {
    let table = grades();
    let column = table["bands"]
        .iter()
        .position(|top| nominal <= *top)
        .ok_or_else(|| anyhow!("fits go up to 500 mm, not {nominal}"))?;
    table
        .get(&format!("IT{grade}"))
        .map(|row| row[column])
        .ok_or_else(|| anyhow!("grade IT{grade} is not known; use 4 to 12"))
}

fn deviation(letter: &str, nominal: f64) -> Result<f64> {
    let file = shaft_file();
    let column = file
        .letters
        .iter()
        .position(|l| l == letter)
        .ok_or_else(|| anyhow!("fit letter `{letter}` is not known"))?;
    let row = file
        .rows
        .iter()
        .find(|row| nominal <= row[0])
        .ok_or_else(|| anyhow!("fits go up to 500 mm, not {nominal}"))?;
    Ok(row[column + 1])
}

fn delta(grade: u32, nominal: f64) -> Result<f64> {
    let row = delta_file()
        .rows
        .iter()
        .find(|row| nominal <= row[0])
        .ok_or_else(|| anyhow!("fits go up to 500 mm, not {nominal}"))?;
    Ok(row[grade as usize - 2])
}

fn split(class: &str) -> Result<(&str, u32)> {
    let at = class.find(|c: char| c.is_ascii_digit()).ok_or_else(|| {
        anyhow!("a tolerance class is a letter and a grade, like H7; got `{class}`")
    })?;
    let (letters, grade) = class.split_at(at);
    let grade = grade
        .parse()
        .map_err(|_| anyhow!("`{class}` has no grade number"))?;
    Ok((letters, grade))
}

pub fn limits(class: &str, nominal: f64) -> Result<Limits> {
    if !(nominal > 0.0 && nominal <= 500.0) {
        bail!("fits cover sizes above 0 and up to 500 mm, not {nominal}");
    }
    let (letters, grade) = split(class)?;
    let width = tolerance(grade, nominal)?;
    let hole = letters.chars().all(|c| c.is_ascii_uppercase());
    let (low, high) = if letters.eq_ignore_ascii_case("js") {
        (-width / 2.0, width / 2.0)
    } else if hole {
        hole_zone(&letters.to_ascii_lowercase(), grade, nominal, width)?
    } else {
        shaft_zone(letters, grade, nominal, width)?
    };
    let to_mm = |um: f64| ((nominal + um / 1000.0) * 1.0e6).round() / 1.0e6;
    Ok(Limits {
        min: to_mm(low),
        max: to_mm(high),
    })
}

fn shaft_zone(letter: &str, grade: u32, nominal: f64, width: f64) -> Result<(f64, f64)> {
    let value = deviation(letter, nominal)?;
    Ok(match letter {
        "c" | "d" | "e" | "f" | "g" | "h" => (value - width, value),
        "k" if !(4..=7).contains(&grade) => (0.0, width),
        _ => (value, value + width),
    })
}

fn hole_zone(letter: &str, grade: u32, nominal: f64, width: f64) -> Result<(f64, f64)> {
    let value = deviation(letter, nominal)?;
    match letter {
        "c" | "d" | "e" | "f" | "g" | "h" => Ok((-value, -value + width)),
        "k" | "m" | "n" if (3..=8).contains(&grade) => {
            let upper = -value + delta(grade, nominal)?;
            Ok((upper - width, upper))
        }
        "p" | "r" | "s" if (3..=7).contains(&grade) => {
            let upper = -value + delta(grade, nominal)?;
            Ok((upper - width, upper))
        }
        "k" | "m" | "n" | "p" | "r" | "s" => bail!(
            "hole {} is tabulated to IT{} here, not IT{grade}",
            letter.to_uppercase(),
            if matches!(letter, "k" | "m" | "n") {
                8
            } else {
                7
            }
        ),
        _ => unreachable!(),
    }
}

pub fn fit(pair: &str, nominal: f64) -> Result<Fit> {
    let (hole, shaft) = pair.split_once('/').ok_or_else(|| {
        anyhow!("a fit is a hole class and a shaft class, like H7/g6; got `{pair}`")
    })?;
    let (hole, shaft) = (limits(hole, nominal)?, limits(shaft, nominal)?);
    let min_clearance = ((hole.min - shaft.max) * 1.0e6).round() / 1.0e6;
    let max_clearance = ((hole.max - shaft.min) * 1.0e6).round() / 1.0e6;
    let kind = if min_clearance >= 0.0 {
        Kind::Clearance
    } else if max_clearance <= 0.0 {
        Kind::Interference
    } else {
        Kind::Transition
    };
    Ok(Fit {
        pair: pair.to_string(),
        nominal,
        hole,
        shaft,
        min_clearance,
        max_clearance,
        kind,
    })
}

pub fn report(fit: &Fit) -> Report {
    let mut rows = vec![
        (
            "hole".into(),
            format!("{:.3} to {:.3} mm", fit.hole.min, fit.hole.max),
        ),
        (
            "shaft".into(),
            format!("{:.3} to {:.3} mm", fit.shaft.min, fit.shaft.max),
        ),
        (
            "clearance".into(),
            format!("{:+.3} to {:+.3} mm", fit.min_clearance, fit.max_clearance),
        ),
        ("type".into(), fit.kind.name().into()),
    ];
    let note = notes()
        .iter()
        .find(|n| n.name.eq_ignore_ascii_case(&fit.pair));
    if let Some(note) = note {
        rows.push(("use".into(), note.usage.clone()));
        rows.push(("cnc".into(), note.cnc.clone()));
        rows.push(("print".into(), note.print.clone()));
    }
    let why = match fit.kind {
        Kind::Clearance => format!(
            "The hole is always larger than the shaft, by {:.0} to {:.0} um.",
            fit.min_clearance * 1000.0,
            fit.max_clearance * 1000.0
        ),
        Kind::Interference => format!(
            "The shaft is always larger than the hole, by {:.0} to {:.0} um, so it must be pressed in.",
            -fit.max_clearance * 1000.0,
            -fit.min_clearance * 1000.0
        ),
        Kind::Transition => {
            "Depending on the parts, the fit can be a slight clearance or a slight interference."
                .into()
        }
    };
    Report {
        title: format!("fit {} at {} mm", fit.pair, fit.nominal),
        rows,
        why,
        formulas: vec![
            "hole H: EI = 0, ES = EI + IT; hole A to G: EI = -es of the same shaft letter".into(),
            "shaft c to h: es from table, ei = es - IT; shaft k to s: ei from table, es = ei + IT"
                .into(),
            "hole K to S: ES = -ei + delta, EI = ES - IT; js: +-IT/2".into(),
            "clearance = hole - shaft, so min = hole min - shaft max, max = hole max - shaft min"
                .into(),
        ],
        sources: SOURCES.iter().map(|s| s.to_string()).collect(),
    }
}
