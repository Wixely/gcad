//! Export readiness is determined by the actual encoder, not a parallel validator.
use super::{glb, scene::Scene, skin::Skin};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Serialize)]
pub struct Report {
    pub version: u32,
    pub format: &'static str,
    pub status: &'static str,
    pub exportable: bool,
    pub engine_verified: bool,
    pub encoded_bytes: Option<usize>,
    /// First encoder failure; not an exhaustive list of invalid inputs.
    pub error: Option<String>,
    /// Present only after successful encoding; describes the destination artifact.
    pub artifact: Option<Value>,
    pub limitations: Vec<&'static str>,
    pub other_formats: Value,
}

/// Performs a full ordinary GLB encode in memory, including PNG compression.
/// Does not write a file, extract root motion, or certify rendering quality.
pub fn inspect(scene: &Scene, skins: &[Skin]) -> Report {
    let mut report = Report {
        version: 1,
        format: "glb-2.0",
        status: "experimental",
        exportable: false,
        engine_verified: false,
        encoded_bytes: None,
        error: None,
        artifact: None,
        limitations: vec![
            "Encoding success does not certify target-engine import or visual quality.",
            "Only core metallic-roughness PBR; unsupported CAD finish fields fail during authoring.",
            "One UV set; textured primitives require UVs and normal maps require tangents.",
            "Tangents are derivative-based, not MikkTSpace; generated mipmaps are not embedded.",
            "Animation uses STEP or LINEAR TRS; skinning supports at most four influences per vertex.",
            "Root-motion extraction requires asset-root-motion; custom extras require consumer integration.",
            "This checks ordinary export after authoring; it does not test GPU preview or runtime shaders.",
        ],
        other_formats: json!({"fbx": {
            "status": "isolated-native-prototype",
            "production_export_available": false,
            "engine_verified": false
        }}),
    };
    match glb::encode_skinned_scene(scene, skins) {
        Err(error) => report.error = Some(format!("{error:#}")),
        Ok(bytes) => {
            // GLB bytes come directly from our encoder, whose first chunk is JSON.
            let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
            let document: Value = serde_json::from_slice(&bytes[20..20 + length])
                .expect("encoder must emit valid JSON");
            let entries = |key: &str| document[key].as_array().cloned().unwrap_or_default();
            let materials: Vec<_> = entries("materials").iter().enumerate().map(|(index, m)| {
                json!({
                    "index": index,
                    "base_colour_texture": m["pbrMetallicRoughness"].get("baseColorTexture").is_some(),
                    "metallic_roughness_texture": m["pbrMetallicRoughness"].get("metallicRoughnessTexture").is_some(),
                    "normal_texture": m.get("normalTexture").is_some(),
                    "occlusion_texture": m.get("occlusionTexture").is_some(),
                    "emissive_texture": m.get("emissiveTexture").is_some(),
                    "alpha_mode": m.get("alphaMode").cloned().unwrap_or(json!("OPAQUE")),
                    "double_sided": m.get("doubleSided").cloned().unwrap_or(json!(false)),
                    "texture_source_inputs": m["extras"].get("gcad_texture_source_inputs").cloned().unwrap_or(json!([])),
                })
            }).collect();
            // Count actual encoded buffer views once per clip, not source f64 data.
            // These payloads exclude JSON, alignment padding and container overhead.
            let animation_payloads: Vec<_> = entries("animations").iter().enumerate().map(|(index, clip)| {
                let mut views = std::collections::BTreeSet::new();
                for sampler in clip["samplers"].as_array().expect("encoded samplers") {
                    for key in ["input", "output"] {
                        let accessor = sampler[key].as_u64().expect("encoded accessor") as usize;
                        views.insert(document["accessors"][accessor]["bufferView"].as_u64().expect("encoded buffer view") as usize);
                    }
                }
                let bytes: u64 = views.iter().map(|&view| document["bufferViews"][view]["byteLength"].as_u64().expect("encoded view length")).sum();
                json!({"index": index, "name": clip["name"], "buffer_view_payload_bytes": bytes})
            }).collect();
            report.artifact = Some(json!({
                "shader_fallbacks": document["extras"].get("gcad_shader_fallbacks").cloned().unwrap_or(json!([])),
                "coordinates": {"unit": "metre", "up": "Y", "handedness": "right", "source_to_destination": "(x,z,-y)/1000"},
                "nodes": entries("nodes").len(),
                "meshes": entries("meshes").len(),
                "skins": entries("skins").len(),
                "animations": entries("animations").len(),
                "animation_payloads": animation_payloads,
                "animation_payload_scope": "Unique input/output buffer views per clip; excludes JSON, alignment padding and GLB overhead. Not runtime memory.",
                "embedded_png_images": entries("images").len(),
                "materials": materials,
                "samplers": entries("samplers"),
                "root_motion_metadata": false,
                "conversions": ["float64 to float32 attributes", "roughness red to green; metallic red to blue", "negative-Y normal maps converted to positive-Y", "base-colour and emissive images remain sRGB; numeric maps remain linear"]
            }));
            report.exportable = true;
            report.encoded_bytes = Some(bytes.len());
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_payloads_measure_encoded_float32_channels_separately() {
        use crate::asset::animation::{Clip, Interpolation, Track, Values};
        let (mut scene, _) = crate::asset::skin::tests::fixture();
        scene.clips = vec![
            Clip { name: "move-and-turn".into(), tracks: vec![
                Track { node: 0, times: vec![0., 1.], interpolation: Interpolation::Linear,
                    values: Values::Translation(vec![[0.; 3], [1.; 3]]) },
                Track { node: 1, times: vec![0., 1.], interpolation: Interpolation::Step,
                    values: Values::Rotation(vec![[0., 0., 0., 1.]; 2]) },
            ] },
            Clip { name: "scale".into(), tracks: vec![
                Track { node: 0, times: vec![0., 0.5, 1.], interpolation: Interpolation::Linear,
                    values: Values::Scale(vec![[1.; 3], [2.; 3], [1.; 3]]) },
            ] },
        ];
        let report = inspect(&scene, &[]);
        assert!(report.exportable, "{:?}", report.error);
        let artifact = report.artifact.unwrap();
        let clips = artifact["animation_payloads"].as_array().unwrap();
        assert_eq!(clips.len(), 2);
        // Two timestamp arrays + two vec3 translations + two quaternions, all f32.
        assert_eq!(clips[0]["buffer_view_payload_bytes"], 72);
        assert_eq!(clips[0]["name"], "move-and-turn");
        // Three timestamps and three vec3 scales, all f32.
        assert_eq!(clips[1]["buffer_view_payload_bytes"], 48);
        assert!(report.encoded_bytes.unwrap() > 120);
        scene.clips.clear();
        assert_eq!(inspect(&scene, &[]).artifact.unwrap()["animation_payloads"], json!([]));
    }

    #[test]
    fn reports_actual_skin_encoding_and_destination_mesh_variants() {
        let (mut scene, weights) = crate::asset::skin::tests::fixture();
        scene.nodes.push(crate::asset::scene::Node {
            name: "unskinned".into(),
            mesh: Some(0),
            ..Default::default()
        });
        let skin = Skin::bind(&scene, 0, vec![1, 2], weights).unwrap();
        let bytes = glb::encode_skinned_scene(&scene, &[skin.clone()]).unwrap();
        let report = inspect(&scene, &[skin]);
        assert!(report.exportable);
        assert!(!report.engine_verified);
        assert_eq!(report.encoded_bytes, Some(bytes.len()));
        let artifact = report.artifact.unwrap();
        assert_eq!(artifact["meshes"], 2);
        assert_eq!(artifact["skins"], 1);
        assert_eq!(artifact["root_motion_metadata"], false);
    }

    #[test]
    fn material_report_tracks_packed_images_and_rejects_missing_uvs() {
        let (mut scene, _) = crate::asset::skin::tests::fixture();
        let map = crate::model::texture::Map {
            width: 1,
            height: 1,
            rgba: vec![128, 128, 255, 255],
        };
        let material = &mut scene.materials[0];
        material.colour_map = Some(map.clone());
        material.roughness_map = Some(map.clone());
        material.metallic_map = Some(map.clone());
        material.normal_map = Some(map.clone());
        material.occlusion_map = Some(map.clone());
        material.emissive_map = Some(map);
        material.alpha = super::super::material::AlphaMode::Blend;
        material.double_sided = true;
        let report = inspect(&scene, &[]);
        assert!(report.exportable, "{:?}", report.error);
        let artifact = report.artifact.unwrap();
        assert_eq!(artifact["embedded_png_images"], 5);
        let m = &artifact["materials"][0];
        for key in [
            "base_colour_texture",
            "metallic_roughness_texture",
            "normal_texture",
            "occlusion_texture",
            "emissive_texture",
            "double_sided",
        ] {
            assert_eq!(m[key], true, "{key}");
        }
        assert_eq!(m["alpha_mode"], "BLEND");
        scene.meshes[0].primitives[0].geometry.texcoords = None;
        scene.meshes[0].primitives[0].geometry.tangents = None;
        let report = inspect(&scene, &[]);
        assert!(!report.exportable);
        assert!(report.error.is_some());
        assert!(report.artifact.is_none());
    }

    #[test]
    fn encoder_failures_are_reported_without_partial_success() {
        let (mut scene, _) = crate::asset::skin::tests::fixture();
        // Valid f64 source coordinates that cannot survive destination f32.
        for position in &mut scene.meshes[0].primitives[0].geometry.positions {
            position[0] += 1e12;
        }
        let error = glb::encode_skinned_scene(&scene, &[]).unwrap_err();
        let report = inspect(&scene, &[]);
        assert!(!report.exportable);
        assert_eq!(report.error.unwrap(), format!("{error:#}"));
        assert!(report.encoded_bytes.is_none());
        assert!(report.artifact.is_none());
    }
}
