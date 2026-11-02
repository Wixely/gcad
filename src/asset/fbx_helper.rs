//! Versioned optional native helper boundary. Never resolves helpers through PATH.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub const ASSIMP_REVISION: &str = "a4643cbd30d3088bbff312d3cc2652f514fb97c0";
pub const ASSIMP_DIFF_SHA256: &str =
    "783bf12bee4103422f8e8d2f74b2f17ae4dd93109b25266cd67baa9838340bf9";
const OUTPUT_LIMIT: usize = 64 * 1024;
const INPUT_LIMIT: u64 = 64 * 1024 * 1024;

fn snapshot_input(
    input: &Path,
    directory: &Path,
    cancelled: &AtomicBool,
) -> Result<(std::path::PathBuf, Vec<u8>)> {
    use sha2::{Digest, Sha256};
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1); // FILE_SHARE_READ: exclude writers while capturing.
    }
    let mut source = options.open(input).context("open GLB for snapshot")?;
    let before = source.metadata()?;
    ensure!(
        before.is_file() && before.len() <= INPUT_LIMIT,
        "input GLB exceeds snapshot limit or is not a file"
    );
    let mut bytes = Vec::new();
    (&mut source)
        .take(INPUT_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    let after = source.metadata()?;
    ensure!(
        bytes.len() as u64 == before.len()
            && after.len() == before.len()
            && after.modified()? == before.modified()?,
        "input GLB changed during snapshot"
    );
    ensure!(
        !cancelled.load(Ordering::Relaxed),
        "helper request cancelled"
    );
    let path = directory.join("input.glb");
    std::fs::write(&path, &bytes).context("write private GLB snapshot")?;
    Ok((path, Sha256::digest(&bytes).to_vec()))
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Handshake {
    pub protocol: u32,
    pub helper: String,
    pub assimp_revision: String,
    pub assimp_diff_sha256: String,
    pub assimp_version: [u32; 3],
    pub material_schema: u32,
    pub conversion_ready: bool,
    pub capabilities: Vec<String>,
}
impl Handshake {
    fn parse(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() <= OUTPUT_LIMIT, "helper response exceeds limit");
        let response: Self =
            serde_json::from_slice(bytes).context("invalid helper handshake JSON")?;
        ensure!(
            response.protocol == 1 && response.helper == "gcad-fbx-helper",
            "unsupported helper protocol or identity"
        );
        ensure!(
            response.assimp_revision == ASSIMP_REVISION,
            "unreviewed Assimp revision"
        );
        ensure!(
            response.assimp_diff_sha256 == ASSIMP_DIFF_SHA256,
            "unreviewed Assimp patch identity"
        );
        ensure!(response.material_schema == 3, "unsupported material schema");
        Ok(response)
    }
}

struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn limited(mut stream: impl Read) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    (&mut stream)
        .take((OUTPUT_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= OUTPUT_LIMIT,
        "helper output exceeds 64 KiB per stream"
    );
    Ok(bytes)
}

/// Inspect an explicitly selected executable. Both streams are drained with
/// bounded buffers; cancellation, timeout and failure reap the direct child.
/// This is a process contract, not a sandbox for untrusted executables.
pub fn inspect(path: &Path, timeout: Duration, cancelled: &AtomicBool) -> Result<Handshake> {
    Handshake::parse(&run(
        path,
        &[std::ffi::OsStr::new("--handshake")],
        timeout,
        cancelled,
    )?)
}

fn run(
    path: &Path,
    args: &[&std::ffi::OsStr],
    timeout: Duration,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    ensure!(
        path.is_absolute(),
        "helper path must be absolute; PATH lookup is disabled"
    );
    ensure!(timeout > Duration::ZERO, "helper timeout must be positive");
    ensure!(
        !cancelled.load(Ordering::Relaxed),
        "helper request cancelled"
    );
    let path = std::fs::canonicalize(path).context("resolve configured helper")?;
    ensure!(path.is_file(), "helper path is not a file");
    let mut command = Command::new(path);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = Running(command.spawn().context("start configured FBX helper")?);
    let out = child.0.stdout.take().unwrap();
    let err = child.0.stderr.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    let errors = sender.clone();
    std::thread::spawn(move || {
        let _ = sender.send((true, limited(out)));
    });
    std::thread::spawn(move || {
        let _ = errors.send((false, limited(err)));
    });
    let started = Instant::now();
    let mut stdout = None;
    let mut stderr = None;
    loop {
        ensure!(
            !cancelled.load(Ordering::Relaxed),
            "helper request cancelled"
        );
        ensure!(started.elapsed() < timeout, "helper request timed out");
        while let Ok((output, bytes)) = receiver.try_recv() {
            let bytes = bytes?;
            if output {
                stdout = Some(bytes);
            } else {
                stderr = Some(bytes);
            }
        }
        if let Some(status) = child.0.try_wait()? {
            ensure!(status.success(), "FBX helper failed with status {status}");
            if let (Some(stdout), Some(_)) = (&stdout, &stderr) {
                return Ok(stdout.clone());
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Explicit experimental GLB-to-package conversion, not production engine support.
/// The timeout applies separately to handshake and conversion child execution.
pub fn convert_experimental(
    helper: &Path,
    input: &Path,
    destination: &Path,
    timeout: Duration,
    cancelled: &AtomicBool,
) -> Result<std::path::PathBuf> {
    let handshake = inspect(helper, timeout, cancelled)?;
    ensure!(
        handshake
            .capabilities
            .iter()
            .any(|c| c == "staged-conversion-v1-experimental"),
        "helper lacks experimental staged conversion"
    );
    let input = std::fs::canonicalize(input).context("resolve input GLB")?;
    ensure!(
        handshake
            .capabilities
            .iter()
            .any(|c| c == "leased-staging-v1"),
        "helper lacks stage leases; rebuild the experimental helper"
    );
    ensure!(input.is_file(), "input GLB is not a file");
    // A separate owned sibling keeps private input out of the published package.
    let snapshot = super::fbx_package::PackageStage::new(destination)?;
    let (input, digest) = snapshot_input(&input, snapshot.path(), cancelled)?;
    let stage = super::fbx_package::PackageStage::new(destination)?;
    let response = run(
        helper,
        &[
            std::ffi::OsStr::new("--convert-leased-v1"),
            input.as_os_str(),
            stage.path().as_os_str(),
        ],
        timeout,
        cancelled,
    )?;
    let manifest: super::fbx_response::Manifest =
        serde_json::from_slice(&response).context("invalid helper conversion response")?;
    stage.publish(|path| {
        use sha2::{Digest, Sha256};
        ensure!(
            !cancelled.load(Ordering::Relaxed),
            "helper request cancelled"
        );
        manifest.validate(path)?;
        let mut captured = Vec::new();
        std::fs::File::open(&input)?
            .take(INPUT_LIMIT + 1)
            .read_to_end(&mut captured)?;
        ensure!(
            captured.len() as u64 <= INPUT_LIMIT && Sha256::digest(&captured).as_slice() == digest,
            "private input snapshot changed during conversion"
        );
        // Only gcad writes receipts, after the helper's complete file-set audit.
        // Bind shader/root-motion settings to the exact published FBX.
        super::fbx_receipts::write(&captured,path)?;
        ensure!(
            !cancelled.load(Ordering::Relaxed),
            "helper request cancelled"
        );
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_is_independent_of_later_source_edits() {
        use sha2::{Digest, Sha256};
        let destination = std::env::temp_dir().join(format!(
            "gcad-snapshot-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let workspace = crate::asset::fbx_package::PackageStage::new(&destination).unwrap();
        let source = workspace.path().join("original.glb");
        std::fs::write(&source, b"captured bytes").unwrap();
        let (path, digest) =
            snapshot_input(&source, workspace.path(), &AtomicBool::new(false)).unwrap();
        std::fs::write(&source, b"later edits").unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"captured bytes");
        assert_eq!(digest, Sha256::digest(b"captured bytes").to_vec());
    }
    fn message() -> serde_json::Value {
        serde_json::json!({"protocol":1,"helper":"gcad-fbx-helper","assimp_revision":ASSIMP_REVISION,
            "assimp_diff_sha256":ASSIMP_DIFF_SHA256,"assimp_version":[6,0,0],"material_schema":3,
            "conversion_ready":false,"capabilities":[]})
    }
    #[test]
    fn handshake_rejects_version_and_dependency_drift_and_extra_output() {
        let good = message();
        assert!(
            !Handshake::parse(&serde_json::to_vec(&good).unwrap())
                .unwrap()
                .conversion_ready
        );
        for (key, bad) in [
            ("protocol", serde_json::json!(2)),
            ("assimp_revision", serde_json::json!("other")),
            ("assimp_diff_sha256", serde_json::json!("other")),
            ("material_schema", serde_json::json!(4)),
            ("helper", serde_json::json!("other")),
        ] {
            let mut value = good.clone();
            value[key] = bad;
            assert!(Handshake::parse(&serde_json::to_vec(&value).unwrap()).is_err());
        }
        let mut bytes = serde_json::to_vec(&good).unwrap();
        bytes.extend_from_slice(b"\nextra output");
        assert!(Handshake::parse(&bytes).is_err());
        assert!(limited(std::io::Cursor::new(vec![0; OUTPUT_LIMIT + 1])).is_err());
        assert_eq!(
            limited(std::io::Cursor::new(vec![0; OUTPUT_LIMIT]))
                .unwrap()
                .len(),
            OUTPUT_LIMIT
        );
    }
    #[test]
    fn helper_requires_explicit_absolute_path_and_observes_pre_cancel() {
        assert!(
            inspect(
                Path::new("gcad_fbx_helper"),
                Duration::from_secs(1),
                &AtomicBool::new(false)
            )
            .is_err()
        );
        let absent = std::env::temp_dir().join("gcad-helper-absent");
        assert!(
            inspect(&absent, Duration::from_secs(1), &AtomicBool::new(true))
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
    }
}
