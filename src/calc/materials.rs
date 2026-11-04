use super::Report;
use anyhow::{Result, bail};
use serde::Deserialize;
use std::sync::OnceLock;

const TABLE: &str = include_str!("data/materials.toml");

pub const SOURCES: [&str; 3] = [
    "https://tabelic.com/threads/thread-engagement-chart/",
    "https://3dprintcalcs.uk/reference/filament-properties/",
    "https://www.engineeringtoolbox.com/engineering-materials-properties-d_1225.html",
];

#[derive(Debug, Clone, Deserialize)]
pub struct Material {
    pub name: String,
    pub aliases: Vec<String>,
    pub density: f64,
    pub tensile: f64,
    pub engagement: [f64; 2],
    pub print: bool,
}

#[derive(Deserialize)]
struct File {
    material: Vec<Material>,
}

pub fn all() -> &'static [Material] {
    static FILE: OnceLock<File> = OnceLock::new();
    &FILE
        .get_or_init(|| {
            toml::from_str(TABLE).unwrap_or_else(|e| panic!("calc table materials: {e}"))
        })
        .material
}

pub fn find(name: &str) -> Result<&'static Material> {
    let wanted = name.trim().to_ascii_lowercase();
    match all()
        .iter()
        .find(|m| m.name == wanted || m.aliases.contains(&wanted))
    {
        Some(material) => Ok(material),
        None => bail!(
            "unknown material `{name}`; known: {}",
            all()
                .iter()
                .map(|m| m.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

pub fn report(material: &Material) -> Report {
    Report {
        title: format!("material {}", material.name),
        rows: vec![
            ("density".into(), format!("{} g/cm3", material.density)),
            ("tensile".into(), format!("{} MPa", material.tensile)),
            (
                "engagement".into(),
                format!(
                    "{} x d (class 8.8 and below), {} x d (10.9 and 12.9)",
                    material.engagement[0], material.engagement[1]
                ),
            ),
            (
                "process".into(),
                if material.print {
                    "3D printed in layers"
                } else {
                    "machined"
                }
                .into(),
            ),
        ],
        why: if material.print {
            "Printed plastic strips easily, so threads need 3 diameters or a heat-set insert."
                .into()
        } else {
            "Softer metals need more thread engagement before the screw breaks before the thread strips.".into()
        },
        formulas: vec!["minimum engagement = factor x nominal diameter".into()],
        sources: SOURCES.iter().map(|s| s.to_string()).collect(),
    }
}
