mod common;

use common::*;
use gcad::inspect::{Options, inspect, view};

fn options(name: &str) -> Options {
    Options {
        view: view(name).expect("a view"),
        centre: None,
        span: None,
        part: None,
        grid: None,
        marks: Vec::new(),
        picks: Vec::new(),
    }
}

#[test]
fn pick_returns_the_surface_point_and_normal() {
    let model = build("ball: blob 0,0,0 r=10\nremesh 0.2");
    let mut o = options("front");
    o.centre = Some([0.0, 0.0, 0.0]);
    o.picks = Vec::new();
    o.span = Some(30.0);
    o.picks = vec![
        (512.0, 512.0),
        (512.0 + 6.0 * (1024.0 - 140.0) / 30.0, 512.0),
    ];
    let picture = inspect(&model, &o).expect("draws");
    let centre = picture.hits[0].as_ref().expect("hits the ball");
    assert!((centre.point[1] + 10.0).abs() < 0.02, "{:?}", centre.point);
    assert!((centre.normal[1] + 1.0).abs() < 0.01, "{:?}", centre.normal);
    let side = picture.hits[1].as_ref().expect("hits the ball");
    assert!((side.point[0] - 6.0).abs() < 0.02, "{:?}", side.point);
    assert!((side.point[1] + 8.0).abs() < 0.05, "{:?}", side.point);
    assert!(
        (side.normal[0] - 0.6).abs() < 0.02 && (side.normal[1] + 0.8).abs() < 0.02,
        "{:?}",
        side.normal
    );
}

#[test]
fn pixel_and_world_agree_in_every_view() {
    let model = build("ball: blob 3,-2,5 r=4\nremesh 0.3");
    for name in ["front", "back", "left", "right", "top", "bottom", "iso"] {
        let picture = inspect(&model, &options(name)).expect("draws");
        let p = [4.0, -1.0, 6.5];
        let (x, y, _) = picture.frame.to_pixel(p);
        let q = picture.frame.on_plane(x, y);
        let d = gcad::mesh::sub(p, q);
        let along = gcad::mesh::dot(d, picture.frame.view.toward);
        let off = gcad::mesh::length(gcad::mesh::sub(
            d,
            gcad::mesh::scale(picture.frame.view.toward, along),
        ));
        assert!(off < 1.0e-9, "{name}: {off}");
    }
}

#[test]
fn part_frames_just_that_part() {
    let model = build("body: blob 0,0,0 r=10\nhead: blob 0,0,14 r=3 blend=1\nremesh 0.3");
    let mut o = options("front");
    o.part = Some("head".into());
    let picture = inspect(&model, &o).expect("draws");
    assert!(picture.frame.span < 9.0, "span {}", picture.frame.span);
    assert!(
        (picture.frame.centre[2] - 14.0).abs() < 1.0,
        "{:?}",
        picture.frame.centre
    );
}

#[test]
fn marks_say_when_the_surface_hides_them() {
    let model = build("ball: blob 0,0,0 r=5\nremesh 0.3");
    let mut o = options("front");
    o.marks = vec![
        ("near".into(), [0.0, -5.5, 0.0]),
        ("inside".into(), [0.0, 0.0, 0.0]),
    ];
    let picture = inspect(&model, &o).expect("draws");
    assert!(
        picture
            .report
            .contains("mark 1 near: 0,-5.5,0 at pixel 512,512\n"),
        "{}",
        picture.report
    );
    assert!(
        picture
            .report
            .contains("inside: 0,0,0 at pixel 512,512, behind the surface"),
        "{}",
        picture.report
    );
}

#[test]
fn unknown_part_lists_the_parts() {
    let model = build("body: blob 0,0,0 r=4\nremesh 0.5");
    let mut o = options("front");
    o.part = Some("tail".into());
    let error = inspect(&model, &o).err().expect("fails").to_string();
    assert!(
        error.contains("no part `tail`") && error.contains("body"),
        "{error}"
    );
}

#[test]
fn solids_show_too() {
    let model = build("rect 10 10\nextrude 4");
    let mut o = options("top");
    o.picks = vec![(512.0, 512.0)];
    let picture = inspect(&model, &o).expect("draws");
    let hit = picture.hits[0].as_ref().expect("hits the top");
    assert!(
        (hit.point[2] - 4.0).abs() < 1.0e-6 && (hit.normal[2] - 1.0).abs() < 1.0e-6,
        "{:?}",
        hit.point
    );
}
