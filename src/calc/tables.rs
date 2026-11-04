use serde::Deserialize;
use std::sync::OnceLock;

const AS568_0XX: &str = include_str!("data/as568_0xx.toml");
const AS568_1XX: &str = include_str!("data/as568_1xx.toml");
const AS568_2XX: &str = include_str!("data/as568_2xx.toml");
const ISO_SERIES: [&str; 5] = [
    include_str!("data/iso3601_1_80.toml"),
    include_str!("data/iso3601_2_65.toml"),
    include_str!("data/iso3601_3_55.toml"),
    include_str!("data/iso3601_5_30.toml"),
    include_str!("data/iso3601_7_00.toml"),
];
const GLAND: &str = include_str!("data/gland.toml");

pub const MM_PER_INCH: f64 = 25.4;
pub const CROSS_SECTIONS: [f64; 5] = [1.78, 2.62, 3.53, 5.33, 6.99];

#[derive(Debug, Clone, PartialEq)]
pub enum Series {
    As568(u32),
    Iso,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ring {
    pub series: Series,
    pub id: f64,
    pub cs: f64,
}

impl Ring {
    pub fn name(&self) -> String {
        match self.series {
            Series::As568(dash) => format!("AS568-{dash:03}"),
            Series::Iso => format!("ISO 3601 {}x{}", self.id, self.cs),
        }
    }

    pub fn spec(&self) -> String {
        match self.series {
            Series::As568(dash) => format!("{dash:03}"),
            Series::Iso => format!("{}x{}", self.id, self.cs),
        }
    }

    pub fn od(&self) -> f64 {
        self.id + 2.0 * self.cs
    }

    pub fn group(&self) -> usize {
        cross_section_group(self.cs)
    }
}

pub fn cross_section_group(cs: f64) -> usize {
    (0..CROSS_SECTIONS.len())
        .min_by(|a, b| {
            (CROSS_SECTIONS[*a] - cs)
                .abs()
                .total_cmp(&(CROSS_SECTIONS[*b] - cs).abs())
        })
        .unwrap_or(0)
}

#[derive(Deserialize)]
struct DashRows {
    rows: Vec<(u32, f64, f64)>,
}

#[derive(Deserialize)]
struct MetricRows {
    rows: Vec<(f64, f64)>,
}

fn parse<T: for<'a> Deserialize<'a>>(text: &str, name: &str) -> T {
    toml::from_str(text).unwrap_or_else(|e| panic!("calc table {name}: {e}"))
}

pub fn rings() -> &'static [Ring] {
    static RINGS: OnceLock<Vec<Ring>> = OnceLock::new();
    RINGS.get_or_init(|| {
        let dash = [AS568_0XX, AS568_1XX, AS568_2XX]
            .into_iter()
            .flat_map(|text| parse::<DashRows>(text, "as568").rows)
            .map(|(dash, id, cs)| Ring {
                series: Series::As568(dash),
                id,
                cs,
            });
        let metric = ISO_SERIES
            .into_iter()
            .flat_map(|text| parse::<MetricRows>(text, "iso3601").rows)
            .map(|(id, cs)| Ring {
                series: Series::Iso,
                id,
                cs,
            });
        dash.chain(metric).collect()
    })
}

pub fn ring(spec: &str) -> Option<&'static Ring> {
    let spec = spec.trim().trim_start_matches(['-', '0']);
    let all = rings();
    if let Ok(dash) = spec.parse::<u32>() {
        return all
            .iter()
            .find(|r| r.series == Series::As568(dash));
    }
    let (id, cs) = spec.split_once(['x', 'X'])?;
    let (id, cs): (f64, f64) = (id.parse().ok()?, cs.parse().ok()?);
    all.iter()
        .find(|r| r.series == Series::Iso && (r.id - id).abs() < 1.0e-6 && (r.cs - cs).abs() < 1.0e-6)
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Span(pub f64, pub f64);

impl Span {
    pub fn mid(&self) -> f64 {
        (self.0 + self.1) / 2.0
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Gland {
    pub kind: String,
    pub cs: f64,
    pub tol: f64,
    pub depth: Span,
    pub width: Span,
    pub width1: Option<Span>,
    pub width2: Option<Span>,
    pub clearance: Option<Span>,
    pub backup_width: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Backup {
    pub bar: f64,
    pub silicone_clearance_factor: f64,
}

#[derive(Deserialize)]
struct GlandFile {
    backup: Backup,
    gland: Vec<Gland>,
}

fn gland_file() -> &'static GlandFile {
    static FILE: OnceLock<GlandFile> = OnceLock::new();
    FILE.get_or_init(|| parse(GLAND, "gland"))
}

pub fn backup() -> &'static Backup {
    &gland_file().backup
}

pub fn gland(kind: &str, group: usize) -> Option<&'static Gland> {
    gland_file()
        .gland
        .iter()
        .filter(|g| g.kind == kind)
        .nth(group)
}
