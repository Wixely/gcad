//! Dependency observations for the browser's virtual filesystem. Kept independent
//! of web APIs so the recording contract can also be tested on native hosts.
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

type Observation = (Option<[u8; 32]>, bool);

#[derive(Clone, Default)]
pub struct Reads(BTreeMap<PathBuf, Vec<Observation>>);

impl Reads {
    pub fn unchanged(&self) -> bool {
        // Preserve the browser's existing conservative cache policy. Recording
        // consumed bytes does not prove that the current virtual files match.
        false
    }

    pub fn files(&self) -> impl Iterator<Item = &Path> {
        self.0.keys().map(PathBuf::as_path)
    }

    pub fn contents(&self) -> Vec<(PathBuf, Option<[u8; 32]>, bool)> {
        self.0
            .iter()
            .flat_map(|(path, observations)| {
                observations
                    .iter()
                    .map(|(hash, file)| (path.clone(), *hash, *file))
            })
            .collect()
    }

    pub fn merge(&mut self, other: Reads) {
        for (path, observations) in other.0 {
            for observation in observations {
                self.insert(path.clone(), observation);
            }
        }
    }

    fn insert(&mut self, path: PathBuf, observation: Observation) {
        let observations = self.0.entry(path).or_default();
        // Retain conflicting reads/existence results for provenance validation;
        // never let the last read silently replace an earlier consumed version.
        if !observations.contains(&observation) {
            observations.push(observation);
        }
    }
}

thread_local! {
    static RECORDING: RefCell<Vec<Reads>> = const { RefCell::new(Vec::new()) };
}

pub fn note(path: &Path, file: bool, content: Option<&[u8]>) {
    RECORDING.with(|stack| {
        if let Some(top) = stack.borrow_mut().last_mut() {
            use sha2::Digest;
            let hash = content.map(|bytes| sha2::Sha256::digest(bytes).into());
            top.insert(path.to_path_buf(), (hash, file));
        }
    });
}

pub fn observe_reads(reads: Reads) {
    RECORDING.with(|stack| {
        if let Some(outer) = stack.borrow_mut().last_mut() {
            outer.merge(reads);
        }
    });
}

pub fn recording<R>(work: impl FnOnce() -> R) -> (R, Reads) {
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            RECORDING.with(|stack| {
                stack.borrow_mut().pop();
            });
        }
    }
    RECORDING.with(|stack| stack.borrow_mut().push(Reads::default()));
    let scope = Scope;
    let result = work();
    let reads = RECORDING.with(|stack| {
        let mut stack = stack.borrow_mut();
        let reads = stack.last().cloned().unwrap_or_default();
        let len = stack.len();
        if len > 1 {
            stack[len - 2].merge(reads.clone());
        }
        reads
    });
    drop(scope);
    (result, reads)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn captures_consumed_bytes_probes_and_nested_reads() {
        let (answer, reads) = recording(|| {
            note(Path::new("part.gcad"), true, Some(b"original"));
            let (_, nested) = recording(|| {
                note(Path::new("optional.gcad"), false, None);
                note(Path::new("part.gcad"), true, None);
            });
            assert_eq!(nested.files().count(), 2);
            observe_reads(nested);
            42
        });
        assert_eq!(answer, 42);
        assert!(!reads.unchanged());
        assert_eq!(reads.files().count(), 2);
        let rows = reads.contents();
        assert!(rows.contains(&(
            PathBuf::from("part.gcad"),
            Some(Sha256::digest(b"original").into()),
            true
        )));
        assert!(rows.contains(&(PathBuf::from("part.gcad"), None, true)));
        assert!(rows.contains(&(PathBuf::from("optional.gcad"), None, false)));
    }

    #[test]
    fn worker_merge_preserves_conflicts_and_deduplicates_identical_reads() {
        let (_, mut reads) = recording(|| note(Path::new("map.png"), true, Some(b"first")));
        let worker = std::thread::spawn(|| {
            recording(|| {
                note(Path::new("map.png"), true, Some(b"second"));
                note(Path::new("map.png"), false, None);
            })
            .1
        })
        .join()
        .unwrap();
        reads.merge(worker.clone());
        reads.merge(worker);
        assert_eq!(reads.files().count(), 1);
        assert_eq!(reads.contents().len(), 3);
    }

    #[test]
    fn caught_nested_panic_does_not_leak_recording_scope() {
        let (_, reads) = recording(|| {
            let failure = std::panic::catch_unwind(|| {
                recording(|| {
                    note(Path::new("failed.gcad"), true, Some(b"bad"));
                    panic!("evaluation failed");
                })
            });
            assert!(failure.is_err());
            note(Path::new("recovered.gcad"), true, Some(b"good"));
        });
        assert_eq!(
            reads.files().collect::<Vec<_>>(),
            vec![Path::new("recovered.gcad")]
        );
        RECORDING.with(|stack| assert!(stack.borrow().is_empty()));
    }
}
