mod common;

use common::*;
use gcad::select::{select_edges, select_faces};
use std::f64::consts::PI;

#[test]
fn variables() {
    assert_volume(
        &build("let w=40 h=w*3/4 t=(w-h)/5\nrect w h\nextrude t"),
        40.0 * 30.0 * 2.0,
        1.0e-6,
    );
}

#[test]
fn groups_and_edges() {
    let model = build("rect 40 30\nbase: extrude 3\nplane base.end\nholes: hole 4 0,0");
    let solid = model.solid.as_ref().expect("a solid");
    let tolerance = model.tolerance();
    let count = |selector: &str| {
        select_edges(selector, solid, &model.groups, tolerance)
            .expect("selects")
            .len()
    };
    assert_eq!(count("base.end&base.side"), 4);
    assert_eq!(count("base.side&base.side"), 4);
    assert_eq!(count("holes.side&base.end"), 4);
    assert_eq!(count("base.end"), 8);
    assert_eq!(count("base.end&base.side|holes.side&base.start"), 8);
}

#[test]
fn directional_faces() {
    let model = build("rect 40 30\nbase: extrude 3\nplane base.end\nrect 10 10\nextrude 4");
    let solid = model.solid.as_ref().expect("a solid");
    let faces = |selector: &str| {
        select_faces(selector, solid, &model.groups, model.tolerance())
            .expect("selects")
            .len()
    };
    assert_eq!(faces(">Z"), 1);
    assert_eq!(faces("+Z"), 2);
    assert_eq!(faces("+X,-X"), 4);
    assert_eq!(faces("all"), 11);
}

#[test]
fn flush_unions_leave_one_face_per_plane() {
    let model = build(
        "rect 40 30\nbase: extrude 10\nrect 10 30 at=15,0\nextrude 20\nplane XZ offset=15\nrect 7 8 at=0,14\nextrude -15",
    );
    let solid = model.solid.as_ref().expect("a solid");
    let faces = |selector: &str| {
        select_faces(selector, solid, &model.groups, model.tolerance())
            .expect("selects")
            .len()
    };
    let edges = |selector: &str| {
        select_edges(selector, solid, &model.groups, model.tolerance())
            .expect("selects")
            .len()
    };
    assert_eq!(faces("all"), 12);
    assert_eq!(faces("<Y"), 1);
    assert_eq!(edges("<Y"), 10);
    assert_volume(&model, 15000.0 + 7.0 * 8.0 * 15.0, 1.0e-6);
}

#[test]
fn groups_survive_later_cuts() {
    let model = build(
        "rect 40 30\nbase: extrude 10\nplane base.end\nrect 10 10\ncut 3\nplane >X\nrect 4 4 at=0,5\ncut 2",
    );
    let solid = model.solid.as_ref().expect("a solid");
    let faces = select_faces("base.end", solid, &model.groups, model.tolerance()).expect("selects");
    assert_eq!(faces.len(), 1);
}

#[test]
fn errors() {
    assert!(failure("rect 10 10\nextrude 1\nfillet 1 nope.end").contains("known groups"));
    assert!(failure("wiggle 3").contains("unknown operation"));
    assert!(failure("rect w 10").contains("let w"));
    assert!(failure("rect 10 10\nplane XZ").contains("extrude or cut it"));
    assert!(failure("rect 10 10 r=5").contains("under half"));
    assert!(failure("rect 10 10\nextrude 2 depth=3").contains("no `depth` argument"));
    assert!(failure("cut 3").contains("needs a sketch"));
    assert!(failure("# not an operation").contains("not an operation"));
}

#[test]
fn measure_distance() {
    let text = summary("rect 40 30\nbase: extrude 10\nmeasure base.end base.start");
    assert!(text.contains("10.000"), "{text}");
}

#[test]
fn mass_properties() {
    let text = summary("rect 20 10\nextrude 5\nmeasure mass");
    assert!(text.contains("centroid 0.000,0.000,2.500"), "{text}");
}

#[test]
fn wall_thickness() {
    let text = summary("rect 40 30\nbase: extrude 20\nshell 2 open=base.end\nmeasure thickness");
    assert!(text.contains("min 2.000"), "{text}");
}

#[test]
fn draft_analysis() {
    let text = summary("rect 20 20\nbase: extrude 10 draft=2\nmeasure draft pull=z");
    assert!(text.contains("0 faces under 1"), "{text}");
}

const CASE: &str = "rect 80 50 r=3\nbase: extrude 30\nshell 2 open=base.end\nbody pcb\nplane XY offset=2\nrect 60 40\nextrude 1.6\nbody battery\nplane XY offset=3.6\nrect 30 20 at=-10,0\nextrude 10\n";

#[test]
fn measure_boxes() {
    let every = summary(&format!("{CASE}measure box"));
    assert!(
        every.contains(
            "pcb 60.000 x 40.000 x 1.600, [-30.000, -20.000, 2.000]..[30.000, 20.000, 3.600]"
        ),
        "{every}"
    );
    assert!(every.contains("all 80.000 x 50.000 x 30.000"), "{every}");
    let faces = summary("rect 40 30\nbase: extrude 10\nmeasure box base.end");
    assert!(
        faces.contains("base.end 40.000 x 30.000 x 0.000"),
        "{faces}"
    );
}

#[test]
fn measure_clearance() {
    let apart = summary(&format!("{CASE}measure clearance main battery"));
    assert!(apart.starts_with("clearance 1.600 from main"), "{apart}");
    assert!(
        apart.contains("box gaps x -45.000 y -35.000 z -13.600"),
        "{apart}"
    );
    let touching = summary(&format!("{CASE}measure clearance pcb battery"));
    assert!(
        touching.starts_with("clearance 0.000, touching"),
        "{touching}"
    );
}

#[test]
fn measure_space() {
    let text = summary(&format!("{CASE}measure space at=20,0,10"));
    assert!(
        text.contains("-x battery, +x main, -y main, +y main, -z pcb, +z open"),
        "{text}"
    );
    assert!(
        text.contains("reach from 20.000,0.000,10.000: -x 15.000 battery, +x 18.000 main, -y 23.000 main, +y 23.000 main, -z 6.400 pcb, +z open"),
        "{text}"
    );
    let error = failure(&format!("{CASE}measure space at=-10,0,5"));
    assert!(error.contains("inside battery"), "{error}");
}

#[test]
fn outside_variables() {
    let lines = gcad::parse::parse_program("let w=10\nrect w w\nextrude 1").expect("parses");
    let model = gcad::model::run_with(&[("w".to_string(), 20.0)], &lines).model;
    assert_volume(&model, 400.0, 1.0e-6);
}

#[test]
fn include_file() {
    let path = scratch("boss.gcad");
    std::fs::write(&path, "circle d\nextrude 5").expect("writes");
    assert_volume(
        &build(&format!("include {path} d=10")),
        PI * 25.0 * 5.0,
        0.0005,
    );
}

#[test]
fn include_brings_in_let() {
    let path = scratch("sizes.gcad");
    std::fs::write(&path, "let w=20 h=d+1").expect("writes");
    assert_volume(
        &build(&format!("let w=5\ninclude {path} d=4\nrect w h\nextrude 2")),
        5.0 * 5.0 * 2.0,
        1.0e-9,
    );
}

#[test]
fn conditional() {
    assert_volume(
        &build("let w=40\nrect w 10\nbase: extrude 5\nif w>30 chamfer 1 base.end&>Y"),
        2000.0 - 20.0,
        1.0e-5,
    );
}

#[test]
fn material_mass() {
    let text = summary(
        "rect 10 10\nextrude 10\nmaterial steel\nbody b\nrect 10 10\nextrude -10\nmaterial pla\nmeasure mass",
    );
    assert!(text.contains("mass 9.090 g"), "{text}");
    let text = summary(
        "rect 10 10\nextrude 10\nmaterial density=2\nbody b\nrect 10 10\nextrude -10\nmeasure mass",
    );
    assert!(
        text.contains("mass 2.000 g without b (no material)"),
        "{text}"
    );
    assert!(failure("rect 10 10\nextrude 10\nmaterial unobtainium").contains("density="));
}

#[test]
fn functions() {
    let text = summary(
        "let a=atan2(3,4) b=sqrt(16)+hypot(3,4) c=max(1,cos(60),2) d=round(2.6)-min(4,abs(-3))",
    );
    assert!(text.contains("b=9 c=2 d=0"), "{text}");
    assert!(text.starts_with("a=36.86989"), "{text}");
    assert!(failure("let a=sqrt(-1)").contains("not a number"));
    assert!(failure("let a=nope(1)").contains("functions are"));
    assert!(failure("let a=atan2(1)").contains("takes two numbers"));
}

#[test]
fn measure_angle() {
    let block = "rect 40 30\nbase: extrude 10\nplane base.end\nh: hole 4 0,0\nbevel: chamfer 4 base.end&>X d2=2\n";
    let faces = summary(&format!("{block}measure angle <Y base.start"));
    assert!(
        faces.contains("angle 90.000 degrees between the face normals"),
        "{faces}"
    );
    let slope = summary(&format!("{block}measure angle bevel.faces base.end"));
    assert!(
        slope.contains(&format!("angle {:.3}", 2.0_f64.atan2(4.0).to_degrees())),
        "{slope}"
    );
    let axis = summary(&format!("{block}measure angle h.side base.end"));
    assert!(
        axis.contains("angle 90.000 degrees between the axis and the face"),
        "{axis}"
    );
}
