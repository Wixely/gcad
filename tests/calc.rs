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
        assert!((ring.id - wanted).abs() < 0.15, "-{dash}: {} vs {wanted}", ring.id);
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
