use std::io;
use std::path::{Component, Path, PathBuf};

#[cfg(any(target_arch = "wasm32", test))]
mod virtual_reads;

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;
#[cfg(target_arch = "wasm32")]
pub use web_time::Instant;

pub fn tidy(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(not(target_arch = "wasm32"))]
mod files {
    use std::io;
    use std::path::{Path, PathBuf};

    type Stamp = Option<(std::time::SystemTime, u64)>;

    #[derive(Clone, Default, PartialEq)]
    struct Seen {
        stamp: Stamp,
        content: Option<[u8; 32]>,
        file: bool,
    }

    #[derive(Clone, Default)]
    pub struct Reads(std::collections::BTreeMap<PathBuf, Vec<Seen>>);

    impl Reads {
        pub fn unchanged(&self) -> bool {
            self.0.iter().all(|(file, observations)| {
                let current = stamp(file);
                let hash = observations.iter().find_map(|seen| seen.content);
                let first = &observations[0];
                observations.iter().all(|seen| {
                    current == seen.stamp
                        && seen.file == first.file
                        && seen.content.is_none_or(|content| Some(content) == hash)
                })
            })
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
                        .map(|seen| (path.clone(), seen.content, seen.file))
                })
                .collect()
        }

        pub fn merge(&mut self, other: Reads) {
            for (file, observations) in other.0 {
                for seen in observations {
                    self.insert(file.clone(), seen);
                }
            }
        }

        fn insert(&mut self, path: PathBuf, seen: Seen) {
            let observations = self.0.entry(path).or_default();
            if !observations.contains(&seen) {
                observations.push(seen);
            }
        }
    }

    thread_local! {
        static RECORDING: std::cell::RefCell<Vec<Reads>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    /// Include inputs consumed by cached evaluation or another worker.
    pub fn observe_reads(reads: Reads) {
        RECORDING.with(|stack| {
            if let Some(outer) = stack.borrow_mut().last_mut() {
                outer.merge(reads);
            }
        });
    }

    pub fn recording<R>(work: impl FnOnce() -> R) -> (R, Reads) {
        RECORDING.with(|stack| stack.borrow_mut().push(Reads::default()));
        let result = work();
        let reads = RECORDING.with(|stack| {
            let mut stack = stack.borrow_mut();
            let reads = stack.pop().unwrap_or_default();
            if let Some(outer) = stack.last_mut() {
                outer.merge(reads.clone());
            }
            reads
        });
        (result, reads)
    }

    fn stamp(path: &Path) -> Stamp {
        let meta = std::fs::metadata(path).ok()?;
        Some((meta.modified().ok()?, meta.len()))
    }

    fn note(path: &Path, was: Stamp, file: bool, content: Option<&[u8]>) {
        RECORDING.with(|stack| {
            if let Some(top) = stack.borrow_mut().last_mut() {
                use sha2::Digest;
                top.insert(
                    path.to_path_buf(),
                    Seen {
                        stamp: was,
                        content: content.map(|bytes| sha2::Sha256::digest(bytes).into()),
                        file,
                    },
                );
            }
        });
    }

    pub fn read(path: &Path) -> io::Result<Vec<u8>> {
        if let Some(snapshot) = egui_bench::revision::covering(path) {
            return snapshot.read(path);
        }
        let was = stamp(path);
        let bytes = std::fs::read(path);
        note(path, was, bytes.is_ok(), bytes.as_deref().ok());
        bytes
    }

    pub fn is_file(path: &Path) -> bool {
        if let Some(snapshot) = egui_bench::revision::covering(path) {
            return snapshot.is_file(path);
        }
        let was = stamp(path);
        let file = path.is_file();
        note(path, was, file, None);
        file
    }

    pub fn is_dir(path: &Path) -> bool {
        if let Some(snapshot) = egui_bench::revision::covering(path) {
            return snapshot.is_dir(path);
        }
        path.is_dir()
    }

    pub fn canonicalize(path: &Path) -> io::Result<PathBuf> {
        if let Some(snapshot) = egui_bench::revision::covering(path) {
            return snapshot.canonicalize(path);
        }
        std::fs::canonicalize(path)
    }

    fn frozen(path: &Path) -> io::Result<()> {
        if egui_bench::revision::covering(path).is_some() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "an earlier revision is read only",
            ));
        }
        Ok(())
    }

    pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
        frozen(path)?;
        std::fs::write(path, bytes)
    }

    pub fn save(path: &Path, text: &str) {
        if frozen(path).is_ok() {
            let _ = std::fs::write(path, text);
        }
    }

    pub fn edited(_path: &Path) -> bool {
        false
    }

    pub fn revert(_path: &Path) {}

    pub fn take_open() -> Option<PathBuf> {
        None
    }

    pub fn status() -> Option<String> {
        None
    }

    pub fn opened(_path: &Path) {}

    pub fn built(_seconds: f64) {}

    fn settings_path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("gcad").join("view.toml"))
    }

    fn settings() -> toml::Table {
        settings_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| t.parse().ok())
            .unwrap_or_default()
    }

    pub fn setting(name: &str) -> Option<String> {
        settings().get(name)?.as_str().map(str::to_string)
    }

    pub fn set_setting(name: &str, value: &str) {
        let Some(path) = settings_path() else { return };
        let mut table = settings();
        table.insert(name.to_string(), value.into());
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, table.to_string());
    }
}

#[cfg(target_arch = "wasm32")]
mod files {
    use std::collections::BTreeMap;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

    static FILES: Mutex<BTreeMap<PathBuf, Arc<[u8]>>> = Mutex::new(BTreeMap::new());

    fn hold<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
        loop {
            match lock.try_lock() {
                Ok(guard) => return guard,
                Err(TryLockError::Poisoned(poisoned)) => return poisoned.into_inner(),
                Err(TryLockError::WouldBlock) => std::hint::spin_loop(),
            }
        }
    }

    fn key(path: &Path) -> PathBuf {
        let path = super::tidy(path);
        path.strip_prefix("/")
            .map(Path::to_path_buf)
            .unwrap_or(path)
    }

    pub fn read(path: &Path) -> io::Result<Vec<u8>> {
        let path = key(path);
        let bytes = hold(&FILES).get(&path).cloned();
        super::virtual_reads::note(&path, bytes.is_some(), bytes.as_deref());
        bytes
            .map(|b| b.to_vec())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such file"))
    }

    pub fn is_file(path: &Path) -> bool {
        let path = key(path);
        let found = hold(&FILES).contains_key(&path);
        super::virtual_reads::note(&path, found, None);
        found
    }

    pub use super::virtual_reads::{Reads, observe_reads, recording};

    pub fn is_dir(path: &Path) -> bool {
        let dir = key(path);
        hold(&FILES)
            .keys()
            .any(|k| k != &dir && k.starts_with(&dir))
    }

    pub fn canonicalize(path: &Path) -> io::Result<PathBuf> {
        Ok(key(path))
    }

    pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
        hold(&FILES).insert(key(path), bytes.into());
        Ok(())
    }

    pub fn list() -> Vec<PathBuf> {
        hold(&FILES).keys().cloned().collect()
    }

    static BUNDLED: Mutex<BTreeMap<PathBuf, Arc<[u8]>>> = Mutex::new(BTreeMap::new());

    const PREFIX: &str = "gcad:";

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub fn bundle(path: &Path, bytes: &[u8]) {
        let path = key(path);
        hold(&BUNDLED).insert(path.clone(), bytes.into());
        let saved = storage().and_then(|s| {
            s.get_item(&format!("{PREFIX}{}", path.display()))
                .ok()
                .flatten()
        });
        let current: Arc<[u8]> = match saved {
            Some(text) => text.as_bytes().into(),
            None => bytes.into(),
        };
        hold(&FILES).insert(path, current);
    }

    static INBOX: Mutex<Option<PathBuf>> = Mutex::new(None);
    static STATUS: Mutex<Option<String>> = Mutex::new(None);
    static OPENED: Mutex<Option<fn(&Path)>> = Mutex::new(None);
    static BUILT: Mutex<Option<fn(f64)>> = Mutex::new(None);

    pub fn on_built(hook: fn(f64)) {
        *hold(&BUILT) = Some(hook);
    }

    pub fn built(seconds: f64) {
        let hook = *hold(&BUILT);
        if let Some(hook) = hook {
            hook(seconds);
        }
    }

    pub fn ask_open(path: &Path) {
        *hold(&INBOX) = Some(key(path));
    }

    pub fn take_open() -> Option<PathBuf> {
        hold(&INBOX).take()
    }

    pub fn set_status(status: Option<String>) {
        *hold(&STATUS) = status;
    }

    pub fn status() -> Option<String> {
        hold(&STATUS).clone()
    }

    pub fn on_open(hook: fn(&Path)) {
        *hold(&OPENED) = Some(hook);
    }

    pub fn opened(path: &Path) {
        let hook = *hold(&OPENED);
        if let Some(hook) = hook {
            hook(&key(path));
        }
    }

    pub fn edited(path: &Path) -> bool {
        let path = key(path);
        let bundled = hold(&BUNDLED).get(&path).cloned();
        let current = hold(&FILES).get(&path).cloned();
        bundled != current
    }

    pub fn save(path: &Path, text: &str) {
        let _ = write(path, text.as_bytes());
        if let Some(storage) = storage() {
            let _ = storage.set_item(&format!("{PREFIX}{}", key(path).display()), text);
        }
    }

    pub fn revert(path: &Path) {
        let path = key(path);
        if let Some(storage) = storage() {
            let _ = storage.remove_item(&format!("{PREFIX}{}", path.display()));
        }
        match hold(&BUNDLED).get(&path).cloned() {
            Some(bytes) => hold(&FILES).insert(path, bytes),
            None => hold(&FILES).remove(&path),
        };
    }

    const SETTING: &str = "gcad-setting:";

    pub fn setting(name: &str) -> Option<String> {
        storage()?
            .get_item(&format!("{SETTING}{name}"))
            .ok()
            .flatten()
    }

    pub fn set_setting(name: &str, value: &str) {
        if let Some(storage) = storage() {
            let _ = storage.set_item(&format!("{SETTING}{name}"), value);
        }
    }

    pub fn restore() {
        let Some(storage) = storage() else {
            return;
        };
        let count = storage.length().unwrap_or(0);
        for index in 0..count {
            let Some(name) = storage.key(index).ok().flatten() else {
                continue;
            };
            let Some(path) = name.strip_prefix(PREFIX) else {
                continue;
            };
            if let Some(text) = storage.get_item(&name).ok().flatten() {
                let _ = write(Path::new(path), text.as_bytes());
            }
        }
    }
}

pub use files::*;

pub fn read_to_string(path: &Path) -> io::Result<String> {
    String::from_utf8(read(path)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn exists(path: &Path) -> bool {
    is_file(path) || is_dir(path)
}

static LATEST_RUN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

thread_local! {
    static RUN: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

pub fn superseding<R>(work: impl FnOnce() -> R) -> R {
    let run = LATEST_RUN.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    let outer = RUN.with(|cell| cell.replace(run));
    let result = work();
    RUN.with(|cell| cell.set(outer));
    result
}

pub fn current_run() -> u64 {
    RUN.with(std::cell::Cell::get)
}

pub fn run_superseded(run: u64) -> bool {
    run != 0 && run != LATEST_RUN.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn superseded() -> bool {
    run_superseded(current_run())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn each<T: Send, R: Send>(items: Vec<T>, f: impl Fn(T) -> R + Sync) -> Vec<Option<R>> {
    let run = RUN.with(std::cell::Cell::get);
    std::thread::scope(|scope| {
        let f = &f;
        let jobs: Vec<_> = items
            .into_iter()
            .map(|item| {
                scope.spawn(move || {
                    RUN.with(|cell| cell.set(run));
                    f(item)
                })
            })
            .collect();
        jobs.into_iter().map(|job| job.join().ok()).collect()
    })
}

#[cfg(target_arch = "wasm32")]
pub fn each<T: Send, R: Send>(items: Vec<T>, f: impl Fn(T) -> R + Sync) -> Vec<Option<R>> {
    use rayon::prelude::*;
    items.into_par_iter().map(|item| Some(f(item))).collect()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn(job: impl FnOnce() + Send + 'static) {
    std::thread::spawn(job);
}

#[cfg(target_arch = "wasm32")]
pub fn spawn(job: impl FnOnce() + Send + 'static) {
    rayon::spawn(job);
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub fn release_memory() {
    unsafe extern "C" {
        fn malloc_trim(pad: usize) -> i32;
    }
    unsafe {
        malloc_trim(0);
    }
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
pub fn release_memory() {}

pub fn wake(ctx: &egui::Context) {
    if cfg!(not(target_arch = "wasm32")) {
        ctx.request_repaint();
    }
}
