//! Digest-bound root-motion transport for FBX consumers. Coordinates remain glTF
//! metre/Y-up/right-handed; engine adapters must convert them explicitly.
use super::{
    animation::{Interpolation, Track, Values},
    scene::Transform,
};
use anyhow::{Context, Result, ensure};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn field<T: DeserializeOwned>(value: &Value, key: &str) -> Result<T> {
    serde_json::from_value(
        value
            .get(key)
            .with_context(|| format!("missing root-motion {key}"))?
            .clone(),
    )
    .with_context(|| format!("invalid root-motion {key}"))
}
fn keys(value: &Value, allowed: &[&str]) -> Result<()> {
    let object = value.as_object().context("root-motion object required")?;
    ensure!(
        object.keys().all(|key| allowed.contains(&key.as_str())),
        "unknown root-motion field"
    );
    Ok(())
}
fn pose(value: &Value) -> Result<Transform> {
    keys(value, &["translation", "rotation"])?;
    let result = Transform {
        translation: field(value, "translation")?,
        rotation: field(value, "rotation")?,
        ..Default::default()
    };
    result.validate()?;
    Ok(result)
}
fn encoded_time(value: f64) -> Result<()> {
    ensure!(
        value.is_finite() && value == (value as f32) as f64,
        "root-motion time differs from the float32 clip timeline; regenerate the root-motion GLB"
    );
    Ok(())
}
fn same_matrix(
    a: monstertruck::modeling::Matrix4,
    b: monstertruck::modeling::Matrix4,
) -> Result<()> {
    let a: &[[f64; 4]; 4] = a.as_ref();
    let b: &[[f64; 4]; 4] = b.as_ref();
    ensure!(
        a.iter()
            .flatten()
            .zip(b.iter().flatten())
            .all(|(a, b)| (a - b).abs() <= 1e-9 * (1. + a.abs().max(b.abs()))),
        "root-motion anchor or loop transform disagrees with channels"
    );
    Ok(())
}
fn name(source: &Value, collection: &str, index: usize) -> Result<String> {
    let entries = source[collection]
        .as_array()
        .context("root-motion reference collection missing")?;
    let name = entries
        .get(index)
        .and_then(|v| v["name"].as_str())
        .context("root-motion named reference missing")?;
    ensure!(
        !name.trim().is_empty()
            && name.len() <= 4096
            && entries.iter().filter(|v| v["name"] == name).count() == 1,
        "root-motion reference name is empty or ambiguous"
    );
    Ok(name.to_owned())
}

// Canonical float animation accessors only; never fetch external buffers. Keep
// source key times instead of relying on an engine's FBX curve resampling.
fn animation_values(
    source: &Value,
    binary: &[u8],
    index: usize,
    kind: &str,
    width: usize,
) -> Result<Vec<Vec<f64>>> {
    let accessor = &source["accessors"][index];
    ensure!(
        accessor["componentType"] == 5126
            && accessor["type"] == kind
            && accessor.get("sparse").is_none()
            && accessor.get("normalized").is_none_or(|v| v == false),
        "motion clip requires dense float accessors"
    );
    let count: usize = field(accessor, "count")?;
    ensure!(
        (1..=65_536).contains(&count),
        "motion clip key limit exceeded"
    );
    let view_index: usize = field(accessor, "bufferView")?;
    let view = &source["bufferViews"][view_index];
    ensure!(view["buffer"] == 0, "motion clip requires embedded buffer");
    let offset = |v: &Value, key: &str| -> Result<usize> {
        match v.get(key) {
            Some(_) => field(v, key),
            None => Ok(0),
        }
    };
    let view_start = offset(view, "byteOffset")?;
    let view_length: usize = field(view, "byteLength")?;
    let view_end = view_start
        .checked_add(view_length)
        .context("motion view overflow")?;
    ensure!(view_end <= binary.len(), "motion view outside binary");
    let view_bytes = &binary[view_start..view_end];
    let start = offset(accessor, "byteOffset")?;
    let stride = match view.get("byteStride") {
        Some(_) => field(view, "byteStride")?,
        None => width * 4,
    };
    ensure!(
        stride >= width * 4 && stride % 4 == 0 && start % 4 == 0,
        "invalid motion accessor stride"
    );
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let at = i
            .checked_mul(stride)
            .and_then(|n| n.checked_add(start))
            .context("motion accessor overflow")?;
        let end = at
            .checked_add(width * 4)
            .context("motion accessor overflow")?;
        let bytes = view_bytes
            .get(at..end)
            .context("motion accessor outside view")?;
        let values: Vec<_> = bytes
            .chunks_exact(4)
            .map(|v| f32::from_le_bytes(v.try_into().unwrap()) as f64)
            .collect();
        ensure!(
            values.iter().all(|v| v.is_finite()),
            "nonfinite motion clip value"
        );
        result.push(values);
    }
    Ok(result)
}

fn clip_tracks(
    source: &Value,
    glb: &[u8],
    json_length: usize,
    index: usize,
    duration: f64,
) -> Result<Value> {
    let at = 20 + json_length;
    ensure!(
        glb.len() >= at + 8 && &glb[at + 4..at + 8] == b"BIN\0",
        "motion clip requires GLB binary"
    );
    let length = u32::from_le_bytes(glb[at..at + 4].try_into()?) as usize;
    let binary = glb
        .get(at + 8..at + 8 + length)
        .context("truncated motion clip binary")?;
    let animation = &source["animations"][index];
    let channels = animation["channels"]
        .as_array()
        .context("motion clip channels required")?;
    ensure!(
        !channels.is_empty() && channels.len() <= 4096,
        "motion clip channel limit exceeded"
    );
    let mut seen = std::collections::HashSet::new();
    let mut tracks = Vec::new();
    let mut total_keys = 0;
    for channel in channels {
        let target = &channel["target"];
        let node: usize = field(target, "node")?;
        let node_name = name(source, "nodes", node)?;
        ensure!(
            source["nodes"][node].get("matrix").is_none(),
            "motion clip requires TRS node"
        );
        let path: String = field(target, "path")?;
        ensure!(
            seen.insert((node, path.clone())),
            "duplicate motion clip channel"
        );
        let (kind, width) = match path.as_str() {
            "translation" | "scale" => ("VEC3", 3),
            "rotation" => ("VEC4", 4),
            _ => anyhow::bail!("unsupported motion clip channel"),
        };
        let sampler_index: usize = field(channel, "sampler")?;
        let sampler = &animation["samplers"][sampler_index];
        let interpolation = match sampler.get("interpolation") {
            Some(value) => value
                .as_str()
                .context("motion interpolation must be a string")?,
            None => "LINEAR",
        };
        ensure!(
            interpolation == "STEP" || interpolation == "LINEAR",
            "unsupported motion clip interpolation"
        );
        let times: Vec<_> =
            animation_values(source, binary, field(sampler, "input")?, "SCALAR", 1)?
                .into_iter()
                .map(|v| v[0])
                .collect();
        let values = animation_values(source, binary, field(sampler, "output")?, kind, width)?;
        ensure!(
            times.len() == values.len()
                && times.iter().all(|t| *t >= 0. && *t <= duration + 1e-6)
                && times.windows(2).all(|w| w[0] < w[1]),
            "invalid motion clip key times"
        );
        total_keys += times.len();
        ensure!(total_keys <= 65_536, "motion clip total key limit exceeded");
        if path == "rotation" {
            ensure!(
                values
                    .iter()
                    .all(|v| (v.iter().map(|x| x * x).sum::<f64>() - 1.).abs() <= 1e-5),
                "invalid motion clip quaternion"
            );
        }
        if path == "scale" {
            ensure!(
                values.iter().flatten().all(|v| v.abs() > 1e-12),
                "singular motion clip scale"
            );
        }
        tracks.push(json!({"node_name":node_name,"path":path,"interpolation":interpolation.to_lowercase(),"times_seconds":times,"values":values}));
    }
    Ok(json!(tracks))
}

pub(super) fn create(glb: &[u8], fbx_sha256: &str) -> Result<Option<Vec<u8>>> {
    ensure!(
        glb.len() >= 28 && &glb[..4] == b"glTF",
        "root-motion receipt requires GLB"
    );
    let length = u32::from_le_bytes(glb[12..16].try_into()?) as usize;
    ensure!(
        length <= glb.len() - 20 && &glb[16..20] == b"JSON",
        "invalid GLB JSON chunk"
    );
    let source: Value = serde_json::from_slice(&glb[20..20 + length])?;
    let Some(motion) = source["extras"].get("gcad_root_motion") else {
        return Ok(None);
    };
    let schema: u32 = field(motion, "schema")?;
    ensure!(schema == 1 || schema == 2, "unsupported root-motion schema");
    ensure!(
        motion["length_unit"] == "metre"
            && motion["up_axis"] == "Y"
            && motion["handedness"] == "right",
        "unsupported root-motion coordinate convention"
    );
    let source_index: usize = field(motion, "source_animation")?;
    let in_place_index: usize = field(motion, "in_place_animation")?;
    let node_index: usize = field(motion, "node")?;
    ensure!(
        source_index != in_place_index,
        "root-motion clip references must differ"
    );
    let source_clip = name(&source, "animations", source_index)?;
    let in_place_clip = name(&source, "animations", in_place_index)?;
    let node = name(&source, "nodes", node_index)?;
    let scene: usize = field(&source, "scene")?;
    ensure!(
        source["scenes"][scene]["nodes"]
            .as_array()
            .is_some_and(|nodes| nodes.contains(&json!(node_index))),
        "root-motion node must be an active scene root"
    );
    let duration_seconds: f64 = field(motion, "duration_seconds")?;
    encoded_time(duration_seconds)?;
    let axes: [bool; 3] = field(motion, "translation_axes")?;
    if schema == 1 {
        keys(
            motion,
            &[
                "schema",
                "source_animation",
                "in_place_animation",
                "node",
                "length_unit",
                "up_axis",
                "handedness",
                "translation_axes",
                "duration_seconds",
                "interpolation",
                "times_seconds",
                "displacements",
                "loop_displacement",
                "rotation_extracted",
            ],
        )?;
        ensure!(
            motion["rotation_extracted"] == false,
            "translation receipt cannot extract rotation"
        );
        // The validator is coordinate agnostic: these arrays stay in glTF metres.
        let translated = super::root_motion::Motion {
            schema: 1,
            source_clip: source_clip.clone(),
            node: node.clone(),
            axes,
            duration_seconds,
            interpolation: field(motion, "interpolation")?,
            times: field(motion, "times_seconds")?,
            displacements_mm: field(motion, "displacements")?,
        };
        ensure!(
            translated.times.len() <= 65_536,
            "root-motion key limit exceeded"
        );
        translated.validate()?;
        for &time in &translated.times {
            encoded_time(time)?;
        }
        let endpoint: [f64; 3] = field(motion, "loop_displacement")?;
        ensure!(
            Some(&endpoint) == translated.displacements_mm.last(),
            "root-motion loop displacement disagrees with keys"
        );
    } else {
        keys(
            motion,
            &[
                "schema",
                "mode",
                "source_animation",
                "in_place_animation",
                "node",
                "length_unit",
                "up_axis",
                "handedness",
                "duration_seconds",
                "rotation_extracted",
                "translation_axes",
                "anchor",
                "channels",
                "loop_transform",
                "composition",
                "loop_composition",
                "max_accumulated_cycles",
            ],
        )?;
        ensure!(
            motion["mode"] == "rigid"
                && motion["rotation_extracted"] == true
                && axes == [true; 3]
                && motion["composition"] == "left-multiply-in-place-root"
                && motion["loop_composition"] == "endpoint-power-times-phase"
                && motion["max_accumulated_cycles"] == 1_000_000,
            "unsupported rigid root-motion convention"
        );
        let anchor = pose(&motion["anchor"])?;
        let channels = motion["channels"]
            .as_array()
            .context("root-motion channels required")?;
        ensure!(
            !channels.is_empty() && channels.len() <= 2,
            "invalid rigid root-motion channel count"
        );
        let mut tracks = Vec::new();
        for channel in channels {
            keys(
                channel,
                &["path", "times_seconds", "interpolation", "values"],
            )?;
            let times: Vec<f64> = field(channel, "times_seconds")?;
            for &time in &times {
                encoded_time(time)?;
            }
            ensure!(times.len() <= 65_536, "root-motion key limit exceeded");
            let interpolation: Interpolation = field(channel, "interpolation")?;
            let values = match channel["path"].as_str() {
                Some("translation") => Values::Translation(field(channel, "values")?),
                Some("rotation") => Values::Rotation(field(channel, "values")?),
                _ => anyhow::bail!("unsupported rigid root-motion channel"),
            };
            tracks.push(Track {
                node: 0,
                times,
                interpolation,
                values,
            });
        }
        let rigid = super::rigid_motion::Motion {
            schema: 1,
            source_clip: source_clip.clone(),
            node: node.clone(),
            duration_seconds,
            rest_translation_mm: anchor.translation,
            rest_rotation_xyzw: anchor.rotation,
            tracks,
        };
        rigid.validate()?;
        same_matrix(rigid.anchor()?.matrix()?, anchor.matrix()?)?;
        same_matrix(
            rigid.sample(duration_seconds, false)?,
            pose(&motion["loop_transform"])?.matrix()?,
        )?;
    }
    ensure!(
        fbx_sha256.len() == 64 && fbx_sha256.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid FBX digest"
    );
    let tracks = clip_tracks(&source, glb, length, in_place_index, duration_seconds)?;
    let receipt = json!({"version":2, "format":"gcad-root-motion", "model_sha256":fbx_sha256,
        "source_glb_sha256":format!("{:x}",Sha256::digest(glb)), "source_clip":source_clip, "in_place_clip":in_place_clip,
        "node_name":node, "motion":motion, "in_place_tracks":tracks});
    let bytes = serde_json::to_vec_pretty(&receipt)?;
    ensure!(
        bytes.len() <= 16 * 1024 * 1024,
        "root-motion receipt exceeds 16 MiB"
    );
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(rigid: bool) -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/game-assets/plate-root-motion.gasset");
        let asset = super::super::author::load(&path, &[]).unwrap();
        if rigid {
            super::super::glb::encode_rigid_root_motion(&asset.scene, &[], 0, 0, "in-place")
                .unwrap()
                .0
        } else {
            super::super::glb::encode_root_motion(
                &asset.scene,
                &[],
                0,
                0,
                [true, true, false],
                "in-place",
            )
            .unwrap()
            .0
        }
    }
    fn change(glb: &[u8], pointer: &str, value: Value) -> Vec<u8> {
        let length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let mut source: Value = serde_json::from_slice(&glb[20..20 + length]).unwrap();
        *source.pointer_mut(pointer).unwrap() = value;
        let mut json = serde_json::to_vec(&source).unwrap();
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        let mut changed = glb[..12].to_vec();
        changed.extend_from_slice(&(json.len() as u32).to_le_bytes());
        changed.extend_from_slice(b"JSON");
        changed.extend_from_slice(&json);
        changed.extend_from_slice(&glb[20 + length..]);
        let total = changed.len() as u32;
        changed[8..12].copy_from_slice(&total.to_le_bytes());
        changed
    }
    #[test]
    fn preserves_both_motion_schemas_with_named_digest_bound_references() {
        for rigid in [false, true] {
            let glb = fixture(rigid);
            let receipt: Value =
                serde_json::from_slice(&create(&glb, &"ab".repeat(32)).unwrap().unwrap()).unwrap();
            assert_eq!(receipt["source_clip"], "travel");
            assert_eq!(receipt["in_place_clip"], "in-place");
            assert_eq!(receipt["node_name"], "root");
            assert_eq!(receipt["model_sha256"], "ab".repeat(32));
            assert_eq!(
                receipt["source_glb_sha256"],
                format!("{:x}", Sha256::digest(&glb))
            );
            assert_eq!(receipt["motion"]["schema"], if rigid { 2 } else { 1 });
            assert_eq!(receipt["version"], 2);
            assert!(!receipt["in_place_tracks"].as_array().unwrap().is_empty());
            let length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
            let source: Value = serde_json::from_slice(&glb[20..20 + length]).unwrap();
            assert_eq!(receipt["motion"], source["extras"]["gcad_root_motion"]);
            assert!(create(&glb, "bad digest").is_err());
            let plain = change(&glb, "/extras", json!({}));
            assert!(create(&plain, &"ab".repeat(32)).unwrap().is_none());
        }
    }
    #[test]
    fn exact_tracks_preserve_step_and_reject_invalid_accessors() {
        let glb = fixture(false);
        let length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let source: Value = serde_json::from_slice(&glb[20..20 + length]).unwrap();
        let sampler = source["animations"][1]["channels"][0]["sampler"]
            .as_u64()
            .unwrap();
        let pointer = format!("/animations/1/samplers/{sampler}/interpolation");
        let step = change(&glb, &pointer, json!("STEP"));
        let receipt: Value =
            serde_json::from_slice(&create(&step, &"ab".repeat(32)).unwrap().unwrap()).unwrap();
        assert_eq!(receipt["in_place_tracks"][0]["interpolation"], "step");
        assert_eq!(receipt["in_place_tracks"][0]["node_name"], "root");
        let output = source["animations"][1]["samplers"][sampler as usize]["output"]
            .as_u64()
            .unwrap();
        for (pointer, value) in [
            (format!("/accessors/{output}/count"), json!(65537)),
            (format!("/accessors/{output}/componentType"), json!(5123)),
            (format!("/accessors/{output}/bufferView"), json!(999999)),
            (
                format!("/animations/1/samplers/{sampler}/interpolation"),
                json!("CUBICSPLINE"),
            ),
        ] {
            assert!(
                create(&change(&glb, &pointer, value), &"ab".repeat(32)).is_err(),
                "{pointer}"
            );
        }
        assert!(create(&step[..step.len() - 8], &"ab".repeat(32)).is_err());
    }
    #[test]
    fn fractional_motion_metadata_uses_the_encoded_clip_timeline() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/game-assets/plate-root-motion-fractional.gasset");
        let asset = super::super::author::load(&path, &[]).unwrap();
        for rigid in [false, true] {
            let glb = if rigid {
                let (bytes, motion) = super::super::glb::encode_rigid_root_motion(
                    &asset.scene,
                    &[],
                    0,
                    0,
                    "in-place",
                )
                .unwrap();
                assert_eq!(motion.duration_seconds, 1.7);
                bytes
            } else {
                let (bytes, motion) = super::super::glb::encode_root_motion(
                    &asset.scene,
                    &[],
                    0,
                    0,
                    [true, true, false],
                    "in-place",
                )
                .unwrap();
                assert_eq!(motion.times[1], 0.3);
                assert_eq!(motion.duration_seconds, 1.7);
                bytes
            };
            let length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
            let source: Value = serde_json::from_slice(&glb[20..20 + length]).unwrap();
            let tracks = clip_tracks(&source, &glb, length, 0, 1.7).unwrap();
            let receipt: Value =
                serde_json::from_slice(&create(&glb, &"ab".repeat(32)).unwrap().unwrap()).unwrap();
            let motion = &receipt["motion"];
            assert_eq!(motion["duration_seconds"], json!(1.7f32 as f64));
            let stale = change(
                &glb,
                "/extras/gcad_root_motion/duration_seconds",
                json!(1.7),
            );
            assert!(
                create(&stale, &"ab".repeat(32))
                    .unwrap_err()
                    .to_string()
                    .contains("regenerate")
            );
            let pointer = if rigid {
                "/extras/gcad_root_motion/channels/0/times_seconds/1"
            } else {
                "/extras/gcad_root_motion/times_seconds/1"
            };
            let stale = change(&glb, pointer, json!(0.3));
            assert!(create(&stale, &"ab".repeat(32)).is_err());
            for track in tracks
                .as_array()
                .unwrap()
                .iter()
                .filter(|t| t["node_name"] == "root")
            {
                let times = if rigid {
                    &motion["channels"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|c| c["path"] == track["path"])
                        .unwrap()["times_seconds"]
                } else if track["path"] == "translation" {
                    &motion["times_seconds"]
                } else {
                    continue;
                };
                assert_eq!(times, &track["times_seconds"]);
            }
        }
    }
    #[test]
    fn rejects_ambiguous_references_bad_conventions_and_inconsistent_motion() {
        for rigid in [false, true] {
            let glb = fixture(rigid);
            for (pointer, value) in [
                ("/extras/gcad_root_motion/schema", json!(99)),
                ("/extras/gcad_root_motion/length_unit", json!("millimetre")),
                ("/extras/gcad_root_motion/node", json!(1)),
                ("/extras/gcad_root_motion/in_place_animation", json!(0)),
                ("/extras/gcad_root_motion/duration_seconds", json!(-1)),
                ("/animations/1/name", json!("travel")),
            ] {
                assert!(
                    create(&change(&glb, pointer, value), &"ab".repeat(32)).is_err(),
                    "{pointer}"
                );
            }
            let (pointer, value) = if rigid {
                (
                    "/extras/gcad_root_motion/loop_transform/translation/0",
                    json!(99),
                )
            } else {
                ("/extras/gcad_root_motion/loop_displacement/0", json!(99))
            };
            assert!(create(&change(&glb, pointer, value), &"ab".repeat(32)).is_err());
            let pointer = if rigid {
                "/extras/gcad_root_motion/channels/0/times_seconds/1"
            } else {
                "/extras/gcad_root_motion/times_seconds/1"
            };
            assert!(create(&change(&glb, pointer, json!(0)), &"ab".repeat(32)).is_err());
        }
        assert!(create(b"bad GLB", &"ab".repeat(32)).is_err());
    }
}
