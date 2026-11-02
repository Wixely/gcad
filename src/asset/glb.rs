//! Experimental static scene GLB adapter, used by opt-in `.gasset` export.
use super::Mesh;
use super::material::AlphaMode;
pub use super::material::Material;
use super::material::png;
#[cfg(test)]
use crate::model::texture::Map;
use anyhow::{Result, ensure};
use serde_json::{Value, json};

#[derive(Default)]
struct Buffer {
    bytes: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Buffer {
    fn view(&mut self, bytes: &[u8], target: Option<u32>) -> usize {
        while self.bytes.len() % 4 != 0 {
            self.bytes.push(0);
        }
        let mut view =
            json!({"buffer": 0, "byteOffset": self.bytes.len(), "byteLength": bytes.len()});
        if let Some(target) = target {
            view["target"] = json!(target);
        }
        self.bytes.extend_from_slice(bytes);
        let index = self.views.len();
        self.views.push(view);
        index
    }

    fn floats<const N: usize>(
        &mut self,
        values: &[[f64; N]],
        kind: &str,
        bounds: bool,
    ) -> Result<usize> {
        let mut bytes = Vec::new();
        let mut min = [f32::INFINITY; N];
        let mut max = [f32::NEG_INFINITY; N];
        for value in values {
            for k in 0..N {
                let v = value[k] as f32;
                ensure!(v.is_finite(), "attribute exceeds float32 range");
                min[k] = min[k].min(v);
                max[k] = max[k].max(v);
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }
        let view = self.view(&bytes, Some(34962));
        let mut accessor =
            json!({"bufferView": view, "componentType": 5126, "count": values.len(), "type": kind});
        if bounds {
            accessor["min"] = json!(min.as_slice());
            accessor["max"] = json!(max.as_slice());
        }
        let index = self.accessors.len();
        self.accessors.push(accessor);
        Ok(index)
    }
}

fn basis(v: [f64; 3]) -> [f64; 3] {
    [v[0], v[2], -v[1]]
}

fn animation_floats<const N: usize>(
    buffer: &mut Buffer,
    values: &[[f64; N]],
    kind: &str,
    bounds: bool,
) -> Result<usize> {
    let accessor = buffer.floats(values, kind, bounds)?;
    buffer
        .views
        .last_mut()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("target");
    Ok(accessor)
}

fn animations(scene: &super::scene::Scene, buffer: &mut Buffer) -> Result<Vec<Value>> {
    use super::animation::{Interpolation, Values, normalized};
    scene.clips.iter().map(|clip| {
        let mut samplers = Vec::new();
        let mut channels = Vec::new();
        for track in &clip.tracks {
            let times: Vec<_> = track.times.iter().map(|t| [(*t as f32) as f64]).collect();
            ensure!(times.iter().all(|t| t[0].is_finite()) && times.windows(2).all(|t| t[1][0] > t[0][0]), "animation times collapse or overflow at float32 precision");
            let input = animation_floats(buffer, &times, "SCALAR", true)?;
            let output = match &track.values {
                Values::Translation(keys) => {
                    let keys: Vec<_> = keys.iter().map(|&v| basis(v).map(|v|v/1000.0)).collect();
                    animation_floats(buffer, &keys, "VEC3", false)?
                }
                Values::Scale(keys) => {
                    ensure!(keys.iter().flatten().all(|v| (*v as f32) != 0.0), "animation scale collapses at float32 precision");
                    let keys: Vec<_> = keys.iter().map(|v| [v[0],v[2],v[1]]).collect();
                    animation_floats(buffer, &keys, "VEC3", false)?
                }
                Values::Rotation(keys) => {
                    let mut converted = Vec::<[f64;4]>::new();
                    for &q in keys {
                        let q = normalized(q);
                        let mut q = [q[0],q[2],-q[1],q[3]];
                        if converted.last().is_some_and(|previous| previous.iter().zip(q).map(|(a,b)|a*b).sum::<f64>() < 0.0) { q = q.map(|v|-v); }
                        converted.push(q);
                    }
                    animation_floats(buffer, &converted, "VEC4", false)?
                }
            };
            let sampler = samplers.len();
            samplers.push(json!({"input":input,"output":output,"interpolation":match track.interpolation { Interpolation::Step=>"STEP", Interpolation::Linear=>"LINEAR" }}));
            channels.push(json!({"sampler":sampler,"target":{"node":track.node,"path":track.values.path()}}));
        }
        Ok(json!({"name":clip.name,"samplers":samplers,"channels":channels}))
    }).collect()
}

/// Convenience wrapper for a single mesh/material. Use encode_scene for instances.
pub fn encode(mesh: &Mesh, material: &Material) -> Result<Vec<u8>> {
    use super::scene::{Node, Primitive, Scene, SceneMesh};
    encode_scene(&Scene {
        consumer:None,
        nodes: vec![Node {
            name: "mesh".into(),
            mesh: Some(0),
            ..Default::default()
        }],
        meshes: vec![SceneMesh {
            name: "mesh".into(),
            primitives: vec![Primitive {
                geometry: mesh.clone(),
                material: 0,
            }],
        }],
        materials: vec![material.clone()],
        clips: Vec::new(),
    })
}

fn primitive(mesh: &Mesh, material: usize, buffer: &mut Buffer) -> Result<Value> {
    // Validate the rounded destination geometry too: float32 can collapse a
    // triangle whose source coordinates are finite and distinct.
    let rounded = |v: f64| (v as f32) as f64;
    let converted = Mesh {
        positions: mesh
            .positions
            .iter()
            .map(|&p| basis(p).map(|v| rounded(v / 1000.0)))
            .collect(),
        normals: mesh
            .normals
            .iter()
            .map(|&n| basis(n).map(rounded))
            .collect(),
        texcoords: mesh
            .texcoords
            .as_ref()
            .map(|uv| uv.iter().map(|v| v.map(rounded)).collect()),
        tangents: mesh.tangents.as_ref().map(|ts| {
            ts.iter()
                .map(|t| [rounded(t[0]), rounded(t[2]), rounded(-t[1]), t[3]])
                .collect()
        }),
        triangles: mesh.triangles.clone(),
    };
    converted.validate()?;
    let mut attributes = json!({
        "POSITION": buffer.floats(&converted.positions, "VEC3", true)?,
        "NORMAL": buffer.floats(&converted.normals, "VEC3", false)?,
    });
    if let Some(uv) = &converted.texcoords {
        attributes["TEXCOORD_0"] = json!(buffer.floats(uv, "VEC2", false)?);
    }
    if let Some(t) = &converted.tangents {
        attributes["TANGENT"] = json!(buffer.floats(t, "VEC4", false)?);
    }
    let index_bytes: Vec<u8> = mesh
        .triangles
        .iter()
        .flatten()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    let view = buffer.view(&index_bytes, Some(34963));
    let indices = buffer.accessors.len();
    buffer.accessors.push(json!({"bufferView": view, "componentType": 5125, "count": index_bytes.len() / 4, "type": "SCALAR"}));

    Ok(json!({"attributes": attributes, "indices": indices, "material": material, "mode": 4}))
}

fn write_material(
    material: &Material,
    buffer: &mut Buffer,
    images: &mut Vec<Value>,
    textures: &mut Vec<Value>,
    samplers: &mut Vec<Value>,
) -> Result<Value> {
    ensure!(material.shader.is_none(), "shader export requires an explicit implemented fallback or dynamic adapter; refusing to drop the effect");
    let sampler_index = samplers.len();
    ensure!(
        material.mip_chain.is_none(),
        "GLB export does not yet transport explicit mip chains"
    );
    if material.has_textures() {
        let sampler = material.sampler;
        samplers.push(json!({"magFilter": sampler.magnification(), "minFilter": sampler.minification(), "wrapS": sampler.wrap_u.gltf(), "wrapT": sampler.wrap_v.gltf()}));
    }
    let mut pbr = json!({"baseColorFactor": material.base_colour, "metallicFactor": material.metallic, "roughnessFactor": material.roughness});
    let packed = material.packed_metal_rough()?;
    let normal = material.portable_normal()?;
    let mut material_json = json!({
        "emissiveFactor": material.emissive, "doubleSided": material.double_sided,
        "alphaMode": match material.alpha { AlphaMode::Opaque => "OPAQUE", AlphaMode::Mask { .. } => "MASK", AlphaMode::Blend => "BLEND" },
    });
    if let AlphaMode::Mask { cutoff } = material.alpha {
        material_json["alphaCutoff"] = json!(cutoff);
    }
    if !material.texture_sources.is_empty() {
        material_json["extras"] = json!({"gcad_texture_source_inputs": material.texture_sources});
    }
    for (key, map) in [
        ("baseColorTexture", material.colour_map.as_ref()),
        ("metallicRoughnessTexture", packed.as_ref()),
        ("normalTexture", normal.as_ref()),
        ("occlusionTexture", material.occlusion_map.as_ref()),
        ("emissiveTexture", material.emissive_map.as_ref()),
    ] {
        if let Some(map) = map {
            let view = buffer.view(&png(map)?, None);
            let index = images.len();
            images.push(json!({"bufferView": view, "mimeType": "image/png"}));
            textures.push(json!({"source": index, "sampler": sampler_index}));
            let mut binding = json!({"index": index});
            match key {
                "normalTexture" => binding["scale"] = json!(material.normal_scale),
                "occlusionTexture" => binding["strength"] = json!(material.occlusion_strength),
                _ => {}
            }
            if key == "baseColorTexture" || key == "metallicRoughnessTexture" {
                pbr[key] = binding;
            } else {
                material_json[key] = binding;
            }
        }
    }
    material_json["pbrMetallicRoughness"] = pbr;

    Ok(material_json)
}

/// Export a validated scene, preserving shared meshes, primitive materials and
/// local TRS. Reflections remain on nodes, as required by glTF winding rules.
/// Every coordinate conversion happens once at this adapter boundary.
pub fn encode_scene(scene: &super::scene::Scene) -> Result<Vec<u8>> {
    encode_skinned_scene(scene, &[])
}

/// Export explicit skin bindings without changing shared unskinned instances.
/// Each bound node gets a mesh variant carrying its own joint/weight attributes.
pub fn encode_skinned_scene(
    scene: &super::scene::Scene,
    bindings: &[super::skin::Skin],
) -> Result<Vec<u8>> {
    encode_with_extras(scene, bindings, None)
}

/// Preserve original clips and append a translation-only in-place variant.
/// Experimental extras use glTF metre/Y-up coordinates, unlike native Motion JSON.
/// Times/duration use the same float32 rounding as animation accessors so STEP
/// channels and extracted motion agree at encoded key boundaries.
pub fn encode_root_motion(
    scene: &super::scene::Scene,
    bindings: &[super::skin::Skin],
    clip: usize,
    node: usize,
    axes: [bool; 3],
    name: &str,
) -> Result<(Vec<u8>, super::root_motion::Motion)> {
    let extracted = super::root_motion::extract(scene, clip, node, axes, name)?;
    let mut derived = scene.clone();
    let in_place = derived.clips.len();
    derived.clips.push(extracted.in_place);
    let motion = &extracted.motion;
    let displacements: Vec<_> = motion
        .displacements_mm
        .iter()
        .map(|&p| basis(p).map(|v| v / 1000.))
        .collect();
    let extras = json!({"gcad_root_motion":{
        "schema":1,"source_animation":clip,"in_place_animation":in_place,"node":node,
        "length_unit":"metre","up_axis":"Y","handedness":"right",
        "translation_axes":[axes[0],axes[2],axes[1]],"duration_seconds":(motion.duration_seconds as f32) as f64,
        "interpolation":motion.interpolation,"times_seconds":motion.times.iter().map(|t|(*t as f32) as f64).collect::<Vec<_>>(),
        "displacements":displacements,"loop_displacement":displacements.last().unwrap(),
        "rotation_extracted":false
    }});
    let bytes = encode_with_extras(&derived, bindings, Some(extras))?;
    Ok((bytes, extracted.motion))
}

/// Full rigid root extraction. Schema 2 retains absolute TR channels because
/// interpolating composed displacement keys would change between-key motion.
pub fn encode_rigid_root_motion(
    scene: &super::scene::Scene,
    bindings: &[super::skin::Skin],
    clip: usize,
    node: usize,
    name: &str,
) -> Result<(Vec<u8>, super::rigid_motion::Motion)> {
    let extracted = super::rigid_motion::extract(scene, clip, node, name)?;
    let mut derived = scene.clone();
    let in_place = derived.clips.len();
    derived.clips.push(extracted.in_place);
    let motion = &extracted.motion;
    let rotation = |q| {
        let [x, y, z, w] = super::animation::normalized(q);
        [x, z, -y, w]
    };
    let pose = |t: super::scene::Transform| json!({"translation":basis(t.translation).map(|v|v/1000.),"rotation":rotation(t.rotation)});
    let anchor = motion.anchor()?;
    let endpoint =
        super::scene::Transform::from_matrix(motion.sample(motion.duration_seconds, false)?)?;
    let channels:Vec<_>=motion.tracks.iter().map(|track| {
        let values=match &track.values {
            super::animation::Values::Translation(values)=>json!(values.iter().map(|&p|basis(p).map(|v|v/1000.)).collect::<Vec<_>>()),
            super::animation::Values::Rotation(values)=>json!(values.iter().copied().map(rotation).collect::<Vec<_>>()),
            super::animation::Values::Scale(_)=>unreachable!("validated rigid motion has no scale"),
        };
        json!({"path":track.values.path(),"times_seconds":track.times.iter().map(|t|(*t as f32) as f64).collect::<Vec<_>>(),"interpolation":track.interpolation,"values":values})
    }).collect();
    let extras = json!({"gcad_root_motion":{
        "schema":2,"mode":"rigid","source_animation":clip,"in_place_animation":in_place,"node":node,
        "length_unit":"metre","up_axis":"Y","handedness":"right","duration_seconds":(motion.duration_seconds as f32) as f64,
        "rotation_extracted":true,"translation_axes":[true,true,true],"anchor":pose(anchor),"channels":channels,
        "loop_transform":pose(endpoint),"composition":"left-multiply-in-place-root",
        "loop_composition":"endpoint-power-times-phase","max_accumulated_cycles":1_000_000
    }});
    let bytes = encode_with_extras(&derived, bindings, Some(extras))?;
    Ok((bytes, extracted.motion))
}

fn encode_with_extras(
    scene: &super::scene::Scene,
    bindings: &[super::skin::Skin],
    extras: Option<Value>,
) -> Result<Vec<u8>> {
    encode_with_weight_storage(scene, bindings, extras, false)
}

pub(crate) fn encode_with_weight_storage(
    scene: &super::scene::Scene,
    bindings: &[super::skin::Skin],
    extras: Option<Value>,
    unorm16: bool,
) -> Result<Vec<u8>> {
    if scene.materials.iter().any(|material| material.shader.is_some()) {
        let (derived,skins,report)=super::shader::freeze_scene(scene,bindings)?;
        let mut extras=extras.unwrap_or_else(|| json!({}));
        ensure!(extras.is_object(), "scene extras must be an object");
        extras["gcad_shader_fallbacks"]=report;
        return encode_with_weight_storage(&derived,&skins,Some(extras),unorm16);
    }
    scene.validate()?;
    let mut bound_nodes = std::collections::HashSet::new();
    for skin in bindings {
        skin.validate(scene)?;
        ensure!(
            bound_nodes.insert(skin.mesh_node),
            "mesh node has multiple skin bindings"
        );
    }
    let mut buffer = Buffer::default();
    let mut meshes = Vec::new();
    for mesh in &scene.meshes {
        let primitives = mesh
            .primitives
            .iter()
            .map(|p| primitive(&p.geometry, p.material, &mut buffer))
            .collect::<Result<Vec<_>>>()?;
        meshes.push(json!({"name": mesh.name, "primitives": primitives}));
    }
    let mut images = Vec::new();
    let mut textures = Vec::new();
    let mut samplers = Vec::new();
    let mut materials = Vec::new();
    for material in &scene.materials {
        materials.push(write_material(
            material,
            &mut buffer,
            &mut images,
            &mut textures,
            &mut samplers,
        )?);
    }
    let mut nodes = Vec::new();
    let mut roots = Vec::new();
    let mut children = vec![Vec::new(); scene.nodes.len()];
    for (i, node) in scene.nodes.iter().enumerate() {
        if let Some(parent) = node.parent {
            children[parent].push(i);
        } else {
            roots.push(i);
        }
    }
    for (i, node) in scene.nodes.iter().enumerate() {
        let t = node.transform;
        let translation = basis(t.translation).map(|v| v / 1000.0);
        let q = t.normalized_rotation();
        let rotation = [q[0], q[2], -q[1], q[3]];
        let scale = [t.scale[0], t.scale[2], t.scale[1]];
        ensure!(
            translation
                .iter()
                .chain(&rotation)
                .chain(&scale)
                .all(|v| (*v as f32).is_finite()),
            "node transform exceeds float32 range"
        );
        ensure!(
            scale.iter().all(|v| (*v as f32) != 0.0),
            "node scale collapses at float32 precision"
        );
        let mut value = json!({"name": node.name, "translation": translation, "rotation": rotation, "scale": scale});
        if let Some(mesh) = node.mesh {
            value["mesh"] = json!(mesh);
        }
        if !children[i].is_empty() {
            value["children"] = json!(children[i]);
        }
        nodes.push(value);
    }
    let mut skins = Vec::new();
    for skin in bindings {
        use monstertruck::modeling::{Matrix4, SquareMatrix};
        let source = &scene.meshes[scene.nodes[skin.mesh_node].mesh.unwrap()];
        let mut primitives = Vec::new();
        for (p, weights) in source.primitives.iter().zip(&skin.weights) {
            let mut value = primitive(&p.geometry, p.material, &mut buffer)?;
            let bytes: Vec<_> = weights
                .iter()
                .flat_map(|w| w.joints)
                .flat_map(u16::to_le_bytes)
                .collect();
            let view = buffer.view(&bytes, Some(34962));
            let joints = buffer.accessors.len();
            buffer.accessors.push(
                json!({"bufferView":view,"componentType":5123,"count":weights.len(),"type":"VEC4"}),
            );
            let values: Vec<_> = weights.iter().map(|w| w.weights).collect();
            let weights = if unorm16 {
                ensure!(
                    values.iter().all(|v| {
                        v.iter()
                            .all(|w| (*w * 65535.0 - (*w * 65535.0).round()).abs() < 1e-8)
                            && v.iter().map(|w| (*w * 65535.0).round() as u32).sum::<u32>() == 65535
                    }),
                    "UNORM16 export requires validated quantised weights"
                );
                let bytes: Vec<_> = values
                    .iter()
                    .flatten()
                    .flat_map(|v| ((*v * 65535.0).round() as u16).to_le_bytes())
                    .collect();
                let view = buffer.view(&bytes, Some(34962));
                let index = buffer.accessors.len();
                buffer
                    .accessors
                    .push(json!({"bufferView":view,"componentType":5123,
                    "normalized":true,"count":values.len(),"type":"VEC4"}));
                index
            } else {
                buffer.floats(&values, "VEC4", false)?
            };
            value["attributes"]["JOINTS_0"] = json!(joints);
            value["attributes"]["WEIGHTS_0"] = json!(weights);
            primitives.push(value);
        }
        // Conjugate by the exact same mm/Z-up -> metre/Y-up basis as geometry.
        let basis = Matrix4::new(
            0.001, 0.0, 0.0, 0.0, 0.0, 0.0, -0.001, 0.0, 0.0, 0.001, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        );
        let inverse = basis.invert().unwrap();
        let matrices = skin
            .inverse_bind
            .iter()
            .map(|bind| {
                let matrix = basis * bind * inverse;
                let mut rounded = matrix;
                let values: &mut [[f64; 4]; 4] = rounded.as_mut();
                for value in values.iter_mut().flatten() {
                    *value = (*value as f32) as f64;
                }
                ensure!(
                    rounded.determinant().is_finite() && rounded.determinant() != 0.0,
                    "inverse bind collapses at float32 precision"
                );
                let columns: &[[f64; 4]; 4] = rounded.as_ref();
                Ok(std::array::from_fn::<_, 16, _>(|i| columns[i / 4][i % 4]))
            })
            .collect::<Result<Vec<_>>>()?;
        let inverse_binds = animation_floats(&mut buffer, &matrices, "MAT4", false)?;
        nodes[skin.mesh_node]["skin"] = json!(skins.len());
        nodes[skin.mesh_node]["mesh"] = json!(meshes.len());
        meshes.push(json!({"name":format!("{}_skinned",source.name),"primitives":primitives}));
        skins.push(json!({"name":format!("{}_skin",scene.nodes[skin.mesh_node].name),"joints":skin.joints,"inverseBindMatrices":inverse_binds}));
    }
    let animations = animations(scene, &mut buffer)?;
    let mut document = json!({
        "asset": {"version": "2.0", "generator": "gcad experimental game-assets"},
        "scene": 0, "scenes": [{"nodes": roots}], "nodes": nodes, "meshes": meshes,
        "materials": materials, "accessors": buffer.accessors, "bufferViews": buffer.views,
        "buffers": [{"byteLength": buffer.bytes.len()}],
    });
    if let Some(extras) = extras {
        document["extras"] = extras;
    }
    if let Some(profile)=&scene.consumer {
        profile.validate_skins(scene,bindings)?;
        if document.get("extras").is_none(){document["extras"]=json!({});}
        document["extras"]["gcad_consumer_profile"]=serde_json::to_value(profile)?;
    }
    if !animations.is_empty() {
        document["animations"] = json!(animations);
    }
    if !skins.is_empty() {
        document["skins"] = json!(skins);
    }
    if !images.is_empty() {
        document["images"] = json!(images);
        document["textures"] = json!(textures);
        document["samplers"] = json!(samplers);
    }
    let mut json_bytes = serde_json::to_vec(&document)?;
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    while buffer.bytes.len() % 4 != 0 {
        buffer.bytes.push(0);
    }
    let total = 28_u64 + json_bytes.len() as u64 + buffer.bytes.len() as u64;
    let total = u32::try_from(total)?;
    let mut result = Vec::new();
    for word in [
        0x46546c67,
        2,
        total,
        u32::try_from(json_bytes.len())?,
        0x4e4f534a,
    ] {
        result.extend_from_slice(&word.to_le_bytes());
    }
    result.extend_from_slice(&json_bytes);
    for word in [u32::try_from(buffer.bytes.len())?, 0x004e4942] {
        result.extend_from_slice(&word.to_le_bytes());
    }
    result.extend_from_slice(&buffer.bytes);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skin_exports_joint_weights_and_converted_inverse_binds_without_skinning_other_instances() {
        let (mut scene, weights) = crate::asset::skin::tests::fixture();
        scene.nodes.push(crate::asset::scene::Node {
            name: "unskinned".into(),
            mesh: Some(0),
            ..Default::default()
        });
        let skin = crate::asset::skin::Skin::bind(&scene, 0, vec![1, 2], weights).unwrap();
        let bytes = encode_skinned_scene(&scene, &[skin.clone()]).unwrap();
        let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let doc: Value = serde_json::from_slice(&bytes[20..20 + length]).unwrap();
        assert_eq!(doc["skins"][0]["joints"], json!([1, 2]));
        assert_eq!(doc["nodes"][0]["skin"], 0);
        assert_eq!(doc["nodes"][0]["mesh"], 1);
        assert_eq!(doc["nodes"][3]["mesh"], 0);
        assert!(doc["nodes"][3].get("skin").is_none());
        assert!(
            doc["meshes"][0]["primitives"][0]["attributes"]
                .get("JOINTS_0")
                .is_none()
        );
        let data = |index: &Value| {
            let a = &doc["accessors"][index.as_u64().unwrap() as usize];
            let v = &doc["bufferViews"][a["bufferView"].as_u64().unwrap() as usize];
            let start = 28 + length + v["byteOffset"].as_u64().unwrap() as usize;
            &bytes[start..start + v["byteLength"].as_u64().unwrap() as usize]
        };
        let attrs = &doc["meshes"][1]["primitives"][0]["attributes"];
        let joints: Vec<_> = data(&attrs["JOINTS_0"])
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
            .collect();
        assert_eq!(joints, vec![0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0]);
        let weights: Vec<_> = data(&attrs["WEIGHTS_0"])
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        assert_eq!(
            weights,
            vec![1.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]
        );
        let matrices: Vec<_> = data(&doc["skins"][0]["inverseBindMatrices"])
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        assert_eq!(matrices.len(), 32);
        assert_eq!(&matrices[28..32], &[0.0, 0.0, 1.0, 1.0]);
        assert!(encode_skinned_scene(&scene, &[skin.clone(), skin]).is_err());
    }

    #[test]
    fn animation_channels_convert_units_axes_and_reject_collapsed_times() {
        use crate::asset::animation::{Clip, Interpolation, Track, Values};
        let mut scene = crate::asset::scene::tests::fixture();
        let q = std::f64::consts::FRAC_1_SQRT_2;
        scene.clips.push(Clip {
            name: "move".into(),
            tracks: vec![
                Track {
                    node: 0,
                    times: vec![0.0, 2.0],
                    interpolation: Interpolation::Linear,
                    values: Values::Translation(vec![[0.0; 3], [1000.0, 2000.0, 3000.0]]),
                },
                Track {
                    node: 1,
                    times: vec![0.0, 2.0],
                    interpolation: Interpolation::Linear,
                    values: Values::Rotation(vec![[0.0, 0.0, 0.0, 1.0], [0.0, 0.0, -q, -q]]),
                },
            ],
        });
        scene.clips.push(Clip {
            name: "resize".into(),
            tracks: vec![Track {
                node: 0,
                times: vec![0.0, 1.0],
                interpolation: Interpolation::Step,
                values: Values::Scale(vec![[1.0; 3], [-2.0, 3.0, 4.0]]),
            }],
        });
        let bytes = encode_scene(&scene).unwrap();
        let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let doc: Value = serde_json::from_slice(&bytes[20..20 + length]).unwrap();
        assert_eq!(doc["animations"].as_array().unwrap().len(), 2);
        let read = |accessor: &Value| {
            let a = &doc["accessors"][accessor.as_u64().unwrap() as usize];
            let view = &doc["bufferViews"][a["bufferView"].as_u64().unwrap() as usize];
            assert!(view.get("target").is_none());
            let start = 28 + length + view["byteOffset"].as_u64().unwrap() as usize;
            let len = view["byteLength"].as_u64().unwrap() as usize;
            bytes[start..start + len]
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .collect::<Vec<_>>()
        };
        let samplers = &doc["animations"][0]["samplers"];
        assert_eq!(read(&samplers[0]["input"]), vec![0.0, 2.0]);
        assert_eq!(
            read(&samplers[0]["output"]),
            vec![0.0, 0.0, -0.0, 1.0, 3.0, -2.0]
        );
        let rotation = read(&samplers[1]["output"]);
        assert!((rotation[5] - q as f32).abs() < 1e-6 && rotation[7] > 0.0);
        assert_eq!(doc["animations"][0]["channels"][1]["target"]["node"], 1);
        assert_eq!(doc["animations"][1]["samplers"][0]["interpolation"], "STEP");
        assert_eq!(
            read(&doc["animations"][1]["samplers"][0]["output"]),
            vec![1.0, 1.0, 1.0, -2.0, 4.0, 3.0]
        );
        scene.clips[0].tracks[0].times = vec![1.0, 1.0 + 1e-10];
        assert!(encode_scene(&scene).is_err());
    }

    #[test]
    fn scene_keeps_instances_material_slots_and_converts_local_trs_once() {
        use crate::asset::scene::{Node, Primitive};
        let mut scene = crate::asset::scene::tests::fixture();
        scene.materials.push(Material::default());
        scene.meshes[0].primitives.push(Primitive {
            geometry: super::super::tests::triangle(),
            material: 1,
        });
        scene.nodes.push(Node {
            name: "copy".into(),
            mesh: Some(0),
            ..Default::default()
        });
        let bytes = encode_scene(&scene).unwrap();
        let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let doc: Value = serde_json::from_slice(&bytes[20..20 + length]).unwrap();
        assert_eq!(doc["meshes"].as_array().unwrap().len(), 1);
        assert_eq!(doc["meshes"][0]["primitives"][1]["material"], 1);
        assert_eq!(doc["nodes"][0]["mesh"], doc["nodes"][2]["mesh"]);
        assert_eq!(doc["nodes"][0]["translation"], json!([1.0, 0.0, 0.0]));
        assert_eq!(doc["nodes"][0]["scale"], json!([-2.0, 4.0, 3.0]));
        assert_eq!(doc["nodes"][1]["translation"], json!([0.0, 0.0, -2.0]));
        assert_eq!(doc["nodes"][1]["children"], json!([0]));
        assert_eq!(doc["scenes"][0]["nodes"], json!([1, 2]));
        assert!(
            (doc["nodes"][1]["rotation"][1].as_f64().unwrap() - std::f64::consts::FRAC_1_SQRT_2)
                .abs()
                < 1e-12
        );
        scene.nodes[0].transform.scale[0] = 1e-50;
        assert!(encode_scene(&scene).is_err());
    }

    #[test]
    fn extended_materials_keep_bindings_alpha_and_sampler_settings() {
        use crate::asset::material::{Filter, NormalConvention, Sampler, Wrap};
        let map = Map {
            width: 1,
            height: 1,
            rgba: vec![120, 40, 240, 128],
        };
        for alpha in [
            AlphaMode::Opaque,
            AlphaMode::Mask { cutoff: 0.3 },
            AlphaMode::Blend,
        ] {
            let material = Material {
                normal_map: Some(map.clone()),
                normal_convention: NormalConvention::NegativeY,
                normal_scale: 0.7,
                occlusion_map: Some(map.clone()),
                occlusion_strength: 0.4,
                emissive: [0.2, 0.3, 0.4],
                emissive_map: Some(map.clone()),
                alpha,
                double_sided: true,
                sampler: Sampler {
                    wrap_u: Wrap::Clamp,
                    wrap_v: Wrap::Mirror,
                    magnify: Filter::Nearest,
                    minify: Filter::Nearest,
                    mipmap: None,
                },
                ..Material::default()
            };
            let bytes = encode(&super::super::tests::triangle(), &material).unwrap();
            let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
            let doc: Value = serde_json::from_slice(&bytes[20..20 + length]).unwrap();
            let m = &doc["materials"][0];
            assert_eq!(m["normalTexture"], json!({"index":0,"scale":0.7}));
            assert_eq!(m["occlusionTexture"], json!({"index":1,"strength":0.4}));
            assert_eq!(m["emissiveTexture"]["index"], 2);
            assert_eq!(m["emissiveFactor"], json!([0.2, 0.3, 0.4]));
            assert_eq!(m["doubleSided"], true);
            assert_eq!(
                m["alphaMode"],
                match alpha {
                    AlphaMode::Opaque => "OPAQUE",
                    AlphaMode::Mask { .. } => "MASK",
                    AlphaMode::Blend => "BLEND",
                }
            );
            assert_eq!(
                m.get("alphaCutoff").cloned(),
                if matches!(alpha, AlphaMode::Mask { .. }) {
                    Some(json!(0.3))
                } else {
                    None
                }
            );
            assert_eq!(
                doc["samplers"][0],
                json!({"wrapS":33071,"wrapT":33648,"magFilter":9728,"minFilter":9728})
            );
            let view =
                &doc["bufferViews"][doc["images"][0]["bufferView"].as_u64().unwrap() as usize];
            let start = 28 + length + view["byteOffset"].as_u64().unwrap() as usize;
            let end = start + view["byteLength"].as_u64().unwrap() as usize;
            assert_eq!(
                image::load_from_memory(&bytes[start..end])
                    .unwrap()
                    .to_rgba8()
                    .as_raw(),
                &[120, 215, 240, 128]
            );
        }
    }

    #[test]
    fn glb_has_aligned_buffers_converted_bounds_and_portable_pngs() {
        let mesh = super::super::tests::triangle();
        let material = Material {
            colour_map: Some(Map {
                width: 1,
                height: 1,
                rgba: vec![40, 100, 210, 255],
            }),
            roughness_map: Some(Map {
                width: 1,
                height: 1,
                rgba: vec![128, 0, 0, 255],
            }),
            ..Material::default()
        };
        let data = encode(&mesh, &material).unwrap();
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        assert_eq!(word(0), 0x46546c67);
        assert_eq!(word(8), data.len());
        let document: Value = serde_json::from_slice(&data[20..20 + word(12)]).unwrap();
        assert_eq!(document["accessors"][0]["min"], json!([0.0, 0.0, -1.0]));
        assert_eq!(document["accessors"][0]["max"], json!([1.0, 0.0, 0.0]));
        let binary_start = 28 + word(12);
        for view in document["bufferViews"].as_array().unwrap() {
            assert_eq!(view["byteOffset"].as_u64().unwrap() % 4, 0);
        }
        for (i, expected) in [vec![40, 100, 210, 255], vec![255, 128, 255, 255]]
            .iter()
            .enumerate()
        {
            let view = &document["bufferViews"]
                [document["images"][i]["bufferView"].as_u64().unwrap() as usize];
            let offset = binary_start + view["byteOffset"].as_u64().unwrap() as usize;
            let length = view["byteLength"].as_u64().unwrap() as usize;
            let decoded = image::load_from_memory(&data[offset..offset + length])
                .unwrap()
                .to_rgba8();
            assert_eq!(decoded.as_raw(), expected);
        }
    }

    #[test]
    fn rejects_invalid_materials_and_float32_triangle_collapse() {
        let mut material = Material::default();
        material.roughness = f64::NAN;
        let mut mesh = super::super::tests::triangle();
        assert!(encode(&mesh, &material).is_err());
        material.roughness = 0.5;
        for p in &mut mesh.positions {
            p[0] += 1e15;
        }
        assert!(encode(&mesh, &material).is_err());
        material.colour_map = Some(Map {
            width: 2,
            height: 2,
            rgba: vec![0; 4],
        });
        assert!(material.validate().is_err());
    }
}
