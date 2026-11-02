//! Opt-in UNORM16 skin weights, checked against the original at declared samples.
//! This measures weight error only, not other float32 export or engine errors.
use super::{scene::Scene, skin::Skin};
use anyhow::{Result, ensure};

pub struct Options {
    pub samples_per_clip: usize,
    pub max_error_mm: f64,
    pub max_angle_degrees: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct Report {
    pub sample_times_seconds: Vec<Vec<f64>>,
    pub rest_pose_checked: bool,
    pub max_position_error_mm: f64,
    pub max_direction_error_degrees: f64,
    pub weight_payload_bytes_before: usize,
    pub weight_payload_bytes_after: usize,
}

// Largest remainder allocation: exact integer sum, stable slot tie breaking,
// and no introduction of an influence into a zero-weight slot.
fn integers(weights: [f64; 4]) -> [u16; 4] {
    let sum: f64 = weights.iter().sum();
    let scaled = weights.map(|v| v / sum * 65535.0);
    let mut result = scaled.map(|v| v.floor() as u16);
    let mut order = [0, 1, 2, 3];
    order.sort_by(|&a, &b| {
        (scaled[b] - scaled[b].floor())
            .total_cmp(&(scaled[a] - scaled[a].floor()))
            .then(a.cmp(&b))
    });
    let remaining = 65535 - result.iter().map(|&v| u32::from(v)).sum::<u32>();
    for &slot in order.iter().take(remaining as usize) {
        result[slot] += 1;
    }
    result
}

/// Encode without mutating source weights. Rest pose, a uniform grid, every
/// authored key and STEP left limits are checked for every clip. These are
/// sampled bounds, not a guarantee between samples or in a target engine.
pub fn encode(scene: &Scene, skins: &[Skin], options: &Options) -> Result<(Vec<u8>, Report)> {
    scene.validate()?;
    ensure!(!skins.is_empty(), "weight quantisation requires a skin");
    ensure!(
        (2..=10001).contains(&options.samples_per_clip),
        "samples must be between 2 and 10001"
    );
    ensure!(
        options.max_error_mm.is_finite()
            && options.max_error_mm >= 0.0
            && options.max_angle_degrees.is_finite()
            && (0.0..=180.0).contains(&options.max_angle_degrees),
        "quantisation tolerances must be finite and nonnegative (angle at most 180 degrees)"
    );
    let mut nodes = std::collections::HashSet::new();
    let mut derived = skins.to_vec();
    let mut vertices = 0usize;
    for skin in &mut derived {
        skin.validate(scene)?;
        ensure!(nodes.insert(skin.mesh_node), "duplicate skin mesh node");
        for primitive in &mut skin.weights {
            vertices = vertices
                .checked_add(primitive.len())
                .ok_or_else(|| anyhow::anyhow!("weight count overflow"))?;
            for vertex in primitive {
                vertex.weights = integers(vertex.weights).map(|v| f64::from(v) / 65535.0);
            }
        }
        skin.validate(scene)?;
    }
    let times: Vec<_> = scene
        .clips
        .iter()
        .map(|clip| super::report::sample_times(clip, options.samples_per_clip))
        .collect();
    ensure!(
        times.iter().map(Vec::len).sum::<usize>() <= 10000,
        "quantisation exceeds 10000 animation samples"
    );
    let mut report = Report {
        sample_times_seconds: times,
        rest_pose_checked: true,
        max_position_error_mm: 0.0,
        max_direction_error_degrees: 0.0,
        weight_payload_bytes_before: vertices
            .checked_mul(16)
            .ok_or_else(|| anyhow::anyhow!("weight payload overflow"))?,
        weight_payload_bytes_after: vertices
            .checked_mul(8)
            .ok_or_else(|| anyhow::anyhow!("weight payload overflow"))?,
    };
    let mut check = |pose: &[super::scene::Transform]| -> Result<()> {
        let original = super::reduce::frame_pose(scene, skins, pose)?;
        let quantized = super::reduce::frame_pose(scene, &derived, pose)?;
        let (distance, angle) = super::reduce::error(&original, &quantized);
        report.max_position_error_mm = report.max_position_error_mm.max(distance);
        report.max_direction_error_degrees = report.max_direction_error_degrees.max(angle);
        ensure!(
            distance <= options.max_error_mm && angle <= options.max_angle_degrees,
            "quantised weights exceed sampled limits: {distance} mm, {angle} degrees"
        );
        Ok(())
    };
    check(&scene.nodes.iter().map(|n| n.transform).collect::<Vec<_>>())?;
    for (clip, times) in scene.clips.iter().zip(&report.sample_times_seconds) {
        for &time in times {
            check(&clip.sample(scene, time, false)?)?;
        }
    }
    let bytes = super::glb::encode_with_weight_storage(scene, &derived, None, true)?;
    Ok((bytes, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::animation::{Clip, Interpolation, Track, Values};

    #[test]
    fn allocation_is_exact_deterministic_and_preserves_zero_slots() {
        assert_eq!(integers([0.5, 0.5, 0., 0.]), [32768, 32767, 0, 0]);
        assert_eq!(integers([0., 0., 1., 0.]), [0, 0, 65535, 0]);
        for i in 0..=10000 {
            let w = i as f64 / 10000.0;
            let q = integers([w, (1. - w) * 0.3, (1. - w) * 0.7, 0.]);
            assert_eq!(q.iter().map(|&v| u32::from(v)).sum::<u32>(), 65535);
            assert_eq!(q[3], 0);
        }
    }

    #[test]
    fn actual_integer_export_checks_motion_and_keeps_original() {
        let (mut scene, weights) = crate::asset::skin::tests::fixture();
        scene.clips.push(Clip {
            name: "step".into(),
            tracks: vec![Track {
                node: 2,
                times: vec![0., 0.37, 1.],
                interpolation: Interpolation::Step,
                values: Values::Translation(vec![[0., 0., 0.], [100., 0., 0.], [200., 0., 0.]]),
            }],
        });
        let skin = Skin::bind(&scene, 0, vec![1, 2], weights).unwrap();
        let before = skin.weight_file().unwrap().to_json().unwrap();
        let options = Options {
            samples_per_clip: 3,
            max_error_mm: 0.1,
            max_angle_degrees: 0.1,
        };
        let (bytes, report) = encode(&scene, &[skin.clone()], &options).unwrap();
        assert!(report.max_position_error_mm > 0.0);
        assert!(report.sample_times_seconds[0].contains(&0.37));
        assert!(report.sample_times_seconds[0].contains(&f64::from_bits(0.37f64.to_bits() - 1)));
        assert_eq!(
            report.weight_payload_bytes_before,
            report.weight_payload_bytes_after * 2
        );
        assert_eq!(before, skin.weight_file().unwrap().to_json().unwrap());
        assert!(
            encode(
                &scene,
                &[skin.clone()],
                &Options {
                    max_error_mm: 0.,
                    ..options
                }
            )
            .is_err()
        );
        let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let doc: serde_json::Value = serde_json::from_slice(&bytes[20..20 + length]).unwrap();
        let accessor = &doc["accessors"]
            [doc["meshes"][1]["primitives"][0]["attributes"]["WEIGHTS_0"]
                .as_u64()
                .unwrap() as usize];
        assert_eq!(accessor["componentType"], 5123);
        assert_eq!(accessor["normalized"], true);
        let view = &doc["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
        let offset = 28 + length + view["byteOffset"].as_u64().unwrap_or(0) as usize;
        for vertex in
            bytes[offset..offset + view["byteLength"].as_u64().unwrap() as usize].chunks_exact(8)
        {
            assert_eq!(
                vertex
                    .chunks_exact(2)
                    .map(|v| u32::from(u16::from_le_bytes(v.try_into().unwrap())))
                    .sum::<u32>(),
                65535
            );
        }
        assert!(
            bytes.len()
                < super::super::glb::encode_skinned_scene(&scene, &[skin])
                    .unwrap()
                    .len()
        );
    }
}
