//! Constrained shader descriptions. No arbitrary code or external resource requests.
//! UVs use gcad's top-left convention; adapters convert destination conventions.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

pub const MAX_DESCRIPTION_BYTES: usize = 16 * 1024;
pub const MAX_TIME_SECONDS: f64 = 1_000_000.;
const MAX_COORDINATE: f64 = 1_000_000.;

/// Resolve within the declared project and record the read for live dependency tracking.
pub fn load(path: &str, directory: &std::path::Path, project: &std::path::Path) -> Result<Description> {
    use anyhow::Context;
    ensure!(!std::path::Path::new(path).is_absolute(), "shader path must be relative to the asset document");
    let path = directory.join(path);
    let root = crate::host::canonicalize(project).context("resolving shader project root")?;
    let resolved = crate::host::canonicalize(&path).context("resolving shader description")?;
    ensure!(resolved.starts_with(root), "shader description lies outside the project root");
    let bytes = crate::host::read(&path).context("reading shader description")?;
    Description::parse(&bytes).with_context(|| format!("shader {}", path.display()))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Description {
    pub version: u32,
    pub template: Template,
    pub scale: [f64; 2],
    pub offset: [f64; 2],
    /// UV units per second, evaluated from explicit preview time, never wall time.
    pub speed: [f64; 2],
    pub fallback: Fallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Template {
    #[serde(rename = "uv-scroll-pbr")]
    UvScrollPbr,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Fallback {
    Reject {},
    Freeze { seconds: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub scale: [f64; 2],
    pub offset: [f64; 2],
}

fn time(seconds: f64) -> Result<()> {
    ensure!(
        seconds.is_finite() && (0.0..=MAX_TIME_SECONDS).contains(&seconds),
        "shader time must be finite and between 0 and {MAX_TIME_SECONDS} seconds"
    );
    Ok(())
}

impl Description {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_DESCRIPTION_BYTES,
            "shader description exceeds 16 KiB"
        );
        let result: Self = serde_json::from_slice(bytes)?;
        result.validate()?;
        Ok(result)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(self.version == 1, "unsupported shader description version");
        // Positive scale keeps the existing tangent orientation. Mirroring/rotation
        // requires its own normal-map contract and is not inferred from this template.
        ensure!(
            self.scale
                .iter()
                .all(|v| v.is_finite() && (0.0001..=10_000.).contains(v)),
            "shader scale must be finite and between 0.0001 and 10000"
        );
        ensure!(
            self.offset
                .iter()
                .all(|v| v.is_finite() && v.abs() <= MAX_COORDINATE),
            "shader offset must be finite and bounded by 1000000 UV units"
        );
        ensure!(
            self.speed
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 10_000.),
            "shader speed must be finite and bounded by 10000 UV units per second"
        );
        if let Fallback::Freeze { seconds } = self.fallback {
            time(seconds)?;
            self.evaluated(seconds)?;
        }
        Ok(())
    }

    fn evaluated(&self, seconds: f64) -> Result<Transform> {
        let offset = std::array::from_fn(|i| self.offset[i] + seconds * self.speed[i]);
        ensure!(
            offset
                .iter()
                .all(|v: &f64| v.is_finite() && v.abs() <= MAX_COORDINATE),
            "evaluated shader offset exceeds 1000000 UV units"
        );
        Ok(Transform {
            scale: self.scale,
            offset,
        })
    }

    pub fn at(&self, seconds: f64) -> Result<Transform> {
        self.validate()?;
        time(seconds)?;
        self.evaluated(seconds)
    }

    /// An exporter must explicitly request the declared static fallback.
    /// Returning a transform does not itself bake UVs or claim shader portability.
    pub fn export_fallback(&self) -> Result<Transform> {
        self.validate()?;
        match self.fallback {
            Fallback::Reject {} => {
                anyhow::bail!("shader requires a dynamic adapter; static export fallback is reject")
            }
            Fallback::Freeze { seconds } => self.at(seconds),
        }
    }
}

impl Transform {
    pub fn apply(&self, uv: [f64; 2]) -> Result<[f64; 2]> {
        let result = std::array::from_fn(|i| uv[i] * self.scale[i] + self.offset[i]);
        ensure!(
            result
                .iter()
                .all(|v: &f64| v.is_finite() && v.abs() <= MAX_COORDINATE),
            "transformed shader UV exceeds finite coordinate bounds"
        );
        Ok(result)
    }
}

/// Bake explicitly declared frozen sampling coordinates into an owned export scene.
/// Vertex order, geometry, hierarchy and animation stay unchanged; skin guards are
/// recreated only for the derived UV/tangent tuples, retaining the original weights.
pub(crate) fn freeze_scene(
    source: &super::scene::Scene,
    bindings: &[super::skin::Skin],
) -> Result<(super::scene::Scene, Vec<super::skin::Skin>, serde_json::Value)> {
    use anyhow::Context;
    source.validate()?;
    for skin in bindings { skin.validate(source)?; }
    let mut scene=source.clone();
    let mut transforms=Vec::new();let mut reports=Vec::new();
    for (index,material) in source.materials.iter().enumerate() {
        let transform=material.shader.as_ref().map(|shader| {
            let transform=shader.export_fallback().with_context(|| format!("shader export material {index}"))?;
            reports.push(serde_json::json!({"material":index,"description":shader,
                "fallback":"frozen-uv","lost_shader_animation":shader.speed != [0.,0.],
                "uv_convention":"top-left","consumer_requirement":"ordinary PBR; shader animation not transferred"}));
            Ok::<_,anyhow::Error>(transform)
        }).transpose()?;
        transforms.push(transform);scene.materials[index].shader=None;
    }
    for mesh in &mut scene.meshes {
        for primitive in &mut mesh.primitives {
            let Some(transform)=transforms[primitive.material] else {continue};
            let uvs=primitive.geometry.texcoords.as_mut().context("shader freeze requires texture coordinates")?;
            for uv in uvs {
                // Test and store the coordinates that the portable float32 encoder uses.
                *uv=transform.apply(*uv)?.map(|v| (v as f32) as f64);
            }
            primitive.geometry=primitive.geometry.generate_tangents()
                .context("shader freeze produced unusable UV/tangent coordinates")?;
        }
    }
    let skins=bindings.iter().map(|skin| {
        let derived=super::skin::Skin::bind(&scene,skin.mesh_node,skin.joints.clone(),skin.weights.clone())?;
        ensure!(derived.inverse_bind==skin.inverse_bind,"shader freeze changed inverse bind matrices");
        Ok(derived)
    }).collect::<Result<Vec<_>>>()?;
    scene.validate()?;
    Ok((scene,skins,serde_json::Value::Array(reports)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_export_preserves_geometry_skin_and_reports_loss() {
        use crate::asset::{Mesh,material::Material,scene::{Scene,SceneMesh,Primitive,Node},skin::{Skin,Weights}};
        let mesh=Mesh {positions:vec![[0.,0.,0.],[1.,0.,0.],[0.,1.,0.]],normals:vec![[0.,0.,1.];3],
            texcoords:Some(vec![[0.,0.],[1.,0.],[0.,1.]]),tangents:None,triangles:vec![[0,1,2]]}.generate_tangents().unwrap();
        let mut effect=parse(&description()).unwrap();effect.fallback=Fallback::Freeze{seconds:4.};
        let scene=Scene {consumer:None,nodes:vec![Node{name:"mesh".into(),mesh:Some(0),..Default::default()},
                Node{name:"base".into(),..Default::default()},Node{name:"tip".into(),parent:Some(1),..Default::default()}],
            meshes:vec![SceneMesh{name:"two materials".into(),primitives:vec![Primitive{geometry:mesh.clone(),material:0},Primitive{geometry:mesh.clone(),material:1}]}],
            materials:vec![Material{shader:Some(effect),..Default::default()},Material::default()],clips:vec![]};
        let weight=Weights{joints:[0,1,0,0],weights:[0.9995,0.0005,0.,0.]};
        let skin=Skin::bind(&scene,0,vec![1,2],vec![vec![weight;3];2]).unwrap();
        let (frozen,bindings,report)=freeze_scene(&scene,&[skin.clone()]).unwrap();
        assert_eq!(frozen.meshes[0].primitives[0].geometry.texcoords.as_ref().unwrap(),
            &vec![[0.75,-1.5],[2.75,-1.5],[0.75,1.5]]);
        for (index,p) in frozen.meshes[0].primitives.iter().enumerate() {
            assert_eq!(p.geometry.positions,mesh.positions);assert_eq!(p.geometry.triangles,mesh.triangles);
            assert_eq!(p.geometry.normals,mesh.normals);assert_eq!(p.geometry.tangents,mesh.tangents);
            if index==1 {assert_eq!(p.geometry,mesh);}
        }
        assert_eq!(scene.meshes[0].primitives[0].geometry,mesh);
        assert!(scene.materials[0].shader.is_some());assert!(frozen.materials[0].shader.is_none());
        assert_eq!(bindings[0].inverse_bind,skin.inverse_bind);
        assert_eq!(bindings[0].weights[0][0].weights,weight.weights);
        skin.validate(&scene).unwrap();bindings[0].validate(&frozen).unwrap();
        assert_eq!(report[0]["lost_shader_animation"],true);
        let bytes=crate::asset::glb::encode_skinned_scene(&scene,&[skin.clone()]).unwrap();
        let length=u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let json:serde_json::Value=serde_json::from_slice(&bytes[20..20+length]).unwrap();
        assert_eq!(json["extras"]["gcad_shader_fallbacks"],report);
        assert_eq!(json["skins"].as_array().unwrap().len(),1);
        let accessor=&json["accessors"][json["meshes"][0]["primitives"][0]["attributes"]["TEXCOORD_0"].as_u64().unwrap() as usize];
        let view=&json["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
        let start=28+length+view["byteOffset"].as_u64().unwrap_or(0) as usize+accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
        let uv:Vec<f32>=bytes[start..start+24].chunks_exact(4).map(|b|f32::from_le_bytes(b.try_into().unwrap())).collect();
        assert_eq!(uv,vec![0.75,-1.5,2.75,-1.5,0.75,1.5]);
        let capabilities=crate::asset::capabilities::inspect(&scene,&[skin.clone()]);
        assert!(capabilities.exportable);assert_eq!(capabilities.artifact.unwrap()["shader_fallbacks"],report);
        let mut rejected=scene.clone();rejected.materials[0].shader.as_mut().unwrap().fallback=Fallback::Reject{};
        assert!(crate::asset::glb::encode_skinned_scene(&rejected,&[skin]).is_err());
        // A valid f64 transform can collapse an entire UV triangle at export precision.
        let collapse=rejected.materials[0].shader.as_mut().unwrap();collapse.fallback=Fallback::Freeze{seconds:0.};
        collapse.scale=[0.0001;2];collapse.offset=[1_000_000.;2];collapse.speed=[0.;2];
        assert!(freeze_scene(&rejected,&[]).is_err());
    }
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn description_reads_track_edits_and_enforce_project_boundary() {
        let root=std::env::temp_dir().join(format!("gcad-shader-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();
        let sub=root.join("project");std::fs::create_dir(&sub).unwrap();
        let path=sub.join("effect.json");
        std::fs::write(&path,serde_json::to_vec(&description()).unwrap()).unwrap();
        let (loaded,reads)=crate::host::recording(|| load("effect.json",&sub,&sub));
        assert!(loaded.is_ok());assert!(reads.files().any(|p| p.ends_with("effect.json")));
        std::fs::write(&path,b"invalid").unwrap();
        assert!(!reads.unchanged());assert!(load("effect.json",&sub,&sub).is_err());
        std::fs::write(&path,serde_json::to_vec(&description()).unwrap()).unwrap();
        assert!(load("effect.json",&sub,&sub).is_ok());
        let outside=root.join("outside.json");std::fs::copy(&path,&outside).unwrap();
        assert!(load("../outside.json",&sub,&sub).is_err());
        assert!(load(path.to_str().unwrap(),&sub,&sub).is_err());
        std::fs::remove_file(outside).unwrap();std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(sub).unwrap();std::fs::remove_dir(root).unwrap();
    }
    fn description() -> serde_json::Value {
        serde_json::json!({"version":1,"template":"uv-scroll-pbr","scale":[2,3],
            "offset":[0.25,-0.5],"speed":[0.125,-0.25],"fallback":{"mode":"reject"}})
    }
    fn parse(value: &serde_json::Value) -> Result<Description> {
        Description::parse(&serde_json::to_vec(value).unwrap())
    }
    #[test]
    fn explicit_time_is_seekable_and_fallback_is_never_implicit() {
        let shader = parse(&description()).unwrap();
        assert_eq!(
            shader.at(4.).unwrap().apply([0.5, 0.25]).unwrap(),
            [1.75, -0.75]
        );
        shader.at(100.).unwrap();
        assert_eq!(shader.at(4.).unwrap(), shader.at(4.).unwrap());
        assert!(shader.export_fallback().is_err());
        let mut frozen = description();
        frozen["fallback"] = serde_json::json!({"mode":"freeze","seconds":4});
        assert_eq!(
            parse(&frozen).unwrap().export_fallback().unwrap(),
            shader.at(4.).unwrap()
        );
    }
    #[test]
    fn rejects_unknown_contracts_and_unbounded_requests() {
        for (key, value) in [
            ("version", serde_json::json!(2)),
            ("template", serde_json::json!("wgsl")),
            ("source", serde_json::json!("file.wgsl")),
            ("scale", serde_json::json!([-1, 1])),
            ("speed", serde_json::json!([10001, 0])),
            ("offset", serde_json::json!([1000001, 0])),
            (
                "fallback",
                serde_json::json!({"mode":"freeze","seconds":-1}),
            ),
            ("fallback", serde_json::json!({"mode":"reject","seconds":0})),
        ] {
            let mut value_under_test = description();
            value_under_test[key] = value;
            assert!(parse(&value_under_test).is_err(), "accepted invalid {key}");
        }
        let mut missing = description();
        missing.as_object_mut().unwrap().remove("fallback");
        assert!(parse(&missing).is_err());
        assert!(Description::parse(&vec![b' '; MAX_DESCRIPTION_BYTES + 1]).is_err());
        assert!(Description::parse(&[0xff]).is_err());
        let shader = parse(&description()).unwrap();
        for seconds in [f64::NAN, f64::INFINITY, -1., MAX_TIME_SECONDS + 1.] {
            assert!(shader.at(seconds).is_err());
        }
        assert!(shader.at(0.).unwrap().apply([f64::NAN, 0.]).is_err());
        let mut overflowing = shader;
        overflowing.speed = [10000., 0.];
        assert!(overflowing.at(MAX_TIME_SECONDS).is_err());
    }
}
