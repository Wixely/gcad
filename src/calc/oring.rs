use super::Report;
use super::tables::{self, MM_PER_INCH, Ring, Span};
use anyhow::{Result, anyhow, bail};
use std::str::FromStr;

pub const MAX_STRETCH_PCT: f64 = 3.0;
pub const MAX_FILL_PCT: f64 = 85.0;
pub const LEAD_IN_DEGREES: f64 = 20.0;
pub const HANDBOOK: &str =
    "https://www.texasseal.com/pdfs/content/products/ORD_5700_parker_O-Ring_handbook_1.pdf";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Application {
    StaticFace,
    StaticRadial,
    Rod,
    Piston,
}

impl Application {
    pub fn gland_kind(self) -> &'static str {
        match self {
            Application::StaticFace => "face",
            Application::StaticRadial => "static",
            Application::Rod | Application::Piston => "dynamic",
        }
    }

    pub fn chart(self) -> &'static str {
        match self {
            Application::StaticFace => "Design Chart 4-3 (face seal glands)",
            Application::StaticRadial => "Design Chart 4-2 (industrial static glands)",
            Application::Rod | Application::Piston => "Design Chart 5-2-a (reciprocating glands)",
        }
    }
}

impl FromStr for Application {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> Result<Self> {
        match text.to_ascii_lowercase().as_str() {
            "face" | "static-face" | "static_face" => Ok(Application::StaticFace),
            "radial" | "static" | "static-radial" | "static_radial" => {
                Ok(Application::StaticRadial)
            }
            "rod" => Ok(Application::Rod),
            "piston" => Ok(Application::Piston),
            other => bail!("application is face, static, rod or piston, not `{other}`"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Material {
    Nitrile,
    Epdm,
    Fluorocarbon,
    Polyurethane,
    Silicone,
    Fluorosilicone,
}

impl Material {
    pub fn halves_clearance(self) -> bool {
        matches!(self, Material::Silicone | Material::Fluorosilicone)
    }
}

impl FromStr for Material {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> Result<Self> {
        match text.to_ascii_lowercase().as_str() {
            "nitrile" | "nbr" | "buna" => Ok(Material::Nitrile),
            "epdm" => Ok(Material::Epdm),
            "fkm" | "fluorocarbon" | "viton" => Ok(Material::Fluorocarbon),
            "polyurethane" | "pu" | "urethane" => Ok(Material::Polyurethane),
            "silicone" | "vmq" => Ok(Material::Silicone),
            "fluorosilicone" | "fvmq" => Ok(Material::Fluorosilicone),
            other => bail!(
                "material is nitrile, epdm, fkm, polyurethane, silicone or fluorosilicone, not `{other}`"
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Input {
    pub application: Application,
    pub diameter: f64,
    pub cross_section: Option<f64>,
    pub pressure_bar: f64,
    pub material: Material,
    pub clearance: Option<f64>,
    pub ring: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Choice {
    pub ring: Ring,
    pub stretch_pct: f64,
    pub squeeze: (f64, f64),
    pub squeeze_pct: (f64, f64),
    pub fill_pct: f64,
    pub rejected: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Design {
    pub input: Input,
    pub choices: Vec<Choice>,
    pub depth: (f64, f64),
    pub depth_mid: f64,
    pub width: f64,
    pub groove_diameter: f64,
    pub seat_diameter: f64,
    pub backup: bool,
    pub backup_reason: String,
    pub clearance: (f64, f64),
    pub chamfer: (f64, f64, f64),
    pub tolerance: f64,
}

impl Design {
    pub fn best(&self) -> &Choice {
        &self.choices[0]
    }

    pub fn groove_inner_diameter(&self) -> f64 {
        match self.input.application {
            Application::StaticFace => self.groove_diameter,
            Application::Rod => self.input.diameter,
            _ => self.groove_diameter,
        }
    }

    pub fn groove_outer_diameter(&self) -> f64 {
        match self.input.application {
            Application::StaticFace => self.groove_diameter + 2.0 * self.width,
            Application::Rod => self.groove_diameter,
            _ => self.input.diameter,
        }
    }
}

pub fn section_loss_pct(stretch_pct: f64) -> f64 {
    let x = stretch_pct;
    if x <= 0.0 {
        0.0
    } else if x <= 3.0 {
        -0.005 + 1.19 * x - 0.19 * x * x - 0.001 * x.powi(3) + 0.008 * x.powi(4)
    } else {
        0.56 + 0.59 * x - 0.0046 * x * x
    }
}

pub fn evaluate(ring: &Ring, depth: (f64, f64), width: f64, stretch_pct: f64, tol: f64) -> Choice {
    let loss = section_loss_pct(stretch_pct);
    let cs = ring.cs * (1.0 - loss / 100.0);
    let squeeze = (cs - tol - depth.1, cs + tol - depth.0);
    let squeeze_pct = (squeeze.0 / ring.cs * 100.0, squeeze.1 / ring.cs * 100.0);
    let area = std::f64::consts::PI / 4.0 * ring.cs * ring.cs;
    let fill_pct = 100.0 * area / (width * (depth.0 + depth.1) / 2.0);
    let mut rejected = Vec::new();
    if stretch_pct > MAX_STRETCH_PCT {
        rejected.push(format!(
            "stretch {stretch_pct:.1}% is over the {MAX_STRETCH_PCT}% limit"
        ));
    }
    if stretch_pct < -MAX_STRETCH_PCT {
        rejected.push(format!(
            "ring is {:.1}% larger than its seat and would sag",
            -stretch_pct
        ));
    }
    if fill_pct > MAX_FILL_PCT {
        rejected.push(format!(
            "gland fill {fill_pct:.0}% is over the {MAX_FILL_PCT}% limit"
        ));
    }
    if squeeze.0 <= 0.0 {
        rejected.push("no squeeze at the loose tolerance limit".into());
    }
    Choice {
        ring: ring.clone(),
        stretch_pct,
        squeeze,
        squeeze_pct,
        fill_pct,
        rejected,
    }
}

fn default_group(diameter: f64) -> usize {
    match diameter {
        d if d < 20.0 => 0,
        d if d < 50.0 => 1,
        d if d < 150.0 => 2,
        d if d < 250.0 => 3,
        _ => 4,
    }
}

fn span_mm(span: Span) -> (f64, f64) {
    (span.0 * MM_PER_INCH, span.1 * MM_PER_INCH)
}

pub fn design(input: &Input) -> Result<Design> {
    if !(input.diameter > 0.0) {
        bail!("diameter must be positive, got {}", input.diameter);
    }
    if input.pressure_bar < 0.0 {
        bail!("pressure cannot be negative, got {}", input.pressure_bar);
    }
    let forced = input
        .ring
        .as_deref()
        .map(|spec| tables::ring(spec).ok_or_else(|| anyhow!("no standard o-ring `{spec}`")))
        .transpose()?;
    let group = match (forced, input.cross_section) {
        (Some(ring), _) => ring.group(),
        (None, Some(cs)) if cs > 0.0 => tables::cross_section_group(cs),
        (None, Some(cs)) => bail!("cross-section must be positive, got {cs}"),
        (None, None) => default_group(input.diameter),
    };
    let kind = input.application.gland_kind();
    let gland =
        tables::gland(kind, group).ok_or_else(|| anyhow!("no {kind} gland for group {group}"))?;
    let handbook = tables::backup();

    let base = gland.clearance.unwrap_or(Span(0.0, 0.0));
    let factor = if input.material.halves_clearance() {
        handbook.silicone_clearance_factor
    } else {
        1.0
    };
    let clearance = (base.0 * MM_PER_INCH * factor, base.1 * MM_PER_INCH * factor);
    let (backup, backup_reason) = if input.pressure_bar > handbook.bar {
        (
            true,
            format!(
                "{} bar is over {} bar (ORD 5700 section 4.0)",
                input.pressure_bar, handbook.bar
            ),
        )
    } else if input.clearance.is_some_and(|c| c > clearance.1) && kind != "face" {
        (
            true,
            format!(
                "{} mm diametral clearance is over the {:.3} mm the gland allows",
                input.clearance.unwrap_or_default(),
                clearance.1
            ),
        )
    } else {
        (
            false,
            format!(
                "{} bar is within {} bar and the gap is tight",
                input.pressure_bar, handbook.bar
            ),
        )
    };

    let width_span = match (backup, gland.width1) {
        (true, Some(span)) => span,
        (true, None) => bail!(
            "{} bar needs a back-up ring, which the face seal table does not cover; use a radial gland",
            input.pressure_bar
        ),
        (false, _) => gland.width,
    };
    let depth = span_mm(gland.depth);
    let depth_mid = (depth.0 + depth.1) / 2.0;
    let width = span_mm(width_span).0 / 2.0 + span_mm(width_span).1 / 2.0;
    let tolerance = gland.tol * MM_PER_INCH;
    let (groove_diameter, seat_diameter) = match input.application {
        Application::StaticFace => (input.diameter, input.diameter),
        Application::StaticRadial | Application::Piston => {
            let g = input.diameter - 2.0 * depth_mid;
            (g, g)
        }
        Application::Rod => (input.diameter + 2.0 * depth_mid, input.diameter),
    };
    if groove_diameter <= 0.0 {
        bail!(
            "a {} mm diameter is too small for this gland depth",
            input.diameter
        );
    }

    let stretch = |ring: &Ring| (seat_diameter - ring.id) / ring.id * 100.0;
    let mut choices: Vec<Choice> = match forced {
        Some(ring) => vec![evaluate(ring, depth, width, stretch(ring), tolerance)],
        None => {
            let wanted = input.cross_section;
            let mut near: Vec<&Ring> = tables::rings()
                .iter()
                .filter(|r| r.group() == group)
                .filter(|r| wanted.is_none_or(|cs| (r.cs - cs).abs() < 0.005))
                .collect();
            near.sort_by(|a, b| {
                (seat_diameter - a.id)
                    .abs()
                    .total_cmp(&(seat_diameter - b.id).abs())
            });
            near.into_iter()
                .take(2)
                .map(|ring| evaluate(ring, depth, width, stretch(ring), tolerance))
                .collect()
        }
    };
    choices.sort_by_key(|c| !c.rejected.is_empty());
    let max_squeeze = choices[0].squeeze.1.max(0.0);
    let radial = max_squeeze;
    let axial = radial / LEAD_IN_DEGREES.to_radians().tan();
    Ok(Design {
        input: input.clone(),
        choices,
        depth,
        depth_mid,
        width,
        groove_diameter,
        seat_diameter,
        backup,
        backup_reason,
        clearance,
        chamfer: (LEAD_IN_DEGREES, radial, axial),
        tolerance,
    })
}

pub fn report(design: &Design) -> Report {
    let input = &design.input;
    let best = design.best();
    let mut rows = Vec::new();
    for (i, c) in design.choices.iter().enumerate() {
        let verdict = if c.rejected.is_empty() {
            "ok".to_string()
        } else {
            format!("rejected: {}", c.rejected.join("; "))
        };
        rows.push((
            format!("size {}", i + 1),
            format!(
                "{} id {} x {} mm | stretch {:+.1}% | squeeze {:.0}-{:.0}% ({:.2}-{:.2} mm) | fill {:.0}% | {verdict}",
                c.ring.name(),
                c.ring.id,
                c.ring.cs,
                c.stretch_pct,
                c.squeeze_pct.0,
                c.squeeze_pct.1,
                c.squeeze.0,
                c.squeeze.1,
                c.fill_pct
            ),
        ));
    }
    rows.push((
        "groove".into(),
        format!(
            "width {:.2} mm, depth {:.2}-{:.2} mm, groove diameter {:.2} mm",
            design.width, design.depth.0, design.depth.1, design.groove_diameter
        ),
    ));
    rows.push((
        "clearance".into(),
        format!(
            "diametral {:.3}-{:.3} mm max",
            design.clearance.0, design.clearance.1
        ),
    ));
    rows.push((
        "back-up ring".into(),
        if design.backup {
            format!(
                "yes, one on the low pressure side: {}",
                design.backup_reason
            )
        } else {
            format!("no: {}", design.backup_reason)
        },
    ));
    rows.push((
        "lead-in".into(),
        format!(
            "{:.0} deg chamfer, {:.2} mm radial x {:.2} mm long",
            design.chamfer.0, design.chamfer.1, design.chamfer.2
        ),
    ));
    let why = format!(
        "{} for a {:.1} mm {} at {} bar: stretch {:+.1}%, squeeze {:.0}-{:.0}%, fill {:.0}%, gland from {}",
        best.ring.name(),
        input.diameter,
        match input.application {
            Application::StaticFace => "face seal",
            Application::StaticRadial => "static radial seal",
            Application::Rod => "rod seal",
            Application::Piston => "piston seal",
        },
        input.pressure_bar,
        best.stretch_pct,
        best.squeeze_pct.0,
        best.squeeze_pct.1,
        best.fill_pct,
        input.application.chart()
    );
    Report {
        title: format!(
            "o-ring {} {} mm, {} bar",
            match input.application {
                Application::StaticFace => "static face",
                Application::StaticRadial => "static radial",
                Application::Rod => "dynamic rod",
                Application::Piston => "dynamic piston",
            },
            input.diameter,
            input.pressure_bar
        ),
        rows,
        why,
        formulas: vec![
            "stretch % = (seat diameter - ring ID) / ring ID x 100, limit 3 %".into(),
            "squeeze min = CS_eff - CS tolerance - depth max; squeeze max = CS_eff + CS tolerance - depth min".into(),
            "CS_eff = CS x (1 - Y/100), Y = -0.005 + 1.19X - 0.19X^2 - 0.001X^3 + 0.008X^4 for X <= 3 % stretch (ORD 5700 Figure 3-3)".into(),
            "fill % = (pi/4 x CS^2) / (groove width x mean depth) x 100, limit 85 %".into(),
            "groove diameter = bore - 2 x depth (piston, static radial), rod + 2 x depth (rod)".into(),
            "back-up ring above 103.5 bar or when the clearance exceeds the gland limit; silicone halves the limit".into(),
            "lead-in: 20 deg chamfer, radial length = max squeeze, axial length = radial / tan 20 deg".into(),
        ],
        sources: vec![
            format!("Parker O-Ring Handbook ORD 5700, {}: {HANDBOOK}", input.application.chart()),
            "ring sizes: AS568 and ISO 3601-1 tables in src/calc/data, see file headers for URLs".into(),
        ],
    }
}
