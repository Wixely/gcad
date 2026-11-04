use gcad::calc::tables::{self, Series};

#[test]
fn as568_dash_numbers_match_the_handbook() {
    let ring = tables::ring("210").expect("-210");
    assert_eq!(ring.series, Series::As568(210));
    assert!((ring.id - 18.64).abs() < 1.0e-9);
    assert!((ring.cs - 3.53).abs() < 1.0e-9);
    assert!((ring.od() - 25.70).abs() < 1.0e-9);
}

#[test]
fn as568_inside_diameters_agree_with_the_inch_table() {
    let inch = [(214, 0.984), (110, 0.362), (45, 3.989), (240, 3.734)];
    for (dash, inches) in inch {
        let ring = tables::ring(&dash.to_string()).expect("dash");
        let wanted = inches * tables::MM_PER_INCH;
        assert!(
            (ring.id - wanted).abs() < 0.15,
            "-{dash}: {} vs {wanted}",
            ring.id
        );
    }
}

#[test]
fn iso_sizes_are_found_by_id_and_cross_section() {
    let ring = tables::ring("40x2.65").expect("40x2.65");
    assert_eq!(ring.series, Series::Iso);
    assert!(tables::ring("40x9").is_none());
}

#[test]
fn every_ring_belongs_to_a_cross_section_group() {
    assert!(tables::rings().len() > 500);
    for ring in tables::rings() {
        let nominal = tables::CROSS_SECTIONS[ring.group()];
        assert!((ring.cs - nominal).abs() < 0.06, "{}", ring.name());
    }
}

#[test]
fn gland_tables_cover_every_cross_section() {
    for kind in ["static", "dynamic", "face"] {
        for group in 0..5 {
            let gland = tables::gland(kind, group).expect("gland row");
            let nominal = tables::CROSS_SECTIONS[group] / tables::MM_PER_INCH;
            assert!((gland.cs - nominal).abs() < 0.002, "{kind} {group}");
            assert!(gland.depth.0 < gland.cs && gland.width.0 > gland.cs);
        }
    }
    assert_eq!(tables::backup().bar, 103.5);
}

use gcad::calc::oring::{self, Application, Input, Material, evaluate};

fn input(application: Application, diameter: f64, pressure: f64) -> Input {
    Input {
        application,
        diameter,
        cross_section: Some(3.53),
        pressure_bar: pressure,
        material: Material::Nitrile,
        clearance: None,
        ring: None,
    }
}

fn near(actual: f64, expected: f64, relative: f64) {
    assert!(
        (actual - expected).abs() <= expected.abs() * relative,
        "{actual} vs {expected}"
    );
}

#[test]
fn face_seal_squeeze_matches_the_handbook_range() {
    let design = oring::design(&input(Application::StaticFace, 25.0, 10.0)).expect("design");
    let best = &design.choices[0];
    assert_eq!(best.ring.name(), "AS568-214");
    near(best.squeeze_pct.0, 20.1, 0.02);
    near(best.squeeze_pct.1, 30.2, 0.02);
    near(best.fill_pct, 80.0, 0.03);
    near(design.depth.0, 0.101 * 25.4, 1.0e-9);
    near(design.depth.1, 0.107 * 25.4, 1.0e-9);
    assert!(best.rejected.is_empty(), "{:?}", best.rejected);
}

#[test]
fn piston_gland_uses_the_reciprocating_depth() {
    let design = oring::design(&input(Application::Piston, 50.0, 10.0)).expect("design");
    near(design.depth.0, 0.121 * 25.4, 1.0e-9);
    near(
        design.groove_diameter,
        50.0 - 2.0 * design.depth_mid,
        1.0e-9,
    );
    assert_eq!(design.choices.len(), 2);
    let first = &design.choices[0];
    assert!(first.stretch_pct.abs() <= 3.0);
    near(
        first.squeeze_pct.0,
        100.0 * (3.53 - 0.1016 - 0.123 * 25.4) / 3.53,
        0.05,
    );
}

#[test]
fn rod_groove_sits_outside_the_rod() {
    let design = oring::design(&input(Application::Rod, 20.0, 10.0)).expect("design");
    near(
        design.groove_diameter,
        20.0 + 2.0 * design.depth_mid,
        1.0e-9,
    );
}

#[test]
fn stretch_over_three_percent_is_rejected() {
    let mut forced = input(Application::StaticRadial, 40.0, 10.0);
    forced.ring = Some("20x3.55".into());
    let design = oring::design(&forced).expect("design");
    let choice = &design.choices[0];
    assert!(choice.stretch_pct > 3.0);
    assert!(choice.rejected.iter().any(|r| r.contains("stretch")));
}

#[test]
fn fill_over_eighty_five_percent_is_rejected() {
    let ring = tables::ring("214").expect("-214");
    let narrow = evaluate(ring, (2.55, 2.65), 4.2, 0.0, 0.1016);
    assert!(narrow.fill_pct > 85.0, "{}", narrow.fill_pct);
    assert!(narrow.rejected.iter().any(|r| r.contains("fill")));
    let roomy = evaluate(ring, (2.55, 2.65), 5.0, 0.0, 0.1016);
    assert!(roomy.fill_pct < 85.0);
}

#[test]
fn fill_matches_the_area_ratio() {
    let design = oring::design(&input(Application::StaticFace, 25.0, 10.0)).expect("design");
    let choice = &design.choices[0];
    let area = std::f64::consts::PI / 4.0 * 3.53 * 3.53;
    let expected = 100.0 * area / (design.width * design.depth_mid);
    near(choice.fill_pct, expected, 0.01);
}

#[test]
fn back_up_ring_is_flagged_above_the_handbook_pressure() {
    let low = oring::design(&input(Application::StaticRadial, 40.0, 100.0)).expect("low");
    let high = oring::design(&input(Application::StaticRadial, 40.0, 150.0)).expect("high");
    assert!(!low.backup);
    assert!(high.backup);
    assert!(high.width > low.width);
    near(high.width, 0.2105 * 25.4, 1.0e-6);
}

#[test]
fn silicone_halves_the_allowed_clearance() {
    let nitrile = oring::design(&input(Application::StaticRadial, 40.0, 10.0)).expect("n");
    let mut silicone_input = input(Application::StaticRadial, 40.0, 10.0);
    silicone_input.material = Material::Silicone;
    let silicone = oring::design(&silicone_input).expect("s");
    near(silicone.clearance.1, nitrile.clearance.1 / 2.0, 1.0e-9);
}

#[test]
fn a_wide_gap_asks_for_a_back_up_ring() {
    let mut gap = input(Application::StaticRadial, 40.0, 10.0);
    gap.clearance = Some(0.5);
    assert!(oring::design(&gap).expect("gap").backup);
}

#[test]
fn lead_in_chamfer_is_twenty_degrees() {
    let design = oring::design(&input(Application::Piston, 50.0, 10.0)).expect("design");
    near(design.chamfer.0, 20.0, 1.0e-9);
    near(
        design.chamfer.2,
        design.chamfer.1 / 20f64.to_radians().tan(),
        1.0e-9,
    );
}

#[test]
fn report_has_a_why_line_formulas_and_sources() {
    let design = oring::design(&input(Application::StaticFace, 25.0, 10.0)).expect("design");
    let text = oring::report(&design).render(true);
    assert!(text.contains("why:"));
    assert!(text.contains("AS568-214"));
    assert!(text.contains("fill"));
    assert!(text.contains("ORD_5700"));
}

#[test]
fn bad_inputs_are_refused() {
    assert!(oring::design(&input(Application::Piston, -5.0, 10.0)).is_err());
    assert!(oring::design(&input(Application::Piston, 50.0, -1.0)).is_err());
    assert!("rubberish".parse::<Material>().is_err());
}
