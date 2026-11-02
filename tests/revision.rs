mod common;

use common::{scratch, volume};
use gcad::model::run_path;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .output()
        .expect("git")
        .status
        .success();
    assert!(ok, "git {args:?}");
}

#[test]
fn an_assembly_opens_as_committed_while_the_files_on_disk_have_moved_on() {
    let dir = std::path::PathBuf::from(scratch("revision"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let dir = dir.canonicalize().expect("canonical");
    git(&dir, &["init", "-q"]);
    let top = dir.join("top.gasm");
    std::fs::write(dir.join("block.gcad"), "rect 10 10\nextrude 10\n").expect("part");
    std::fs::write(&top, "part block block.gcad\n").expect("assembly");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-qm", "ordered"]);
    std::fs::write(dir.join("block.gcad"), "rect 20 20\nextrude 10\n").expect("part");

    let on_disk = volume(&run_path(&top, &[], None).expect("runs").model);
    assert!((on_disk - 4000.0).abs() < 1.0, "{on_disk}");

    let revs = egui_bench::revision::list(&dir, 5).expect("history");
    let snapshot = egui_bench::revision::Snapshot::open(&top, &revs[0].sha).expect("snapshot");
    egui_bench::revision::set_active(Some(Arc::new(snapshot)));
    let committed = volume(&run_path(&top, &[], None).expect("runs at revision").model);
    egui_bench::revision::set_active(None);

    assert!((committed - 1000.0).abs() < 1.0, "{committed}");
    assert_eq!(
        std::fs::read_to_string(dir.join("block.gcad")).expect("read"),
        "rect 20 20\nextrude 10\n"
    );
}
