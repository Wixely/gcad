mod common;

use gcad::model::{AssemblyInstance, Model};
use monstertruck::modeling::{Deg, Matrix4, SquareMatrix, Vector3};
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    let dir = PathBuf::from(common::scratch(name));
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in [
        ("block.gcad", "rect 2 3\nextrude 4\n"),
        ("single.gasm", "part inner block.gcad\nmove inner 1,2,3\n"),
        (
            "pair.gasm",
            "part a single.gasm\npart b block.gcad\nmove b 5,0,0\n",
        ),
        (
            "top.gasm",
            "part left pair.gasm\npart right pair.gasm\nmove left 10,0,0\nrotate left 90 axis=z\npart selected pair.gasm body=a\n",
        ),
    ] {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir
}

fn child<'a>(instances: &'a [AssemblyInstance], name: &str) -> &'a AssemblyInstance {
    instances.iter().find(|i| i.name == name).unwrap()
}

fn matrix(actual: Matrix4, expected: Matrix4) {
    for column in 0..4 {
        for row in 0..4 {
            assert!((actual[column][row] - expected[column][row]).abs() < 1e-10);
        }
    }
}

fn built() -> Model {
    let run =
        gcad::model::run_path(&fixture("assembly_hierarchy").join("top.gasm"), &[], None).unwrap();
    for (line, result) in &run.steps {
        assert!(result.is_ok(), "{}: {result:?}", line.text);
    }
    run.model
}

#[test]
fn nested_instance_metadata_preserves_renames_selection_and_world_frames() {
    let mut model = built();
    let left = child(&model.assembly_instances, "left");
    let right = child(&model.assembly_instances, "right");
    let selected = child(&model.assembly_instances, "selected");
    let move_left =
        Matrix4::from_angle_z(Deg(90.)) * Matrix4::from_translation(Vector3::new(10., 0., 0.));
    let inner = Matrix4::from_translation(Vector3::new(1., 2., 3.));
    matrix(left.placement, move_left);
    matrix(right.placement, Matrix4::identity());
    assert_eq!(left.bodies, ["left.a", "left.b"]);
    assert_eq!(selected.bodies, ["selected"]);
    assert_eq!(selected.children.len(), 1);
    assert_eq!(selected.children[0].name, "a");
    assert_eq!(selected.children[0].children[0].name, "inner");
    assert_eq!(selected.children[0].children[0].bodies, ["selected"]);
    matrix(
        child(&child(&left.children, "a").children, "inner").placement,
        move_left * inner,
    );
    matrix(model.placements["left.a"], move_left * inner);
    matrix(model.placements["right.a"], inner);
    let unchanged = model.assembly_instances.clone();
    let line = gcad::parse::parse_line(1, "move left.a 0,0,7").unwrap();
    model.apply(&line).unwrap();
    let left = child(&model.assembly_instances, "left");
    matrix(left.placement, move_left); // Only one descendant moved.
    matrix(
        child(&child(&left.children, "a").children, "inner").placement,
        Matrix4::from_translation(Vector3::new(0., 0., 7.)) * move_left * inner,
    );
    assert_eq!(
        child(&model.assembly_instances, "right"),
        child(&unchanged, "right")
    );
    assert_eq!(
        child(&left.children, "b"),
        child(&child(&unchanged, "left").children, "b")
    );
    let before_pattern = model.assembly_instances.clone();
    let line = gcad::parse::parse_line(2, "pattern right.a count=2 step=0,0,9").unwrap();
    model.apply(&line).unwrap();
    let copied = child(&model.assembly_instances, "right.a_2");
    assert_eq!(copied.bodies, ["right.a_2"]);
    assert!(copied.children.is_empty()); // The command copies this body only.
    matrix(
        copied.placement,
        Matrix4::from_translation(Vector3::new(0., 0., 9.)) * inner,
    );
    assert_eq!(
        &model.assembly_instances[..before_pattern.len()],
        &before_pattern
    );
}

#[cfg(feature = "game-assets")]
#[test]
fn nested_asset_export_preserves_world_geometry_sharing_and_motion() {
    use gcad::asset::{author, scene::Transform};
    use monstertruck::modeling::{InnerSpace, Point3, Transform as _};
    let dir = fixture("asset_assembly_hierarchy");
    let top = std::fs::read_to_string(dir.join("top.gasm")).unwrap();
    std::fs::write(
        dir.join("top.gasm"),
        top.replace(
            "rotate left 90 axis=z",
            "axis diagonal 0,0,0 1,2,3\nrotate left 37 axis=diagonal",
        ),
    )
    .unwrap();
    std::fs::write(dir.join("pair.gasm"), "part a single.gasm\npart b block.gcad\nmove b 5,0,0\naxis pivot 0,0,0 0,0,1\njoint hinge a b turn about=pivot\n").unwrap();
    let text = "asset 1\nsource cad top.gasm\ntessellate cad tolerance=0.1\nuv cad projection=box tile=10\nnode first source=cad\nnode second source=cad translation=100,0,0 scale=-1.25,0.75,1.5\nmotion move first joint=left.hinge from=0 to=30 duration=1 rate=24 max_error=0.01\n";
    let path = dir.join("nested.gasset");
    std::fs::write(&path, text).unwrap();
    let asset = author::load_with_faces(&path, &[]).unwrap();
    let scene = &asset.scene;
    let index = |name: &str| scene.nodes.iter().position(|n| n.name == name).unwrap();
    let first = index("first");
    let second = index("second");
    let left = index("first/left/");
    let a = index("first/left/a/");
    assert_eq!(scene.nodes[left].parent, Some(first));
    assert_eq!(scene.nodes[a].parent, Some(left));
    assert_eq!(scene.nodes[index("first/left.a")].parent, Some(a));
    assert_eq!(
        scene.nodes[index("first/selected")].parent,
        Some(index("first/selected/a/"))
    );
    let run = gcad::model::run_path(&dir.join("top.gasm"), &[], None).unwrap();
    assert!(run.steps.iter().all(|(_, r)| r.is_ok()));
    let rest_world = scene.world_transforms().unwrap();
    let mut posed = run.model.clone();
    for sample in 0..=48 {
        let time = sample as f64 / 48.;
        posed
            .apply(&gcad::parse::parse_line(1, &format!("pose left.hinge {}", time * 30.)).unwrap())
            .unwrap();
        let mut animated = scene.clone();
        for (node, pose) in animated
            .nodes
            .iter_mut()
            .zip(scene.clips[0].sample(scene, time, false).unwrap())
        {
            node.transform = pose;
        }
        let world = animated.world_transforms().unwrap();
        for (body, _) in run.model.named_solids() {
            let one = index(&format!("first/{body}"));
            let two = index(&format!("second/{body}"));
            assert_eq!(scene.nodes[one].mesh, scene.nodes[two].mesh);
            assert!(asset.face_map(one).unwrap().face_count() > 0);
            assert!(asset.face_map(two).unwrap().face_count() > 0);
            for (node, owner, model) in [(one, first, &posed), (two, second, &run.model)] {
                let expected = rest_world[owner]
                    * model
                        .placements
                        .get(&body)
                        .copied()
                        .unwrap_or_else(Matrix4::identity);
                let mesh = &scene.meshes[scene.nodes[node].mesh.unwrap()];
                for primitive in &mesh.primitives {
                    for p in &primitive.geometry.positions {
                        let p = Point3::new(p[0], p[1], p[2]);
                        let error = (world[node].transform_point(p) - expected.transform_point(p))
                            .magnitude();
                        assert!(
                            error < if sample % 2 == 0 { 1e-8 } else { 0.01 },
                            "{body}: {error}"
                        );
                    }
                }
            }
        }
    }
    let bytes = asset.encode_glb().unwrap();
    let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let doc: serde_json::Value = serde_json::from_slice(&bytes[20..20 + length]).unwrap();
    let mut imported = scene.clone();
    for (i, node) in imported.nodes.iter_mut().enumerate() {
        let value = &doc["nodes"][i];
        assert_eq!(value["name"].as_str().unwrap(), scene.nodes[i].name);
        let values = |key: &str| {
            value[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect::<Vec<_>>()
        };
        let t = values("translation");
        let r = values("rotation");
        let s = values("scale");
        node.transform = Transform {
            translation: [t[0], t[1], t[2]],
            rotation: [r[0], r[1], r[2], r[3]],
            scale: [s[0], s[1], s[2]],
        };
        node.parent = None;
    }
    for (parent, value) in doc["nodes"].as_array().unwrap().iter().enumerate() {
        if let Some(children) = value["children"].as_array() {
            for child in children {
                imported.nodes[child.as_u64().unwrap() as usize].parent = Some(parent);
            }
        }
    }
    let world = imported.world_transforms().unwrap();
    for (i, node) in scene.nodes.iter().enumerate() {
        assert_eq!(imported.nodes[i].parent, node.parent);
        for p in [Point3::new(0., 0., 0.), Point3::new(2., 3., 4.)] {
            let expected = rest_world[i].transform_point(p);
            let actual =
                world[i].transform_point(Point3::new(p.x / 1000., p.z / 1000., -p.y / 1000.));
            assert!(
                (actual - Point3::new(expected.x / 1000., expected.z / 1000., -expected.y / 1000.))
                    .magnitude()
                    < 1e-8
            );
        }
    }
    std::fs::write(
        &path,
        text.replace("source cad top.gasm", "source cad top.gasm body=left.a")
            .lines()
            .filter(|l| !l.starts_with("motion "))
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    let selected = author::load(&path, &[]).unwrap();
    assert_eq!(
        selected
            .scene
            .nodes
            .iter()
            .filter(|n| n.mesh.is_some())
            .count(),
        2
    );
    assert!(
        selected
            .scene
            .nodes
            .iter()
            .any(|n| n.name == "first/left/a/")
    );
    assert!(
        !selected
            .scene
            .nodes
            .iter()
            .any(|n| n.name == "first/right/")
    );
}
