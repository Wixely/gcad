//! Package-owned, digest-bound shader metadata for explicit engine attachment.
use anyhow::{Context, Result, ensure};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};

pub(super) fn create(glb:&[u8],fbx_sha256:&str)->Result<Option<Vec<u8>>> {
    ensure!(glb.len()>=28 && &glb[..4]==b"glTF", "shader receipt requires GLB");
    let length=u32::from_le_bytes(glb[12..16].try_into()?) as usize;
    ensure!(length<=glb.len()-20 && &glb[16..20]==b"JSON", "invalid GLB JSON chunk");
    let source:Value=serde_json::from_slice(&glb[20..20+length])?;
    let Some(reports)=source["extras"].get("gcad_shader_fallbacks") else {return Ok(None)};
    let reports=reports.as_array().context("invalid shader fallback report")?;
    ensure!(reports.len()<=1024,"shader material limit exceeded");
    let materials=source["materials"].as_array().context("shader materials missing")?;
    let mut entries=Vec::new();let mut indices=std::collections::HashSet::new();let mut names=std::collections::HashSet::new();
    for report in reports {
        let index=usize::try_from(report["material"].as_u64().context("shader material index missing")?)?;
        ensure!(index<materials.len()&&indices.insert(index),"invalid or duplicate shader material");
        ensure!(report["fallback"]=="frozen-uv" && report["uv_convention"]=="top-left", "unsupported shader fallback");
        let description:super::shader::Description=serde_json::from_value(report["description"].clone())?;
        description.export_fallback()?;
        let name=materials[index]["name"].as_str().map(str::to_owned).unwrap_or_else(||format!("materials_{index}"));
        ensure!(!name.is_empty()&&name.len()<=4096&&names.insert(name.clone()),"invalid shader material name");
        entries.push(json!({"material_name":name,"description":description}));
    }
    if entries.is_empty(){return Ok(None)}
    ensure!(fbx_sha256.len()==64&&fbx_sha256.bytes().all(|b|b.is_ascii_hexdigit()),"invalid FBX digest");
    let receipt=json!({"version":1,"format":"gcad-frozen-uv-scroll","model_sha256":fbx_sha256,
        "source_glb_sha256":format!("{:x}",Sha256::digest(glb)),"materials":entries});
    let bytes=serde_json::to_vec_pretty(&receipt)?;
    ensure!(bytes.len()<=1024*1024,"shader receipt exceeds 1 MiB");
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shader_receipt_binds_artifact_and_rejects_unsupported_fallbacks() {
        let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/game-assets/plate-shader-frozen.gasset");
        let asset=crate::asset::author::load(&path,&[]).unwrap();let glb=asset.encode_glb().unwrap();
        let bytes=create(&glb,&"ab".repeat(32)).unwrap().unwrap();let receipt:Value=serde_json::from_slice(&bytes).unwrap();
        assert_eq!(receipt["model_sha256"],"ab".repeat(32));
        assert_eq!(receipt["source_glb_sha256"],format!("{:x}",Sha256::digest(&glb)));
        assert_eq!(receipt["materials"][0]["material_name"],"materials_0");
        assert_eq!(receipt["materials"][0]["description"]["fallback"]["seconds"],2.0);
        assert!(create(&glb,"invalid digest").is_err());assert!(create(b"not GLB",&"ab".repeat(32)).is_err());
        // Rebuild the JSON chunk to exercise unsupported fallback rejection.
        for (from,to) in [("frozen-uv","unsupported"),("top-left","bottom-up")] {
            let length=u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
            let mut document:Value=serde_json::from_slice(&glb[20..20+length]).unwrap();
            let report=&mut document["extras"]["gcad_shader_fallbacks"][0];
            let key=if from=="frozen-uv" {"fallback"} else {"uv_convention"};
            report[key]=json!(to);
            let mut json=serde_json::to_vec(&document).unwrap();while json.len()%4!=0{json.push(b' ');}
            let mut changed=glb[..12].to_vec();changed.extend_from_slice(&(json.len() as u32).to_le_bytes());
            changed.extend_from_slice(b"JSON");changed.extend_from_slice(&json);changed.extend_from_slice(&glb[20+length..]);
            let total=changed.len() as u32;changed[8..12].copy_from_slice(&total.to_le_bytes());
            assert!(create(&changed,&"ab".repeat(32)).is_err());
        }
        let mut plain=asset.scene.clone();plain.materials[0].shader=None;
        assert!(create(&crate::asset::glb::encode_scene(&plain).unwrap(),&"ab".repeat(32)).unwrap().is_none());
    }
}
