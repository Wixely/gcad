use super::*;
use crate::asset::{
    author,
    skin::{Assignment, Skin},
};
use std::{
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn wait(
    harness: &mut egui_kittest::Harness<'_, Option<Document>>,
    condition: impl Fn(&Document) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        harness.step();
        let document = harness.state().as_ref().unwrap();
        if condition(document) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "rig reload timed out: {:?}",
            document.error
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn bind(path: &Path, output: &Path, joint: &str) {
    bind_node(path, output, "plate", joint);
}

fn bind_node(path: &Path, output: &Path, node: &str, joint: &str) {
    let asset = author::load_for_binding(path, &[], node).unwrap();
    let mesh = asset
        .scene
        .nodes
        .iter()
        .position(|n| n.name == node)
        .unwrap();
    let joint = asset
        .scene
        .nodes
        .iter()
        .position(|n| n.name == joint)
        .unwrap();
    let skin = Skin::assign(&asset.scene, mesh, vec![joint], Assignment::Rigid).unwrap();
    std::fs::write(output, asset.weight_file(&skin).unwrap().to_json().unwrap()).unwrap();
}

fn positions(document: &Document) -> Vec<[f32; 3]> {
    document
        .preview
        .as_ref()
        .unwrap()
        .frame
        .scene
        .surfaces
        .iter()
        .flat_map(|surface| surface.positions.iter().copied())
        .collect()
}

fn moved(document: &Document, original: &[[f32; 3]], dz: f32) -> bool {
    let current = positions(document);
    current.len() == original.len()
        && current.iter().zip(original).all(|(a, b)| {
            (a[0] - b[0]).abs() < 1e-5
                && (a[1] - b[1]).abs() < 1e-5
                && (a[2] - b[2] - dz).abs() < 1e-5
        })
}

#[test]
fn binding_and_rig_edits_update_live_pose_and_require_explicit_rebind() {
    use egui_kittest::kittest::Queryable as _;
    let root = std::env::temp_dir().join(format!(
        "gcad-live-rig-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("model.gasset");
    let part = root.join("part.gcad");
    let weights = root.join("model.weights.json");
    std::fs::write(&part, "rect 20 10\nextrude 2\n").unwrap();
    let source = "asset 1\nproject .\nsource plate part.gcad body=main\ntessellate plate tolerance=0.1\nuv plate projection=box tile=10\nnode plate source=plate\nnode base\nnode tip parent=base translation=0,0,2\nclip lift interpolation=linear\nkey lift tip time=0 translation=0,0,2\nkey lift tip time=1 translation=0,0,12\nskin plate weights=model.weights.json\n";
    std::fs::write(&path, source).unwrap();
    bind(&path, &weights, "base");
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1000., 800.))
        .with_context_setup(egui_bench::install)
        .build_ui_state(
            |ui, document: &mut Option<Document>| {
                let document = document.get_or_insert_with(|| {
                    let mut document = Document::open(path.clone(), vec![], ui.ctx().clone());
                    document.initial_pose(Some("lift".into()), 1.);
                    document
                });
                document.ui(ui);
            },
            None,
        );
    wait(&mut harness, Document::ready);
    let initial = positions(harness.state().as_ref().unwrap());
    assert!(!initial.is_empty());
    let camera = Camera {
        zoom: 1.8,
        ..Camera::default()
    };
    let preview = harness
        .state_mut()
        .as_mut()
        .unwrap()
        .preview
        .as_mut()
        .unwrap();
    preview.camera = camera;
    preview.weight_joint = Some("base".into());

    // Changing only the external binding must change the displayed deformation.
    bind(&path, &weights, "tip");
    wait(&mut harness, |d| d.ready() && moved(d, &initial, 10.));
    let preview = harness.state().as_ref().unwrap().preview.as_ref().unwrap();
    assert_eq!(preview.camera, camera);
    assert_eq!(preview.time, 1.);
    assert!(
        preview.weight_joint.is_none(),
        "removed palette selection must clear"
    );

    // Animation edits do not invalidate bind/rest data.
    let animated = source.replace("translation=0,0,12", "translation=0,0,22");
    std::fs::write(&path, &animated).unwrap();
    wait(&mut harness, |d| d.ready() && moved(d, &initial, 20.));
    let tip = harness
        .state()
        .as_ref()
        .unwrap()
        .preview
        .as_ref()
        .unwrap()
        .frame
        .rig
        .iter()
        .find(|node| node.name == "tip")
        .unwrap();
    assert_eq!(tip.point.unwrap()[2], 22.);

    // A rest-pose edit makes the saved binding stale. Keep the last valid frame.
    let changed_rest = animated.replace(
        "tip parent=base translation=0,0,2",
        "tip parent=base translation=0,0,4",
    );
    std::fs::write(&path, changed_rest).unwrap();
    wait(&mut harness, |d| d.error.is_some() && !d.loading);
    assert!(
        harness
            .state()
            .as_ref()
            .unwrap()
            .error
            .as_ref()
            .unwrap()
            .contains("weight file rig changed; explicit rebinding is required")
    );
    assert!(moved(harness.state().as_ref().unwrap(), &initial, 20.));
    assert!(
        harness
            .query_by_label("Showing last valid model.")
            .is_some()
    );
    bind(&path, &weights, "tip");
    wait(&mut harness, |d| d.ready() && moved(d, &initial, 18.));
    let preview = harness.state().as_ref().unwrap().preview.as_ref().unwrap();
    assert_eq!(preview.camera, camera);
    assert_eq!(preview.time, 1.);
    assert_eq!(
        preview.asset.scene.clips[preview.clip.unwrap()].name,
        "lift"
    );
    drop(harness);
    for file in [&path, &part, &weights] {
        std::fs::remove_file(file).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn consumer_controls_survive_source_reload_and_invalid_profile_recovery() {
    use egui_kittest::kittest::Queryable as _;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(format!(
        "consumer-live-{}-{}", std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("model.gasset");
    let source = include_str!("../../examples/game-assets/plate-consumer-profile.gasset")
        .replace("asset 1", "asset 1\nproject ../..")
        .replace("../parts/plate.gcad", "../../examples/parts/plate.gcad");
    std::fs::write(&path, &source).unwrap();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1000., 800.))
        .with_context_setup(egui_bench::install)
        .build_ui_state(|ui, document: &mut Option<Document>| {
            document.get_or_insert_with(|| Document::open(path.clone(), vec![], ui.ctx().clone())).ui(ui);
        }, None);
    wait(&mut harness, Document::ready);
    let initial = positions(harness.state().as_ref().unwrap());
    harness.get_by_label("Consumer preview").click();
    harness.run_steps(4);
    harness.get_by_label("Preview LOD").click();
    harness.run_steps(4);
    harness.get_by_label("LOD 1").click();
    harness.run_steps(4);
    let low = positions(harness.state().as_ref().unwrap());
    assert!(low.len() < initial.len());
    harness.get_by_label("Show collision source meshes").click();
    harness.run_steps(4);
    harness.get_by_label("Box volumes (X-ray)").click();
    harness.run_steps(4);
    let visible = positions(harness.state().as_ref().unwrap());
    assert_eq!(visible.len(), low.len() * 2);
    let camera = harness.state().as_ref().unwrap().preview.as_ref().unwrap().camera;
    let moved_source = source.replace("node model translation=10,20,30", "node model translation=10,20,50");
    std::fs::write(&path, &moved_source).unwrap();
    wait(&mut harness, |d| d.ready() && moved(d, &visible, 20.));
    let preview = harness.state().as_ref().unwrap().preview.as_ref().unwrap();
    assert_eq!(preview.camera, camera);
    assert_eq!(preview.consumer.level, Some(1));
    assert!(preview.consumer.collision);
    assert!(preview.consumer.boxes);
    assert!(preview.frame.collision_boxes[0].corners.iter().all(|p| p[2] >= 50.));
    std::fs::write(&path, moved_source.replace("height=0.15", "height=0.8")).unwrap();
    wait(&mut harness, |d| d.error.is_some() && !d.loading);
    assert!(moved(harness.state().as_ref().unwrap(), &visible, 20.));
    assert!(harness.query_by_label("Showing last valid model.").is_some());
    std::fs::write(&path, &source).unwrap();
    wait(&mut harness, |d| d.ready() && moved(d, &visible, 0.));
    assert_eq!(harness.state().as_ref().unwrap().preview.as_ref().unwrap().consumer.level, Some(1));
    drop(harness);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn skinned_consumer_playback_switches_lods_and_keeps_weight_and_rig_controls() {
    use egui_kittest::kittest::Queryable as _;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(format!(
        "consumer-playback-{}-{}", std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("model.gasset");
    let mut source = include_str!("../../examples/game-assets/plate-consumer-profile.gasset")
        .replace("asset 1", "asset 1\nproject ../..")
        .replace("../parts/plate.gcad", "../../examples/parts/plate.gcad");
    source.push_str("\nnode bone parent=model\nclip lift interpolation=linear\nkey lift bone time=0 translation=0,0,0\nkey lift bone time=1 translation=0,0,10\n");
    std::fs::write(&path, &source).unwrap();
    let high = root.join("high.weights.json");
    let low = root.join("low.weights.json");
    bind_node(&path, &high, "detail_high", "bone");
    bind_node(&path, &low, "detail_low", "bone");
    source.push_str("skin detail_high weights=high.weights.json\nskin detail_low weights=low.weights.json\n");
    std::fs::write(&path, &source).unwrap();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1100., 1000.))
        .with_context_setup(egui_bench::install)
        .build_ui_state(|ui, document: &mut Option<Document>| {
            document.get_or_insert_with(|| Document::open(path.clone(), vec![], ui.ctx().clone())).ui(ui);
        }, None);
    wait(&mut harness, Document::ready);
    harness.get_by_label("Skeleton (X-ray)").click();
    harness.run_steps(4);
    harness.get_by_label("Joint weights").click();
    harness.run_steps(4);
    harness.get_by_label("Weights: bone").click();
    harness.run_steps(4);
    harness.get_by_label("Animation").click();
    harness.run_steps(4);
    harness.get_by_label("lift").click();
    harness.run_steps(4);
    let rest = positions(harness.state().as_ref().unwrap());
    harness.get_by_label("Play").click();
    harness.run_steps(8);
    harness.get_by_label("Pause").click();
    harness.run_steps(4);
    let document = harness.state().as_ref().unwrap();
    let preview = document.preview.as_ref().unwrap();
    let time = preview.time;
    assert!(time > 0. && time < 1.);
    assert!(moved(document, &rest, (10. * time) as f32));
    let camera = preview.camera;
    harness.get_by_label("Consumer preview").click();
    harness.run_steps(4);
    harness.get_by_label("Preview LOD").click();
    harness.run_steps(4);
    harness.get_by_label("LOD 1").click();
    harness.run_steps(4);
    let document = harness.state().as_ref().unwrap();
    let preview = document.preview.as_ref().unwrap();
    assert!(positions(document).len() < rest.len());
    assert_eq!(preview.time, time);
    assert_eq!(preview.camera, camera);
    assert!(preview.skeleton);
    assert_eq!(preview.weight_joint.as_deref(), Some("bone"));
    assert_eq!(preview.frame.scene.surfaces.len(), 1);
    let reference = weights::frame(&preview.asset, preview.clip, time, Some("bone")).unwrap();
    assert_eq!(preview.frame.scene.surfaces[0].positions, reference.scene.surfaces[1].positions);
    assert!(preview.frame.rig.iter().any(|n| n.name=="bone" &&
        n.point.is_some_and(|p| (p[2]-(30.+10.*time) as f32).abs()<1e-4)));
    drop(harness);
    for file in [&path, &high, &low] { std::fs::remove_file(file).unwrap(); }
    std::fs::remove_dir(root).unwrap();
}
