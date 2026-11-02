//! Native preview reloads run off the UI thread, including dependency polling.
use crate::{
    asset::author::{self, Asset},
    host,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

type Stamp = Option<(std::time::SystemTime, u64)>;
type State = BTreeMap<PathBuf, Stamp>;

fn stamp(path: &Path) -> Stamp {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

fn refresh(state: &State) -> State {
    state
        .keys()
        .map(|path| (path.clone(), stamp(path)))
        .collect()
}

pub(super) enum Event {
    Loading,
    Loaded(Asset, host::Reads),
    Failed(String),
}

pub(super) struct Reload {
    pub events: mpsc::Receiver<Event>,
    stop: mpsc::Sender<()>,
}

impl Drop for Reload {
    fn drop(&mut self) {
        let _ = self.stop.send(());
    }
}

impl Reload {
    pub fn start(
        path: PathBuf,
        vars: Vec<(String, f64)>,
        reads: host::Reads,
        ctx: egui::Context,
    ) -> Self {
        let (send, events) = mpsc::channel();
        let (stop, cancelled) = mpsc::channel();
        std::thread::spawn(move || {
            let mut state: State = reads.files().map(|p| (p.to_path_buf(), stamp(p))).collect();
            state.insert(path.clone(), stamp(&path));
            // Also catch edits between the initial load and opening the window.
            let mut dirty = reads.files().next().is_none() || !reads.unchanged();
            loop {
                match cancelled.recv_timeout(Duration::from_millis(200)) {
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    _ => return,
                }
                let current = refresh(&state);
                if current != state {
                    state = current;
                    dirty = true;
                    // Require one quiet poll to coalesce a burst of saves.
                    if send.send(Event::Loading).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                    continue;
                }
                if !dirty {
                    continue;
                }
                if send.send(Event::Loading).is_err() {
                    return;
                }
                ctx.request_repaint();
                let (result, reads) = host::recording(|| author::load(&path, &vars));
                // Discover missing and newly referenced dependencies even on failure.
                for file in reads.files() {
                    state
                        .entry(file.to_path_buf())
                        .or_insert_with(|| stamp(file));
                }
                if !reads.unchanged() || refresh(&state) != state {
                    state = refresh(&state);
                    continue;
                }
                dirty = false;
                let event = match result {
                    Ok(asset) => {
                        // Successful loads retire dependencies no longer consumed.
                        // Keep the validated stamps: restatting here could swallow
                        // an edit made after the consistency check above.
                        state.retain(|path, _| reads.files().any(|p| p == path));
                        Event::Loaded(asset, reads)
                    }
                    Err(error) => Event::Failed(format!("Source invalid: {error:#}")),
                };
                if send.send(event).is_err() {
                    return;
                }
                ctx.request_repaint();
            }
        });
        Self { events, stop }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next_result(reload: &Reload) -> Event {
        let deadline = std::time::Instant::now() + Duration::from_secs(45);
        loop {
            let event = reload
                .events
                .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                .expect("reload worker did not deliver a result");
            if !matches!(event, Event::Loading) {
                if let Event::Failed(error) = &event {
                    eprintln!("{error}");
                }
                return event;
            }
        }
    }

    #[test]
    fn dependency_edits_failure_and_new_missing_file_recover() {
        let root = std::env::temp_dir().join(format!(
            "gcad-preview-reload-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let source = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/parts/plate.gcad"),
        )
        .unwrap();
        let part = root.join("part.gcad");
        std::fs::write(&part, &source).unwrap();
        let path = root.join("preview.gasset");
        let document = "asset 1\nsource plate part.gcad body=main\ntessellate plate tolerance=0.1\nuv plate projection=box tile=25\n";
        std::fs::write(&path, document).unwrap();
        let (initial, reads) = host::recording(|| author::load(&path, &[]));
        initial.unwrap();
        assert!(reads.files().any(|p| p == part));
        let reload = Reload::start(path.clone(), vec![], reads, egui::Context::default());

        // A dependency save alone must trigger a reload.
        std::fs::write(&part, source.replace("w=40", "w=45")).unwrap();
        assert!(matches!(next_result(&reload), Event::Loaded(_, _)));
        std::fs::write(&part, "invalid operation\n").unwrap();
        assert!(matches!(next_result(&reload), Event::Failed(_)));
        std::fs::write(&part, &source).unwrap();
        assert!(matches!(next_result(&reload), Event::Loaded(_, _)));

        // Discover a missing dependency on a failed load, then recover solely
        // from its creation, without touching the asset document again.
        std::fs::write(&path, document.replace("part.gcad", "new.gcad")).unwrap();
        assert!(matches!(next_result(&reload), Event::Failed(_)));
        std::fs::write(root.join("new.gcad"), &source).unwrap();
        match next_result(&reload) {
            Event::Loaded(_, reads) => {
                assert!(reads.files().any(|p| p == root.join("new.gcad")));
                assert!(!reads.files().any(|p| p == part));
            }
            _ => panic!("new dependency did not recover"),
        }
        drop(reload);
        // Exact files only; do not recursively remove a computed directory.
        for file in [&path, &part, &root.join("new.gcad")] {
            std::fs::remove_file(file).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }
}
