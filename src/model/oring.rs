use super::Model;
use super::args::{Args, label_of, positive};
use super::features::{Depth, Removal};
use crate::calc::oring::{Application, Design, Input, Material, design};
use crate::geometry::Profile;
use crate::parse::Line;
use anyhow::{Result, bail};

const BORE_MARGIN: f64 = 1.0;

impl Model {
    pub(crate) fn op_oring_groove(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(
            line,
            &["application", "d"],
            &[
                "at",
                "offset",
                "pressure",
                "material",
                "cs",
                "ring",
                "clearance",
            ],
            false,
        )?;
        self.require_empty_sketch("oring_groove")?;
        let application: Application = args.text("application")?.parse()?;
        let diameter = positive(args.number("d", &self.scope)?, "o-ring groove diameter")?;
        let material: Material = args
            .values
            .get("material")
            .copied()
            .unwrap_or("nitrile")
            .parse()?;
        let input = Input {
            application,
            diameter,
            cross_section: args.optional_number("cs", &self.scope)?,
            pressure_bar: args
                .optional_number("pressure", &self.scope)?
                .unwrap_or(0.0),
            material,
            clearance: args.optional_number("clearance", &self.scope)?,
            ring: args.values.get("ring").map(|r| r.to_string()),
        };
        let planned = design(&input)?;
        if let Some(bad) = planned.best().rejected.first() {
            bail!(
                "{} does not suit this gland: {bad}",
                planned.best().ring.name()
            );
        }
        let center = args.point("at", &self.scope)?;
        let along = args.optional_number("offset", &self.scope)?.unwrap_or(0.0);
        let frame = self.sketch_frame();
        let (cut_frame, depth) = match application {
            Application::StaticFace => (frame, planned.depth_mid),
            _ => (frame.offset(along + planned.width), planned.width),
        };
        let (inner, outer) = ring_diameters(&planned);
        let profiles = vec![
            Profile::Circle {
                center,
                diameter: outer,
            },
            Profile::Circle {
                center,
                diameter: inner,
            },
        ];
        let summary = self.remove(
            &label_of(line),
            Removal {
                frame: cut_frame,
                profiles,
                depth: Depth::Blind(depth),
                taper: 0.0,
                side: "side",
                end: "end",
            },
        )?;
        Ok(format!(
            "{} groove {:.2} wide x {:.2} deep, ring {}; {summary}",
            application_name(application),
            planned.width,
            planned.depth_mid,
            planned.best().ring.name()
        ))
    }
}

fn application_name(application: Application) -> &'static str {
    match application {
        Application::StaticFace => "face",
        Application::StaticRadial => "static radial",
        Application::Rod => "rod",
        Application::Piston => "piston",
    }
}

fn ring_diameters(design: &Design) -> (f64, f64) {
    match design.input.application {
        Application::StaticFace => (
            design.groove_diameter,
            design.groove_diameter + 2.0 * design.width,
        ),
        Application::Rod => (design.input.diameter - BORE_MARGIN, design.groove_diameter),
        _ => (design.groove_diameter, design.input.diameter + BORE_MARGIN),
    }
}
