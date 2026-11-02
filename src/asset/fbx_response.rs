//! Check the helper's bounded manifest and exact staged file set before publication.
use anyhow::{Result, ensure};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, io::Read, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Artifact {
    path: String,
    bytes: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    protocol: u32,
    artifacts: Vec<Artifact>,
}

impl Manifest {
    pub(super) fn validate(&self, stage: &Path) -> Result<()> {
        ensure!(
            self.protocol == 1,
            "unsupported conversion response protocol"
        );
        ensure!(
            !self.artifacts.is_empty() && self.artifacts.len() <= 1025,
            "invalid artifact count"
        );
        let mut expected = BTreeMap::new();
        let mut total = 0u64;
        for artifact in &self.artifacts {
            let texture = artifact
                .path
                .strip_prefix("textures/image_")
                .and_then(|s| s.strip_suffix(".png"))
                .and_then(|s| {
                    s.parse::<u32>()
                        .ok()
                        .filter(|n| n.to_string() == s && *n < 1024)
                });
            ensure!(
                artifact.path == "model.fbx" || texture.is_some(),
                "invalid artifact path"
            );
            let limit = if texture.is_some() {
                16 * 1024 * 1024
            } else {
                256 * 1024 * 1024
            };
            ensure!(
                artifact.bytes > 0 && artifact.bytes <= limit,
                "invalid artifact size"
            );
            total += artifact.bytes;
            ensure!(total <= 512 * 1024 * 1024, "package size limit exceeded");
            ensure!(
                expected
                    .insert(artifact.path.clone(), artifact.bytes)
                    .is_none(),
                "duplicate artifact path"
            );
        }
        ensure!(expected.contains_key("model.fbx"), "package has no FBX");
        let mut actual = BTreeMap::new();
        for entry in fs::read_dir(stage)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if entry.file_name() == "textures" {
                ensure!(
                    kind.is_dir() && !kind.is_symlink(),
                    "texture directory is not a real directory"
                );
                let mut count = 0;
                for image in fs::read_dir(entry.path())? {
                    let image = image?;
                    ensure!(image.file_type()?.is_file(), "non-file texture artifact");
                    let name = image
                        .file_name()
                        .into_string()
                        .map_err(|_| anyhow::anyhow!("invalid artifact name"))?;
                    actual.insert(format!("textures/{name}"), image.metadata()?.len());
                    count += 1;
                    ensure!(count <= 1024, "too many staged textures");
                }
                ensure!(count > 0, "unexpected empty texture directory");
            } else {
                ensure!(kind.is_file(), "non-file package artifact");
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("invalid artifact name"))?;
                actual.insert(name, entry.metadata()?.len());
            }
            ensure!(actual.len() <= 1025, "too many staged artifacts");
        }
        ensure!(
            actual == expected,
            "staged artifacts differ from helper manifest"
        );
        // Full semantic/pixel readback is performed inside the pinned helper.
        // These checks reject wrong file types despite a plausible file manifest.
        for name in expected.keys() {
            let mut file = fs::File::open(stage.join(name))?;
            let signature: &[u8] = if name == "model.fbx" {
                b"Kaydara FBX Binary  \0\x1a\0"
            } else {
                b"\x89PNG\r\n\x1a\n"
            };
            let mut bytes = vec![0; signature.len()];
            file.read_exact(&mut bytes)?;
            ensure!(bytes == signature, "artifact file signature differs");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_exact_files_and_rejects_bad_manifests() {
        let dest = std::env::temp_dir().join(format!(
            "gcad-manifest-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let stage = crate::asset::fbx_package::PackageStage::new(&dest).unwrap();
        let bytes = b"Kaydara FBX Binary  \0\x1a\0";
        fs::write(stage.path().join("model.fbx"), bytes).unwrap();
        let good = serde_json::json!({"protocol":1,"artifacts":[{"path":"model.fbx","bytes":bytes.len()}]});
        let parse = |v| serde_json::from_value::<Manifest>(v).unwrap();
        parse(good.clone()).validate(stage.path()).unwrap();
        for path in [
            "../model.fbx",
            "textures/../model.fbx",
            "textures/image_00.png",
            "textures/image_1024.png",
            "C:/outside",
        ] {
            let mut value = good.clone();
            value["artifacts"][0]["path"] = path.into();
            assert!(parse(value).validate(stage.path()).is_err());
        }
        let mut wrong = good.clone();
        wrong["artifacts"][0]["bytes"] = 1.into();
        assert!(parse(wrong).validate(stage.path()).is_err());
        let mut duplicate = good.clone();
        duplicate["artifacts"]
            .as_array_mut()
            .unwrap()
            .push(good["artifacts"][0].clone());
        assert!(parse(duplicate).validate(stage.path()).is_err());
        fs::write(stage.path().join("extra"), b"unexpected").unwrap();
        assert!(parse(good.clone()).validate(stage.path()).is_err());
        fs::remove_file(stage.path().join("extra")).unwrap();
        fs::write(stage.path().join("model.fbx"), vec![0; bytes.len()]).unwrap();
        assert!(parse(good).validate(stage.path()).is_err());
    }
}
