#![cfg(all(feature = "game-assets", not(target_arch = "wasm32")))]

#[test]
#[ignore = "build reviewed gcad_fbx_helper and set GCAD_FBX_HELPER"]
fn consumer_profile_transport_and_invalid_role_publication_are_atomic() {
    use std::{path::PathBuf,sync::atomic::AtomicBool,time::Duration};
    use sha2::{Digest,Sha256};
    let helper=PathBuf::from(std::env::var_os("GCAD_FBX_HELPER").expect("helper"));
    let asset=gcad::asset::author::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/game-assets/plate-consumer-profile.gasset"),&[]).unwrap();
    let glb=asset.encode_glb().unwrap();
    let destination=std::env::temp_dir().join(format!("gcad-consumer-package-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let stage=gcad::asset::fbx_package::PackageStage::new(&destination).unwrap();
    let input=stage.path().join("source.glb");let output=stage.path().join("package");std::fs::write(&input,&glb).unwrap();
    gcad::asset::fbx_helper::convert_experimental(&helper,&input,&output,Duration::from_secs(60),&AtomicBool::new(false)).unwrap();
    let receipt:serde_json::Value=serde_json::from_slice(&std::fs::read(output.join("gcad-consumer-profile.json")).unwrap()).unwrap();
    assert_eq!(receipt["profile"],serde_json::to_value(asset.scene.consumer.unwrap()).unwrap());
    assert_eq!(receipt["model_sha256"],format!("{:x}",Sha256::digest(std::fs::read(output.join("model.fbx")).unwrap())));
    let length=u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let mut source:serde_json::Value=serde_json::from_slice(&glb[20..20+length]).unwrap();
    source["extras"]["gcad_consumer_profile"]["lods"][0]["levels"][1]["nodes"]=serde_json::json!(["collision"]);
    let mut json=serde_json::to_vec(&source).unwrap();while json.len()%4!=0{json.push(b' ');}
    let mut bad=glb[..12].to_vec();bad.extend_from_slice(&(json.len() as u32).to_le_bytes());bad.extend_from_slice(b"JSON");bad.extend_from_slice(&json);bad.extend_from_slice(&glb[20+length..]);
    let length=bad.len() as u32;bad[8..12].copy_from_slice(&length.to_le_bytes());
    let invalid=stage.path().join("invalid.glb");let refused=stage.path().join("refused");std::fs::write(&invalid,&bad).unwrap();
    assert!(gcad::asset::fbx_helper::convert_experimental(&helper,&invalid,&refused,Duration::from_secs(60),&AtomicBool::new(false)).is_err());
    assert!(!refused.exists());assert_eq!(std::fs::read(&input).unwrap(),glb);assert_eq!(std::fs::read(&invalid).unwrap(),bad);
}

#[test]
#[ignore = "build reviewed gcad_fbx_helper and set GCAD_FBX_HELPER"]
fn fractional_root_motion_rejects_stale_timeline_before_publication() {
    use std::{path::PathBuf, sync::atomic::AtomicBool, time::Duration};
    let helper=PathBuf::from(std::env::var_os("GCAD_FBX_HELPER").expect("helper"));
    let fixture=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/game-assets/plate-root-motion-fractional.gasset");
    let asset=gcad::asset::author::load(&fixture,&[]).unwrap();
    let destination=std::env::temp_dir().join(format!("gcad-fractional-motion-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let stage=gcad::asset::fbx_package::PackageStage::new(&destination).unwrap();
    for rigid in [false,true] {
        let glb=if rigid {gcad::asset::glb::encode_rigid_root_motion(&asset.scene,&[],0,0,"in-place").unwrap().0}
            else {gcad::asset::glb::encode_root_motion(&asset.scene,&[],0,0,[true,true,false],"in-place").unwrap().0};
        let input=stage.path().join(format!("source-{rigid}.glb"));let output=stage.path().join(format!("package-{rigid}"));
        std::fs::write(&input,&glb).unwrap();
        gcad::asset::fbx_helper::convert_experimental(&helper,&input,&output,Duration::from_secs(60),&AtomicBool::new(false)).unwrap();
        let receipt:serde_json::Value=serde_json::from_slice(&std::fs::read(output.join("gcad-root-motion.json")).unwrap()).unwrap();
        assert_eq!(receipt["motion"]["duration_seconds"],serde_json::json!(1.7f32 as f64));
        let length=u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let mut source:serde_json::Value=serde_json::from_slice(&glb[20..20+length]).unwrap();
        source["extras"]["gcad_root_motion"]["duration_seconds"]=serde_json::json!(1.7);
        let mut json=serde_json::to_vec(&source).unwrap();while json.len()%4!=0{json.push(b' ');}
        let mut stale=glb[..12].to_vec();stale.extend_from_slice(&(json.len() as u32).to_le_bytes());stale.extend_from_slice(b"JSON");stale.extend_from_slice(&json);stale.extend_from_slice(&glb[20+length..]);
        let total=stale.len() as u32;stale[8..12].copy_from_slice(&total.to_le_bytes());
        let bad_input=stage.path().join(format!("stale-{rigid}.glb"));let bad_output=stage.path().join(format!("refused-{rigid}"));std::fs::write(&bad_input,&stale).unwrap();
        let error=gcad::asset::fbx_helper::convert_experimental(&helper,&bad_input,&bad_output,Duration::from_secs(60),&AtomicBool::new(false)).unwrap_err();
        assert!(format!("{error:#}").contains("regenerate"));assert!(!bad_output.exists());
        assert_eq!(std::fs::read(&input).unwrap(),glb);assert_eq!(std::fs::read(&bad_input).unwrap(),stale);
    }
}

#[test]
#[ignore = "build reviewed gcad_fbx_helper and set GCAD_FBX_HELPER"]
fn root_motion_receipts_preserve_both_schemas_and_reject_invalid_metadata_atomically() {
    use std::{path::PathBuf, sync::atomic::AtomicBool, time::Duration};
    use sha2::{Digest, Sha256};
    let helper = PathBuf::from(std::env::var_os("GCAD_FBX_HELPER").expect("helper"));
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/game-assets/plate-root-motion.gasset");
    let asset = gcad::asset::author::load(&fixture, &[]).unwrap();
    let destination = std::env::temp_dir().join(format!("gcad-motion-package-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let workspace = gcad::asset::fbx_package::PackageStage::new(&destination).unwrap();
    for rigid in [false, true] {
        let glb = if rigid { gcad::asset::glb::encode_rigid_root_motion(&asset.scene, &[], 0, 0, "in-place").unwrap().0 }
            else { gcad::asset::glb::encode_root_motion(&asset.scene, &[], 0, 0, [true,true,false], "in-place").unwrap().0 };
        let input = workspace.path().join(format!("source-{rigid}.glb"));
        let output = workspace.path().join(format!("package-{rigid}"));
        std::fs::write(&input, &glb).unwrap();
        gcad::asset::fbx_helper::convert_experimental(&helper, &input, &output,
            Duration::from_secs(60), &AtomicBool::new(false)).unwrap();
        let receipt: serde_json::Value = serde_json::from_slice(&std::fs::read(output.join("gcad-root-motion.json")).unwrap()).unwrap();
        let model = std::fs::read(output.join("model.fbx")).unwrap();
        assert_eq!(receipt["model_sha256"], format!("{:x}", Sha256::digest(&model)));
        assert_eq!(receipt["source_glb_sha256"], format!("{:x}", Sha256::digest(&glb)));
        assert_eq!(receipt["source_clip"], "travel"); assert_eq!(receipt["in_place_clip"], "in-place");
        assert_eq!(receipt["node_name"], "root");
        let length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let mut document: serde_json::Value = serde_json::from_slice(&glb[20..20+length]).unwrap();
        assert_eq!(receipt["motion"], document["extras"]["gcad_root_motion"]);
        assert_eq!(std::fs::read(&input).unwrap(), glb);
        document["extras"]["gcad_root_motion"]["schema"] = serde_json::json!(99);
        let mut json = serde_json::to_vec(&document).unwrap(); while json.len()%4 != 0 { json.push(b' '); }
        let mut invalid = glb[..12].to_vec(); invalid.extend_from_slice(&(json.len() as u32).to_le_bytes());
        invalid.extend_from_slice(b"JSON"); invalid.extend_from_slice(&json); invalid.extend_from_slice(&glb[20+length..]);
        let total = invalid.len() as u32; invalid[8..12].copy_from_slice(&total.to_le_bytes());
        let bad_input = workspace.path().join(format!("invalid-{rigid}.glb"));
        let bad_output = workspace.path().join(format!("rejected-{rigid}"));
        std::fs::write(&bad_input, invalid).unwrap();
        assert!(gcad::asset::fbx_helper::convert_experimental(&helper, &bad_input, &bad_output,
            Duration::from_secs(60), &AtomicBool::new(false)).is_err());
        assert!(!bad_output.exists());
        assert_eq!(std::fs::read(output.join("model.fbx")).unwrap(), model);
    }
    // Allowing untextured materials must not permit textures without provenance.
    let textured = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/game-assets/plate-single-root.gasset");
    let mut bytes = gcad::asset::author::load(&textured, &[]).unwrap().encode_glb().unwrap();
    let key = b"gcad_texture_source_inputs";
    let at = bytes.windows(key.len()).position(|part| part == key).unwrap();
    bytes[at+key.len()-1] = b'z';
    let input = workspace.path().join("missing-provenance.glb");
    let output = workspace.path().join("missing-provenance-package");
    std::fs::write(&input, bytes).unwrap();
    assert!(gcad::asset::fbx_helper::convert_experimental(&helper, &input, &output,
        Duration::from_secs(60), &AtomicBool::new(false)).is_err());
    assert!(!output.exists());
}

#[test]
#[ignore = "build reviewed gcad_fbx_helper and set GCAD_FBX_HELPER and GCAD_FBX_INPUT"]
fn relocated_helper_preserves_unicode_paths() {
    use std::{fs, path::PathBuf, sync::atomic::AtomicBool, time::Duration};
    let helper = PathBuf::from(std::env::var_os("GCAD_FBX_HELPER").expect("helper"));
    let input = PathBuf::from(std::env::var_os("GCAD_FBX_INPUT").expect("input"));
    let destination = std::env::temp_dir().join(format!("gcad-unicode-test-{}-{}",
        std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let workspace = gcad::asset::fbx_package::PackageStage::new(&destination).unwrap();
    for depth in [0, 5] {
        let mut directory = workspace.path().join(format!("relocated café 模型-{depth}"));
        for _ in 0..depth { directory = directory.join("long-path-segment-abcdefghijklmnopqrstuvwxyz-0123456789"); }
        if depth > 0 { assert!(directory.as_os_str().len() > 260); }
        fs::create_dir_all(&directory).unwrap();
        let relocated = directory.join(if cfg!(windows) { "helper entrée.exe" } else { "helper entrée" });
        fs::copy(&helper, &relocated).unwrap();
        let source = directory.join("source façade.glb");
        fs::copy(&input, &source).unwrap();
        let before = fs::read(&source).unwrap();
        let output = directory.join("export résultat");
        gcad::asset::fbx_helper::convert_experimental(&relocated, &source, &output,
            Duration::from_secs(60), &AtomicBool::new(false)).unwrap();
        for mode in ["--inspect-materials-v1", "--inspect-skin-v1"] {
            let result = std::process::Command::new(&relocated).arg(mode)
                .arg(output.join("model.fbx")).output().unwrap();
            assert!(result.status.success(), "{mode}: {}", String::from_utf8_lossy(&result.stderr));
            let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
            assert_eq!(report["protocol"], 1);
        }
        assert_eq!(fs::read(source).unwrap(), before);
        assert!(output.join("textures").is_dir());
    }
}

#[test]
#[ignore = "set GCAD_FBX_HELPER to the reviewed helper"]
fn shader_receipt_is_published_atomically_and_bound_to_fbx() {
    use std::{path::PathBuf, sync::atomic::AtomicBool, time::Duration};
    use sha2::{Digest,Sha256};
    let helper=PathBuf::from(std::env::var_os("GCAD_FBX_HELPER").expect("helper"));
    let destination=std::env::temp_dir().join(format!("gcad-shader-package-{}-{}",std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let workspace=gcad::asset::fbx_package::PackageStage::new(&destination).unwrap();
    let path=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/game-assets/plate-shader-frozen.gasset");
    let asset=gcad::asset::author::load(&path,&[]).unwrap();let mut glb=asset.encode_glb().unwrap();
    let input=workspace.path().join("source.glb");std::fs::write(&input,&glb).unwrap();
    let output=workspace.path().join("package");let cancelled=AtomicBool::new(false);
    gcad::asset::fbx_helper::convert_experimental(&helper,&input,&output,Duration::from_secs(60),&cancelled).unwrap();
    let receipt:serde_json::Value=serde_json::from_slice(&std::fs::read(output.join("gcad-shaders.json")).unwrap()).unwrap();
    assert_eq!(receipt["model_sha256"],format!("{:x}",Sha256::digest(std::fs::read(output.join("model.fbx")).unwrap())));
    assert_eq!(receipt["source_glb_sha256"],format!("{:x}",Sha256::digest(&glb)));
    assert!(output.join("textures/image_0.png").is_file());
    let offset=glb.windows(9).position(|v|v==b"frozen-uv").unwrap();glb[offset..offset+9].copy_from_slice(b"broken-uv");
    std::fs::write(&input,&glb).unwrap();let invalid=workspace.path().join("invalid");
    assert!(gcad::asset::fbx_helper::convert_experimental(&helper,&input,&invalid,Duration::from_secs(60),&cancelled).is_err());
    assert!(!invalid.exists(),"invalid metadata must prevent publication");
    assert!(output.join("gcad-shaders.json").is_file(),"existing package preserved");
}

/// Exercise native readback independently of the Unity adapter and renderer.
#[test]
#[ignore = "set GCAD_FBX_HELPER, GCAD_FBX_INPUT and GCAD_FBX_MODEL to the refined fixture"]
fn skin_inspection_preserves_small_weights_and_rejects_invalid_inputs() {
    use std::{collections::BTreeSet, fs, path::PathBuf, process::Command};
    let helper = PathBuf::from(std::env::var_os("GCAD_FBX_HELPER").expect("helper"));
    let model = PathBuf::from(std::env::var_os("GCAD_FBX_MODEL").expect("converted FBX"));
    let source = fs::read(std::env::var_os("GCAD_FBX_INPUT").expect("source GLB")).unwrap();
    assert_eq!(&source[..4], b"glTF");
    let json_len = u32::from_le_bytes(source[12..16].try_into().unwrap()) as usize;
    let doc: serde_json::Value = serde_json::from_slice(&source[20..20 + json_len]).unwrap();
    let binary = &source[28 + json_len..];
    let mut expected = BTreeSet::new();
    for mesh in doc["meshes"].as_array().unwrap() {
        for primitive in mesh["primitives"].as_array().unwrap() {
            let Some(index) = primitive["attributes"]["WEIGHTS_0"].as_u64() else { continue };
            let accessor = &doc["accessors"][index as usize];
            let normalized=accessor["normalized"].as_bool().unwrap_or(false);
            let component=accessor["componentType"].as_u64().unwrap();
            assert!((component==5126&&!normalized)||(component==5123&&normalized), "float or UNORM16 weight fixture required");
            let component_bytes=if component==5123 {2} else {4};
            assert_eq!(accessor["type"], "VEC4");
            let view = &doc["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
            let start = (view["byteOffset"].as_u64().unwrap_or(0)
                + accessor["byteOffset"].as_u64().unwrap_or(0)) as usize;
            let stride = view["byteStride"].as_u64().unwrap_or((component_bytes*4) as u64) as usize;
            for vertex in 0..accessor["count"].as_u64().unwrap() as usize {
                for slot in 0..4 {
                    let offset = start + vertex * stride + slot * component_bytes;
                    let weight = if component==5123 {
                        f32::from(u16::from_le_bytes(binary[offset..offset+2].try_into().unwrap()))/65535.0
                    } else {f32::from_le_bytes(binary[offset..offset + 4].try_into().unwrap())};
                    if weight > 0.0 { expected.insert(weight.to_bits()); }
                }
            }
        }
    }
    let expected: Vec<f64> = expected.into_iter().map(|bits| f32::from_bits(bits) as f64).collect();
    assert!(expected.iter().any(|&w| w < 0.001), "fixture must exercise Unity's pruning cutoff");
    let before = fs::read(&model).unwrap();
    let result = Command::new(&helper).arg("--inspect-skin-v1").arg(&model).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let contract: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(contract["protocol"], 1);
    let meshes = contract["meshes"].as_array().unwrap();
    assert!(!meshes.is_empty());
    let mut actual = BTreeSet::new();
    for mesh in meshes {
        let bones = mesh["bones"].as_array().unwrap();
        for bone in bones {
            assert!(doc["nodes"].as_array().unwrap().iter().any(|n| n["name"] == *bone));
        }
        for vertex in mesh["vertices"].as_array().unwrap() {
            let weights = vertex["weights"].as_array().unwrap();
            let joints = vertex["joints"].as_array().unwrap();
            assert_eq!(weights.len(), joints.len());
            assert!((1..=4).contains(&weights.len()));
            assert_eq!(vertex["position"].as_array().unwrap().len(), 3);
            assert!(vertex["position"].as_array().unwrap().iter().all(|v| v.as_f64().unwrap().is_finite()));
            assert!((weights.iter().map(|w| w.as_f64().unwrap()).sum::<f64>() - 1.0).abs() < 1e-7);
            for (weight, joint) in weights.iter().zip(joints) {
                assert!((joint.as_u64().unwrap() as usize) < bones.len());
                let weight = weight.as_f64().unwrap();
                assert!(weight.is_finite() && weight > 0.0);
                actual.insert(weight.to_bits());
            }
        }
    }
    let actual: Vec<f64> = actual.into_iter().map(f64::from_bits).collect();
    for (from, to) in [(&expected, &actual), (&actual, &expected)] {
        assert!(from.iter().all(|x| to.iter().any(|y| (x - y).abs() < 1e-7)),
            "complete source weight values must survive readback in both directions");
    }
    assert_eq!(fs::read(&model).unwrap(), before);
    let root = std::env::temp_dir().join(format!("gcad-skin-input-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir(&root).unwrap();
    let invalid = root.join("invalid.fbx");
    fs::write(&invalid, b"not FBX").unwrap();
    let oversized = root.join("oversized.fbx");
    fs::File::create(&oversized).unwrap().set_len(256 * 1024 * 1024 + 1).unwrap();
    for path in [PathBuf::from("relative.fbx"), invalid.clone(), oversized.clone(), root.join("absent.fbx")] {
        let rejected = Command::new(&helper).arg("--inspect-skin-v1").arg(path).output().unwrap();
        assert!(!rejected.status.success());
        assert!(rejected.stdout.is_empty(), "failed inspection must not emit a partial contract");
        assert!(!rejected.stderr.is_empty());
    }
    assert_eq!(fs::read(&invalid).unwrap(), b"not FBX");
    fs::remove_file(invalid).unwrap();fs::remove_file(oversized).unwrap();fs::remove_dir(root).unwrap();
}

#[test]
#[ignore = "build gcad_fbx_protocol_fixture and set GCAD_FBX_PROTOCOL_FIXTURE"]
fn recovery_waits_for_native_helper_after_owner_releases_lease() {
    use std::{fs, io::Write, process::{Child, Command}, time::{Duration, Instant}};
    struct Running(Child);
    impl Drop for Running { fn drop(&mut self) { let _=self.0.kill(); let _=self.0.wait(); } }
    let fixture=std::env::var_os("GCAD_FBX_PROTOCOL_FIXTURE").expect("fixture path");
    let destination=std::env::temp_dir().join(format!("gcad-recovery-test-{}-{}",std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let workspace=gcad::asset::fbx_package::PackageStage::new(&destination).unwrap();
    let stage=workspace.path().join(".gcad-fbx-10-20-30");
    let marker=stage.with_extension("lease");
    let mut owner=fs::OpenOptions::new().read(true).write(true).create_new(true).open(&marker).unwrap();
    owner.write_all(b"gcad-fbx-stage-v1\n").unwrap();
    owner.lock_shared().unwrap();
    fs::create_dir(&stage).unwrap();
    let mut command=Command::new(fixture);
    command.arg("--hold-stage-lease").arg(&stage);
    #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
    let helper=Running(command.spawn().unwrap());
    let deadline=Instant::now()+Duration::from_secs(10);
    while !stage.join("ready").exists() {
        assert!(Instant::now()<deadline,"helper did not acquire lease");
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(owner);
    assert!(gcad::asset::fbx_package::recover(workspace.path()).unwrap().is_empty());
    assert!(stage.exists());
    drop(helper);
    assert_eq!(gcad::asset::fbx_package::recover(workspace.path()).unwrap(),vec![stage.clone()]);
    assert!(!stage.exists() && !marker.exists());
}

#[test]
#[ignore = "build reviewed gcad_fbx_helper and set GCAD_FBX_HELPER and GCAD_FBX_INPUT"]
fn converts_and_publishes_complete_package() {
    use std::{path::PathBuf, sync::atomic::AtomicBool, time::Duration};
    let helper = PathBuf::from(std::env::var_os("GCAD_FBX_HELPER").expect("helper path"));
    let input = PathBuf::from(std::env::var_os("GCAD_FBX_INPUT").expect("GLB fixture path"));
    let destination = std::env::temp_dir().join(format!("gcad-convert-test-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let workspace = gcad::asset::fbx_package::PackageStage::new(&destination).unwrap();
    let output = workspace.path().join("package");
    let cancelled = AtomicBool::new(false);
    let published = gcad::asset::fbx_helper::convert_experimental(&helper, &input, &output,
        Duration::from_secs(60), &cancelled).unwrap();
    assert_eq!(published, output);
    assert!(output.join("model.fbx").is_file());
    assert!(std::fs::read_dir(output.join("textures")).unwrap().count() > 0);
    let before = std::fs::read(output.join("model.fbx")).unwrap();
    // Independent consumer metadata must come from the published FBX itself.
    let inspected = std::process::Command::new(&helper)
        .arg("--inspect-materials-v1").arg(output.join("model.fbx")).output().unwrap();
    assert!(inspected.status.success(), "{}", String::from_utf8_lossy(&inspected.stderr));
    let contract: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(contract["protocol"], 1);
    assert_eq!(contract["materialSchema"], 3);
    let source = std::fs::read(&input).unwrap();
    let json_len = u32::from_le_bytes(source[12..16].try_into().unwrap()) as usize;
    let reference: serde_json::Value = serde_json::from_slice(&source[20..20+json_len]).unwrap();
    let expected = reference["materials"].as_array().unwrap();
    let actual = contract["materials"].as_array().unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, material) in expected.iter().enumerate() {
        let fallback = format!("materials_{index}");
        let name = material["name"].as_str().unwrap_or(&fallback);
        let found = actual.iter().find(|m| m["name"] == name).unwrap();
        for (key, value) in [
            ("Metallic", material["pbrMetallicRoughness"]["metallicFactor"].as_f64().unwrap_or(1.)),
            ("Roughness", material["pbrMetallicRoughness"]["roughnessFactor"].as_f64().unwrap_or(1.)),
            ("NormalScale", material["normalTexture"]["scale"].as_f64().unwrap_or(1.)),
            ("OcclusionStrength", material["occlusionTexture"]["strength"].as_f64().unwrap_or(1.)),
            ("AlphaCutoff", material["alphaCutoff"].as_f64().unwrap_or(0.5)),
            ("BaseAlpha", material["pbrMetallicRoughness"]["baseColorFactor"][3].as_f64().unwrap_or(1.)),
        ] { assert!((found[key].as_f64().unwrap() - value).abs() < 1e-6, "{name}: {key}"); }
        for (key, values, fallback) in [
            ("BaseColour", &material["pbrMetallicRoughness"]["baseColorFactor"], 1.),
            ("Emission", &material["emissiveFactor"], 0.),
        ] { for axis in 0..3 {
            assert!((found[key][axis].as_f64().unwrap() - values[axis].as_f64().unwrap_or(fallback)).abs() < 1e-6);
        } }
        assert_eq!(found["AlphaMode"].as_str().unwrap(), material["alphaMode"].as_str().unwrap_or("OPAQUE"));
        assert_eq!(found["DoubleSided"].as_bool().unwrap(), material["doubleSided"].as_bool().unwrap_or(false));
        for (slot, texture) in [
            ("BaseColour", &material["pbrMetallicRoughness"]["baseColorTexture"]),
            ("Roughness", &material["pbrMetallicRoughness"]["metallicRoughnessTexture"]),
            ("Metallic", &material["pbrMetallicRoughness"]["metallicRoughnessTexture"]),
            ("Normal", &material["normalTexture"]), ("Occlusion", &material["occlusionTexture"]),
            ("Emission", &material["emissiveTexture"]),
        ] {
            let map = found["maps"].as_array().unwrap().iter().find(|m| m["slot"] == slot);
            if let Some(texture) = texture["index"].as_u64() {
                let image = reference["textures"][texture as usize]["source"].as_u64().unwrap();
                assert_eq!(map.unwrap()["path"], format!("textures/image_{image}.png"));
                if let Some(index) = reference["textures"][texture as usize]["sampler"].as_u64() {
                    let sampler = &reference["samplers"][index as usize];
                    for (field, source_field, fallback) in [("WrapU", "wrapS", 10497), ("WrapV", "wrapT", 10497),
                        ("MagFilter", "magFilter", -1), ("MinFilter", "minFilter", -1)] {
                        assert_eq!(map.unwrap()[field].as_i64().unwrap(), sampler[source_field].as_i64().unwrap_or(fallback));
                    }
                }
                let channel = match slot { "Roughness" => Some(1), "Metallic" => Some(2), "Occlusion" => Some(0), _ => None };
                if let Some(channel) = channel { assert_eq!(map.unwrap()["channel"], channel); }
            } else { assert!(map.is_none()); }
        }
    }
    assert_eq!(std::fs::read(output.join("model.fbx")).unwrap(), before);
    // Corrupt only the schema property name, retaining an otherwise readable FBX.
    let mut changed = before.clone();
    let key = b"gcad|PbrVersion";
    let offsets: Vec<_> = changed.windows(key.len()).enumerate()
        .filter_map(|(i, bytes)| (bytes == key).then_some(i)).collect();
    assert!(!offsets.is_empty());
    for at in offsets { changed[at..at+key.len()].copy_from_slice(b"gcad|BadVersion"); }
    let invalid_contract = workspace.path().join("invalid-contract.fbx");
    std::fs::write(&invalid_contract, changed).unwrap();
    let rejected = std::process::Command::new(&helper)
        .arg("--inspect-materials-v1").arg(&invalid_contract).output().unwrap();
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    std::fs::remove_file(invalid_contract).unwrap();
    assert!(gcad::asset::fbx_helper::convert_experimental(&helper, &input, &output,
        Duration::from_secs(60), &cancelled).is_err());
    assert_eq!(std::fs::read(output.join("model.fbx")).unwrap(), before);
    // A failed conversion must remove its temporary output, retaining all inputs.
    let invalid = workspace.path().join("invalid.glb");
    std::fs::write(&invalid, b"invalid GLB").unwrap();
    let failed = workspace.path().join("failed");
    assert!(gcad::asset::fbx_helper::convert_experimental(&helper, &invalid, &failed,
        Duration::from_secs(60), &cancelled).is_err());
    assert!(!failed.exists());
    assert_eq!(std::fs::read_dir(workspace.path()).unwrap().count(), 2);
}

/// Explicit native integration test; ordinary tests need no C++ helper build.
#[test]
#[ignore = "build gcad_fbx_protocol_fixture and set GCAD_FBX_PROTOCOL_FIXTURE"]
fn process_boundary_rejects_failures_limits_timeout_and_cancellation() {
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
    use std::time::{Duration, Instant};
    let fixture = std::path::PathBuf::from(std::env::var_os("GCAD_FBX_PROTOCOL_FIXTURE").expect("fixture path"));
    assert!(fixture.is_absolute());
    let root = std::env::temp_dir().join(format!("gcad-helper-process-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&root).unwrap();
    for mode in ["bad-json", "stdout", "stderr", "failure", "timeout", "cancel"] {
        let path = root.join(format!("{mode}{}", std::env::consts::EXE_SUFFIX));
        std::fs::copy(&fixture, &path).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancel = if mode == "cancel" {
            let flag = cancelled.clone();
            Some(std::thread::spawn(move || { std::thread::sleep(Duration::from_millis(100)); flag.store(true, Ordering::Relaxed); }))
        } else { None };
        let timeout = if mode == "timeout" { Duration::from_millis(150) } else { Duration::from_secs(5) };
        let started = Instant::now();
        let error = gcad::asset::fbx_helper::inspect(&path, timeout, &cancelled).unwrap_err().to_string();
        if mode == "timeout" { assert!(error.contains("timed out"), "{error}"); }
        if mode == "cancel" { assert!(error.contains("cancelled"), "{error}"); }
        assert!(started.elapsed() < Duration::from_secs(10));
        if let Some(cancel) = cancel { cancel.join().unwrap(); }
        // On Windows this also proves the executable is no longer held by the child.
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
}
