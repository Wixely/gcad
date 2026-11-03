mod common;

use common::*;
use gcad::model::Model;
use std::f64::consts::PI;

fn mesh_of(model: &Model) -> std::sync::Arc<gcad::mesh::Mesh> {
    model.meshes().last().expect("a mesh body").1.clone()
}

fn mesh_volume(model: &Model) -> f64 {
    mesh_of(model).volume()
}

fn near(actual: f64, expected: f64, relative: f64) {
    assert!(
        (actual - expected).abs() <= expected.abs() * relative,
        "got {actual:.4}, expected {expected:.4} within {:.2}%",
        relative * 100.0
    );
}

fn extent(model: &Model) -> ([f64; 3], [f64; 3]) {
    mesh_of(model).bounds().expect("vertices")
}

#[test]
fn blob_sphere() {
    let model = build("ball: blob 0,0,0 r=10\nremesh 0.4");
    near(mesh_volume(&model), 4.0 / 3.0 * PI * 1000.0, 0.01);
    assert_eq!(mesh_of(&model).open_edges(), 0);
}

#[test]
fn blob_ellipsoid_turned() {
    let model = build("egg: blob 0,0,0 r=10,4,2 rz=90\nremesh 0.2");
    near(mesh_volume(&model), 4.0 / 3.0 * PI * 80.0, 0.02);
    let (lo, hi) = extent(&model);
    assert!((hi[0] - lo[0] - 8.0).abs() < 0.2 && (hi[1] - lo[1] - 20.0).abs() < 0.2);
}

#[test]
fn limb_capsule() {
    let model = build("arm: limb 0,0,0 0,0,20 r=3\nremesh 0.25");
    near(
        mesh_volume(&model),
        PI * 9.0 * 20.0 + 4.0 / 3.0 * PI * 27.0,
        0.01,
    );
}

#[test]
fn blend_fills_the_join() {
    let hard = mesh_volume(&build("a: blob 0,0,0 r=5\nb: blob 8,0,0 r=5\nremesh 0.25"));
    let soft = mesh_volume(&build(
        "a: blob 0,0,0 r=5\nb: blob 8,0,0 r=5 blend=3\nremesh 0.25",
    ));
    assert!(soft > hard * 1.02, "blend {soft:.2} vs hard {hard:.2}");
}

#[test]
fn blob_cut() {
    let model = build("ball: blob 0,0,0 r=10\ncore: blob 0,0,0 r=5 mode=cut\nremesh 0.3");
    near(
        mesh_volume(&model),
        4.0 / 3.0 * PI * (1000.0 - 125.0),
        0.015,
    );
}

#[test]
fn mirror_blockout_names_the_other_side() {
    let model = build(
        "body: blob 0,0,0 r=4\near_l: blob 5,0,4 r=1.5 blend=1\nmirror x of=ear_l\nremesh 0.2",
    );
    let mesh = mesh_of(&model);
    assert!(
        mesh.parts.contains(&"ear_r".to_string()),
        "{:?}",
        mesh.parts
    );
    let (lo, hi) = extent(&model);
    assert!((lo[0] + hi[0]).abs() < 0.05, "{lo:?} {hi:?}");
}

#[test]
fn select_part_moves_only_that_part() {
    let model = build(
        "body: blob 0,0,0 r=5\nhead: blob 0,0,9 r=3 blend=1\nremesh 0.25\nselect head\nmove 0,0,4",
    );
    let (lo, hi) = extent(&model);
    assert!((hi[2] - 16.0).abs() < 0.1, "top {}", hi[2]);
    assert!((lo[2] + 5.0).abs() < 0.1, "bottom {}", lo[2]);
}

#[test]
fn select_falloff_keeps_far_vertices_still() {
    let before = build("bar: limb 0,0,0 40,0,0 r=2\nremesh 0.3");
    let after = build(
        "bar: limb 0,0,0 40,0,0 r=2\nremesh 0.3\nselect sphere=40,0,0 r=4 falloff=6\nmove 0,0,5",
    );
    let (a, b) = (mesh_of(&before), mesh_of(&after));
    let far = a
        .positions
        .iter()
        .zip(&b.positions)
        .filter(|(p, _)| p[0] < 40.0 - 4.0 - 6.0)
        .map(|(p, q)| (p[2] - q[2]).abs())
        .fold(0.0, f64::max);
    let tip = b.positions.iter().map(|p| p[2]).fold(f64::MIN, f64::max);
    assert_eq!(far, 0.0);
    assert!((tip - 7.0).abs() < 0.05, "tip {tip}");
}

#[test]
fn scale_selection_about_its_centre() {
    let model = build("ball: blob 0,0,0 r=4\nremesh 0.2\nselect all\nscale 2");
    near(mesh_volume(&model), 4.0 / 3.0 * PI * 64.0 * 8.0, 0.01);
}

#[test]
fn rotate_selection() {
    let model =
        build("arm: limb 0,0,0 10,0,0 r=1\nremesh 0.2\nselect all\nrotate 90 axis=z about=0,0,0");
    let (lo, hi) = extent(&model);
    assert!(
        (hi[1] - 11.0).abs() < 0.05 && (hi[0] - 1.0).abs() < 0.05,
        "{lo:?} {hi:?}"
    );
}

#[test]
fn inflate_grows_a_sphere() {
    let model = build("ball: blob 0,0,0 r=5\nremesh 0.2\nselect all\ninflate 1");
    near(mesh_volume(&model), 4.0 / 3.0 * PI * 216.0, 0.01);
}

#[test]
fn smooth_rounds_a_box_edge() {
    let model = build("bar: limb 0,0,0 0,0,10 r=2\nremesh 0.25\nselect all\nsmooth 10");
    let v = mesh_volume(&model);
    let start = PI * 4.0 * 10.0 + 4.0 / 3.0 * PI * 8.0;
    assert!(v < start && v > start * 0.85, "{v} vs {start}");
}

#[test]
fn select_mirror_edits_both_sides() {
    let start = build("body: blob 0,0,0 r=6\nremesh 0.25");
    let model = build(
        "body: blob 0,0,0 r=6\nremesh 0.25\nselect sphere=4,0,4 r=1.5 mirror=x falloff=1\ninflate -0.5",
    );
    let (before, after) = (mesh_of(&start), mesh_of(&model));
    let pulled_in = |side: f64| {
        before
            .positions
            .iter()
            .zip(&after.positions)
            .filter(|(p, _)| p[0] * side > 0.0)
            .map(|(p, q)| gcad::mesh::length(gcad::mesh::sub(*p, *q)))
            .fold(0.0, f64::max)
    };
    near(pulled_in(1.0), 0.5, 0.01);
    near(pulled_in(-1.0), 0.5, 0.01);
}

#[test]
fn mesh_import_obj() {
    let path = scratch("tetra.obj");
    std::fs::write(
        &path,
        "o tet\nv 0 0 0\nv 10 0 0\nv 0 10 0\nv 0 0 10\nf 1 3 2\nf 1 2 4\nf 1 4 3\nf 2 3 4\n",
    )
    .expect("writes");
    let model = build(&format!("import {path} mesh"));
    near(mesh_volume(&model), 1000.0 / 6.0, 1.0e-9);
    assert_eq!(mesh_of(&model).parts, vec!["tet".to_string()]);
}

#[test]
fn mesh_export_obj_keeps_parts() {
    let model = build("body: blob 0,0,0 r=4\nhead: blob 0,0,6 r=2 blend=1\nremesh 0.3");
    let path = scratch("parts.obj");
    gcad::export::export_model(&model, std::path::Path::new("parts.gcad"), &path).expect("exports");
    let text = std::fs::read_to_string(&path).expect("written");
    let back = gcad::mesh::read_obj(&text).expect("reads");
    assert_eq!(back.parts, vec!["body".to_string(), "head".to_string()]);
    near(back.volume(), mesh_volume(&model), 1.0e-9);
}

#[test]
fn mesh_and_solid_bodies_share_a_file() {
    let model = build("rect 10 10\nextrude 2\nbody figure\nball: blob 0,0,8 r=3\nremesh 0.3");
    assert_volume(&model, 200.0, 1.0e-6);
    near(mesh_volume(&model), 4.0 / 3.0 * PI * 27.0, 0.02);
}

#[test]
fn solid_ops_refuse_a_mesh_body() {
    let error = failure("ball: blob 0,0,0 r=3\nfillet 1 all");
    assert!(error.contains("is a mesh"), "{error}");
}

#[test]
fn select_unknown_part_lists_the_parts() {
    let error = failure("body: blob 0,0,0 r=3\nremesh 0.5\nselect tail");
    assert!(
        error.contains("no part `tail`") && error.contains("body"),
        "{error}"
    );
}

#[test]
fn creature_example() {
    let source = std::fs::read_to_string(format!(
        "{}/examples/mesh/creature.gcad",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("example exists");
    let model = build(&source);
    let mesh = mesh_of(&model);
    assert_eq!(
        mesh.parts,
        [
            "torso", "head", "snout", "ear_l", "ear_r", "arm_l", "arm_r", "leg_l", "leg_r"
        ]
    );
    assert_eq!(mesh.open_edges(), 0);
    near(mesh.volume(), 2652.355, 0.001);
}

fn coarse_ball() -> &'static str {
    "ball: blob 0,0,0 r=10\nremesh 1.5"
}

#[test]
fn refine_hits_the_edge_length_and_stays_closed() {
    let model = build(&format!("{}\nselect all\nrefine 0.5", coarse_ball()));
    let mesh = mesh_of(&model);
    assert_eq!(mesh.open_edges(), 0);
    let longest = mesh
        .triangles
        .iter()
        .flat_map(|t| (0..3).map(move |k| (t[k], t[(k + 1) % 3])))
        .map(|(a, b)| {
            gcad::mesh::length(gcad::mesh::sub(
                mesh.positions[a as usize],
                mesh.positions[b as usize],
            ))
        })
        .fold(0.0, f64::max);
    assert!(longest <= 0.5, "longest edge {longest}");
}

#[test]
fn refine_only_inside_the_selection() {
    let before = mesh_of(&build(coarse_ball()));
    let model = build(&format!(
        "{}\nselect sphere=0,0,10 r=3\nrefine 0.3",
        coarse_ball()
    ));
    let mesh = mesh_of(&model);
    assert_eq!(mesh.open_edges(), 0);
    let below = |m: &gcad::mesh::Mesh| {
        m.triangles
            .iter()
            .filter(|t| t.iter().all(|&i| m.positions[i as usize][2] < 0.0))
            .count()
    };
    assert_eq!(below(&mesh), below(&before));
    assert!(mesh.triangles.len() > before.triangles.len() * 2);
}

#[test]
fn refine_follows_the_curve() {
    let model = build("ball: blob 0,0,0 r=10\nremesh 4\nselect all\nrefine 0.4");
    let mesh = mesh_of(&model);
    let worst = mesh
        .positions
        .iter()
        .map(|p| (gcad::mesh::length(*p) - 10.0).abs())
        .fold(0.0, f64::max);
    assert!(worst < 0.25, "a new vertex sits {worst} off the sphere");
}

#[test]
fn crease_cuts_a_groove_of_the_given_depth() {
    let start = mesh_of(&build("slab: blob 0,0,0 r=10,10,3\nremesh 0.25"));
    let model = build(
        "slab: blob 0,0,0 r=10,10,3\nremesh 0.25\nselect all\ncrease -5,0,3 5,0,3 depth=0.4 width=0.8",
    );
    let mesh = mesh_of(&model);
    let deepest = start
        .positions
        .iter()
        .zip(&mesh.positions)
        .map(|(a, b)| a[2] - b[2])
        .fold(0.0, f64::max);
    assert!((deepest - 0.4).abs() < 0.03, "deepest {deepest}");
    let far = start
        .positions
        .iter()
        .zip(&mesh.positions)
        .filter(|(a, _)| a[1].abs() > 0.9)
        .map(|(a, b)| gcad::mesh::length(gcad::mesh::sub(*a, *b)))
        .fold(0.0, f64::max);
    assert_eq!(far, 0.0);
}

#[test]
fn pinch_draws_vertices_together_along_the_surface() {
    let start = mesh_of(&build(coarse_ball()));
    let model = build(&format!(
        "{}\nselect sphere=0,0,10 r=4\npinch 0.5 toward=0,0,10",
        coarse_ball()
    ));
    let mesh = mesh_of(&model);
    let (mut before, mut after) = (0.0, 0.0);
    for (a, b) in start.positions.iter().zip(&mesh.positions) {
        let to_tip = |p: &[f64; 3]| gcad::mesh::length(gcad::mesh::sub(*p, [0.0, 0.0, 10.0]));
        if to_tip(a) < 4.0 {
            before += to_tip(a);
            after += to_tip(b);
        }
    }
    near(after / before, 0.5, 0.05);
}

#[test]
fn flatten_onto_a_plane() {
    let model = build(
        "ball: blob 0,0,0 r=5\nremesh 0.2\nselect box=-10,-10,3,10,10,10\nflatten at=0,0,3 normal=z",
    );
    let (_, hi) = extent(&model);
    assert!((hi[2] - 3.0).abs() < 1.0e-9, "top {}", hi[2]);
    let h = 2.0;
    let cap = PI * h * h * (3.0 * 5.0 - h) / 3.0;
    near(mesh_volume(&model), 4.0 / 3.0 * PI * 125.0 - cap, 0.01);
}

#[test]
fn noise_is_repeatable_and_bounded() {
    let source = "ball: blob 0,0,0 r=5\nremesh 0.2\nselect all\nnoise 0.3 size=1 seed=7";
    let (a, b) = (mesh_of(&build(source)), mesh_of(&build(source)));
    assert_eq!(a.positions, b.positions);
    let off = a
        .positions
        .iter()
        .map(|p| (gcad::mesh::length(*p) - 5.0).abs())
        .fold(0.0, f64::max);
    assert!(off > 0.05 && off < 0.35, "{off}");
    let other = mesh_of(&build(
        "ball: blob 0,0,0 r=5\nremesh 0.2\nselect all\nnoise 0.3 size=1 seed=8",
    ));
    assert_ne!(a.positions, other.positions);
}

#[test]
fn relax_evens_triangles_without_shrinking() {
    let lumpy =
        "ball: blob 0,0,0 r=5\nremesh 0.4\nselect sphere=0,0,5 r=3\nrefine 0.15\nselect all";
    let start = mesh_of(&build(lumpy));
    let model = build(&format!("{lumpy}\nrelax 20"));
    let mesh = mesh_of(&model);
    near(mesh.volume(), start.volume(), 0.01);
    let spread = |m: &gcad::mesh::Mesh| {
        let lengths: Vec<f64> = m
            .triangles
            .iter()
            .flat_map(|t| (0..3).map(move |k| (t[k], t[(k + 1) % 3])))
            .map(|(a, b)| {
                gcad::mesh::length(gcad::mesh::sub(
                    m.positions[a as usize],
                    m.positions[b as usize],
                ))
            })
            .collect();
        let mean = lengths.iter().sum::<f64>() / lengths.len() as f64;
        (lengths.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / lengths.len() as f64).sqrt()
            / mean
    };
    assert!(
        spread(&mesh) < spread(&start),
        "{} vs {}",
        spread(&mesh),
        spread(&start)
    );
}

#[test]
fn decimate_to_a_budget_keeps_the_shape() {
    let model = build("ball: blob 0,0,0 r=10\nremesh 0.3\nselect all\ndecimate 2000");
    let mesh = mesh_of(&model);
    assert!(
        mesh.triangles.len() <= 2000 && mesh.triangles.len() > 1900,
        "{}",
        mesh.triangles.len()
    );
    assert_eq!(mesh.open_edges(), 0);
    near(mesh.volume(), 4.0 / 3.0 * PI * 1000.0, 0.02);
}

#[test]
fn decimate_by_share_leaves_unselected_triangles() {
    let source = "body: blob 0,0,0 r=5\nhead: blob 0,0,8 r=3 blend=1\nremesh 0.25";
    let start = mesh_of(&build(source));
    let model = build(&format!("{source}\nselect body\ndecimate 25%"));
    let mesh = mesh_of(&model);
    assert_eq!(mesh.open_edges(), 0);
    let high = |m: &gcad::mesh::Mesh| {
        m.triangles
            .iter()
            .filter(|t| t.iter().all(|&i| m.positions[i as usize][2] > 7.0))
            .count()
    };
    assert_eq!(high(&mesh), high(&start));
    assert!(mesh.triangles.len() < start.triangles.len() * 7 / 10);
}

#[test]
fn decimate_keeps_part_labels() {
    let model = build(
        "body: blob 0,0,0 r=5\nhead: blob 0,0,8 r=3 blend=1\nremesh 0.25\nselect all\ndecimate 1500\nselect head\nmove 0,0,2",
    );
    let (_, hi) = extent(&model);
    assert!(hi[2] > 12.5, "{}", hi[2]);
}

#[test]
fn fox_example_fits_a_game_budget() {
    let source = std::fs::read_to_string(format!(
        "{}/examples/mesh/fox.gcad",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("example exists");
    let lines = gcad::parse::parse_program(&source).expect("parses");
    let run = gcad::model::run_with(&[("budget".to_string(), 8000.0)], &lines);
    if let Some((line, Err(error))) = run.steps.last() {
        panic!("line {} `{}`: {error:#}", line.number, line.text);
    }
    let mesh = mesh_of(&run.model);
    assert_eq!(mesh.triangles.len(), 8000);
    assert_eq!(mesh.open_edges(), 0);
    assert_eq!(mesh.parts.len(), 38);
    assert!(mesh.parts.iter().any(|p| p == "toe2_r"));
}

#[test]
fn flatten_cut_trues_a_sole_without_crushing_the_paw() {
    let source =
        "paw: blob 0,0,2 r=2,3,2\nremesh 0.15\nselect all\nflatten at=0,0,0.6 normal=z cut";
    let start = mesh_of(&build("paw: blob 0,0,2 r=2,3,2\nremesh 0.15"));
    let mesh = mesh_of(&build(source));
    let (lo, hi) = mesh.bounds().expect("vertices");
    assert!((lo[2] - 0.6).abs() < 1.0e-9, "sole at {}", lo[2]);
    assert!(
        (hi[2] - start.bounds().expect("vertices").1[2]).abs() < 1.0e-9,
        "top {}",
        hi[2]
    );
    let untouched = start
        .positions
        .iter()
        .zip(&mesh.positions)
        .filter(|(p, _)| p[2] > 0.6)
        .all(|(p, q)| p == q);
    assert!(untouched);
}

#[test]
fn crease_drawn_just_off_the_surface_still_cuts() {
    let model = build(
        "slab: blob 0,0,0 r=10,10,3\nremesh 0.25\nselect all\ncrease -5,0,3.6 5,0,3.6 depth=0.4 width=0.8",
    );
    let start = mesh_of(&build("slab: blob 0,0,0 r=10,10,3\nremesh 0.25"));
    let mesh = mesh_of(&model);
    let deepest = start
        .positions
        .iter()
        .zip(&mesh.positions)
        .map(|(a, b)| a[2] - b[2])
        .fold(0.0, f64::max);
    assert!((deepest - 0.4).abs() < 0.03, "deepest {deepest}");
}

#[test]
fn colour_one_part() {
    let model = build(
        "body: blob 0,0,0 r=4\nhead: blob 0,0,6 r=2 blend=1\nremesh 0.3\ncolor #ffffff part=head\ncolor #d2691e",
    );
    let path = scratch("coloured.obj");
    gcad::export::export_model(&model, std::path::Path::new("coloured.gcad"), &path)
        .expect("exports");
    let obj = std::fs::read_to_string(&path).expect("obj");
    let mtl = std::fs::read_to_string(path.replace(".obj", ".mtl")).expect("mtl beside it");
    assert!(
        obj.contains("mtllib coloured.mtl") && obj.contains("usemtl head"),
        "{obj:.200}"
    );
    assert!(mtl.contains("newmtl head\nKd 1 1 1"), "{mtl}");
    assert!(mtl.contains("newmtl body\nKd 0.8235"), "{mtl}");
}

#[test]
fn colour_unknown_part_lists_the_parts() {
    let error = failure("body: blob 0,0,0 r=4\nremesh 0.5\ncolor red part=tail");
    assert!(
        error.contains("no part `tail`") && error.contains("body"),
        "{error}"
    );
}

#[test]
fn decimate_keeps_the_border_between_parts_fine() {
    let source = "left: blob -3,0,0 r=3,3,3\nright: blob 3,0,0 r=3,3,3 blend=1\nremesh 0.15";
    let before = mesh_of(&build(source));
    let model = build(&format!("{source}\nselect all\ndecimate 1500"));
    let mesh = mesh_of(&model);
    let border = |m: &gcad::mesh::Mesh| {
        m.triangles
            .iter()
            .filter(|t| {
                let l = t.map(|i| m.labels[i as usize]);
                l[0] != l[1] || l[1] != l[2]
            })
            .count()
    };
    let kept = border(&mesh) as f64 / border(&before) as f64;
    let overall = mesh.triangles.len() as f64 / before.triangles.len() as f64;
    assert!(
        kept > overall * 4.0,
        "kept {kept:.3} of the border against {overall:.3} overall"
    );
    let wandered = mesh
        .positions
        .iter()
        .zip(&mesh.labels)
        .filter(|(p, l)| (p[0] < 0.0) != (mesh.parts[**l as usize] == "left") && p[0].abs() > 0.05)
        .count();
    assert_eq!(
        wandered, 0,
        "vertices whose part is the other side of the border"
    );
}

#[test]
fn pinch_with_mirror_pulls_each_side_to_its_own_point() {
    let source = "body: blob 0,0,0 r=8,3,3\nremesh 0.25";
    let start = mesh_of(&build(source));
    let mesh = mesh_of(&build(&format!(
        "{source}\nselect sphere=8,0,0 r=1.5 mirror=x falloff=0.5\npinch 0.5"
    )));
    let most = start
        .positions
        .iter()
        .zip(&mesh.positions)
        .map(|(a, b)| gcad::mesh::length(gcad::mesh::sub(*a, *b)))
        .fold(0.0, f64::max);
    assert!(most < 1.0, "a vertex moved {most}");
    let left = mesh.positions.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
    let right = mesh.positions.iter().map(|p| p[0]).fold(f64::MIN, f64::max);
    assert!((left + right).abs() < 0.05, "{left} {right}");
}

#[test]
fn shell_is_its_own_closed_surface() {
    let model = build("head: blob 0,0,0 r=5\neye: blob 0,-4.9,1 r=1 mode=shell\nremesh 0.4");
    let mesh = mesh_of(&model);
    assert_eq!(mesh.open_edges(), 0);
    assert!(mesh.parts.contains(&"eye".to_string()));
    let eye: Vec<_> = mesh
        .positions
        .iter()
        .zip(&mesh.labels)
        .filter(|(_, l)| mesh.parts[**l as usize] == "eye")
        .map(|(p, _)| *p)
        .collect();
    let worst = eye
        .iter()
        .map(|p| (gcad::mesh::length(gcad::mesh::sub(*p, [0.0, -4.9, 1.0])) - 1.0).abs())
        .fold(0.0, f64::max);
    assert!(worst < 0.02, "shell vertices sit {worst} off the sphere");
    near(
        mesh.volume(),
        4.0 / 3.0 * PI * 125.0 + 4.0 / 3.0 * PI,
        0.015,
    );
}

#[test]
fn gloss_goes_to_the_mtl() {
    let model = build("body: blob 0,0,0 r=4\nnose: blob 0,-4,0 r=0.5 mode=shell\nremesh 0.3\ncolor #111111 part=nose gloss=0.8");
    let path = scratch("gloss.obj");
    gcad::export::export_model(&model, std::path::Path::new("gloss.gcad"), &path).expect("exports");
    let mtl = std::fs::read_to_string(path.replace(".obj", ".mtl")).expect("mtl");
    assert!(mtl.contains("newmtl nose\nKd 0.0667 0.0667 0.0667\nKs 0.8 0.8 0.8\nNs 802"), "{mtl}");
    let error = failure("body: blob 0,0,0 r=4\nremesh 0.5\ncolor red part=body gloss=2");
    assert!(error.contains("gloss goes from 0"), "{error}");
}

#[test]
fn soft_colour_fades_across_the_border() {
    let source = "left: blob -3,0,0 r=3\nright: blob 3,0,0 r=3 blend=1\nremesh 0.2\ncolor #000000\ncolor #ffffff part=right soft=2";
    let model = build(source);
    let (_, mesh, colours, _) = model.mesh_shading().into_iter().next().expect("a mesh");
    let at = |x: f64| {
        let (i, _) = mesh
            .positions
            .iter()
            .enumerate()
            .filter(|(_, p)| p[2] > 0.0 && p[1].abs() < 0.5)
            .min_by(|a, b| (a.1[0] - x).abs().total_cmp(&(b.1[0] - x).abs()))
            .expect("a vertex");
        colours[i][0]
    };
    assert!(at(5.5) > 0.99, "far right {}", at(5.5));
    assert!(at(-5.5) < 0.01, "far left {}", at(-5.5));
    let middle = at(0.0);
    assert!((0.3..0.7).contains(&middle), "at the border {middle}");
    assert!(at(1.0) > middle && at(1.0) < 0.99, "inside the fade {}", at(1.0));
}
