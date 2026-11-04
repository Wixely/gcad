mod common;

use common::*;
use std::f64::consts::PI;

fn frustum(a: f64, b: f64, h: f64) -> f64 {
    h / 3.0 * (a + b + (a * b).sqrt())
}

#[test]
fn extrude() {
    assert_bounds(
        &build("rect 10 10\nextrude 5"),
        [-5.0, -5.0, 0.0],
        [5.0, 5.0, 5.0],
    );
}

#[test]
fn extrude_reversed() {
    assert_bounds(
        &build("rect 10 10\nextrude -5"),
        [-5.0, -5.0, -5.0],
        [5.0, 5.0, 0.0],
    );
}

#[test]
fn extrude_draft() {
    let top = 20.0 - 2.0 * 10.0 * 5.0_f64.to_radians().tan();
    assert_volume(
        &build("rect 20 20\nextrude 10 draft=5"),
        frustum(400.0, top * top, 10.0),
        0.0005,
    );
}

#[test]
fn extrude_symmetric() {
    assert_bounds(
        &build("rect 10 10\nextrude 10 both"),
        [-5.0, -5.0, -5.0],
        [5.0, 5.0, 5.0],
    );
}

#[test]
fn extrude_up_to_face() {
    let model = build(
        "rect 40 30\nbase: extrude 10\nplane XY offset=20\nrect 10 10\nextrude upto=base.end",
    );
    assert_volume(&model, 13000.0, 1.0e-6);
}

#[test]
fn cut_blind() {
    assert_volume(
        &build("rect 40 30\nbase: extrude 10\nplane base.end\nrect 20 10\ncut 4"),
        11200.0,
        1.0e-6,
    );
}

#[test]
fn cut_from_inside_a_cylinder() {
    let model = build(
        "plane YZ\ncircle 24\nb: extrude 65\nplane XY offset=-10\nrect 70 30 at=32.5,0\ncut 2",
    );
    let segment = 144.0 * (10.0_f64 / 12.0).acos() - 10.0 * 44.0_f64.sqrt();
    assert_bounds(&model, [0.0, -12.0, -10.0], [65.0, 12.0, 12.0]);
    assert_volume(&model, (PI * 144.0 - segment) * 65.0, 1.0e-4);
}

#[test]
fn cut_beside_a_taller_block() {
    assert_volume(
        &build(
            "rect 40 30\nbase: extrude 10\nrect 10 30 at=15,0\nextrude 20\nplane base.end\nrect 20 10 at=10,0\ncut 4",
        ),
        14200.0,
        1.0e-6,
    );
}

#[test]
fn cut_through() {
    assert_volume(
        &build("rect 40 30\nbase: extrude 10\nplane base.end\nrect 10 10\ncut thru"),
        11000.0,
        1.0e-6,
    );
}

#[test]
fn cut_draft() {
    let d = 4.0 * 3.0_f64.to_radians().tan();
    let pocket =
        4.0 / 6.0 * (200.0 + (20.0 - 2.0 * d) * (10.0 - 2.0 * d) + 4.0 * (20.0 - d) * (10.0 - d));
    let model = build("rect 40 30\nbase: extrude 10\nplane base.end\nrect 20 10\ncut 4 draft=3");
    assert_volume(&model, 12000.0 - pocket, 0.0005);
}

#[test]
fn revolve_full() {
    assert_volume(
        &build("plane XZ\nrect 4 10 at=8,5\nrevolve 360 axis=y"),
        PI * 64.0 * 10.0,
        0.0005,
    );
}

#[test]
fn revolve_partial() {
    assert_volume(
        &build("plane XZ\nrect 4 10 at=8,5\nrevolve 90 axis=y"),
        PI * 64.0 * 10.0 / 4.0,
        0.0005,
    );
}

#[test]
fn sweep_bent_path() {
    let model = build("circle 6\npath 0,0,0 0,0,20 30,0,20 30,20,20 r=8\nsweep");
    assert_volume(&model, PI * 9.0 * (38.0 + 8.0 * PI), 0.0005);
}

#[test]
fn sweep_straight_path() {
    assert_volume(
        &build("rect 4 4\npath 0,0,0 10,10,10\nsweep"),
        16.0 * 300.0_f64.sqrt(),
        1.0e-6,
    );
}

#[test]
fn sweep_helix() {
    let length = 3.0 * ((20.0 * PI).powi(2) + 25.0).sqrt();
    assert_volume(
        &build("circle 2\nhelix r=10 pitch=5 turns=3\nsweep"),
        PI * length,
        0.005,
    );
}

#[test]
fn loft() {
    let model = build("rect 20 20\nsection\nplane XY offset=10\nrect 10 10\nsection\nloft");
    assert_volume(&model, frustum(400.0, 100.0, 10.0), 0.0005);
}

#[test]
fn shell() {
    let model = build("rect 40 30\nbase: extrude 20\nshell 2 open=base.end");
    assert_volume(&model, 24000.0 - 36.0 * 26.0 * 18.0, 1.0e-6);
}

#[test]
fn face_draft() {
    let top = 20.0 - 2.0 * 10.0 * 5.0_f64.to_radians().tan();
    let model = build("rect 20 20\nbase: extrude 10\ndraft 5 base.side neutral=base.start");
    assert_volume(&model, frustum(400.0, top * top, 10.0), 0.0005);
}

#[test]
fn push_face() {
    assert_volume(
        &build("rect 40 30\nbase: extrude 3\npush base.end 2"),
        6000.0,
        1.0e-6,
    );
}

#[test]
fn revolve_cut() {
    let model = build(
        "rect 40 40\nbase: extrude 10\nplane XZ\nrect 4 4 at=10,10\nrevolve 360 axis=y mode=cut",
    );
    assert_volume(&model, 16000.0 - PI * (144.0 - 64.0) * 2.0, 0.0005);
}

#[test]
fn sweep_cut() {
    let model =
        build("rect 40 40\nbase: extrude 10\ncircle 4\npath -30,0,10 30,0,10\nsweep mode=cut");
    assert_volume(&model, 16000.0 - 2.0 * PI * 40.0, 0.0005);
}

#[test]
fn loft_cut() {
    let model = build(
        "rect 40 40\nbase: extrude 10\nplane base.end offset=1\nrect 22 22\nsection\nplane base.end offset=-5\nrect 10 10\nsection\nloft mode=cut",
    );
    assert_volume(&model, 16000.0 - frustum(400.0, 100.0, 5.0), 0.0005);
}

#[test]
fn thin_extrude() {
    assert_volume(
        &build("rect 40 30\nextrude 10 thin=2"),
        (1200.0 - 36.0 * 26.0) * 10.0,
        1.0e-6,
    );
}

#[test]
fn extrude_up_to_offset_face() {
    let model = build(
        "rect 40 30\nbase: extrude 10\nplane XY offset=20\nrect 10 10\nextrude upto=base.end offset=-2",
    );
    assert_volume(&model, 13000.0, 1.0e-6);
}

#[test]
fn extrude_to_next_face() {
    let model = build("rect 40 30\nbase: extrude 10\nplane XY offset=20\nrect 10 10\nextrude next");
    assert_volume(&model, 13000.0, 1.0e-6);
}

#[test]
fn rib() {
    let model = build(
        "rect 40 20 at=20,0\nbase: extrude 2\nplane YZ offset=2\nrect 16 29 at=0,15.5\nwall: extrude 2\nplane XZ\npen 4,22\nline 24,2\nrib 2",
    );
    assert_volume(&model, 1600.0 + 928.0 - 32.0 + 400.0, 0.0005);
}

#[test]
fn smooth_loft() {
    let model = build(
        "rect 20 20\nsection\nplane XY offset=10\nrect 10 10\nsection\nplane XY offset=20\nrect 20 20\nsection\nloft smooth",
    );
    let v = volume(&model);
    assert!(v > 2000.0 && v < 2.0 * frustum(400.0, 100.0, 10.0), "{v}");
}

#[test]
fn loft_mixed_profiles() {
    let model = build("circle 20\nsection\nplane XY offset=10\nngon 20 6\nsection\nloft");
    let v = volume(&model);
    assert!(
        v > 0.95 * frustum(PI * 100.0, 259.8, 10.0) && v < 1.05 * frustum(PI * 100.0, 259.8, 10.0),
        "{v}"
    );
}

#[test]
fn loft_many_matching_edges() {
    let model =
        build("ngon 20 250\nsection\nplane XY offset=10\nngon 10 250\nsection\nloft smooth");
    let v = volume(&model);
    let expected = frustum(PI * 100.0, PI * 25.0, 10.0);
    assert!(v > 0.98 * expected && v < 1.02 * expected, "{v}");
}

#[test]
fn sweep_smooth_path() {
    let model = build("circle 2\npath 0,0,0 10,0,10 20,0,0 smooth\nsweep");
    let v = volume(&model);
    assert!(v > PI * 28.28 && v < PI * 40.0, "{v}");
}

#[test]
fn sweep_twist() {
    assert_volume(
        &build("rect 4 2\npath 0,0,0 0,0,20\nsweep twist=90"),
        160.0,
        0.002,
    );
}

#[test]
fn sweep_scale() {
    assert_volume(
        &build("circle 4\npath 0,0,0 0,0,10\nsweep scale=0.5"),
        PI / 3.0 * 10.0 * 7.0,
        0.002,
    );
}

fn threaded_section(pitch: f64, radius: f64, external: bool) -> f64 {
    threaded_slice(pitch, radius, external, 2_000)
}

fn threaded_volume(pitch: f64, length: f64, external: bool, reach: impl Fn(f64) -> f64) -> f64 {
    let slices = 2_000;
    (0..slices)
        .map(|i| {
            let z = length * (i as f64 + 0.5) / slices as f64;
            threaded_slice(pitch, reach(z), external, 4_000)
        })
        .sum::<f64>()
        * length
        / slices as f64
}

fn threaded_slice(pitch: f64, radius: f64, external: bool, steps: usize) -> f64 {
    let crest = 3.0;
    let root = crest - 5.0 / 8.0 * 3.0_f64.sqrt() / 2.0 * pitch;
    let profile = |s: f64| {
        let s = s / pitch;
        let r = if s < 1.0 / 8.0 {
            crest
        } else if s < 7.0 / 16.0 {
            crest + (root - crest) * (s - 1.0 / 8.0) / (5.0 / 16.0)
        } else if s < 11.0 / 16.0 {
            root
        } else {
            root + (crest - root) * (s - 11.0 / 16.0) / (5.0 / 16.0)
        };
        if external {
            r.min(radius)
        } else {
            r.max(radius)
        }
    };
    let mean = (0..steps)
        .map(|i| profile(pitch * (i as f64 + 0.5) / steps as f64).powi(2))
        .sum::<f64>()
        / steps as f64;
    PI * mean
}

fn thread_turns(model: &gcad::model::Model) -> Vec<f64> {
    use monstertruck::modeling::*;
    let solid = &model.solids()[0];
    let mut turns = Vec::new();
    for edge in solid.boundaries()[0].edge_iter() {
        let (front, back) = (edge.front().point(), edge.back().point());
        if (back.z - front.z).abs() < 1.0 {
            continue;
        }
        let curve = edge.oriented_curve();
        let (t0, t1) = curve.range_tuple();
        let (p, q) = (curve.subs(t0), curve.subs(t0 + (t1 - t0) * 0.01));
        let swept = (p.x * q.y - p.y * q.x) * (q.z - p.z).signum();
        turns.push(swept);
    }
    turns
}

#[test]
fn modelled_thread() {
    assert_volume(
        &build("circle 6\nrod: extrude 3\nthread M6 on=rod.side"),
        3.0 * threaded_section(1.0, 3.0, true),
        2.0e-4,
    );
}

#[test]
fn thread_turns_right_handed() {
    let turns = thread_turns(&build("circle 6\nrod: extrude 3\nthread M6 on=rod.side"));
    assert!(
        !turns.is_empty() && turns.iter().all(|t| *t > 0.0),
        "{turns:?}"
    );
    let turns = thread_turns(&build(
        "circle 6\nrod: extrude 3\nthread M6 on=rod.side left",
    ));
    assert!(
        !turns.is_empty() && turns.iter().all(|t| *t < 0.0),
        "{turns:?}"
    );
}

#[test]
fn thread_on_an_undersized_rod_with_a_fine_pitch() {
    assert_volume(
        &build("circle 5.9\nrod: extrude 3\nthread M6 on=rod.side pitch=0.75"),
        3.0 * threaded_section(0.75, 2.95, true),
        2.0e-4,
    );
}

#[test]
fn tapped_thread() {
    assert_volume(
        &build(
            "rect 20 20\nbase: extrude 4\nplane base.end\nh: hole 5 0,0 thread=M6\nthread M6 on=h.side",
        ),
        1600.0 - 4.0 * threaded_section(1.0, 2.5, false),
        2.0e-5,
    );
}

#[test]
fn thread_up_to_a_bolt_head() {
    let head = 3.0 * 3.0_f64.sqrt() / 2.0 * 25.0 * 4.0;
    assert_volume(
        &build(
            "ngon 10 6\nhead: extrude 4\nplane head.end\ncircle 6\nshank: extrude 4\nthread M6 on=shank.side",
        ),
        head + 4.0 * threaded_section(1.0, 3.0, true),
        2.0e-4,
    );
}

#[test]
fn thread_runs_out_into_chamfers() {
    assert_volume(
        &build(
            "circle 6\nrod: extrude 6\nchamfer 1 rod.end&rod.side|rod.start&rod.side\nthread M6 on=rod.side",
        ),
        threaded_volume(1.0, 6.0, true, |z| 3.0_f64.min(2.0 + z).min(8.0 - z)),
        2.0e-4,
    );
}

#[test]
fn left_hand_thread_runs_out_into_a_chamfer() {
    assert_volume(
        &build(
            "circle 5.9\nrod: extrude 6\nchamfer 0.8 rod.end&rod.side\nthread M6 on=rod.side left pitch=0.75",
        ),
        threaded_volume(0.75, 6.0, true, |z| 2.95_f64.min(8.15 - z)),
        2.0e-4,
    );
}

#[test]
fn tapped_thread_under_a_countersink() {
    assert_volume(
        &build(
            "rect 20 20\nbase: extrude 4\nplane base.end\nh: hole 5 0,0 csink=7,90 thread=M6\nthread M6 on=h.side",
        ),
        1600.0 - threaded_volume(1.0, 4.0, false, |z| 2.5_f64.max(z - 0.5)),
        2.0e-5,
    );
}

#[test]
fn thread_needs_a_round_face_that_fits() {
    assert!(
        failure("circle 6\nrod: extrude 3\nchamfer 0.3 rod.end&rod.side\nthread M6 on=rod.side")
            .contains("past the thread's root")
    );
    assert!(
        failure("circle 6\nrod: extrude 3\nfillet 1 rod.end&rod.side\nthread M6 on=rod.side")
            .contains("not a round")
    );
    assert!(failure("circle 8\nrod: extrude 3\nthread M6 on=rod.side").contains("rod between"));
    assert!(failure("rect 6 6\nbox: extrude 3\nthread M6 on=box.side").contains("round face"));
    assert!(failure("circle 6\nrod: extrude 3\nthread M7 on=rod.side").contains("unknown thread"));
    assert!(
        failure("circle 6\nrod: extrude 3\nthread M6 on=rod.side\nchamfer 0.5 rod.end")
            .contains("unchanged")
    );
}

#[test]
fn shell_two_openings() {
    assert_volume(
        &build("rect 40 30\nbase: extrude 20\nshell 2 open=base.end,base.start"),
        (1200.0 - 936.0) * 20.0,
        1.0e-6,
    );
}

#[test]
fn shell_revolved() {
    let model = build(
        "plane XZ\npen 0,0\nline 10,0\narc 0,10 center=0,0\nclose\ndome: revolve 360\nshell 1 open=dome.caps",
    );
    assert_volume(&model, 2.0 / 3.0 * PI * (1000.0 - 729.0), 0.002);
}

#[test]
fn draft_curved() {
    let top = 10.0 - 10.0 * 5.0_f64.to_radians().tan();
    let model = build("circle 20\nbase: extrude 10\ndraft 5 base.side neutral=base.start");
    assert_volume(
        &model,
        PI / 3.0 * 10.0 * (100.0 + top * top + 10.0 * top),
        0.001,
    );
}

#[test]
fn push_curved() {
    assert_volume(
        &build("circle 20\nbase: extrude 10\npush base.end -2"),
        PI * 100.0 * 8.0,
        0.0005,
    );
}

#[test]
fn wrap_text() {
    let model = build(
        "circle 20\nbase: extrude 20\nplane XZ\ntext HI size=8 at=-5,6\nwrap base.side depth=0.5",
    );
    assert!(volume(&model) < volume(&build("circle 20\nbase: extrude 20")) - 1.0);
}

#[test]
fn wrap_rectangle() {
    let plain = volume(&build("circle 40\nbase: extrude 20"));
    let cut = volume(&build(
        "circle 40\nbase: extrude 20\nplane XZ\nrect 10 6 at=0,10\nwrap base.side depth=1",
    ));
    let raised = volume(&build(
        "circle 40\nbase: extrude 20\nplane XZ\nrect 10 6 at=0,10\nwrap base.side depth=1 raise",
    ));
    let angle = 10.0 / 20.0;
    let removed = 0.5 * angle * (400.0 - 361.0) * 6.0;
    let added = 0.5 * angle * (441.0 - 400.0) * 6.0;
    assert!(
        ((plain - cut) - removed).abs() < removed * 0.02,
        "{}",
        plain - cut
    );
    assert!(
        ((raised - plain) - added).abs() < added * 0.02,
        "{}",
        raised - plain
    );
    assert!(
        failure("circle 40\nbase: extrude 20\nplane XY\nrect 4 4\nwrap base.side depth=1")
            .contains("along the cylinder's axis")
    );
}

#[test]
fn thicken_face() {
    assert_volume(
        &build("circle 20\nbase: extrude 10\nthicken base.side 1"),
        PI * 121.0 * 10.0,
        0.001,
    );
}

#[test]
fn stack_same_size() {
    assert_volume(
        &build("rect 20 20\nbase: extrude 10\nplane base.end\nrect 20 20\nextrude 5"),
        6000.0,
        1.0e-6,
    );
}

#[test]
fn notch_flush_with_sides() {
    assert_volume(
        &build("rect 20 20\nbase: extrude 10\nplane base.end\nrect 10 20 at=5,0\ncut 5"),
        3000.0,
        1.0e-6,
    );
}

#[test]
fn cut_on_an_outer_wall() {
    let model = build(
        "circle 24\ncircle 15\nring: extrude 4.5\nplane ring.end\ncircle 24\ncircle 20\ncut 1.5",
    );
    let annulus = |a: f64, b: f64| PI / 4.0 * (a * a - b * b);
    assert_volume(
        &model,
        annulus(24.0, 15.0) * 4.5 - annulus(24.0, 20.0) * 1.5,
        1.0e-4,
    );
}

#[test]
fn cut_on_a_bore() {
    let model = build(
        "circle 24\ncircle 15\nring: extrude 4.5\nplane ring.end\ncircle 20\ncircle 15\ncut 1.5",
    );
    let annulus = |a: f64, b: f64| PI / 4.0 * (a * a - b * b);
    assert_volume(
        &model,
        annulus(24.0, 15.0) * 4.5 - annulus(20.0, 15.0) * 1.5,
        1.0e-4,
    );
}

#[test]
fn step_flush_with_sides() {
    assert_volume(
        &build("rect 20 20\nbase: extrude 10\nplane base.end\nrect 20 10 at=0,5\nextrude 5"),
        5000.0,
        1.0e-6,
    );
}

#[test]
fn shell_follows_rounded_corners() {
    assert_volume(
        &build("rect 40 30\nbase: extrude 20\nfillet 3 base.side&base.side\nshell 2 open=base.end"),
        (1200.0 - (4.0 - PI) * 9.0) * 20.0 - (36.0 * 26.0 - (4.0 - PI)) * 18.0,
        1.0e-4,
    );
}

#[test]
fn shell_as_thick_as_the_rounding() {
    assert_volume(
        &build("rect 40 30\nbase: extrude 20\nfillet 2 base.side&base.side\nshell 2 open=base.end"),
        (1200.0 - (4.0 - PI) * 4.0) * 20.0 - 36.0 * 26.0 * 18.0,
        1.0e-4,
    );
}

#[test]
fn shell_follows_a_rounded_floor() {
    let outside = 24000.0 - 140.0 * SPANDREL * 4.0 + 4.0 * ROUND_CORNER_OVERLAP * 8.0;
    let inside = 38.0 * 28.0 * 19.0 - 132.0 * SPANDREL + 4.0 * ROUND_CORNER_OVERLAP;
    assert_volume(
        &build(
            "rect 40 30\nbase: extrude 20\nfillet 2 base.start&base.side\nshell 1 open=base.end",
        ),
        outside - inside,
        1.0e-4,
    );
}

#[test]
fn extrude_twisted() {
    let model = build("rect 10 2 at=5,0\nextrude 10 twist=90");
    assert_volume(&model, 200.0, 1.0e-3);
    let (min, max) = bounds(&model);
    assert!(
        max[1] > 10.0 && min[1] > -1.01,
        "turns counterclockwise: {min:?} {max:?}"
    );
    let down = build("rect 10 2 at=5,0\nextrude -10 twist=90");
    let (min, max) = bounds(&down);
    assert!(
        min[1] < -10.0 && max[1] < 1.01,
        "right-handed going down: {min:?} {max:?}"
    );
}

#[test]
fn extrude_scaled() {
    assert_volume(
        &build("rect 20 20\nextrude 10 scale=0.5"),
        frustum(400.0, 100.0, 10.0),
        1.0e-6,
    );
}

fn spur_area(teeth: f64, module: f64) -> f64 {
    volume(&build(&format!("gear {teeth} {module}\nextrude 1")))
}

#[test]
fn helical_gear() {
    let area = spur_area(20.0, 2.0);
    let model = build("gear 20 2 helix=20\nextrude 10");
    assert_volume(&model, area * 10.0, 2.0e-4);
    let twist = (10.0 * 20.0_f64.to_radians().tan() / 20.0).to_degrees();
    let lines = gcad::parse::parse_program("gear 20 2 helix=20\nextrude 10").expect("parses");
    let said = gcad::model::run(&lines).steps[0]
        .1
        .as_ref()
        .expect("runs")
        .clone();
    assert!(
        said.contains(&format!("{:.3} degrees per mm", twist / 10.0)),
        "{said}"
    );
}

#[test]
fn bevel_gear() {
    let area = spur_area(20.0, 2.0);
    let scale: f64 = 1.0 - 5.0 / 20.0;
    assert_volume(
        &build("gear 20 2 cone=45\nextrude 5"),
        area * 5.0 * (1.0 + scale + scale * scale) / 3.0,
        2.0e-4,
    );
    assert!(failure("gear 20 2 cone=45\nextrude 25").contains("cone apex"));
}

const OPEN_BOX: &str = "rect 40 30\nbox: extrude 20\nshell 2 open=box.end\n";

#[test]
fn lip_on_a_rim() {
    let open = 1200.0 * 20.0 - 36.0 * 26.0 * 18.0;
    assert_volume(
        &build(&format!("{OPEN_BOX}lip >Z 3")),
        open - (1200.0 - 38.0 * 28.0) * 3.0,
        1.0e-6,
    );
}

#[test]
fn groove_on_a_rim() {
    let open = 1200.0 * 20.0 - 36.0 * 26.0 * 18.0;
    assert_volume(
        &build(&format!("{OPEN_BOX}groove >Z 3")),
        open - (38.4 * 28.4 - 36.0 * 26.0) * 3.2,
        1.0e-6,
    );
    assert!(failure(&format!("{OPEN_BOX}lip >Z 3 w=2")).contains("wall is only"));
}

#[test]
fn screw_boss() {
    let plate = "rect 60 40\nextrude 4\nplane XY offset=4\n";
    assert_volume(
        &build(&format!("{plate}boss 8 10 0,0 15,0 hole=3")),
        9600.0 + 2.0 * PI * (16.0 - 2.25) * 10.0,
        2.0e-4,
    );
    let (r, h, b) = (4.0, 10.0, 1.6);
    let steps = 4000;
    let rib: f64 = (0..steps)
        .map(|i| {
            let z = -b / 2.0 + b * (i as f64 + 0.5) / steps as f64;
            let x0 = (r * r - z * z).sqrt();
            let height = |x: f64| 0.75 * h * (2.0 * r - x) / (1.2 * r);
            let (a, c) = (x0, 2.0 * r);
            (height(a) + height(c)) / 2.0 * (c - a) * b / steps as f64
        })
        .sum();
    assert_volume(
        &build(&format!("{plate}boss 8 10 0,0 ribs=3")),
        9600.0 + PI * 16.0 * 10.0 + 3.0 * rib,
        5.0e-4,
    );
}

#[test]
fn boss_for_an_insert() {
    let plate = "rect 60 40\nextrude 4\nplane XY offset=4\n";
    let model = build(&format!("{plate}posts: boss 7 10 0,0 hole=M3 fit=insert"));
    assert_volume(&model, 9600.0 + PI * 12.25 * 10.0 - PI * 4.0 * 6.7, 2.0e-4);
}

#[test]
fn vents_through_one_wall() {
    let open = 1200.0 * 20.0 - 36.0 * 26.0 * 18.0;
    let slot = 17.0 * 3.0 + PI * 2.25;
    assert_volume(
        &build(&format!(
            "{OPEN_BOX}plane >X\nvent 20 3 0,14 count=3 step=0,-5"
        )),
        open - 3.0 * slot * 2.0,
        1.0e-5,
    );
}

#[test]
fn snap_hook() {
    let (l, t, w, hook) = (8.0, 1.5, 6.0, 1.5);
    let rise = hook / 30.0_f64.to_radians().tan();
    let open = 1200.0 * 20.0 - 36.0 * 26.0 * 18.0;
    assert_volume(
        &build(&format!(
            "{OPEN_BOX}plane XY offset=2\nsnap {l} {t} {w} 0,5 dir=90"
        )),
        open + w * (t * l + hook * rise / 2.0),
        1.0e-5,
    );
}

fn bend(degrees: f64, r: f64, t: f64, width: f64) -> f64 {
    degrees.to_radians() / 2.0 * ((r + t).powi(2) - r * r) * width
}

fn allowance(degrees: f64, r: f64, t: f64, k: f64) -> f64 {
    degrees.to_radians() * (r + k * t)
}

const SHEET: &str = "sheet 1 r=1\nrect 40 30\nbase: tab\n";

#[test]
fn sheet_flange() {
    let model = build(&format!("{SHEET}flange base.end&>X 10"));
    assert_volume(&model, 1200.0 + bend(90.0, 1.0, 1.0, 30.0) + 300.0, 1.0e-4);
    assert_bounds(&model, [-20.0, -15.0, 0.0], [22.0, 15.0, 12.0]);
    let down = build(&format!("{SHEET}flange base.start&<Y 8"));
    assert_volume(&down, 1200.0 + bend(90.0, 1.0, 1.0, 40.0) + 320.0, 1.0e-4);
    assert_bounds(&down, [-20.0, -17.0, -9.0], [20.0, 15.0, 1.0]);
}

#[test]
fn sheet_flange_at_an_angle() {
    let model = build("sheet 2 r=3\nrect 50 30\nbase: tab\nflange base.start&>Y 15 angle=45");
    assert_volume(
        &model,
        3000.0 + bend(45.0, 3.0, 2.0, 50.0) + 15.0 * 50.0 * 2.0,
        1.0e-4,
    );
}

#[test]
fn sheet_flange_on_a_flange() {
    let model = build(&format!(
        "{SHEET}walls: flange base.end&>X 10\nflange >X&walls.end 5\nflange walls.side&walls.face 4"
    ));
    let flanges = 300.0 + 150.0 + 2.0 * 40.0;
    let bends = bend(90.0, 1.0, 1.0, 30.0) * 2.0 + bend(90.0, 1.0, 1.0, 10.0) * 2.0;
    assert_volume(&model, 1200.0 + flanges + bends, 1.0e-4);
}

#[test]
fn sheet_unfolds() {
    let ba = allowance(90.0, 1.0, 1.0, 0.44);
    let model = build(&format!(
        "{SHEET}plane base.end\nhole 3 -10,0\nwalls: flange base.end&base.side 10\nplane >X\nslot 10 4 at=0,8\ncut thru\nunfold"
    ));
    let flat = 1200.0 + 2.0 * (ba + 10.0) * 30.0 + 2.0 * (ba + 10.0) * 40.0;
    let holes = PI * 2.25 + 2.0 * (6.0 * 4.0 + PI * 4.0);
    assert_volume(&model, flat - holes, 1.0e-4);
    assert_bounds(
        &model,
        [-30.0 - ba, -25.0 - ba, 0.0],
        [30.0 + ba, 25.0 + ba, 1.0],
    );
    let note = gcad::model::run(
        &gcad::parse::parse_program(&format!(
            "{SHEET}walls: flange base.end&>X 10\nplane base.end\nrect 4 4\ncut 0.5\nunfold"
        ))
        .expect("parses"),
    );
    let said = note
        .steps
        .last()
        .expect("steps")
        .1
        .as_ref()
        .expect("unfolds")
        .clone();
    assert!(said.contains("1 cut(s) or hole(s)"), "{said}");
}

const BLANK: &str = "sheet 1 r=1\nrect 60 20\nbase: tab\n";

#[test]
fn sheet_bends_a_flat_blank() {
    let ba = allowance(90.0, 1.0, 1.0, 0.44);
    let text = format!("{BLANK}plane base.end\nh: hole 4 24,0\nfold: bend 10,-1 10,1 angle=90");
    let model = build(&text);
    let near = 10.0 - ba / 2.0 + 30.0;
    let far = 30.0 - (10.0 + ba / 2.0);
    let hole = PI * 4.0;
    assert_volume(
        &model,
        (near + far) * 20.0 - hole + bend(90.0, 1.0, 1.0, 20.0),
        1.0e-4,
    );
    let edge = 10.0 - ba / 2.0 + 2.0;
    assert_bounds(&model, [-30.0, -10.0, 0.0], [edge, 10.0, 2.0 + far]);
    let solid = model.solid.as_ref().expect("solid");
    let faces = gcad::select::select_faces("h.side", solid, &model.groups, model.tolerance())
        .expect("selects");
    assert!(!faces.is_empty());
    let moved = gcad::select::faces(solid);
    assert!(faces.iter().all(|&i| {
        moved[i]
            .vertex_iter()
            .all(|v| v.point().x > edge - 1.0 - 1.0e-6 && v.point().z > 2.0)
    }));
    let flat = build(&format!("{text}\nunfold"));
    assert_volume(&flat, 1200.0 - hole, 1.0e-4);
    assert_bounds(&flat, [-30.0, -10.0, 0.0], [30.0, 10.0, 1.0]);
    let down = build(&format!("{BLANK}bend -10,-1 -10,1 angle=-90"));
    assert_bounds(
        &down,
        [-10.0 + ba / 2.0 - 2.0, -10.0, -1.0 - (20.0 - ba / 2.0)],
        [30.0, 10.0, 1.0],
    );
    assert!(failure(&format!("{BLANK}bend 0,-1 0,1")).contains("origin"));
    assert!(
        failure(&format!(
            "{BLANK}plane base.end\nhole 2 10,0\nbend 10,-1 10,1"
        ))
        .contains("straight edges")
    );
    assert!(failure(&format!("{BLANK}flange base.end&>X 5\nbend 10,-1 10,1")).contains("both"));
}

#[test]
fn sheet_blank_with_any_outline() {
    let text =
        "sheet 1 r=1\nslot 60 20\ncircle 6 at=20,0\nbase: tab\nfold: bend -10,-1 -10,1 angle=-90";
    let flat = build(&format!("{text}\nunfold"));
    assert_volume(&flat, 800.0 + PI * 100.0 - PI * 9.0, 1.0e-4);
    let dxf = gcad::model::flat_dxf(flat.sheet.as_ref().expect("sheet"), flat.solid.as_ref());
    assert!(dxf.matches("OUTLINE").count() > 40, "{dxf}");
    assert_eq!(dxf.matches("\nBEND\n").count(), 1);
    assert!(failure(&format!("{text}\nflange base.end&>X 5")).contains("`bend`"));
}

#[test]
fn sheet_darts_stiffen_a_bend() {
    let base = format!("{BLANK}fold: bend 10,-1 10,1 angle=90\n");
    let plain = build(&base);
    let darted = build(&format!("{base}dart fold -4 4 l=8 w=4"));
    let faces = |model: &gcad::model::Model| {
        gcad::select::faces(model.solid.as_ref().expect("solid")).len()
    };
    assert!(faces(&darted) > faces(&plain) + 8);
    let change = (volume(&darted) - volume(&plain)) / volume(&plain);
    assert!(change.abs() < 1.0e-2, "{change}");
    assert!(failure(&format!("{base}dart fold 0 w=2")).contains("wider"));
    assert!(failure(&format!("{base}dart fold 9 w=4")).contains("runs off"));
    assert!(failure(&format!("{base}dart nothing")).contains("not a bend"));
}

#[test]
fn sheet_flange_inset_and_hem() {
    let model = build(&format!(
        "{SHEET}flange base.end&>X 10 inset=5\nflange base.end&<X 10 inset=2,8\nflange base.end&>Y 6 angle=180"
    ));
    let flanges = 2.0 * (bend(90.0, 1.0, 1.0, 20.0) + 200.0) + bend(180.0, 1.0, 1.0, 40.0) + 240.0;
    assert_volume(&model, 1200.0 + flanges, 1.0e-4);
    let flat = build(&format!(
        "{SHEET}flange base.end&>X 10 inset=5\nflange base.end&<X 10 inset=2,8\nflange base.end&>Y 6 angle=180\nunfold"
    ));
    let ba = allowance(90.0, 1.0, 1.0, 0.44);
    assert_volume(
        &flat,
        1200.0 + 2.0 * (ba + 10.0) * 20.0 + (2.0 * ba + 6.0) * 40.0,
        1.0e-4,
    );
    assert!(failure(&format!("{SHEET}flange base.end&>X 10 inset=20")).contains("nothing"));
    assert!(
        failure("sheet 1 r=0\nrect 40 30\nbase: tab\nflange base.end&>X 5 angle=180")
            .contains("hem")
    );
}

#[test]
fn structural_members() {
    let straight = |kind: &str, area: f64| {
        let round = kind.starts_with("pipe") || kind.starts_with("rod");
        assert_volume(
            &build(&format!("member {kind} 0,0,0 0,0,250")),
            area * 250.0,
            if round { 5.0e-4 } else { 1.0e-6 },
        );
    };
    straight("tube 40x30x3", 1200.0 - 34.0 * 24.0);
    straight("bar 40x10", 400.0);
    straight("pipe 33.7x2.6", PI / 4.0 * (33.7 * 33.7 - 28.5 * 28.5));
    straight("rod 12", PI * 36.0);
    straight("angle 40x30x4", 40.0 * 4.0 + 26.0 * 4.0);
    straight("channel 50x25x3", 50.0 * 3.0 + 2.0 * 22.0 * 3.0);
    assert!(failure("member tube 40x40x25 0,0,0 0,0,10").contains("walls"));
}

#[test]
fn member_materials() {
    let model = build(
        "body panel\nrect 10 10\nextrude 1\nmaterial pla\nbeam: member bar 10x20 0,0,0 100,0,0 material=wood\nframe: member bar 4x4 0,0,40 100,0,40 100,100,40 material=steel yield=355",
    );
    assert_eq!(model.materials["panel"].name, "pla");
    assert_eq!(model.materials["beam"].name, "wood");
    assert_eq!(model.materials["beam"].density, 0.6);
    for name in ["frame_1", "frame_2"] {
        assert_eq!(model.materials[name].name, "steel");
        assert_eq!(model.materials[name].mechanics.strength, Some(355.0));
    }
    assert!(failure("member bar 10x10 0,0,0 100,0,0 material=unknown").contains("density="));
    assert!(
        failure("member bar 10x10 0,0,0 100,0,0 material=wood density=-1").contains("above zero")
    );
}

#[test]
fn linear_member_pattern() {
    let single = build("beam: member bar 10x20 0,0,0 0,100,0 material=wood count=1 step=40,0,0");
    assert_eq!(single.members.len(), 1);
    let path =
        build("frame: member bar 10x20 0,0,0 100,0,0 100,100,0 material=wood count=3 step=0,0,50");
    assert_eq!(path.members.len(), 6);
    assert_eq!(path.materials.len(), 6);
    let source = "joist: member bar 10x20 0,0,0 0,100,0 material=wood count=100 step=40,0,-1";
    let model = build(source);
    assert_eq!(model.members.len(), 100);
    assert_eq!(model.solids().len(), 100);
    assert_eq!(model.materials.len(), 100);
    assert!(model.materials.values().all(|m| m.name == "wood"));
    assert_bounds(&model, [-5.0, 0.0, -109.0], [3965.0, 100.0, 10.0]);
    assert_volume(&model, 2_000_000.0, 1.0e-6);
    let items = gcad::bom::items(&model, std::path::Path::new("joists.gcad"));
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].names.len(), 100);
    assert!(common::summary(source).contains("100 instances"));
    assert!(!common::summary(&format!("{source}\nmeasure mass")).contains("no material"));
}

#[test]
fn circular_member_pattern() {
    let model = build(
        "axis hub 10,20,0 10,20,1\nspoke: member bar 4x6 30,20,0 110,20,0 material=aluminium count=4 angle=360 axis=hub",
    );
    assert_eq!(model.members.len(), 4);
    assert_eq!(model.materials.len(), 4);
    assert!(model.materials.values().all(|m| m.name == "aluminium"));
    assert_bounds(&model, [-90.0, -80.0, -3.0], [110.0, 120.0, 3.0]);
    assert_volume(&model, 4.0 * 4.0 * 6.0 * 80.0, 1.0e-6);
    for options in [
        "count=2.5 step=10,0,0",
        "count=0 step=10,0,0",
        "count=2",
        "count=2 angle=360",
        "count=2 step=10,0,0 angle=360",
    ] {
        assert!(!failure(&format!("member bar 4x6 0,0,0 0,0,100 {options}")).is_empty());
    }
}

#[test]
fn member_pattern_preserves_library_material() {
    let dir = std::path::PathBuf::from(common::scratch("member_pattern_material"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("materials.toml"),
        "[timber]\ndensity = 0.45\nE = 11\ncolor = \"#d8b47a\"\nroughness = 0.75\n",
    )
    .unwrap();
    let lines = gcad::parse::parse_program("beam: member bar 10x20 0,0,0 0,100,0 material=timber count=3 step=40,0,0\npattern beam_1 count=2 step=0,0,40").unwrap();
    let run = gcad::model::run_in(Some(dir), &lines);
    for (_, result) in &run.steps {
        assert!(result.is_ok(), "{result:?}");
    }
    let model = run.model;
    assert_eq!(model.members.len(), 4);
    for name in &model.members {
        let m = &model.materials[name];
        assert_eq!(m.density, 0.45);
        assert_eq!(m.mechanics.young, Some(11.0));
        assert!(m.finish.is_some());
        assert_eq!(model.colours[name], model.colours["beam_1"]);
    }
}

#[test]
fn member_row_tapers_and_skips() {
    let model = build(
        "let gap=300\nstud: member bar 20x40 0,0,0 0,0,1000 material=c24 count=8 step=100,0,0 end_step=100,0,-50 when=(k*100<gap)+(k*100>=500)",
    );
    assert_eq!(model.members.len(), 6);
    assert_eq!(model.materials.len(), 6);
    let names: Vec<&String> = model.members.iter().collect();
    assert_eq!(
        names,
        ["stud_1", "stud_2", "stud_3", "stud_6", "stud_7", "stud_8"]
    );
    let lengths = [
        1000.0,
        1000.0 - 50.0,
        1000.0 - 100.0,
        1000.0 - 250.0,
        1000.0 - 300.0,
        1000.0 - 350.0,
    ];
    assert_volume(&model, lengths.iter().sum::<f64>() * 800.0, 1.0e-6);
    let items = gcad::bom::items(&model, std::path::Path::new("studs.gcad"));
    assert_eq!(items.len(), 6);
    assert!(
        common::summary("stud: member bar 20x40 0,0,0 0,0,100 count=4 step=50,0,0 when=k>1")
            .contains("2 skipped")
    );
    assert!(common::failure("member bar 20x40 0,0,0 0,0,100 step=50,0,0").contains("count"));
    assert!(
        common::failure("member bar 20x40 0,0,0 0,0,50 0,50,50 count=2 step=5,0,0 end_step=5,0,1")
            .contains("two points")
    );
    assert!(
        common::failure("member bar 20x40 0,0,0 0,0,100 count=2 step=50,0,0 when=nothing")
            .contains("nothing")
    );
}

#[test]
fn mitred_frame() {
    let model = build("frame: member tube 40x40x3 0,0,0 500,0,0 500,300,0 0,300,0 closed");
    let area = 1600.0 - 34.0 * 34.0;
    assert_volume(&model, area * 1600.0, 1.0e-5);
    assert_bounds(&model, [-20.0, -20.0, -20.0], [520.0, 320.0, 20.0]);
    let solids = model.solids();
    for pair in solids.windows(2) {
        assert!(gcad::geometry::overlap_volume(pair[0], pair[1], 64) < 1.0e-3);
    }
    let items = gcad::bom::items(&model, std::path::Path::new("frame.gcad"));
    let cut: Vec<(usize, &str, &str)> = items
        .iter()
        .map(|i| (i.names.len(), i.file.as_str(), i.body.as_str()))
        .collect();
    assert_eq!(
        cut,
        [
            (2, "tube 40x40x3", "540.0 long, cut 45.0/45.0"),
            (2, "tube 40x40x3", "340.0 long, cut 45.0/45.0"),
        ]
    );
}

fn assert_within(started: std::time::Instant, release_limit_s: f64) {
    let scale = if cfg!(debug_assertions) { 10.0 } else { 1.0 };
    let elapsed = started.elapsed().as_secs_f64();
    assert!(elapsed < release_limit_s * scale, "took {elapsed:.1}s");
}

#[test]
fn twisted_extrude_with_a_hole() {
    let started = std::time::Instant::now();
    let model = build("rect 20 20\ncircle 8\nextrude 40 twist=720");
    assert_volume(&model, (400.0 - 16.0 * PI) * 40.0, 1.0e-3);
    assert_within(started, 5.0);
}

#[test]
fn boss_on_a_twisted_face() {
    let started = std::time::Instant::now();
    let model = build(
        "rect 20 20\ncircle 8\nthread: extrude 40 twist=720\nplane thread.end\ncircle 12\ncircle 9\nboss: extrude 10",
    );
    assert_volume(
        &model,
        (400.0 - 16.0 * PI) * 40.0 + PI / 4.0 * (144.0 - 81.0) * 10.0,
        1.0e-3,
    );
    assert_within(started, 8.0);
}

#[test]
fn boss_glued_onto_a_face() {
    let model = build(
        "rect 40 30\nbase: extrude 10\nplane base.end\ncircle 12 at=5,0\ncircle 4 at=5,0\nboss: extrude 8\nplane base.end\nrect 6 6 at=-12,0\npad: extrude 3",
    );
    assert_volume(
        &model,
        12000.0 + PI / 4.0 * (144.0 - 16.0) * 8.0 + 108.0,
        1.0e-4,
    );
    assert_eq!(model.solids().len(), 1);
}

#[test]
fn boss_over_an_edge_still_joins() {
    let model =
        build("rect 40 30\nbase: extrude 10\nplane base.end\nrect 20 10 at=15,0\nboss: extrude 5");
    assert_volume(&model, 12000.0 + 200.0 * 5.0, 1.0e-6);
}

#[test]
fn cut_where_the_tool_wall_meets_a_corner_fillet_start() {
    let model = build(
        "plane XY\nrect 8.94 7.3 r=0.3\nshell: extrude 3.26\nplane <Y offset=0.5\nrect 8.34 2.56 at=0,1.63 r=1\nmouth: cut 7",
    );
    assert_volume(
        &model,
        79.565 - 4.0 * (1.0 - PI / 4.0) * 0.09 * 3.26,
        5.0e-4,
    );
}
