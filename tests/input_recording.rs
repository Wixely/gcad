#![cfg(not(target_arch = "wasm32"))]

use gcad::host;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn directory() -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("input-recording-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn consumed_bytes_and_missing_observations_survive_later_file_changes() {
    let root = directory();
    let source = root.join("source.bin");
    let optional = root.join("optional.bin");
    std::fs::write(&source, b"consumed").unwrap();
    let (_, reads) = host::recording(|| {
        assert_eq!(host::read(&source).unwrap(), b"consumed");
        assert!(host::is_file(&source));
        assert!(!host::is_file(&optional));
        assert!(host::read(&optional).is_err());
    });
    assert!(reads.unchanged());
    assert_eq!(reads.files().count(), 2);
    let consumed: [u8; 32] = Sha256::digest(b"consumed").into();
    let before = reads.contents();
    assert!(before.contains(&(source.clone(), Some(consumed), true)));
    assert!(before.contains(&(optional.clone(), None, false)));
    std::fs::write(&source, b"a new and longer version").unwrap();
    std::fs::write(&optional, b"now present").unwrap();
    assert!(!reads.unchanged());
    assert_eq!(reads.contents(), before);
    std::fs::remove_file(source).unwrap();
    std::fs::remove_file(optional).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn nested_and_worker_merges_keep_conflicts_and_deduplicate_identical_reads() {
    let root = directory();
    let source = root.join("source.bin");
    std::fs::write(&source, b"first").unwrap();
    let (_, mut reads) = host::recording(|| {
        host::read(&source).unwrap();
        std::fs::write(&source, b"other").unwrap();
        host::recording(|| host::read(&source).unwrap());
    });
    let first: [u8; 32] = Sha256::digest(b"first").into();
    let other: [u8; 32] = Sha256::digest(b"other").into();
    assert!(
        reads
            .contents()
            .contains(&(source.clone(), Some(first), true))
    );
    assert!(
        reads
            .contents()
            .contains(&(source.clone(), Some(other), true))
    );
    assert!(!reads.unchanged());
    let worker_path = source.clone();
    let worker = std::thread::spawn(move || {
        host::recording(|| {
            std::fs::remove_file(&worker_path).unwrap();
            assert!(!host::is_file(&worker_path));
        })
        .1
    })
    .join()
    .unwrap();
    reads.merge(worker.clone());
    let (_, outer) = host::recording(|| host::observe_reads(reads.clone()));
    assert_eq!(outer.contents(), reads.contents());
    assert!(!outer.unchanged());
    let once = reads.contents();
    reads.merge(worker);
    assert_eq!(reads.contents(), once);
    assert!(once.contains(&(source, None, false)));
    assert_eq!(reads.files().count(), 1);
    assert!(!reads.unchanged());
    std::fs::remove_dir(root).unwrap();
}
