mod common;

use common::scratch;
use gcad::model::{Model, run_path};

const BOX: &str = "rect 40 40\nextrude 10\nbody lid\nplane XY offset=10\nrect 40 40\nextrude 2\n";

fn files(test: &str, assembly: &str, parts: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(scratch(test));
    std::fs::create_dir_all(&dir).expect("dir");
    for (name, text) in parts {
        std::fs::write(dir.join(name), text).expect("part");
    }
    let path = dir.join("top.gasm");
    std::fs::write(&path, assembly).expect("assembly");
    path
}

fn steps(
    test: &str,
    assembly: &str,
    parts: &[(&str, &str)],
) -> (Model, Vec<Result<String, String>>) {
    let run = run_path(&files(test, assembly, parts), &[], None).expect("runs");
    let results = run
        .steps
        .iter()
        .map(|(_, r)| r.as_ref().cloned().map_err(|e| format!("{e:#}")))
        .collect();
    (run.model, results)
}

fn built(test: &str, assembly: &str, parts: &[(&str, &str)]) -> (Model, String) {
    let (model, results) = steps(test, assembly, parts);
    match results.last() {
        Some(Ok(text)) => (model, text.clone()),
        Some(Err(error)) => panic!("{error}"),
        None => panic!("empty"),
    }
}

fn failed(test: &str, assembly: &str, parts: &[(&str, &str)]) -> String {
    match steps(test, assembly, parts).1.last() {
        Some(Err(error)) => error.clone(),
        other => panic!("expected a failure, got {other:?}"),
    }
}

const HINGE: &str = "part box box.gcad\naxis hinge -20,20,10 20,20,10\n";

#[test]
fn parts_from_files() {
    let (model, _) = built(
        "parts_from_files",
        "part box box.gcad\npart spare box.gcad body=lid\nmove spare 0,0,20",
        &[("box.gcad", BOX)],
    );
    assert_eq!(model.body_names(), ["box.main", "box.lid", "spare"]);
    let spare = gcad::geometry::bounds(&model.named_body("spare").expect("spare"));
    assert!((spare.min().z - 30.0).abs() < 1.0e-6, "{spare:?}");
}

#[test]
fn part_variables() {
    let (model, _) = built(
        "part_variables",
        "let size=20\npart plate plate.gcad w=size",
        &[("plate.gcad", "let w=10\nrect w w\nextrude 1")],
    );
    let plate = gcad::geometry::bounds(&model.named_body("plate").expect("plate"));
    assert!((plate.max().x - 10.0).abs() < 1.0e-6, "{plate:?}");
}

#[test]
fn hinge_opens_the_lid() {
    let (model, _) = built(
        "hinge_opens_the_lid",
        &format!("{HINGE}joint open box.lid box.main turn about=hinge min=-120 max=0 at=-90"),
        &[("box.gcad", BOX)],
    );
    let b = gcad::geometry::bounds(&model.named_body("box.lid").expect("lid"));
    assert!(
        (b.min().z - 10.0).abs() < 1.0e-6 && (b.max().z - 50.0).abs() < 1.0e-6,
        "{b:?}"
    );
    assert!(
        (b.min().y - 20.0).abs() < 1.0e-6 && (b.max().y - 22.0).abs() < 1.0e-6,
        "{b:?}"
    );
}

#[test]
fn slide_moves_a_drawer() {
    let parts = [(
        "drawer.gcad",
        "rect 40 40\nextrude 10\nbody drawer\nplane XY offset=10\nrect 30 30\nextrude 5",
    )];
    let (model, _) = built(
        "slide_moves_a_drawer",
        "part chest drawer.gcad\njoint pull chest.drawer chest.main slide along=0,-1,0 min=0 max=30 at=25",
        &parts,
    );
    let b = gcad::geometry::bounds(&model.named_body("chest.drawer").expect("drawer"));
    assert!(
        (b.min().y + 40.0).abs() < 1.0e-6 && (b.max().y + 10.0).abs() < 1.0e-6,
        "{b:?}"
    );
    let error = failed(
        "slide_out_of_range",
        "part chest drawer.gcad\njoint pull chest.drawer chest.main slide along=0,-1,0 min=0 max=30 at=40",
        &parts,
    );
    assert!(error.contains("goes from 0 to 30"), "{error}");
}

#[test]
fn pose_moves_children() {
    let (model, _) = built(
        "pose_moves_children",
        &format!(
            "{HINGE}part knob knob.gcad\nmove knob 0,-10,12\njoint open box.lid box.main turn about=hinge min=-120 max=0\njoint fix knob box.lid slide along=0,0,1 min=0 max=0\npose open -90"
        ),
        &[("box.gcad", BOX), ("knob.gcad", "circle 6\nextrude 4")],
    );
    let b = gcad::geometry::bounds(&model.named_body("knob").expect("knob"));
    assert!(b.min().y > 21.9 && b.max().y < 26.1, "{b:?}");
}

#[test]
fn clear_assembly() {
    let (_, text) = built(
        "clear_assembly",
        &format!(
            "{HINGE}joint open box.lid box.main turn about=hinge min=-120 max=0 at=-90\ninterference none"
        ),
        &[("box.gcad", BOX)],
    );
    assert!(text.contains("no overlaps"), "{text}");
}

#[test]
fn sweep_finds_a_clash() {
    let (_, text) = built(
        "sweep_finds_a_clash",
        &format!(
            "{HINGE}joint open box.lid box.main turn about=hinge min=-60 max=60\ninterference joint=open steps=4"
        ),
        &[("box.gcad", BOX)],
    );
    assert!(
        text.contains("at 30.0°") && text.contains("box.main and box.lid"),
        "{text}"
    );
}

#[test]
fn sweep_keeps_reporting_parts_that_do_not_move() {
    let (_, text) = built(
        "sweep_keeps_reporting_parts_that_do_not_move",
        &format!(
            "{HINGE}part spare box.gcad body=lid\nmove spare 0,0,-5\njoint open box.lid box.main turn about=hinge min=-60 max=60\ninterference joint=open steps=4"
        ),
        &[("box.gcad", BOX)],
    );
    assert_eq!(text.matches("box.main and spare").count(), 5, "{text}");
    assert!(text.contains("at 30.0°: box.main and box.lid"), "{text}");
}

#[test]
fn concentric_far_from_the_origin() {
    let (_, text) = built(
        "concentric_far_from_the_origin",
        "part plate plate.gcad\npart s screw.gcad\nrotate s -90 axis=y\nmove s -35,58,280\nconcentric s:shank.side plate:h.side near=-35,58,280",
        &[
            (
                "screw.gcad",
                "circle 3.28\nshank: extrude -40\nplane XY\ncircle 6.5\nhead: extrude 4",
            ),
            (
                "plate.gcad",
                "plane YZ\nrect 140 300 at=0,150\np: extrude -35\nplane p.end\nh: hole 4.5 -58,280 -58,131",
            ),
        ],
    );
    assert!(text.contains("radius 1.640 in 2.250"), "{text}");
}

#[test]
fn strict_interference_fails() {
    let error = failed(
        "strict_interference_fails",
        &format!(
            "{HINGE}joint open box.lid box.main turn about=hinge min=0 max=60 at=45\ninterference none"
        ),
        &[("box.gcad", BOX)],
    );
    assert!(error.contains("overlap"), "{error}");
}

#[test]
fn meshing_gears_may_touch() {
    const BLOCK: &str = "rect 10 10\nextrude 5\n";
    let top = |module: f64| {
        format!(
            "part base block.gcad\nmove base 100,0,0\npart a block.gcad\npart b block.gcad\nmove b 9.9,0,0\naxis wa 0,0,0 0,0,1\naxis wb 9.9,0,0 9.9,0,1\njoint turn a base turn about=wa\njoint spin b base turn about=wb\ncouple spin turn ratio=2 module={module} face=5 teeth=20 gear=a pinion=b\ninterference none"
        )
    };
    let (_, text) = built(
        "meshing_gears_may_touch",
        &top(2.0),
        &[("block.gcad", BLOCK)],
    );
    assert!(text.contains("no overlaps"), "{text}");
    let error = failed(
        "meshing_gears_may_touch_fine",
        &top(1.0),
        &[("block.gcad", BLOCK)],
    );
    assert!(error.contains("a and b share"), "{error}");
}

#[test]
fn geometry_stays_in_parts() {
    let error = failed(
        "geometry_stays_in_parts",
        "part box box.gcad\nrect 10 10",
        &[("box.gcad", BOX)],
    );
    assert!(error.contains("part (.gcad) file"), "{error}");
    let error = common::failure(
        "rect 10 10\nextrude 1\nbody b\nrect 5 5\nextrude 1\njoint j b main slide along=1,0,0",
    );
    assert!(error.contains("assembly (.gasm) file"), "{error}");
}

#[test]
fn part_errors_name_the_file() {
    let error = failed(
        "part_errors_name_the_file",
        "part bad bad.gcad",
        &[("bad.gcad", "rect 10 10\nfillet 1 all")],
    );
    assert!(error.contains("bad.gcad:2"), "{error}");
}

const PLATE: &str = "rect 40 30\nbase: extrude 5\nplane base.end\nhole: hole 4.2 10,5 -10,-5\n";
const PIN: &str = "pin: circle 4\npin: extrude 20\n";

fn centre(model: &Model, name: &str) -> [f64; 3] {
    let b = gcad::geometry::bounds(&model.named_body(name).expect("part"));
    let c = b.center();
    [c.x, c.y, c.z]
}

#[test]
fn concentric_pin_in_a_hole() {
    let (model, text) = built(
        "concentric_pin_in_a_hole",
        "part plate plate.gcad\npart pin pin.gcad\nrotate pin 90 axis=y\nmove pin 8,4,30\nconcentric pin:pin.side plate:hole.side\nflush pin:pin.start plate:base.end",
        &[("plate.gcad", PLATE), ("pin.gcad", PIN)],
    );
    let [x, y, z] = centre(&model, "pin");
    assert!(
        (x - 10.0).abs() < 1.0e-6 && (y - 5.0).abs() < 1.0e-6,
        "{x} {y} {text}"
    );
    assert!((z - 15.0).abs() < 1.0e-6 || (z + 5.0).abs() < 1.0e-6, "{z}");
}

#[test]
fn concentric_picks_the_hole_near_a_point() {
    let (model, _) = built(
        "concentric_picks_the_hole_near_a_point",
        "part plate plate.gcad\npart pin pin.gcad\nconcentric pin:pin.side plate:hole.side near=-10,-5,0\nflush pin:pin.start plate:base.end offset=1",
        &[("plate.gcad", PLATE), ("pin.gcad", PIN)],
    );
    let [x, y, z] = centre(&model, "pin");
    assert!(
        (x + 10.0).abs() < 1.0e-6 && (y + 5.0).abs() < 1.0e-6 && (z - 16.0).abs() < 1.0e-6,
        "{x} {y} {z}"
    );
}

#[test]
fn holes_line_up() {
    let parts = [("plate.gcad", PLATE)];
    let (_, text) = built(
        "holes_line_up",
        "part a plate.gcad\npart b plate.gcad\nmove b 0,0,5\naligned b:hole.side a:hole.side",
        &parts,
    );
    assert!(text.contains("2 hole(s) line up"), "{text}");
    let error = failed(
        "holes_do_not_line_up",
        "part a plate.gcad\npart b plate.gcad\nmove b 0.5,0,5\naligned b:hole.side a:hole.side",
        &parts,
    );
    assert!(error.contains("0.500 off"), "{error}");
}

#[test]
fn a_transform_in_a_later_body_keeps_earlier_bodies_groups() {
    let two = "body a\nrect 10 10\nblock: extrude 5\nplane block.end\nh: hole 2 0,0\nbody b\nrect 10 10\nother: extrude 5\nrotate 90 axis=x\n";
    let pin = "circle 1\nshank: extrude 5\n";
    let (model, _) = built(
        "transform_in_a_later_body_keeps_groups",
        "part p two.gcad body=a\npart s pin.gcad\nconcentric s:shank.side p:h.side",
        &[("two.gcad", two), ("pin.gcad", pin)],
    );
    let [x, y, _] = centre(&model, "s");
    assert!(x.abs() < 1.0e-6 && y.abs() < 1.0e-6, "{x} {y}");
}

const LID_WITH_HOLES: &str = "rect 40 40\nextrude 10\nplane XY offset=10\npilot: hole 2 15,15 depth=8\nbody lid\nplane XY offset=10\nrect 40 40\ntop: extrude 2\nplane top.end\nscrews: hole 2.5 15,15\n";
const SCREW: &str = "circle 4\nhead: extrude 1.5\ncircle 2\nshank: extrude -10\n";

#[test]
fn a_screw_locks_the_hinge() {
    let parts = [("box.gcad", LID_WITH_HOLES), ("screw.gcad", SCREW)];
    let screwed = format!(
        "{HINGE}joint open box.lid box.main turn about=hinge min=-120 max=0\npart screw screw.gcad\nconcentric screw:shank.side box.lid:screws.side\nflush screw:head.start box.lid:top.end\nconcentric screw:shank.side box.main:pilot.side\n"
    );
    let (model, text) = built("a_screw_locks_the_hinge", &screwed, &parts);
    assert!(text.contains("ties it to `box.main`"), "{text}");
    let b = gcad::geometry::bounds(&model.named_body("screw").expect("screw"));
    assert!(
        (b.min().z - 2.0).abs() < 1.0e-6 && (b.max().z - 13.5).abs() < 1.0e-6,
        "{b:?}"
    );
    let error = failed(
        "a_screw_locks_the_hinge_pose",
        &format!("{screwed}pose open -30"),
        &parts,
    );
    assert!(
        error.contains("pull apart") && error.contains("box.main:pilot.side"),
        "{error}"
    );
    let (model, _) = built(
        "an_unscrewed_lid_opens",
        &format!(
            "{HINGE}joint open box.lid box.main turn about=hinge min=-120 max=0\npart screw screw.gcad\nconcentric screw:shank.side box.lid:screws.side\npose open -90"
        ),
        &parts,
    );
    let b = gcad::geometry::bounds(&model.named_body("screw").expect("screw"));
    assert!(
        b.max().z - b.min().z < 4.1,
        "the screw turns with the lid: {b:?}"
    );
}

#[test]
fn a_sub_assembly_keeps_its_mates() {
    let parts = [
        ("box.gcad", LID_WITH_HOLES),
        ("screw.gcad", SCREW),
        (
            "boxed.gasm",
            "part box box.gcad\naxis hinge -20,20,10 20,20,10\njoint open box.lid box.main turn about=hinge min=-120 max=0\npart screw screw.gcad\nconcentric screw:shank.side box.lid:screws.side\nflush screw:head.start box.lid:top.end\nconcentric screw:shank.side box.main:pilot.side\n",
        ),
    ];
    let error = failed(
        "a_sub_assembly_keeps_its_mates",
        "part kit boxed.gasm\npose kit.open -30\n",
        &parts,
    );
    assert!(error.contains("pull apart"), "{error}");
}

#[test]
fn a_sub_assembly_moves_as_one() {
    let parts = [
        ("block.gcad", "rect 20 20\nb: extrude 10\n"),
        ("pin.gcad", "circle 5\np: extrude 30\n"),
        (
            "drive.gasm",
            "part base block.gcad\npart pin pin.gcad\naxis a 0,0,0 0,0,1\njoint spin pin base turn about=a\n",
        ),
    ];
    let (model, _) = built(
        "a_sub_assembly_moves_as_one",
        "part az drive.gasm\nmove az 50,0,0\npart arm pin.gcad\nmove arm 10,0,0\njoint swing arm az.base turn about=az.a\npose swing 90\n",
        &parts,
    );
    let base = extent(&model, "az.base");
    assert!(
        near(base.min().x, 40.0) && near(base.max().x, 60.0),
        "{base:?}"
    );
    let pin = extent(&model, "az.pin");
    assert!(near((pin.min().x + pin.max().x) / 2.0, 50.0), "{pin:?}");
    let arm = extent(&model, "arm");
    let centre = (
        (arm.min().x + arm.max().x) / 2.0,
        (arm.min().y + arm.max().y) / 2.0,
    );
    assert!(
        near(centre.0, 50.0) && near(centre.1, -40.0),
        "the arm swings about the moved axis: {centre:?}"
    );
}

const SLAB: &str = "rect 100 100\nplate: extrude 10\n";
const ROD: &str = "circle 5\nrod: extrude 40\n";

#[test]
fn sub_assemblies_build_a_shared_part_once() {
    let parts = [
        ("pin.gcad", "let r=5\ncircle 2*r\np: extrude 30\n"),
        (
            "pair.gasm",
            "let r=5\npart a pin.gcad r=r\npart b pin.gcad r=r+1\n",
        ),
        ("left.gasm", "part pins pair.gasm\npart c pin.gcad r=5\n"),
        (
            "right.gasm",
            "part pins pair.gasm r=6\npart c pin.gcad r=5\n",
        ),
    ];
    let (model, _) = built(
        "sub_assemblies_build_a_shared_part_once",
        "part l left.gasm\npart r right.gasm\n",
        &parts,
    );
    let faces = |name: &str| -> Vec<_> {
        model
            .named_body(name)
            .expect("part")
            .boundaries()
            .iter()
            .flat_map(|shell| shell.face_iter().map(|face| face.id()).collect::<Vec<_>>())
            .collect()
    };
    assert_eq!(faces("l.c"), faces("r.c"));
    assert_eq!(faces("l.pins.a"), faces("l.c"));
    assert_eq!(faces("l.pins.b"), faces("r.pins.a"));
    assert_ne!(faces("r.pins.a"), faces("r.pins.b"));
    let wide = extent(&model, "r.pins.b");
    assert!(near(wide.max().x - wide.min().x, 14.0), "{wide:?}");
}

#[test]
fn assemblies_that_hold_each_other_fail() {
    let error = failed(
        "assemblies_that_hold_each_other_fail",
        "part a a.gasm\n",
        &[("a.gasm", "part b b.gasm\n"), ("b.gasm", "part a a.gasm\n")],
    );
    assert!(error.contains("nested more than 16 deep"), "{error}");
}

fn extent(
    model: &Model,
    part: &str,
) -> monstertruck::modeling::BoundingBox<monstertruck::modeling::Point3> {
    gcad::geometry::bounds(&model.named_body(part).expect("part"))
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1.0e-3
}

#[test]
fn tangent_lays_a_rod_on_a_plate() {
    let parts = [("plate.gcad", SLAB), ("rod.gcad", ROD)];
    let (model, text) = built(
        "tangent_lays_a_rod_on_a_plate",
        "part plate plate.gcad\npart rod rod.gcad\ntangent rod:rod.side plate:plate.end\n",
        &parts,
    );
    let b = extent(&model, "rod");
    assert!(
        near(b.min().z, 10.0) && near(b.max().z, 15.0),
        "{text} {b:?}"
    );
}

#[test]
fn distance_between_axes() {
    let parts = [("rod.gcad", ROD)];
    let (model, _) = built(
        "distance_between_axes",
        "part a rod.gcad\npart b rod.gcad\nmove b 30,0,0\ndistance b:rod.side a:rod.side 50\n",
        &parts,
    );
    let b = extent(&model, "b");
    assert!(near(b.min().x, 47.5) && near(b.max().x, 52.5), "{b:?}");
}

#[test]
fn parallel_and_angle_turn_parts() {
    let parts = [("plate.gcad", SLAB), ("rod.gcad", ROD)];
    let (model, _) = built(
        "parallel_turns_a_rod",
        "part a rod.gcad\npart b rod.gcad\nmove b 30,0,0\nrotate b 30 axis=x\nparallel b:rod.end a:rod.end\n",
        &parts,
    );
    let b = extent(&model, "b");
    assert!(near(b.max().z - b.min().z, 40.0), "{b:?}");
    let (model, text) = built(
        "angle_tilts_a_plate",
        "part base plate.gcad\npart lid plate.gcad\nmove lid 0,0,50\nangle lid:plate.end base:plate.end 30\n",
        &parts,
    );
    let b = extent(&model, "lid");
    let tilted = 100.0 * 0.5 + 10.0 * 3f64.sqrt() / 2.0;
    assert!(near(b.max().z - b.min().z, tilted), "{text} {b:?}");
}

#[test]
fn a_parallel_mate_stops_a_joint() {
    let parts = [("rod.gcad", ROD)];
    let assembly = "part a rod.gcad\npart b rod.gcad\npart c rod.gcad\nmove b 50,0,0\nmove c 0,30,0\naxis tilt 50,0,0 60,0,0\njoint t b a turn about=tilt\ndistance c:rod.side a:rod.side 20\nparallel c:rod.end b:rod.end\n";
    let (model, _) = built("a_parallel_mate_stops_a_joint", assembly, &parts);
    assert!(near(extent(&model, "c").min().y, 17.5));
    let error = failed(
        "a_parallel_mate_stops_a_joint_pose",
        &format!("{assembly}pose t 20\n"),
        &parts,
    );
    assert!(
        error.contains("pull apart parallel c:rod.end b:rod.end"),
        "{error}"
    );
    let error = failed(
        "a_second_mate_that_does_not_hold",
        "part a rod.gcad\npart b rod.gcad\nmove b 50,0,0\nrotate b 10 axis=x\npart c rod.gcad\nmove c 0,30,0\ndistance c:rod.side a:rod.side 20\nparallel c:rod.end b:rod.end\n",
        &parts,
    );
    assert!(error.contains("already hangs off `a`"), "{error}");
}

#[test]
fn a_rod_turns_and_slides_on_one_axis() {
    let parts = [("rod.gcad", ROD), ("plate.gcad", SLAB)];
    let assembly = "part plate plate.gcad\npart rod rod.gcad\naxis up 0,0,0 0,0,1\njoint spin rod plate turn about=up\njoint push rod plate slide along=0,0,1 min=0 max=20\npose push 10\npose spin 90\n";
    let (model, _) = built("a_rod_turns_and_slides", assembly, &parts);
    let b = extent(&model, "rod");
    assert!(near(b.min().z, 10.0) && near(b.max().z, 50.0), "{b:?}");
    let error = failed(
        "turns_and_slides_that_do_not_commute",
        "part plate plate.gcad\npart rod rod.gcad\naxis up 0,0,0 0,0,1\njoint spin rod plate turn about=up\njoint push rod plate slide along=1,0,0\n",
        &parts,
    );
    assert!(error.contains("depend on their order"), "{error}");
}

#[test]
fn coupled_joints_move_together() {
    let parts = [("rod.gcad", ROD), ("plate.gcad", SLAB)];
    let gears = "part frame plate.gcad\npart a rod.gcad\npart b rod.gcad\npart rack rod.gcad\nmove b 20,0,0\nmove rack 0,40,0\naxis ax 0,0,0 0,0,1\naxis bx 20,0,0 20,0,1\njoint ja a frame turn about=ax\njoint jb b frame turn about=bx min=-60 max=60\njoint slide rack frame slide along=1,0,0\ncouple jb ja ratio=-0.5\ncouple slide ja ratio=0.1\n";
    let (model, _) = built("coupled_joints", &format!("{gears}pose ja 120\n"), &parts);
    let value = |name: &str| {
        model
            .joints
            .iter()
            .find(|j| j.name == name)
            .expect("joint")
            .value
    };
    assert!(near(value("jb"), -60.0) && near(value("slide"), 12.0));
    assert!(near(extent(&model, "rack").min().x, 12.0 - 2.5));
    let error = failed(
        "driven_joint_posed",
        &format!("{gears}pose jb 10\n"),
        &parts,
    );
    assert!(error.contains("driven by `ja`"), "{error}");
    let error = failed(
        "driven_past_its_range",
        &format!("{gears}pose ja 150\n"),
        &parts,
    );
    assert!(error.contains("`jb` goes from -60 to 60"), "{error}");
}

#[test]
fn couple_losses_are_checked() {
    let parts = [("rod.gcad", ROD), ("plate.gcad", SLAB)];
    let pair = "part frame plate.gcad\npart a rod.gcad\npart b rod.gcad\nmove b 20,0,0\naxis ax 0,0,0 0,0,1\naxis bx 20,0,0 20,0,1\njoint ja a frame turn about=ax\njoint jb b frame turn about=bx\n";
    for (name, couple, wanted) in [
        (
            "couple_over_one",
            "couple jb ja ratio=-2 efficiency=1.5",
            "at most 1",
        ),
        (
            "couple_friction_alone",
            "couple jb ja ratio=-2 friction=0.1",
            "goes with `lead=`",
        ),
        (
            "couple_both_ways",
            "couple jb ja ratio=-2 lead=5 efficiency=0.5",
            "or `efficiency=`",
        ),
    ] {
        let error = failed(name, &format!("{pair}{couple}\n"), &parts);
        assert!(error.contains(wanted), "{error}");
    }
}

#[test]
fn materials_follow_parts() {
    let parts = [("rod.gcad", "circle 10\nextrude 10\nmaterial aluminium\n")];
    let (_, text) = built(
        "materials_follow_parts",
        "part a rod.gcad\npart b rod.gcad\nmove b 20,0,0\nmaterial b steel\nmeasure mass\n",
        &parts,
    );
    let volume = std::f64::consts::PI * 25.0 * 10.0;
    let grams = volume * (2.7 + 7.85) / 1000.0;
    let said: f64 = text
        .split("mass ")
        .nth(1)
        .and_then(|t| t.split(' ').next())
        .and_then(|t| t.parse().ok())
        .expect("mass");
    assert!((said - grams).abs() < grams * 2.0e-3, "{text}");
}

#[test]
fn clearance_between_parts() {
    let parts = [
        (
            "case.gcad",
            "rect 80 50 r=3\nbase: extrude 30\nshell 2 open=base.end\n",
        ),
        ("lid.gcad", "rect 80 50 r=3\nextrude 2\n"),
        ("board.gcad", "rect 60 40\nextrude 1.6\n"),
    ];
    let placed = "part case case.gcad\npart lid lid.gcad\nmove lid 0,0,30\npart board board.gcad\nmove board 0,0,5\n";
    let (_, gap) = built(
        "clearance_gap",
        &format!("{placed}measure clearance board lid\n"),
        &parts,
    );
    assert!(gap.starts_with("clearance 23.400 from board"), "{gap}");
    let (_, space) = built(
        "clearance_space",
        &format!("{placed}measure space\n"),
        &parts,
    );
    assert!(space.contains("-z board, +z lid"), "{space}");
    assert!(
        space.contains("-x 38.000 case, +x 38.000 case, -y 23.000 case"),
        "{space}"
    );
}

#[test]
fn bill_of_materials_counts_parts() {
    let parts = [
        (
            "rod.gcad",
            "let d=10\ncircle d\nextrude 10\nmaterial steel\n",
        ),
        ("plate.gcad", SLAB),
    ];
    let path = files(
        "bill_of_materials_counts_parts",
        "part base plate.gcad\npart a rod.gcad\npart b rod.gcad\npart c rod.gcad d=20\nmove b 20,0,0\nmove c 40,0,0\n",
        &parts,
    );
    let run = run_path(&path, &[], None).expect("runs");
    let items = gcad::bom::items(&run.model, &path);
    let summary: Vec<(usize, &str, &str)> = items
        .iter()
        .map(|i| (i.names.len(), i.file.as_str(), i.variant.as_str()))
        .collect();
    assert_eq!(
        summary,
        [
            (1, "plate.gcad", ""),
            (2, "rod.gcad", ""),
            (1, "rod.gcad", "d=20")
        ]
    );
    assert!(items[0].grams.is_none());
    let grams = std::f64::consts::PI * 25.0 * 10.0 * 7.85 / 1000.0;
    assert!((items[1].grams.expect("steel") - grams).abs() < grams * 2.0e-3);
    let table = gcad::bom::table(&items);
    assert!(
        table.ends_with("4 parts, 36.98 g without the parts that have no material"),
        "{table}"
    );
    assert!(
        gcad::bom::csv(&items)
            .lines()
            .nth(2)
            .expect("row")
            .starts_with("2,rod.gcad,main,,steel,")
    );
}

#[test]
fn exploded_views_move_parts_apart() {
    let parts = [("box.gcad", LID_WITH_HOLES), ("screw.gcad", SCREW)];
    let assembly = format!(
        "{HINGE}joint open box.lid box.main turn about=hinge min=-120 max=0\npart screw screw.gcad\nconcentric screw:shank.side box.lid:screws.side\nflush screw:head.start box.lid:top.end\nexplode box.lid 0,0,20\nexplode screw 0,0,10\ninterference none\n"
    );
    let (model, _) = built("exploded_views", &assembly, &parts);
    let top = |scale: f64, index: usize| {
        let parts = model.exploded_parts(scale);
        gcad::geometry::bounds(&parts[index].0).max().z
    };
    let names = model.body_names();
    let (lid, screw) = (
        names.iter().position(|n| n == "box.lid").expect("lid"),
        names.iter().position(|n| n == "screw").expect("screw"),
    );
    assert!(near(top(0.0, lid), 12.0) && near(top(1.0, lid), 32.0));
    assert!(near(top(0.0, screw), 13.5) && near(top(1.0, screw), 43.5));
    assert!(near(top(0.5, screw), 28.5));
    let open: Vec<f64> = model
        .joints
        .iter()
        .map(|j| if j.name == "open" { -90.0 } else { j.value })
        .collect();
    let moved = gcad::model::posed(&model.joints, &open);
    let offsets = gcad::model::explode_offsets(&model.joints, &model.explode, &moved);
    let lid = offsets["box.lid"];
    assert!(
        near(lid.x, 0.0) && near(lid.y, 20.0) && near(lid.z, 0.0),
        "{lid:?}"
    );
    let screw = offsets["screw"];
    assert!(near(screw.y, 30.0) && near(screw.z, 0.0), "{screw:?}");
}

const BAR: &str = "let L=40\nrect L+8 8 r=3.9 at=L/2,0\nbar: extrude 3\nplane bar.end\na: hole 3 0,0\nb: hole 3 L,0\n";

const FOUR_BAR: &str = "let ground=40 crank=15 coupler=40 rocker=30
let d=hypot(ground,crank) along=(coupler*coupler-rocker*rocker+d*d)/(2*d)
let h=sqrt(coupler*coupler-along*along)
let cx=along*ground/d+h*crank/d cy=crank-along*crank/d+h*ground/d
part base plate.gcad
part crankbar bar.gcad L=crank
part couplerbar bar.gcad L=coupler
part rockerbar bar.gcad L=rocker
rotate crankbar 90 axis=z
move couplerbar 0,crank,3
rotate couplerbar atan2(cy-crank,cx) axis=z about=0,crank,0
move rockerbar ground,0,0
rotate rockerbar atan2(cy,cx-ground) axis=z about=ground,0,0
axis a 0,0,0 0,0,1
axis knee 0,crank,0 0,crank,1
axis pivot ground,0,0 ground,0,1
joint drive crankbar base turn about=a
joint bend couplerbar crankbar turn about=knee
joint rock rockerbar base turn about=pivot
concentric couplerbar:b.side rockerbar:b.side
";

fn rocker_angle(crank_degrees: f64) -> f64 {
    let (ground, crank, coupler, rocker) = (40.0f64, 15.0f64, 40.0f64, 30.0f64);
    let b = (
        crank * crank_degrees.to_radians().cos(),
        crank * crank_degrees.to_radians().sin(),
    );
    let (dx, dy) = (ground - b.0, -b.1);
    let d = dx.hypot(dy);
    let along = (coupler * coupler - rocker * rocker + d * d) / (2.0 * d);
    let h = (coupler * coupler - along * along).sqrt();
    let c = (
        b.0 + along * dx / d - h * dy / d,
        b.1 + along * dy / d + h * dx / d,
    );
    (c.1).atan2(c.0 - ground).to_degrees()
}

#[test]
fn a_four_bar_linkage_follows_its_crank() {
    let parts = [("plate.gcad", SLAB), ("bar.gcad", BAR)];
    let (model, text) = built("four_bar", &format!("{FOUR_BAR}pose drive 60\n"), &parts);
    let value = |name: &str| {
        model
            .joints
            .iter()
            .find(|j| j.name == name)
            .expect("joint")
            .value
    };
    let turned = rocker_angle(150.0) - rocker_angle(90.0);
    assert!(
        (value("rock") - turned).abs() < 1.0e-3,
        "{text}: rock {} want {turned}",
        value("rock")
    );
    assert!(
        model
            .mates
            .iter()
            .all(|m| m.holds([monstertruck::modeling::Matrix4::from_scale(1.0); 2]))
    );
}

#[test]
fn pattern_copies_a_part_and_its_mates() {
    let plate = "rect 60 20\nbase: extrude 5\nplane base.end\nholes: hole 3 -20,0 0,0 20,0\n";
    let pin = "circle 5\nhead: extrude 1\ncircle 3\npin: extrude -5\n";
    let parts = [("plate.gcad", plate), ("pin.gcad", pin)];
    let pinned = "part plate plate.gcad\npart p pin.gcad\nconcentric p:pin.side plate:holes.side near=-20,0,5\nflush p:head.start plate:base.end\npattern p holes=plate:holes.side\n";
    let (model, text) = built("pattern_holes", pinned, &parts);
    assert!(text.contains("p_2, p_3, each with 2 mate(s)"), "{text}");
    let mut centres: Vec<f64> = ["p", "p_2", "p_3"]
        .iter()
        .map(|n| {
            let b = extent(&model, n);
            assert!(near(b.max().z, 6.0) && near(b.min().z, 0.0), "{n}: {b:?}");
            (b.min().x + b.max().x) / 2.0
        })
        .collect();
    centres.sort_by(f64::total_cmp);
    assert!(
        near(centres[0], -20.0) && near(centres[1], 0.0) && near(centres[2], 20.0),
        "{centres:?}"
    );
    assert_eq!(
        model.joints.iter().filter(|j| j.parent == "plate").count(),
        3
    );
    let (model, _) = built(
        "pattern_step",
        "part plate plate.gcad\npart p pin.gcad\npattern p count=3 step=10,0,0\n",
        &parts,
    );
    assert!(near(
        extent(&model, "p_3").min().x - extent(&model, "p").min().x,
        20.0
    ));
    let (model, _) = built(
        "pattern_turn",
        "part plate plate.gcad\npart p pin.gcad\nmove p 20,0,0\naxis up 0,0,0 0,0,1\npattern p count=4 angle=360 axis=up\n",
        &parts,
    );
    let b = extent(&model, "p_2");
    assert!(near((b.min().y + b.max().y) / 2.0, 20.0), "{b:?}");
    let error = failed(
        "pattern_unmated",
        "part plate plate.gcad\npart p pin.gcad\npattern p holes=plate:holes.side\n",
        &parts,
    );
    assert!(error.contains("mate it into one first"), "{error}");
}

const GEAR: &str = "let z=20\ngear z 2\ncircle 6\nteeth: extrude 6\n";

fn meshed(test: &str, phase: f64) -> Vec<Result<String, String>> {
    let assembly = [
        "part frame frame.gcad",
        "part a gear.gcad z=20",
        "part b gear.gcad z=12",
        "move b 32,0,0",
        &format!("rotate b {phase} axis=z about=32,0,0"),
        "axis wheel 0,0,0 0,0,1",
        "axis pinion 32,0,0 32,0,1",
        "joint drive a frame turn about=wheel",
        "joint follow b frame turn about=pinion",
        "couple follow drive ratio=-20/12",
        "pose drive 7",
        "interference none",
    ]
    .join("\n");
    let frame = "plane XY offset=-20\nrect 80 40 at=16,0\nextrude 5\n";
    steps(
        test,
        &assembly,
        &[("gear.gcad", GEAR), ("frame.gcad", frame)],
    )
    .1
}

#[test]
fn gears_in_mesh_turn_without_touching() {
    let results = meshed("gears_in_mesh", 15.0);
    assert!(results.iter().all(Result::is_ok), "{results:?}");
    let failed = meshed("gears_out_of_phase", 0.0);
    let clash = failed
        .iter()
        .find_map(|r| r.as_ref().err())
        .expect("teeth clash");
    assert!(clash.contains("a and b"), "{clash}");
}

#[test]
fn robot_arm_example() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/robot/arm.gasm");
    let fingers = |vars: &[(String, f64)]| {
        let run = run_path(&path, vars, None).expect("runs");
        let failed: Vec<String> = run
            .steps
            .iter()
            .filter_map(|(line, r)| r.as_ref().err().map(|e| format!("{}: {e:#}", line.number)))
            .collect();
        assert!(failed.is_empty(), "{failed:?}");
        assert_eq!(run.model.body_names().len(), 34);
        ["finger_a", "finger_b"].map(|name| {
            gcad::geometry::bounds(&run.model.named_body(name).expect("finger")).center()
        })
    };
    let [a0, b0] = fingers(&[]);
    let [a1, b1] = fingers(&[("jaw".into(), 5.0)]);
    let (da, db) = (a1 - a0, b1 - b0);
    let length = |v: [f64; 3]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
    assert!((length([da.x, da.y, da.z]) - 5.0).abs() < 1.0e-6, "{da:?}");
    assert!(
        length([da.x + db.x, da.y + db.y, da.z + db.z]) < 1.0e-6,
        "{da:?} {db:?}"
    );
}

#[test]
fn planetary_gears_turn_without_touching() {
    let assembly = [
        "part ring ring.gcad",
        "part sun gear.gcad",
        "part planet gear.gcad a=190",
        "move planet 18,0,0",
        "part carrier carrier.gcad",
        "axis centre 0,0,0 0,0,1",
        "axis pin 18,0,0 18,0,1",
        "joint drive carrier ring turn about=centre",
        "joint input sun ring turn about=centre",
        "couple input drive ratio=1+54/18",
        "joint spin planet carrier turn about=pin",
        "couple spin drive ratio=-54/18",
        "pattern planet count=3 angle=360 axis=centre",
        "pose drive 25",
        "interference none",
    ]
    .join("\n");
    let parts = [
        (
            "ring.gcad",
            "circle 70\ngear 54 1 internal\nteeth: extrude 6\n",
        ),
        (
            "gear.gcad",
            "let a=0\ngear 18 1 angle=a\ncircle 6\nteeth: extrude 6\n",
        ),
        (
            "carrier.gcad",
            "plane XY offset=7\ncircle 50\nplate: extrude 2\n",
        ),
    ];
    let (model, _) = built("planetary", &assembly, &parts);
    let names = model.body_names();
    assert!(names.contains(&"planet_3".to_string()), "{names:?}");
    let value = |name: &str| {
        model
            .joints
            .iter()
            .find(|j| j.name == name)
            .map(|j| j.value)
            .expect(name)
    };
    assert!((value("input") - 100.0).abs() < 1.0e-9);
    assert!((value("spin_3") + 75.0).abs() < 1.0e-9);
    let (_, out_of_phase) = steps(
        "planetary_clash",
        &assembly.replace("a=190", "a=180"),
        &parts,
    );
    assert!(out_of_phase.last().is_some_and(Result::is_err));
}

#[test]
fn standard_parts_from_the_library() {
    let plate = "rect 40 20\ntop: extrude 6\nplane top.end\nholes: hole M3 -10,0 10,0 cbore=M3\n";
    let (model, _) = built(
        "standard_parts_from_the_library",
        "part plate plate.gcad\npart s1 shcs:M3x10\nconcentric s1:shank.side plate:holes.side near=-10,0,6\nflush s1:head.start plate:holes.cbore_floor\npattern s1 holes=plate:holes.side\npart w1 washer:M3\nconcentric w1:bore.side plate:holes.side near=-10,0,0\nflush w1:washer.end plate:top.start\npart n1 nut:M3\nconcentric n1:bore.side plate:holes.side near=-10,0,0\nflush n1:nut.end plate:top.start offset=0.5\ninterference none",
        &[("plate.gcad", plate)],
    );
    let path =
        std::path::PathBuf::from(scratch("standard_parts_from_the_library")).join("top.gasm");
    let items = gcad::bom::items(&model, &path);
    let summary: Vec<(usize, &str, Option<&str>)> = items
        .iter()
        .map(|i| (i.names.len(), i.file.as_str(), i.material.as_deref()))
        .collect();
    assert_eq!(
        summary,
        [
            (1, "plate.gcad", None),
            (2, "ISO 4762 M3x10", Some("steel")),
            (1, "ISO 7089 M3", Some("steel")),
            (1, "ISO 4032 M3", Some("steel")),
        ]
    );
    let pi = std::f64::consts::PI;
    let hexagon = |s: f64| s * s * 3.0_f64.sqrt() / 2.0;
    let screw = pi * 2.75 * 2.75 * 3.0 - hexagon(2.5) * 1.3 + pi * 1.5 * 1.5 * 10.0;
    let washer = pi * (3.5 * 3.5 - 1.6 * 1.6) * 0.5;
    let nut = (hexagon(5.5) - pi * 1.5 * 1.5) * 2.4;
    for (item, expected) in items[1..].iter().zip([screw, washer, nut]) {
        assert!(
            (item.volume - expected).abs() < expected * 1.0e-3,
            "{} is {}, expected {expected}",
            item.file,
            item.volume
        );
    }
}

#[test]
fn step_assembly_as_a_part() {
    let step = format!(
        "{}/tests/fixtures/occt-instances.step",
        env!("CARGO_MANIFEST_DIR")
    );
    let (model, _) = built(
        "step_assembly_as_a_part",
        &format!("part rig {step}\nmove rig.pin_2 0,0,20"),
        &[],
    );
    assert_eq!(
        model.body_names(),
        ["rig.plate", "rig.pin", "rig.pin_2", "rig.pin_3"]
    );
    let path = std::path::PathBuf::from(scratch("step_assembly_as_a_part")).join("top.gasm");
    let counts: Vec<usize> = gcad::bom::items(&model, &path)
        .iter()
        .map(|i| i.names.len())
        .collect();
    assert_eq!(counts, [1, 3]);
    let pin = gcad::geometry::bounds(&model.named_body("rig.pin_3").expect("pin"));
    assert!((pin.min().x - 28.0).abs() < 1.0e-6, "{pin:?}");
}

#[test]
fn step_part_in_its_own_units() {
    let step = |name: &str| format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let (mm, _) = built(
        "step_units_mm",
        &format!("part rig {}", step("occt-instances.step")),
        &[],
    );
    let (cm, _) = built(
        "step_units_cm",
        &format!("part rig {}", step("occt-instances-cm.step")),
        &[],
    );
    let pin = |model: &Model| model.named_body("rig.pin_3").expect("pin");
    let ratio = gcad::geometry::volume(&pin(&cm)) / gcad::geometry::volume(&pin(&mm));
    assert!((ratio - 1000.0).abs() < 1.0e-6, "{ratio}");
    let (small, large) = (
        gcad::geometry::bounds(&pin(&mm)),
        gcad::geometry::bounds(&pin(&cm)),
    );
    for k in 0..2 {
        let centre = |b: &monstertruck::modeling::BoundingBox<monstertruck::modeling::Point3>| {
            (b.min()[k] + b.max()[k]) / 2.0
        };
        assert!(
            (centre(&small) - centre(&large)).abs() < 1.0e-6,
            "{small:?} {large:?}"
        );
    }
}

#[test]
fn step_voids_outside_the_solid_are_parts() {
    let step = format!(
        "{}/tests/fixtures/voids-apart.step",
        env!("CARGO_MANIFEST_DIR")
    );
    let (model, summaries) = built("voids_apart", &format!("part m {step}"), &[]);
    assert_eq!(
        model.body_names(),
        ["m.voids_apart", "m.voids_apart_2"],
        "{summaries:?}"
    );
    let volume = |name: &str| gcad::geometry::volume(&model.named_body(name).expect("body"));
    assert!((volume("m.voids_apart") - 1000.0).abs() < 1.0e-6);
    assert!((volume("m.voids_apart_2") - 8.0).abs() < 1.0e-6);
}

#[test]
fn kicad_board_in_inches_and_surface_models() {
    let step = format!(
        "{}/tests/fixtures/kicad-inch-board.step",
        env!("CARGO_MANIFEST_DIR")
    );
    let (model, summaries) = built("kicad_board", &format!("part pcb {step}"), &[]);
    assert_eq!(
        model.body_names(),
        ["pcb.board", "pcb.chip", "pcb.chip_2"],
        "{summaries:?}"
    );
    let volume = |name: &str| gcad::geometry::volume(&model.named_body(name).expect("body"));
    assert!((volume("pcb.board") - 196.0).abs() < 1.0e-6);
    let cube = 2.54_f64.powi(3);
    for chip in ["pcb.chip", "pcb.chip_2"] {
        assert!((volume(chip) - cube).abs() < cube * 1.0e-6, "{chip}");
    }
    let near = |a: f64, b: f64| (a - b).abs() < 1.0e-6;
    let first = gcad::geometry::bounds(&model.named_body("pcb.chip").expect("chip"));
    let second = gcad::geometry::bounds(&model.named_body("pcb.chip_2").expect("chip"));
    assert!(
        near(first.min().x, 7.46) && near(first.min().y, 2.46) && near(first.min().z, 1.0),
        "{first:?}"
    );
    assert!(
        near(second.min().x, 7.46) && near(second.min().y, 7.54),
        "{second:?}"
    );
}

#[test]
fn case_and_lid_with_screws() {
    let path = std::path::PathBuf::from(format!(
        "{}/examples/assemblies/case.gasm",
        env!("CARGO_MANIFEST_DIR")
    ));
    let run = gcad::model::run_path(&path, &[], None).expect("runs");
    let (line, last) = run.steps.last().expect("steps");
    assert!(last.is_ok(), "line {}: {last:?}", line.number);
    let items = gcad::bom::items(&run.model, &path);
    assert_eq!(items.len(), 3);
    assert_eq!(items[2].file, "ISO 10642 M3x8");
    assert_eq!(items[2].names.len(), 4);
}

#[test]
fn bearings_dowels_and_standoffs() {
    let (model, _) = built(
        "bearings_dowels_and_standoffs",
        "part b bearing:608\npart d dowel:4x16\nmove d 30,0,0\npart s standoff:M3x10\nmove s 60,0,0",
        &[],
    );
    let path = std::path::PathBuf::from(scratch("bearings_dowels_and_standoffs")).join("top.gasm");
    let items = gcad::bom::items(&model, &path);
    let pi = std::f64::consts::PI;
    let ring = |a: f64, b: f64| pi / 4.0 * (a * a - b * b);
    let wall = 7.0;
    let bearing = ring(22.0, 8.0) * 7.0 - 2.0 * ring(22.0 - 0.7 * wall, 8.0 + 0.7 * wall) * 0.35;
    let chamfer = 2.0 * pi * (2.0 - 0.4 / 3.0) * 0.4 * 0.4 / 2.0;
    let dowel = pi * 4.0 * 16.0 - 2.0 * chamfer;
    let standoff = (5.5 * 5.5 * 3.0_f64.sqrt() / 2.0 - pi * 1.25 * 1.25) * 10.0;
    let expected = [
        ("608 bearing 8x22x7", bearing, "steel"),
        ("ISO 8734 4x16", dowel, "steel"),
        ("hex standoff M3x10", standoff, "brass"),
    ];
    for (item, (file, volume, material)) in items.iter().zip(expected) {
        assert_eq!(item.file, file);
        assert_eq!(item.material.as_deref(), Some(material));
        assert!(
            (item.volume - volume).abs() < volume * 1.0e-3,
            "{file} is {}, expected {volume}",
            item.volume
        );
    }
    let bore = gcad::geometry::bounds(&model.named_body("b").expect("bearing"));
    assert!((bore.max().x - 11.0).abs() < 1.0e-6 && (bore.max().z - 7.0).abs() < 1.0e-6);
}

static KEPT_RUNS: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn kept_parts_rebuild_only_what_changed() {
    let _one_viewer = KEPT_RUNS.lock().unwrap_or_else(|e| e.into_inner());
    let path = files(
        "kept_parts_rebuild_only_what_changed",
        "part s sub.gasm\npart c block.gcad w=30\n",
        &[
            ("block.gcad", "let w=10\nrect w 10\nextrude 10\n"),
            ("other.gcad", "rect 10 10\nextrude 10\n"),
            ("sub.gasm", "part a block.gcad\npart b other.gcad\n"),
        ],
    );
    let build = || {
        let run = gcad::model::snapshots_path_kept(&path, &[]).expect("builds");
        run.snapshots.last().expect("a line").model.clone()
    };
    let faces = |model: &Model, name: &str| -> Vec<_> {
        model
            .named_body(name)
            .expect("part")
            .boundaries()
            .iter()
            .flat_map(|shell| shell.face_iter().map(|face| face.id()).collect::<Vec<_>>())
            .collect()
    };
    let edit = |name: &str, text: &str| {
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(path.with_file_name(name), text).expect("edit");
    };
    let first = build();
    edit("other.gcad", "rect 10 10\nextrude 25\n");
    let second = build();
    assert_eq!(faces(&first, "s.a"), faces(&second, "s.a"));
    assert_eq!(faces(&first, "c"), faces(&second, "c"));
    assert_ne!(faces(&first, "s.b"), faces(&second, "s.b"));
    let tall = gcad::geometry::volume(&second.named_body("s.b").expect("s.b"));
    assert!((tall - 2500.0).abs() < 1.0, "{tall}");
    edit(
        "sub.gasm",
        "part a block.gcad\npart b other.gcad\nlet unused=1\n",
    );
    let third = build();
    assert_eq!(faces(&second, "s.a"), faces(&third, "s.a"));
    assert_eq!(faces(&second, "s.b"), faces(&third, "s.b"));
    edit("block.gcad", "let w=10\nrect w 20\nextrude 10\n");
    let fourth = build();
    assert_ne!(faces(&third, "s.a"), faces(&fourth, "s.a"));
    assert_ne!(faces(&third, "c"), faces(&fourth, "c"));
    assert_eq!(faces(&third, "s.b"), faces(&fourth, "s.b"));
    let wide = gcad::geometry::volume(&fourth.named_body("c").expect("c"));
    assert!((wide - 6000.0).abs() < 1.0, "{wide}");
    let mut read: Vec<String> = gcad::model::snapshots_path_kept(&path, &[])
        .expect("builds")
        .files
        .iter()
        .filter_map(|file| Some(file.file_name()?.to_string_lossy().into_owned()))
        .collect();
    read.retain(|name| name.ends_with(".gcad") || name.ends_with(".gasm"));
    assert_eq!(read, ["block.gcad", "other.gcad", "sub.gasm", "top.gasm"]);
}

#[test]
fn a_newer_build_stops_the_older_one() {
    let _one_viewer = KEPT_RUNS.lock().unwrap_or_else(|e| e.into_inner());
    let path = files(
        "a_newer_build_stops_the_older_one",
        "part a block.gcad\npart b block.gcad\n",
        &[("block.gcad", "rect 10 10\nextrude 10\n")],
    );
    let (steps, parts) = gcad::host::superseding(|| {
        gcad::host::superseding(|| ());
        let steps = run_path(&path, &[], None).expect("reads").steps;
        let parts = gcad::host::each(vec![()], |_| gcad::host::superseded());
        (steps, parts)
    });
    assert_eq!(steps.len(), 1);
    let error = format!("{:#}", steps[0].1.as_ref().expect_err("stopped"));
    assert!(error.contains("a newer build took over"), "{error}");
    assert_eq!(parts, [Some(true)]);
    assert!(!gcad::host::superseded());
}

#[test]
fn kept_parts_follow_their_files() {
    let _one_viewer = KEPT_RUNS.lock().unwrap_or_else(|e| e.into_inner());
    let path = files(
        "kept_parts_follow_their_files",
        "part a block.gcad\n",
        &[("block.gcad", "rect 10 10\nextrude 10\n")],
    );
    let volume = |path: &std::path::Path| {
        let run = gcad::model::snapshots_path_kept(path, &[]).expect("builds");
        let model = &run.snapshots.last().expect("a line").model;
        gcad::geometry::volume(&model.named_body("a").expect("part a"))
    };
    assert!((volume(&path) - 1000.0).abs() < 1.0);
    std::fs::write(&path, "part a block.gcad\nmove a 5,0,0\n").expect("assembly");
    assert!((volume(&path) - 1000.0).abs() < 1.0);
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(
        path.with_file_name("block.gcad"),
        "rect 10 10\nextrude 20\n",
    )
    .expect("part");
    assert!((volume(&path) - 2000.0).abs() < 1.0);
}

#[test]
fn see_through_parts() {
    let glass = "rect 10 10\nextrude 2\ncolor #9cc7d9 opacity=0.3\n";
    let (model, _) = built(
        "see_through_parts",
        "part a glass.gcad\npart b glass.gcad\ncolor b red opacity=0.6",
        &[("glass.gcad", glass)],
    );
    assert_eq!(model.opacities.get("a"), Some(&0.3));
    assert_eq!(model.opacities.get("b"), Some(&0.6));
    let error = failed(
        "see_through_bad",
        "part a glass.gcad\ncolor a red opacity=0",
        &[("glass.gcad", glass)],
    );
    assert!(error.contains("opacity must be above 0"), "{error}");
}
