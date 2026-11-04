use gcad::calc::{fit, materials, thread};

fn near(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} vs {expected}"
    );
}

fn gcad(args: &[&str]) -> (bool, String) {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_gcad"))
        .args(args)
        .output()
        .expect("runs");
    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

mod fits {
    use super::*;

    fn limits(class: &str, nominal: f64) -> (f64, f64) {
        let c = fit::limits(class, nominal).expect(class);
        (c.min, c.max)
    }

    #[test]
    fn h7_g6_at_25() {
        let f = fit::fit("H7/g6", 25.0).expect("fit");
        near(f.hole.min, 25.000, 1e-9);
        near(f.hole.max, 25.021, 1e-9);
        near(f.shaft.min, 24.980, 1e-9);
        near(f.shaft.max, 24.993, 1e-9);
        near(f.min_clearance, 0.007, 1e-9);
        near(f.max_clearance, 0.041, 1e-9);
        assert_eq!(f.kind, fit::Kind::Clearance);
    }

    #[test]
    fn h7_p6_is_an_interference_fit() {
        let f = fit::fit("H7/p6", 25.0).expect("fit");
        near(f.shaft.min, 25.022, 1e-9);
        near(f.shaft.max, 25.035, 1e-9);
        assert_eq!(f.kind, fit::Kind::Interference);
        near(f.max_clearance, -0.001, 1e-9);
    }

    #[test]
    fn h7_h6_at_10_sits_on_the_band_edge() {
        let f = fit::fit("H7/h6", 10.0).expect("fit");
        near(f.hole.max, 10.015, 1e-9);
        near(f.shaft.min, 9.991, 1e-9);
        near(f.shaft.max, 10.000, 1e-9);
    }

    #[test]
    fn h11_c11_and_f7_h6() {
        let (lo, hi) = limits("c11", 10.0);
        near(lo, 9.830, 1e-9);
        near(hi, 9.920, 1e-9);
        let f = fit::fit("F7/h6", 20.0).expect("fit");
        near(f.hole.min, 20.020, 1e-9);
        near(f.hole.max, 20.041, 1e-9);
    }

    #[test]
    fn transition_fit_and_hole_letters_with_delta() {
        let f = fit::fit("H7/k6", 25.0).expect("fit");
        assert_eq!(f.kind, fit::Kind::Transition);
        let (lo, hi) = limits("N7", 25.0);
        near(lo, 24.972, 1e-9);
        near(hi, 24.993, 1e-9);
        let (lo, hi) = limits("P7", 25.0);
        near(lo, 24.965, 1e-9);
        near(hi, 24.986, 1e-9);
    }

    #[test]
    fn js_is_symmetric() {
        let (lo, hi) = limits("js6", 25.0);
        near(lo, 24.9935, 1e-9);
        near(hi, 25.0065, 1e-9);
    }

    #[test]
    fn bad_classes_are_refused() {
        assert!(fit::fit("H7", 25.0).is_err());
        assert!(fit::fit("H7/q6", 25.0).is_err());
        assert!(fit::fit("H7/g6", 600.0).is_err());
        assert!(fit::fit("H7/g6", 0.0).is_err());
    }

    #[test]
    fn report_carries_notes_and_sources() {
        let f = fit::fit("H7/g6", 25.0).expect("fit");
        let text = fit::report(&f).render(true);
        assert!(text.contains("hole") && text.contains("shaft"));
        assert!(text.contains("print") && text.contains("cnc"));
        assert!(text.contains("mechsheet.com"));
    }

    #[test]
    fn cli_prints_the_fit() {
        let (ok, text) = gcad(&["calc", "fit", "H7/g6", "25"]);
        assert!(ok, "{text}");
        assert!(text.contains("25.021") && text.contains("24.980"));
    }
}

mod threads {
    use super::*;

    #[test]
    fn m6_dimensions() {
        let t = thread::find("M6").expect("M6");
        near(t.pitch, 1.0, 1e-9);
        near(t.minor_internal(), 4.917, 1e-3);
        near(t.pitch_dia(), 5.350, 1e-3);
        near(t.minor_external(), 4.773, 1e-3);
        near(t.tap_drill, 5.0, 1e-9);
    }

    #[test]
    fn fine_pitch_is_found_by_pitch() {
        let t = thread::find("M8x1").expect("M8x1");
        near(t.pitch, 1.0, 1e-9);
        assert!(thread::find("M8x2").is_err());
        assert!(thread::find("M99").is_err());
    }

    #[test]
    fn clearance_and_heads() {
        let t = thread::find("M6").expect("M6");
        assert_eq!(thread::clearance(&t), Some([6.4, 6.6, 7.0]));
        let heads = thread::heads(&t);
        let socket = heads
            .iter()
            .find(|h| h.standard == "ISO 4762")
            .expect("4762");
        near(socket.head_dia, 10.0, 1e-9);
        near(socket.bore_dia, 11.0, 1e-9);
        near(socket.bore_depth, 6.4, 1e-9);
        assert!(heads.iter().any(|h| h.standard == "ISO 7380"));
        let flat = heads
            .iter()
            .find(|h| h.standard == "ISO 10642")
            .expect("10642");
        near(flat.bore_dia, 13.64, 1e-9);
    }

    #[test]
    fn unified_and_pipe_threads() {
        let t = thread::find("#8-32").expect("#8-32");
        near(t.major, 4.1656, 1e-3);
        near(t.tap_drill, 3.454, 1e-2);
        assert!(thread::clearance(&t).is_none());
        let g = thread::find("G1/4").expect("G1/4");
        near(g.major, 13.157, 1e-9);
        near(g.pitch, 1.337, 1e-3);
        near(g.minor_internal(), 11.445, 1e-3);
        assert!(thread::find("1/4-20").is_ok());
        assert!(thread::find("1/4-28").is_ok());
    }

    #[test]
    fn torque_for_m6_class_8_8() {
        let t = thread::find("M6").expect("M6");
        let t88 = thread::torque(&t, "8.8", 0.2).expect("torque");
        near(t88.stress_area, 20.1, 0.1);
        near(t88.newton_metres, 11.9, 0.3);
        assert!(thread::torque(&t, "9.9", 0.2).is_err());
        let g = thread::find("G1/4").expect("G");
        assert!(thread::torque(&g, "8.8", 0.2).is_err());
    }

    #[test]
    fn engagement_by_material() {
        let alu = materials::find("alu").expect("alu");
        let steel = materials::find("steel").expect("steel");
        near(thread::engagement(alu, "8.8", 6.0), 12.0, 1e-9);
        near(thread::engagement(alu, "10.9", 6.0), 15.0, 1e-9);
        near(thread::engagement(steel, "8.8", 6.0), 7.5, 1e-9);
    }

    #[test]
    fn blind_hole_depth_check() {
        let t = thread::find("M6").expect("M6");
        let alu = materials::find("aluminium").expect("alu");
        let short = thread::check_depth(&t, &alu, "8.8", 14.0, false);
        assert!(!short.ok);
        near(short.usable, 11.0, 1e-9);
        near(short.required, 12.0, 1e-9);
        near(short.needed_depth, 15.0, 1e-9);
        let long = thread::check_depth(&t, &alu, "8.8", 16.0, false);
        assert!(long.ok);
        let through = thread::check_depth(&t, &alu, "8.8", 12.0, true);
        assert!(through.ok);
    }

    #[test]
    fn cli_thread_report() {
        let (ok, text) = gcad(&["calc", "thread", "M6", "--material", "alu", "--depth", "14"]);
        assert!(ok, "{text}");
        assert!(text.contains("tap drill") && text.contains("5"));
        assert!(text.contains("short") && text.contains("why:"));
        let (_, full) = gcad(&["calc", "thread", "M6", "--formulas"]);
        assert!(full.contains("source:") && full.contains("N.m"));
        let (ok, text) = gcad(&["calc", "thread", "M99"]);
        assert!(!ok && text.contains("unknown thread"));
    }
}

mod thread_op {
    use gcad::calc::thread::short_note;
    use gcad::model::run;
    use gcad::parse::parse_program;

    fn message(source: &str) -> String {
        let result = run(&parse_program(source).expect("parses"));
        match result.steps.last() {
            Some((_, Ok(text))) => text.clone(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_hole_shorter_than_one_diameter_is_flagged() {
        let text = message(
            "rect 20 20\nbase: extrude 4\nplane base.end\nh: hole 5 0,0 thread=M6\nthread M6 on=h.side",
        );
        assert!(
            text.contains("warning") && text.contains("minimum engagement"),
            "{text}"
        );
    }

    #[test]
    fn a_deep_enough_hole_is_not() {
        let text = message(
            "rect 20 20\nbase: extrude 8\nplane base.end\nh: hole 5 0,0 thread=M6\nthread M6 on=h.side",
        );
        assert!(!text.contains("warning"), "{text}");
        assert!(short_note("M6", 6.0).is_none());
        assert!(short_note("M6", 4.0).is_some());
    }
}

mod material_table {
    use super::*;

    #[test]
    fn every_material_has_properties_and_engagement() {
        for name in [
            "steel",
            "aluminium",
            "stainless",
            "brass",
            "pla",
            "petg",
            "abs",
            "nylon",
        ] {
            let m = materials::find(name).expect(name);
            assert!(m.density > 0.5 && m.engagement[0] >= 1.0, "{name}");
            assert!(m.engagement[1] >= m.engagement[0], "{name}");
        }
        assert!(materials::find("unobtainium").is_err());
    }

    #[test]
    fn cli_material() {
        let (ok, text) = gcad(&["calc", "material", "petg"]);
        assert!(ok, "{text}");
        assert!(text.contains("density"));
    }
}
