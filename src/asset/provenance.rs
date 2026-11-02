//! Portable source identity and persisted, conservative mesh binding guards.
//! Callers must capture every dependency actually read during CAD evaluation.
use super::Mesh;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Native paths resolve against the process directory; browser paths resolve
/// against the virtual filesystem root and need not exist (missing probes count).
pub(crate) fn input_path(path: &std::path::Path) -> Result<std::path::PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        Ok(crate::host::tidy(&std::path::absolute(path)?))
    }
    #[cfg(target_arch = "wasm32")]
    {
        Ok(crate::host::canonicalize(path)?)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inputs {
    /// Project-relative, slash-separated path; never an absolute host path.
    pub source: String,
    pub body: String,
    pub instance: String,
    pub parameters: BTreeMap<String, f64>,
    /// Content hashes of source and all transitive inputs, keyed by relative path.
    pub dependencies: BTreeMap<String, [u8; 32]>,
    /// File/existence outcomes, including failed reads and missing optional files.
    pub observations: BTreeMap<String, bool>,
    pub tolerance_mm: f64,
    /// Include kernel/tessellator and preparation algorithm versions, not just CAD version.
    pub generator: String,
}

fn relative_id(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && !value
                .chars()
                .any(|c| c.is_control()
                    || matches!(c, '\\' | ':' | '?' | '*' | '"' | '<' | '>' | '|'))
            && value
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != ".." && part.trim() == part),
        "provenance paths must be portable relative identifiers"
    );
    Ok(())
}

impl Inputs {
    pub fn validate(&self) -> Result<()> {
        relative_id(&self.source)?;
        relative_id(&self.instance)?;
        ensure!(!self.body.trim().is_empty(), "source body must be named");
        ensure!(
            !self.generator.trim().is_empty(),
            "generator version is required"
        );
        ensure!(
            self.tolerance_mm.is_finite() && self.tolerance_mm > 0.0,
            "invalid tessellation tolerance"
        );
        ensure!(
            self.parameters
                .iter()
                .all(|(name, value)| !name.trim().is_empty() && value.is_finite()),
            "invalid source parameter"
        );
        for path in self.dependencies.keys() {
            relative_id(path)?;
            ensure!(
                self.observations.get(path) == Some(&true),
                "content hash requires a successful file observation"
            );
        }
        for path in self.observations.keys() {
            relative_id(path)?;
        }
        ensure!(
            self.dependencies.contains_key(&self.source),
            "source content hash is missing"
        );
        Ok(())
    }

    /// Hash supplied bytes without reading files or disclosing machine paths.
    pub fn content_hash(bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }

    /// Replace dependency state from an evaluation enclosed in host::recording.
    /// Paths resolve against the native working directory or browser virtual
    /// root. Existing dependencies must lie within project_root; absent external
    /// discovery probes are omitted. No files are reread: hashes describe the
    /// bytes actually consumed. This does not track direct std::fs or worker-thread
    /// reads; callers must merge worker recordings before invoking this method.
    pub fn with_recorded_reads(
        self,
        project_root: &std::path::Path,
        reads: &crate::host::Reads,
    ) -> Result<Self> {
        self.with_observations(project_root, reads.contents())
    }

    fn with_observations(
        mut self,
        project_root: &std::path::Path,
        observations: Vec<(std::path::PathBuf, Option<[u8; 32]>, bool)>,
    ) -> Result<Self> {
        let root = input_path(project_root)?;
        self.dependencies.clear();
        self.observations.clear();
        for (path, content, file) in observations {
            let absolute = input_path(&path)?;
            // Discovery may probe an absent user material library outside the
            // project. It contributes no input. If it appears on a later load,
            // the existing-file observation below still rejects that dependency.
            if !absolute.starts_with(&root) && !file && content.is_none() {
                continue;
            }
            let relative = absolute.strip_prefix(&root).map_err(|_| {
                anyhow::anyhow!("recorded dependency lies outside the project root")
            })?;
            let id = relative
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("dependency path is not UTF-8"))?
                .replace('\\', "/");
            relative_id(&id)?;
            if let Some(previous) = self.observations.insert(id.clone(), file) {
                ensure!(previous == file, "conflicting dependency observations");
            }
            if let Some(hash) = content {
                ensure!(file, "dependency changed during recording; evaluate again");
                if let Some(previous) = self.dependencies.insert(id, hash) {
                    ensure!(previous == hash, "conflicting dependency contents");
                }
            }
        }
        self.canonical()
    }

    pub(crate) fn canonical(mut self) -> Result<Self> {
        self.validate()?;
        for value in self.parameters.values_mut() {
            if *value == 0.0 {
                *value = 0.0;
            }
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    version: u32,
    inputs: Inputs,
    mesh_sha256: [u8; 32],
}

impl Binding {
    /// Capture only after all operations which reorder or split vertices.
    /// Rebinding is explicit: create a new record, never silently update one.
    pub fn capture(inputs: Inputs, mesh: &Mesh) -> Result<Self> {
        Ok(Self {
            version: 1,
            inputs: inputs.canonical()?,
            mesh_sha256: mesh.binding_stamp()?.0,
        })
    }

    pub fn require_current(&self, inputs: &Inputs, mesh: &Mesh) -> Result<()> {
        ensure!(self.version == 1, "unsupported asset binding version");
        self.inputs.validate()?;
        ensure!(
            self.inputs == inputs.clone().canonical()?,
            "CAD inputs changed; explicit rebinding is required"
        );
        mesh.require_binding(super::BindingStamp(self.mesh_sha256))
    }

    pub fn to_json(&self) -> Result<Vec<u8>> {
        ensure!(self.version == 1, "unsupported asset binding version");
        self.inputs.validate()?;
        Ok(serde_json::to_vec_pretty(self)?)
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        let mut value: Self = serde_json::from_slice(bytes)?;
        ensure!(value.version == 1, "unsupported asset binding version");
        value.inputs = value.inputs.canonical()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_recording_rejects_mid_evaluation_changes_across_nested_and_worker_reads() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::path::absolute(format!(
            "target/asset-conflicts-{}-{nonce}",
            std::process::id()
        ))
        .unwrap();
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("source.bin");
        let optional = root.join("optional.bin");
        let mut template = inputs();
        template.source = "source.bin".into();
        std::fs::write(&source, b"first").unwrap();
        let (_, initial) = crate::host::recording(|| {
            assert_eq!(crate::host::read(&source).unwrap(), b"first");
            assert!(crate::host::is_file(&source));
        });
        assert!(initial.unchanged());
        template
            .clone()
            .with_recorded_reads(&root, &initial)
            .unwrap();

        let (_, nested) = crate::host::recording(|| {
            crate::host::read(&source).unwrap();
            std::fs::write(&source, b"other").unwrap();
            crate::host::recording(|| crate::host::read(&source).unwrap());
        });
        assert!(!nested.unchanged());
        let error = template
            .clone()
            .with_recorded_reads(&root, &nested)
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("conflicting dependency contents")
        );

        let worker_path = source.clone();
        let worker = std::thread::spawn(move || {
            crate::host::recording(|| crate::host::read(&worker_path).unwrap()).1
        })
        .join()
        .unwrap();
        let mut merged = initial.clone();
        merged.merge(worker.clone());
        merged.merge(worker);
        assert!(!merged.unchanged());
        assert!(
            template
                .clone()
                .with_recorded_reads(&root, &merged)
                .is_err()
        );
        // Independent identical recordings remain usable after merging.
        let mut identical = initial.clone();
        identical.merge(initial.clone());
        assert_eq!(identical.contents().len(), initial.contents().len());
        template
            .clone()
            .with_recorded_reads(&root, &identical)
            .unwrap();

        let (_, changed_existence) = crate::host::recording(|| {
            crate::host::read(&source).unwrap();
            assert!(!crate::host::is_file(&optional));
            std::fs::write(&optional, b"created during evaluation").unwrap();
            assert!(crate::host::is_file(&optional));
        });
        assert!(!changed_existence.unchanged());
        let error = template
            .with_recorded_reads(&root, &changed_existence)
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("conflicting dependency observations")
        );
        std::fs::remove_file(optional).unwrap();
        std::fs::remove_file(source).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn repeated_observations_reject_changed_contents_and_existence() {
        let root = std::path::Path::new("target/observation-project");
        let mut template = inputs();
        template.source = "part.gcad".into();
        let source = root.join("part.gcad");
        let hash = Inputs::content_hash(b"consumed source bytes");
        let base = vec![
            (source.clone(), Some(hash), true),
            (source.clone(), None, true),
        ];
        let mut consistent = base.clone();
        consistent.push((root.join("missing.png"), None, false));
        consistent.push((root.join("../user/materials.toml"), None, false));
        let captured = template
            .clone()
            .with_observations(root, consistent)
            .unwrap();
        assert_eq!(captured.dependencies["part.gcad"], hash);
        assert!(!captured.observations["missing.png"]);
        assert_eq!(captured.observations.len(), 2);
        for conflicting in [
            (
                source.clone(),
                Some(Inputs::content_hash(b"later version")),
                true,
            ),
            (source, None, false),
            (root.join("../outside.png"), Some(hash), true),
            (root.join("../user/materials.toml"), None, true),
            (root.join("../user/materials.toml"), Some(hash), true),
        ] {
            let mut observations = base.clone();
            observations.push(conflicting);
            assert!(
                template
                    .clone()
                    .with_observations(root, observations)
                    .is_err()
            );
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn recorded_cad_reads_preserve_hashes_and_missing_file_observations() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::path::absolute(format!(
            "target/asset-recording-{}-{nonce}",
            std::process::id()
        ))
        .unwrap();
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("prop.gcad");
        let optional = root.join("optional.toml");
        let bytes = b"rect 1000 500\nextrude 250\n";
        std::fs::write(&source, bytes).unwrap();
        let (mesh, reads) = crate::host::recording(|| {
            let text = String::from_utf8(crate::host::read(&source).unwrap()).unwrap();
            assert!(!crate::host::is_file(&optional));
            assert!(crate::host::read(&optional).is_err());
            let program = crate::parse::parse_program(&text).unwrap();
            let model = crate::model::run(&program).model;
            Mesh::from_solid(model.solid.as_ref().unwrap(), 0.1).unwrap()
        });
        let mut template = inputs();
        template.source = "prop.gcad".into();
        let captured = template.clone().with_recorded_reads(&root, &reads).unwrap();
        assert_eq!(
            captured.dependencies["prop.gcad"],
            Inputs::content_hash(bytes)
        );
        assert_eq!(captured.observations["optional.toml"], false);
        let binding = Binding::capture(captured.clone(), &mesh).unwrap();
        let json = String::from_utf8(binding.to_json().unwrap()).unwrap();
        assert!(!json.contains("asset-recording-"));
        // Recording must describe the consumed bytes even if the file later changes.
        std::fs::write(&source, b"rect 1000 500\nextrude 251\n").unwrap();
        assert_eq!(
            template.clone().with_recorded_reads(&root, &reads).unwrap(),
            captured
        );
        std::fs::write(&optional, b"new optional input").unwrap();
        let (_, changed_reads) = crate::host::recording(|| {
            crate::host::read(&source).unwrap();
            assert!(crate::host::is_file(&optional));
        });
        let changed = template
            .clone()
            .with_recorded_reads(&root, &changed_reads)
            .unwrap();
        assert!(binding.require_current(&changed, &mesh).is_err());
        assert!(
            template
                .with_recorded_reads(&root.join("subproject"), &reads)
                .is_err()
        );
        std::fs::remove_file(source).unwrap();
        std::fs::remove_file(optional).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn recorded_model_includes_repeated_part_dependencies() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "asset-part-reads-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&root).unwrap();
        let part = b"rect 10 20\nextrude 3\n";
        std::fs::write(root.join("part.gcad"), part).unwrap();
        std::fs::write(
            root.join("assembly.gasm"),
            "part first part.gcad\npart second part.gcad\n",
        )
        .unwrap();
        let (run, reads) =
            crate::model::run_path_recorded(&root.join("assembly.gasm"), &[]).unwrap();
        assert!(run.steps.iter().all(|(_, result)| result.is_ok()));
        let records = reads.contents();
        assert!(
            records
                .iter()
                .any(|(path, hash, exists)| path.ends_with("part.gcad")
                    && *hash == Some(Inputs::content_hash(part))
                    && *exists)
        );
        assert!(
            records
                .iter()
                .any(|(path, hash, exists)| path.ends_with("assembly.gasm")
                    && hash.is_some()
                    && *exists)
        );
        std::fs::remove_file(root.join("part.gcad")).unwrap();
        std::fs::remove_file(root.join("assembly.gasm")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    fn inputs() -> Inputs {
        Inputs {
            source: "parts/prop.gcad".into(),
            body: "main".into(),
            instance: "assembly/prop".into(),
            parameters: BTreeMap::from([("width".into(), 1000.0), ("offset".into(), -0.0)]),
            dependencies: BTreeMap::from([
                (
                    "parts/prop.gcad".into(),
                    Inputs::content_hash(b"rect 1000 1000\nextrude 100"),
                ),
                ("materials.toml".into(), Inputs::content_hash(b"material")),
            ]),
            observations: BTreeMap::from([
                ("parts/prop.gcad".into(), true),
                ("materials.toml".into(), true),
            ]),
            tolerance_mm: 0.1,
            generator: "gcad-test/tessellator-1/box-uv-1".into(),
        }
    }
    #[test]
    fn binding_round_trip_allows_uv_edits_but_detects_input_and_geometry_changes() {
        let mesh = crate::asset::tests::triangle();
        let input = inputs();
        let binding = Binding::capture(input.clone(), &mesh).unwrap();
        let bytes = binding.to_json().unwrap();
        let restored = Binding::from_json(&bytes).unwrap();
        assert_eq!(bytes, restored.to_json().unwrap());
        let mut uv_edit = mesh.clone();
        uv_edit.texcoords.as_mut().unwrap()[0][0] = 0.25;
        restored.require_current(&input, &uv_edit).unwrap();
        for change in [
            |i: &mut Inputs| {
                i.parameters.insert("width".into(), 999.0);
            },
            |i: &mut Inputs| {
                i.dependencies
                    .insert("materials.toml".into(), Inputs::content_hash(b"changed"));
            },
            |i: &mut Inputs| i.body = "other".into(),
            |i: &mut Inputs| i.instance = "assembly/other".into(),
            |i: &mut Inputs| i.tolerance_mm = 0.2,
            |i: &mut Inputs| i.generator = "new-version".into(),
        ] {
            let mut changed = input.clone();
            change(&mut changed);
            assert!(restored.require_current(&changed, &mesh).is_err());
        }
        let mut changed_mesh = mesh;
        changed_mesh.positions[1][0] = 999.0;
        assert!(restored.require_current(&input, &changed_mesh).is_err());
    }

    #[test]
    fn rejects_host_paths_missing_dependencies_and_unknown_record_versions() {
        let mesh = crate::asset::tests::triangle();
        for path in [
            "/absolute",
            "C:/private/part",
            "../part",
            "a/../part",
            "a\\part",
            "https://host/part",
            "a//part",
        ] {
            let mut input = inputs();
            input.source = path.into();
            input.dependencies.insert(path.into(), [0; 32]);
            assert!(Binding::capture(input, &mesh).is_err());
        }
        let mut input = inputs();
        input.dependencies.clear();
        assert!(Binding::capture(input, &mesh).is_err());
        let bytes = Binding::capture(inputs(), &mesh)
            .unwrap()
            .to_json()
            .unwrap();
        let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        json["version"] = 2.into();
        assert!(Binding::from_json(&serde_json::to_vec(&json).unwrap()).is_err());
        json["version"] = 1.into();
        json["unknown"] = true.into();
        assert!(Binding::from_json(&serde_json::to_vec(&json).unwrap()).is_err());
    }
}
