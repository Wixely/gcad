mod common;

use common::scratch;
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;

static ONE_STORE: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn store() -> PathBuf {
    let store = PathBuf::from(scratch("stored_parts")).join("store");
    let _ = std::fs::remove_dir_all(&store);
    gcad::model::store_parts_in(Some(store.clone()));
    store
}

#[cfg(unix)]
fn stored(store: &Path) -> Vec<(PathBuf, u64)> {
    use std::os::unix::fs::MetadataExt;
    let mut found: Vec<_> = std::fs::read_dir(store)
        .map(|listing| {
            listing
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    Some((entry.path(), entry.metadata().ok()?.ino()))
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

#[cfg(unix)]
fn write(dir: &Path, name: &str, text: &str) {
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(dir.join(name), text).expect("writes");
}

#[cfg(unix)]
#[test]
fn stored_parts_follow_their_files() {
    let _one = ONE_STORE.lock().unwrap_or_else(|e| e.into_inner());
    let store = store();
    let dir = PathBuf::from(scratch("stored_parts")).join("model");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    write(&dir, "top.gasm", "part a block.gcad\n");
    write(
        &dir,
        "block.gcad",
        "rect 10 10\nextrude 10\nmaterial custom\n",
    );
    let built = || {
        let run = gcad::model::run_path(&dir.join("top.gasm"), &[], None).expect("reads");
        let error = run
            .steps
            .iter()
            .find_map(|(_, r)| r.as_ref().err().map(|e| format!("{e:#}")));
        assert!(error.is_none(), "{error:?}");
        let solid = run.model.named_body("a").expect("part a");
        let density = run.model.materials.get("a").map(|m| m.density);
        (gcad::geometry::volume(&solid), density)
    };
    write(
        &dir,
        "materials.toml",
        "[custom]\ndensity = 2\nE = 1\nnu = 0.3\n",
    );
    let first = built();
    assert!((first.0 - 1000.0).abs() < 1.0, "{first:?}");
    let kept = stored(&store);
    assert_eq!(kept.len(), 1);

    assert_eq!(built(), first);
    let reused = stored(&store);
    assert_eq!(reused.len(), 1);
    assert_eq!(reused, kept, "the second build reads the stored part");

    write(
        &dir,
        "block.gcad",
        "rect 10 10\nextrude 20\nmaterial custom\n",
    );
    let taller = built();
    assert!((taller.0 - 2000.0).abs() < 1.0, "{taller:?}");

    write(
        &dir,
        "materials.toml",
        "[custom]\ndensity = 5\nE = 1\nnu = 0.3\n",
    );
    assert_eq!(built().1, Some(5.0));

    std::fs::create_dir_all(dir.join("sub")).expect("dir");
    write(
        &dir,
        "sub/block.gcad",
        "rect 10 10\nextrude 20\nmaterial custom\n",
    );
    write(&dir, "top.gasm", "part a sub/block.gcad\n");
    assert_eq!(built().1, Some(5.0));
    write(
        &dir,
        "sub/materials.toml",
        "[custom]\ndensity = 7\nE = 1\nnu = 0.3\n",
    );
    assert_eq!(
        built().1,
        Some(7.0),
        "a library that appears beside the part is read"
    );

    for (file, _) in stored(&store) {
        std::fs::write(file, b"not a stored part").expect("writes");
    }
    assert_eq!(built().1, Some(7.0));
}

#[cfg(unix)]
#[test]
fn stored_sub_assemblies_follow_their_parts() {
    let _one = ONE_STORE.lock().unwrap_or_else(|e| e.into_inner());
    let store = PathBuf::from(scratch("stored_subs")).join("store");
    let _ = std::fs::remove_dir_all(&store);
    gcad::model::store_parts_in(Some(store.clone()));
    let dir = PathBuf::from(scratch("stored_subs")).join("model");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    write(&dir, "top.gasm", "part s sub.gasm\n");
    write(
        &dir,
        "sub.gasm",
        "part a block.gcad\npart b block.gcad\nmove b 0,0,10\naxis up 0,0,0 0,0,1\njoint spin b a turn about=up\n",
    );
    write(&dir, "block.gcad", "rect 10 10\nextrude 10\n");
    let built = || {
        let run = gcad::model::run_path(&dir.join("top.gasm"), &[], None).expect("reads");
        let error = run
            .steps
            .iter()
            .find_map(|(_, r)| r.as_ref().err().map(|e| format!("{e:#}")));
        assert!(error.is_none(), "{error:?}");
        let volume = gcad::geometry::volume(&run.model.named_body("s.b").expect("s.b"));
        let joints: Vec<String> = run.model.joints.iter().map(|j| j.name.clone()).collect();
        (volume, joints)
    };
    let first = built();
    assert!((first.0 - 1000.0).abs() < 1.0, "{first:?}");
    assert_eq!(first.1, ["s.spin"]);
    let kept = stored(&store);
    assert_eq!(kept.len(), 2, "the part and the sub-assembly");
    assert_eq!(built(), first);
    assert_eq!(stored(&store), kept, "both come back from the store");
    write(&dir, "block.gcad", "rect 10 10\nextrude 30\n");
    let taller = built();
    assert!((taller.0 - 3000.0).abs() < 1.0, "{taller:?}");
}

#[test]
fn stored_meshes_come_back_the_same() {
    let _one = ONE_STORE.lock().unwrap_or_else(|e| e.into_inner());
    let store = store();
    let meshes = store.with_file_name("meshes");
    let _ = std::fs::remove_dir_all(&meshes);
    let lines = gcad::parse::parse_program("circle 20\ns: extrude 30\nplane s.end\nhole 6 0,0\n")
        .expect("parses");
    let solid = gcad::model::run(&lines).model.solid.expect("a solid");
    let (fresh, used) = gcad::sim::mesh::mesh_part(&solid, 4.0).expect("meshes");
    assert_eq!(std::fs::read_dir(&meshes).expect("stored").count(), 1);
    let (kept, again) = gcad::sim::mesh::mesh_part(&solid, 4.0).expect("meshes");
    assert_eq!(used, again);
    assert_eq!(fresh.nodes, kept.nodes);
    assert_eq!(fresh.tets, kept.tets);
    assert_eq!(fresh.boundary, kept.boundary);
    assert_eq!(fresh.uvs, kept.uvs);
    let (coarser, _) = gcad::sim::mesh::mesh_part(&solid, 6.0).expect("meshes");
    assert_ne!(coarser.nodes.len(), fresh.nodes.len());
    assert_eq!(std::fs::read_dir(&meshes).expect("stored").count(), 2);
}
