//! Publish a verified FBX/PNG directory as one unit. No existing destination is replaced.
use anyhow::{Context, Result, bail, ensure};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub struct PackageStage {
    path: PathBuf,
    destination: PathBuf,
    published: bool,
    lease: Option<fs::File>,
    lease_path: PathBuf,
}

impl PackageStage {
    /// Reserve a private sibling directory on the destination filesystem. The
    /// caller runs conversion and independent validation here before publishing.
    pub fn new(destination: &Path) -> Result<Self> {
        ensure!(
            destination.is_absolute(),
            "package destination must be absolute"
        );
        let name = destination
            .file_name()
            .context("package destination needs a name")?;
        let parent = fs::canonicalize(destination.parent().context("package parent missing")?)?;
        ensure!(parent.is_dir(), "package parent is not a directory");
        let destination = parent.join(name);
        ensure!(
            !destination.try_exists()? && fs::symlink_metadata(&destination).is_err(),
            "package destination already exists"
        );
        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        for _ in 0..128 {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(".gcad-fbx-{}-{epoch}-{id}", std::process::id()));
            let lease_path = path.with_extension("lease");
            let mut lease = match fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&lease_path)
            {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e).context("reserve FBX stage lease"),
            };
            lease.write_all(b"gcad-fbx-stage-v1\n")?;
            lease.lock_shared()?;
            // The directory appears only after its shared lease is held.
            match fs::create_dir(&path) {
                Ok(()) => {
                    return Ok(Self {
                        path,
                        destination,
                        published: false,
                        lease: Some(lease),
                        lease_path,
                    });
                }
                Err(e) => {
                    drop(lease);
                    let _ = fs::remove_file(&lease_path);
                    if e.kind() != std::io::ErrorKind::AlreadyExists {
                        return Err(e).context("create FBX package stage");
                    }
                }
            }
        }
        bail!("cannot reserve FBX package stage")
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Validation is mandatory and runs before the atomic rename. On failure,
    /// cancellation (reported by the validator), or unwind, Drop removes staging.
    /// This is atomic visibility, not a power-loss durability guarantee.
    pub fn publish(mut self, validate: impl FnOnce(&Path) -> Result<()>) -> Result<PathBuf> {
        validate(&self.path)?;
        rename_new(&self.path, &self.destination)
            .context("publish FBX package without replacement")?;
        self.published = true;
        Ok(self.destination.clone())
    }
}

impl Drop for PackageStage {
    fn drop(&mut self) {
        let cleaned = if !self.published {
            // Only the exact sibling directory successfully created by new().
            // remove_dir_all does not traverse directory symlinks.
            fs::remove_dir_all(&self.path).is_ok()
        } else {
            true
        };
        drop(self.lease.take());
        if cleaned {
            let _ = fs::remove_file(&self.lease_path);
        }
    }
}

/// Remove abandoned, version-marked stages only. Live gcad/helper leases are
/// skipped. Published packages and older unmarked stages are never selected.
pub fn recover(parent: &Path) -> Result<Vec<PathBuf>> {
    let parent = fs::canonicalize(parent)?;
    let mut removed = Vec::new();
    for entry in fs::read_dir(&parent)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(ids) = name.strip_prefix(".gcad-fbx-") else {
            continue;
        };
        let parts: Vec<_> = ids.split('-').collect();
        if parts.len() != 3
            || parts
                .iter()
                .any(|s| s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()))
        {
            continue;
        }
        let stage = entry.path();
        let lease_path = stage.with_extension("lease");
        let Ok(meta) = fs::symlink_metadata(&lease_path) else {
            continue;
        };
        if !meta.file_type().is_file() {
            continue;
        }
        let mut lease = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lease_path)?;
        match lease.try_lock() {
            Ok(()) => (),
            Err(std::fs::TryLockError::WouldBlock) => continue,
            Err(error) => return Err(error).context("check FBX stage lease"),
        }
        let mut marker = Vec::new();
        (&mut lease).take(64).read_to_end(&mut marker)?;
        if marker != b"gcad-fbx-stage-v1\n" {
            continue;
        }
        ensure!(
            fs::symlink_metadata(&stage)?.file_type().is_dir(),
            "stage changed during recovery"
        );
        fs::remove_dir_all(&stage).context("remove abandoned FBX stage")?;
        drop(lease);
        fs::remove_file(&lease_path).context("remove abandoned FBX lease")?;
        removed.push(stage);
    }
    Ok(removed)
}

#[cfg(windows)]
fn rename_new(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(source: *const u16, destination: *const u16, flags: u32) -> i32;
    }
    let wide = |path: &Path| -> std::io::Result<Vec<u16>> {
        let mut result: Vec<_> = path.as_os_str().encode_wide().collect();
        if result.contains(&0) {
            return Err(std::io::ErrorKind::InvalidInput.into());
        }
        result.push(0);
        Ok(result)
    };
    let source = wide(source)?;
    let destination = wide(destination)?;
    // No REPLACE_EXISTING and no COPY_ALLOWED: destination must not exist and
    // both paths must be on the same volume. Buffers are NUL-terminated and live.
    if unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), 0) } == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn rename_new(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    unsafe extern "C" {
        fn renameat2(
            old_dir: i32,
            old: *const std::ffi::c_char,
            new_dir: i32,
            new: *const std::ffi::c_char,
            flags: u32,
        ) -> i32;
    }
    let source = CString::new(source.as_os_str().as_bytes())?;
    let destination = CString::new(destination.as_os_str().as_bytes())?;
    // AT_FDCWD, RENAME_NOREPLACE. Unsupported filesystems fail closed; there is
    // no check-then-rename fallback that could replace a concurrent destination.
    if unsafe { renameat2(-100, source.as_ptr(), -100, destination.as_ptr(), 1) } != 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
fn rename_new(_: &Path, _: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic package publication unsupported on this platform",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn workspace() -> PackageStage {
        let name = format!(
            "gcad-package-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        PackageStage::new(&std::env::temp_dir().join(name)).unwrap()
    }
    fn contents(stage: &PackageStage) {
        fs::write(stage.path().join("model.fbx"), b"fixture FBX").unwrap();
        fs::create_dir(stage.path().join("textures")).unwrap();
        fs::write(stage.path().join("textures/image_0.png"), b"fixture PNG").unwrap();
    }
    #[test]
    fn recovery_skips_live_stages_and_removes_only_marked_abandoned_stages() {
        let workspace = workspace();
        let active = PackageStage::new(&workspace.path().join("active")).unwrap();
        contents(&active);
        assert!(recover(workspace.path()).unwrap().is_empty());
        let mut abandoned = PackageStage::new(&workspace.path().join("abandoned")).unwrap();
        contents(&abandoned);
        let abandoned_path = abandoned.path.clone();
        let lease_path = abandoned.lease_path.clone();
        // Simulate the process releasing its handles without Rust destructors.
        drop(abandoned.lease.take());
        std::mem::forget(abandoned);
        let unmarked = workspace.path().join(".gcad-fbx-1-2-3");
        fs::create_dir(&unmarked).unwrap();
        fs::write(unmarked.join("keep"), b"unrecognized").unwrap();
        assert_eq!(
            recover(workspace.path()).unwrap(),
            vec![abandoned_path.clone()]
        );
        assert!(!abandoned_path.exists() && !lease_path.exists());
        assert!(active.path().join("model.fbx").exists());
        assert!(unmarked.join("keep").exists());
    }
    #[test]
    fn publishes_complete_directory_only_after_validation() {
        let workspace = workspace();
        let destination = workspace.path().join("package");
        let stage = PackageStage::new(&destination).unwrap();
        let staging = stage.path().to_owned();
        contents(&stage);
        assert!(!destination.exists());
        let result = stage
            .publish(|path| {
                assert!(!destination.exists());
                assert_eq!(fs::read(path.join("textures/image_0.png"))?, b"fixture PNG");
                Ok(())
            })
            .unwrap();
        assert_eq!(result, destination);
        assert!(!staging.exists());
        assert_eq!(fs::read(result.join("model.fbx")).unwrap(), b"fixture FBX");
    }
    #[test]
    fn validation_failure_cleans_stage_without_publishing() {
        let workspace = workspace();
        let destination = workspace.path().join("package");
        let stage = PackageStage::new(&destination).unwrap();
        let staging = stage.path().to_owned();
        contents(&stage);
        assert!(
            stage
                .publish(|_| bail!("cancelled or invalid package"))
                .is_err()
        );
        assert!(!staging.exists() && !destination.exists());
    }
    #[test]
    fn concurrent_destination_is_never_replaced_even_when_empty() {
        for nonempty in [false, true] {
            let workspace = workspace();
            let destination = workspace.path().join("package");
            let stage = PackageStage::new(&destination).unwrap();
            let staging = stage.path().to_owned();
            contents(&stage);
            assert!(
                stage
                    .publish(|_| {
                        fs::create_dir(&destination)?;
                        if nonempty {
                            fs::write(destination.join("keep"), b"existing")?;
                        }
                        Ok(())
                    })
                    .is_err()
            );
            assert!(!staging.exists());
            assert!(destination.is_dir());
            assert!(!destination.join("model.fbx").exists());
            if nonempty {
                assert_eq!(fs::read(destination.join("keep")).unwrap(), b"existing");
            }
            assert!(PackageStage::new(&destination).is_err());
        }
    }
}
