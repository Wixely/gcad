//! Validate all consumer sidecars before writing them into the owned package stage.
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

pub(super) fn write(glb: &[u8], stage: &std::path::Path) -> Result<()> {
    let mut hash = Sha256::new();
    let mut file = std::fs::File::open(stage.join("model.fbx"))?;
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    let digest = format!("{:x}", hash.finalize());
    let receipts = [
        (
            "gcad-consumer-profile.json",
            super::consumer_receipt::create(glb, &digest)?,
        ),
        (
            "gcad-shaders.json",
            super::shader_receipt::create(glb, &digest)?,
        ),
        (
            "gcad-root-motion.json",
            super::motion_receipt::create(glb, &digest)?,
        ),
    ];
    for (name, bytes) in receipts {
        if let Some(bytes) = bytes {
            let mut file = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(stage.join(name))?;
            file.write_all(&bytes)?;
        }
    }
    Ok(())
}
