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
