//! Versioned, line-oriented asset authoring. Paths are relative to the document.
pub use super::surface_settings::SurfaceSettings;
use super::{
    Mesh,
    animation::{Clip, Interpolation, Track, Values},
    bake,
    scene::{Node, Primitive, Scene, SceneMesh, Transform},
};
use anyhow::{Context, Result, bail, ensure};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug)]
pub struct Source {
    pub path: String,
    pub body: String,
    pub tolerance_mm: f64,
    pub tile_mm: f64,
    pub rotation_degrees: f64,
    pub unwrap: Option<super::unwrap::Settings>,
    pub refinement: Option<(u32, usize)>,
    pub slicing: Option<SliceSettings>,
    pub normal_crease_degrees: Option<f64>,
    pub atlas: Option<AtlasSettings>,
    pub surface: Option<SurfaceSettings>,
    pub shader: Option<String>,
    pub maps: Vec<super::texture_input::Input>,
}

#[derive(Clone, Debug)]
pub struct SliceSettings {
    pub axis: usize,
    pub planes_mm: Vec<f64>,
    pub triangle_budget: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct AtlasSettings {
    pub width: u32,
    pub height: u32,
    pub padding: u32,
    pub density: f64,
    pub filter: super::material::Filter,
    pub mips: Option<u32>,
    pub mask_coverage: bool,
}

#[derive(Debug)]
pub struct Document {
    pub consumer: Option<super::consumer::Profile>,
    pub face_weights: Vec<FaceWeights>,
    pub face_uvs: Vec<FaceUv>,
    pub face_materials: Vec<FaceMaterial>,
    pub sources: BTreeMap<String, Source>,
    pub nodes: Vec<NodeSpec>,
    pub clips: Vec<Clip>,
    pub skins: BTreeMap<String, String>,
    pub project: Option<String>,
    // Clip node indices refer to these names until CAD expansion is complete.
    clip_targets: Vec<String>,
    motions: Vec<super::motion::Motion>,
}

#[derive(Clone, Debug)]
pub struct FaceMaterial {
    pub node: String,
    pub selection: String,
    pub settings: SurfaceSettings,
}

#[derive(Clone, Debug)]
pub struct FaceUv {
    pub node: String,
    pub selection: String,
    pub transform: super::uv_edit::Transform,
}

#[derive(Clone, Debug)]
pub struct FaceWeights {
    pub node: String,
    pub selection: String,
    pub influences: Vec<(String, f64)>,
    pub boundary: super::face_weights::Boundary,
}

pub struct Asset {
    pub scene: Scene,
    pub skins: Vec<super::skin::Skin>,
    // Per-node provenance: identical shared meshes can originate from different CAD bodies.
    inputs: BTreeMap<usize, super::provenance::Inputs>,
    // Per-node because shared geometry need not have the same CAD face ordering.
    face_maps: BTreeMap<usize, Vec<super::faces::FaceMap>>,
    face_sources: BTreeMap<usize, super::face_author::Source>,
}

impl Asset {
    /// A derived preview has no authoring face/binding provenance. Never carry
    /// source vertex selectors across topology-changing preview operations.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn preview_scene(scene: Scene, skins: Vec<super::skin::Skin>) -> Result<Self> {
        scene.validate()?;
        for skin in &skins { skin.validate(&scene)?; }
        Ok(Self { scene, skins, inputs: BTreeMap::new(), face_maps: BTreeMap::new(), face_sources: BTreeMap::new() })
    }

    /// Validate the persisted source and selector against this load's evaluation,
    /// then resolve lineage into the prepared node's current triangle indices.
    pub fn resolve_faces(
        &self,
        node: usize,
        record: &super::faces::SelectionFile,
    ) -> Result<Vec<usize>> {
        self.face_map(node)?;
        Ok(self.resolve_face_primitives(node, record)?.remove(0))
    }

    /// One triangle-index list per current material primitive, including empty lists.
    pub fn resolve_face_primitives(
        &self,
        node: usize,
        record: &super::faces::SelectionFile,
    ) -> Result<Vec<Vec<usize>>> {
        let maps = self.primitive_face_maps(node)?;
        let raw = self
            .face_sources
            .get(&node)
            .context("face selection requires project ROOT")?
            .resolve(record)?;
        let labels = raw.face_map.triangle_faces(&raw.mesh)?;
        let faces: Vec<_> = raw.triangles.iter().map(|&i| labels[i]).collect();
        let selection = raw.face_map.select(&raw.mesh, &faces)?;
        let mesh = self.scene.nodes[node]
            .mesh
            .context("face selection requires mesh node")?;
        maps.iter()
            .zip(&self.scene.meshes[mesh].primitives)
            .map(|(map, primitive)| map.resolve(&primitive.geometry, &selection))
            .collect()
    }
    /// Available only from load_with_faces; checked against current node geometry.
    pub fn face_map(&self, node: usize) -> Result<&super::faces::FaceMap> {
        let maps = self.primitive_face_maps(node)?;
        ensure!(
            maps.len() == 1,
            "use primitive face maps for a partitioned node"
        );
        Ok(&maps[0])
    }

    pub fn primitive_face_maps(&self, node: usize) -> Result<&[super::faces::FaceMap]> {
        let maps = self
            .face_maps
            .get(&node)
            .context("node has no captured face map")?;
        let mesh = self
            .scene
            .nodes
            .get(node)
            .and_then(|n| n.mesh)
            .context("face map requires mesh node")?;
        let primitives = &self
            .scene
            .meshes
            .get(mesh)
            .context("mesh is absent")?
            .primitives;
        ensure!(
            primitives.len() == maps.len(),
            "face map primitive count changed"
        );
        for (map, primitive) in maps.iter().zip(primitives) {
            map.triangle_faces(&primitive.geometry)?;
        }
        Ok(maps)
    }
    pub fn weight_file(&self, skin: &super::skin::Skin) -> Result<super::skin::WeightFile> {
        skin.validate(&self.scene)?;
        let inputs = self
            .inputs
            .get(&skin.mesh_node)
            .context("asset binding requires `project ROOT` in the asset file")?;
        skin.weight_file()?
            .with_inputs(&self.scene, skin.mesh_node, inputs)
    }
    pub fn encode_glb(&self) -> Result<Vec<u8>> {
        super::glb::encode_skinned_scene(&self.scene, &self.skins)
    }
}

#[derive(Debug)]
pub struct NodeSpec {
    pub name: String,
    pub source: Option<String>,
    pub parent: Option<String>,
    pub transform: Transform,
}

impl Document {
    pub fn parse(text: &str) -> Result<Self> {
        let mut sources = BTreeMap::new();
        let mut tessellation = BTreeMap::new();
        let mut refinement = BTreeMap::new();
        let mut slicing = BTreeMap::new();
        let mut normal_creases = BTreeMap::new();
        let mut atlases = BTreeMap::new();
        let mut surfaces = BTreeMap::new();
        let mut shaders = BTreeMap::new();
        let mut face_materials = Vec::new();
        let mut face_uvs = Vec::new();
        let mut face_weights = Vec::new();
        let mut maps: BTreeMap<String, Vec<super::texture_input::Input>> = BTreeMap::new();
        let mut uv = BTreeMap::new();
        let mut unwraps = BTreeMap::new();
        let mut header = false;
        let mut nodes = Vec::new();
        let mut clips = BTreeMap::new();
        let mut keys = Vec::new();
        let mut skins = BTreeMap::new();
        let mut project = None;
        let mut motions = Vec::new();
        let mut consumer:Option<super::consumer::Profile>=None;
        for (number, text) in text.lines().enumerate() {
            let text = text.trim();
            if text.is_empty() || text.starts_with('#') {
                continue;
            }
            let line = crate::parse::parse_line(number + 1, text)?;
            ensure!(
                line.label.is_none(),
                "asset line {}: labels are unsupported",
                number + 1
            );
            let mut named = BTreeMap::new();
            for (key, value) in line.named {
                ensure!(
                    named.insert(key, value).is_none(),
                    "asset line {}: duplicate attribute",
                    number + 1
                );
            }
            if !header {
                ensure!(
                    line.op == "asset" && line.positional == ["1"] && named.is_empty(),
                    "first asset command must be `asset 1`"
                );
                header = true;
                continue;
            }
            let p = &line.positional;
            match line.op.as_str() {
                "consumer" => {
                    ensure!(p==&["unity"]&&named.len()==1&&named.get("version").is_some_and(|v|v=="1")&&consumer.is_none(),"consumer expects unity version=1, once");
                    consumer=Some(Default::default());
                }
                "socket" => {
                    ensure!(p.len()==1&&named.len()==1&&named.contains_key("node"),"socket expects NAME node=NODE");
                    consumer.as_mut().context("declare consumer before roles")?.sockets.push(super::consumer::Socket{name:p[0].clone(),node:named["node"].clone()});
                }
                "collider" => {
                    ensure!(p.len()==1&&named.len()==1&&named.contains_key("shape"),"collider expects NODE shape=box|mesh|convex");
                    let shape=match named["shape"].as_str(){"box"=>super::consumer::Shape::Box,"mesh"=>super::consumer::Shape::Mesh,"convex"=>super::consumer::Shape::Convex,_=>bail!("unknown collider shape")};
                    consumer.as_mut().context("declare consumer before roles")?.colliders.push(super::consumer::Collider{node:p[0].clone(),shape});
                }
                "lod_group" => {
                    ensure!(p.len()==1&&named.len()==1&&named.contains_key("root"),"lod_group expects NAME root=NODE");
                    let profile=consumer.as_mut().context("declare consumer before roles")?;
                    ensure!(!profile.lods.iter().any(|g|g.name==p[0]),"duplicate LOD group");
                    profile.lods.push(super::consumer::Lod{name:p[0].clone(),root:named["root"].clone(),levels:Vec::new()});
                }
                "lod_level" => {
                    ensure!(p.len()==1&&named.len()==2&&named.contains_key("height")&&named.contains_key("nodes"),"lod_level expects GROUP height=FRACTION nodes=NODE,...");
                    let profile=consumer.as_mut().context("declare consumer before roles")?;
                    let group=profile.lods.iter_mut().find(|g|g.name==p[0]).context("declare LOD group before levels")?;
                    group.levels.push(super::consumer::Level{screen_height:named["height"].parse()?,nodes:named["nodes"].split(',').map(str::to_owned).collect()});
                }
                "face_weights" => {
                    ensure!(
                        p.len() == 2 && p.iter().all(|s| !s.trim().is_empty()),
                        "face_weights expects NODE SELECTION.json"
                    );
                    ensure!(
                        named.len() == 3
                            && named
                                .keys()
                                .all(|k| matches!(k.as_str(), "joints" | "weights" | "boundary")),
                        "face_weights requires joints, weights and boundary only"
                    );
                    let joints: Vec<_> = named["joints"].split(',').collect();
                    let values = named["weights"]
                        .split(',')
                        .map(str::parse::<f64>)
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    ensure!(
                        joints.len() == values.len(),
                        "face_weights joint/weight counts differ"
                    );
                    let pairs: Vec<_> = joints.into_iter().zip(values).collect();
                    super::face_weights::validate_influences(&pairs)?;
                    let boundary = match named["boundary"].as_str() {
                        "reject-shared" => super::face_weights::Boundary::RejectShared,
                        "split" => super::face_weights::Boundary::Split,
                        _ => bail!("face_weights boundary must be reject-shared or split"),
                    };
                    face_weights.push(FaceWeights {
                        node: p[0].clone(),
                        selection: p[1].clone(),
                        influences: pairs.into_iter().map(|(n, w)| (n.to_string(), w)).collect(),
                        boundary,
                    });
                }
                "face_uv" => {
                    ensure!(
                        p.len() == 2 && !p[0].trim().is_empty() && !p[1].trim().is_empty(),
                        "face_uv expects NODE SELECTION.json and transform settings"
                    );
                    ensure!(
                        !named.is_empty()
                            && named
                                .keys()
                                .all(|k| matches!(k.as_str(), "scale" | "rotation" | "offset")),
                        "face_uv accepts scale, rotation and offset only"
                    );
                    let transform = super::uv_edit::Transform {
                        scale: tuple(named.get("scale"), [1., 1.])?,
                        rotation_degrees: named
                            .get("rotation")
                            .map(|v| v.parse())
                            .transpose()?
                            .unwrap_or(0.),
                        offset: tuple(named.get("offset"), [0., 0.])?,
                    };
                    transform.validate()?;
                    face_uvs.push(FaceUv {
                        node: p[0].clone(),
                        selection: p[1].clone(),
                        transform,
                    });
                }
                "face_material" => {
                    ensure!(
                        p.len() == 2,
                        "face_material expects NODE SELECTION.json and surface settings"
                    );
                    ensure!(
                        !p[0].trim().is_empty() && !p[1].trim().is_empty(),
                        "face_material target and record must be nonempty"
                    );
                    face_materials.push(FaceMaterial {
                        node: p[0].clone(),
                        selection: p[1].clone(),
                        settings: SurfaceSettings::parse(&named)?,
                    });
                }
                "normals" => {
                    ensure!(
                        p.len() == 1 && named.len() == 1 && named.contains_key("crease"),
                        "normals expects SOURCE crease=DEGREES"
                    );
                    let angle: f64 = named["crease"].parse()?;
                    ensure!(
                        angle.is_finite() && (0.0..=180.0).contains(&angle),
                        "normal crease angle must be between 0 and 180 degrees"
                    );
                    ensure!(
                        normal_creases.insert(p[0].clone(), angle).is_none(),
                        "duplicate normal settings"
                    );
                }
                "map" => {
                    ensure!(
                        p.len() == 3
                            && named.len() == 2
                            && named.contains_key("license")
                            && named.contains_key("origin"),
                        "map expects SOURCE SLOT PATH license=DECLARATION origin=DECLARATION"
                    );
                    let slot = super::texture_input::Slot::parse(&p[1])?;
                    ensure!(
                        !named["license"].trim().is_empty() && !named["origin"].trim().is_empty(),
                        "map license and origin must be declared"
                    );
                    let inputs = maps.entry(p[0].clone()).or_default();
                    ensure!(
                        !inputs.iter().any(|input| input.slot == slot),
                        "duplicate map slot for source"
                    );
                    inputs.push(super::texture_input::Input {
                        slot,
                        path: p[2].clone(),
                        license: named["license"].clone(),
                        origin: named["origin"].clone(),
                    });
                }
                "shader" => {
                    ensure!(p.len() == 2 && named.is_empty(), "shader expects SOURCE DESCRIPTION.json");
                    ensure!(shaders.insert(p[0].clone(), p[1].clone()).is_none(), "duplicate shader settings");
                }
                "surface" => {
                    ensure!(p.len() == 1, "surface expects one source name");
                    let settings = super::surface_settings::SurfaceSettings::parse(&named)?;
                    ensure!(
                        surfaces.insert(p[0].clone(), settings).is_none(),
                        "duplicate surface settings"
                    );
                }
                "motion" => {
                    ensure!(
                        p.len() == 2
                            && named.keys().all(|k| [
                                "joint",
                                "from",
                                "to",
                                "duration",
                                "rate",
                                "max_error",
                                "max_rate"
                            ]
                            .contains(&k.as_str()))
                            && ["joint", "from", "to", "duration", "rate"]
                                .iter()
                                .all(|k| named.contains_key(*k)),
                        "motion expects CLIP INSTANCE joint=NAME from=VALUE to=VALUE duration=SECONDS rate=HZ [max_error=MM max_rate=HZ]"
                    );
                    let motion = super::motion::Motion {
                        name: p[0].clone(),
                        instance: p[1].clone(),
                        joint: named["joint"].clone(),
                        from: named["from"].parse()?,
                        to: named["to"].parse()?,
                        duration: named["duration"].parse()?,
                        rate: named["rate"].parse()?,
                        max_error_mm: named.get("max_error").map(|v| v.parse()).transpose()?,
                        max_rate: named.get("max_rate").map(|v| v.parse()).transpose()?,
                    };
                    motion.intervals()?;
                    motions.push(motion);
                }
                "project" => {
                    ensure!(
                        p.len() == 1 && named.is_empty() && project.is_none(),
                        "project expects one root path, declared once"
                    );
                    project = Some(p[0].clone());
                }
                "skin" => {
                    ensure!(
                        p.len() == 1 && named.len() == 1 && named.contains_key("weights"),
                        "skin expects NODE weights=PATH"
                    );
                    ensure!(
                        skins
                            .insert(p[0].clone(), named.remove("weights").unwrap())
                            .is_none(),
                        "duplicate skin node"
                    );
                }
                "clip" => {
                    ensure!(
                        p.len() == 1 && named.len() == 1 && named.contains_key("interpolation"),
                        "clip expects NAME interpolation=linear|step"
                    );
                    let interpolation = match named["interpolation"].as_str() {
                        "linear" => Interpolation::Linear,
                        "step" => Interpolation::Step,
                        _ => bail!("unsupported clip interpolation"),
                    };
                    ensure!(
                        clips.insert(p[0].clone(), interpolation).is_none(),
                        "duplicate clip name"
                    );
                }
                "key" => {
                    ensure!(
                        p.len() == 2 && named.len() == 2 && named.contains_key("time"),
                        "key expects CLIP NODE time=SECONDS translation=X,Y,Z|rotation=X,Y,Z,W|scale=X,Y,Z"
                    );
                    let time: f64 = named["time"].parse()?;
                    let values = if named.contains_key("translation") {
                        Values::Translation(vec![tuple(named.get("translation"), [0.0; 3])?])
                    } else if named.contains_key("rotation") {
                        Values::Rotation(vec![tuple(named.get("rotation"), [0.0; 4])?])
                    } else if named.contains_key("scale") {
                        Values::Scale(vec![tuple(named.get("scale"), [0.0; 3])?])
                    } else {
                        bail!("key requires one translation, rotation or scale value");
                    };
                    keys.push((p[0].clone(), p[1].clone(), time, values));
                }
                "node" => {
                    ensure!(
                        p.len() == 1
                            && named.keys().all(|k| matches!(
                                k.as_str(),
                                "source" | "parent" | "translation" | "rotation" | "scale"
                            )),
                        "node expects NAME [source=NAME] [parent=NAME] [translation=X,Y,Z] [rotation=X,Y,Z,W] [scale=X,Y,Z]"
                    );
                    let transform = Transform {
                        translation: tuple(named.get("translation"), [0.0; 3])?,
                        rotation: tuple(named.get("rotation"), [0.0, 0.0, 0.0, 1.0])?,
                        scale: tuple(named.get("scale"), [1.0; 3])?,
                    };
                    transform.validate()?;
                    nodes.push(NodeSpec {
                        name: p[0].clone(),
                        source: named.remove("source"),
                        parent: named.remove("parent"),
                        transform,
                    });
                }
                "source" => {
                    ensure!(
                        p.len() == 2 && named.keys().all(|k| k == "body"),
                        "source expects NAME PATH [body=NAME]"
                    );
                    ensure!(
                        !p[0].is_empty() && !p[0].contains('/'),
                        "invalid source name"
                    );
                    ensure!(
                        Path::new(&p[1])
                            .extension()
                            .is_some_and(|e| e.eq_ignore_ascii_case("gcad")
                                || e.eq_ignore_ascii_case("gasm")),
                        "asset sources must be .gcad or .gasm files"
                    );
                    let assembly = Path::new(&p[1])
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("gasm"));
                    let body = named
                        .remove("body")
                        .unwrap_or_else(|| if assembly { "*".into() } else { "main".into() });
                    ensure!(
                        sources.insert(p[0].clone(), (p[1].clone(), body)).is_none(),
                        "duplicate source name"
                    );
                }
                "tessellate" => {
                    ensure!(
                        p.len() == 1 && named.len() == 1 && named.contains_key("tolerance"),
                        "tessellate expects NAME tolerance=MM"
                    );
                    let tolerance = positive(&named["tolerance"])?;
                    ensure!(
                        tessellation.insert(p[0].clone(), tolerance).is_none(),
                        "duplicate tessellation settings"
                    );
                }
                "slice" => {
                    ensure!(
                        p.len() == 1
                            && named.len() == 3
                            && ["axis", "planes", "triangle_budget"]
                                .iter()
                                .all(|k| named.contains_key(*k)),
                        "slice expects NAME axis=x|y|z planes=MM,... triangle_budget=N"
                    );
                    let axis = match named["axis"].as_str() {
                        "x" => 0,
                        "y" => 1,
                        "z" => 2,
                        _ => bail!("slice axis must be x/y/z"),
                    };
                    let planes_mm = named["planes"]
                        .split(',')
                        .map(|v| v.parse::<f64>())
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    let triangle_budget = named["triangle_budget"].parse::<usize>()?;
                    ensure!(
                        !planes_mm.is_empty()
                            && planes_mm.len() <= 1024
                            && planes_mm.iter().all(|v| v.is_finite())
                            && planes_mm.windows(2).all(|w| w[1] > w[0])
                            && triangle_budget > 0,
                        "invalid slice settings"
                    );
                    ensure!(
                        slicing
                            .insert(
                                p[0].clone(),
                                SliceSettings {
                                    axis,
                                    planes_mm,
                                    triangle_budget
                                }
                            )
                            .is_none(),
                        "duplicate slice settings"
                    );
                }
                "refine" => {
                    ensure!(
                        p.len() == 1
                            && named.len() == 2
                            && named.contains_key("levels")
                            && named.contains_key("triangle_budget"),
                        "refine expects NAME levels=N triangle_budget=N"
                    );
                    let levels: u32 = named["levels"].parse()?;
                    let budget: usize = named["triangle_budget"].parse()?;
                    ensure!(
                        (1..=8).contains(&levels) && budget > 0,
                        "invalid refinement settings"
                    );
                    ensure!(
                        refinement.insert(p[0].clone(), (levels, budget)).is_none(),
                        "duplicate refinement settings"
                    );
                }
                "atlas" => {
                    ensure!(
                        p.len() == 1
                            && named.keys().all(|k| [
                                "width", "height", "padding", "density", "filter", "mips",
                                "coverage"
                            ]
                            .contains(&k.as_str()))
                            && ["width", "height", "padding", "density", "filter"]
                                .iter()
                                .all(|k| named.contains_key(*k)),
                        "atlas expects NAME width=PIXELS height=PIXELS padding=PIXELS density=PIXELS_PER_UV filter=nearest|linear [mips=MAX_LEVEL coverage=mask]"
                    );
                    let settings = AtlasSettings {
                        mask_coverage: match named.get("coverage").map(String::as_str) {
                            None => false,
                            Some("mask") => true,
                            _ => bail!("atlas coverage must be mask"),
                        },
                        mips: named.get("mips").map(|v| v.parse()).transpose()?,
                        width: named["width"].parse()?,
                        height: named["height"].parse()?,
                        padding: named["padding"].parse()?,
                        density: positive(&named["density"])?,
                        filter: match named["filter"].as_str() {
                            "nearest" => super::material::Filter::Nearest,
                            "linear" => super::material::Filter::Linear,
                            _ => bail!("atlas filter must be nearest or linear"),
                        },
                    };
                    ensure!(
                        !settings.mask_coverage || settings.mips.is_some(),
                        "atlas coverage=mask requires explicit mips"
                    );
                    ensure!(
                        settings.mips.is_none_or(|level| (1..=13).contains(&level)
                            && settings.filter == super::material::Filter::Linear),
                        "explicit atlas mips require filter=linear and a maximum level in 1..13"
                    );
                    ensure!(
                        (16..=8192).contains(&settings.width)
                            && (16..=8192).contains(&settings.height)
                            && settings.padding >= 1
                            && settings.padding <= settings.width.min(settings.height) / 4,
                        "invalid atlas dimensions or padding"
                    );
                    ensure!(
                        atlases.insert(p[0].clone(), settings).is_none(),
                        "duplicate atlas settings"
                    );
                }
                "uv" => {
                    if named.get("projection").is_some_and(|p| p == "unwrap") {
                        ensure!(p.len() == 1 && named.keys().all(|k|
                            matches!(k.as_str(), "projection" | "resolution" | "padding")),
                            "uv expects NAME projection=unwrap resolution=PIXELS padding=PIXELS");
                        let settings = super::unwrap::Settings {
                            resolution: named.get("resolution").context("unwrap resolution is required")?.parse()?,
                            padding: named.get("padding").context("unwrap padding is required")?.parse()?,
                        };
                        settings.validate()?;
                        ensure!(uv.insert(p[0].clone(), (1., 0.)).is_none(), "duplicate UV settings");
                        unwraps.insert(p[0].clone(), settings);
                        continue;
                    }
                    ensure!(
                        p.len() == 1
                            && named
                                .keys()
                                .all(|k| matches!(k.as_str(), "projection" | "tile" | "rotation")),
                        "uv expects NAME projection=box tile=MM [rotation=DEGREES]"
                    );
                    ensure!(
                        named.get("projection").is_some_and(|p| p == "box"),
                        "only box projection is supported"
                    );
                    let tile = positive(named.get("tile").context("UV tile size is required")?)?;
                    let turn: f64 = named.get("rotation").map_or(Ok(0.0), |v| v.parse())?;
                    ensure!(turn.is_finite(), "UV rotation must be finite");
                    ensure!(
                        uv.insert(p[0].clone(), (tile, turn)).is_none(),
                        "duplicate UV settings"
                    );
                }
                other => bail!("asset line {}: unknown command `{other}`", number + 1),
            }
        }
        ensure!(header && !sources.is_empty(), "asset must contain a source");
        ensure!(
            tessellation
                .keys()
                .chain(uv.keys())
                .chain(refinement.keys())
                .chain(slicing.keys())
                .chain(normal_creases.keys())
                .chain(atlases.keys())
                .chain(surfaces.keys())
                .chain(shaders.keys())
                .chain(maps.keys())
                .all(|k| sources.contains_key(k)),
            "asset settings reference an unknown source"
        );
        let sources = sources
            .into_iter()
            .map(|(name, (path, body))| {
                let tolerance_mm = *tessellation
                    .get(&name)
                    .context("source requires explicit tessellation settings")?;
                let &(tile_mm, rotation_degrees) = uv
                    .get(&name)
                    .context("source requires explicit UV settings")?;
                let refinement = refinement.get(&name).copied();
                let slicing = slicing.get(&name).cloned();
                let normal_crease_degrees = normal_creases.get(&name).copied();
                ensure!(
                    refinement.is_none() || slicing.is_none(),
                    "choose refine or slice per source"
                );
                let atlas = atlases.get(&name).copied();
                let unwrap = unwraps.get(&name).copied();
                ensure!(unwrap.is_none() || atlas.is_none(),
                    "unwrap already packs an atlas; do not combine it with atlas rebaking");
                let surface = surfaces.get(&name).cloned();
                let shader = shaders.remove(&name);
                let maps = maps.remove(&name).unwrap_or_default();
                Ok((
                    name,
                    Source {
                        path,
                        body,
                        tolerance_mm,
                        tile_mm,
                        rotation_degrees,
                        unwrap,
                        refinement,
                        slicing,
                        normal_crease_degrees,
                        atlas,
                        surface,
                        shader,
                        maps,
                    },
                ))
            })
            .collect::<Result<_>>()?;
        let mut document = Self {
            consumer,
            face_uvs,
            face_weights,
            face_materials,
            sources,
            nodes,
            clips: Vec::new(),
            skins,
            project,
            clip_targets: Vec::new(),
            motions,
        };
        let nodes = document.scene_nodes()?;
        let mut targets: Vec<String> = nodes.iter().map(|n| n.name.clone()).collect();
        let sources: Vec<_> = document.sources.values().collect();
        let mut motion_names = std::collections::HashSet::new();
        for motion in &document.motions {
            ensure!(
                !clips.contains_key(&motion.name) && motion_names.insert(&motion.name),
                "duplicate motion/clip name"
            );
            let source = nodes
                .iter()
                .find(|n| n.name == motion.instance)
                .and_then(|n| n.mesh)
                .context("motion requires an authored source instance")?;
            ensure!(
                Path::new(&sources[source].path)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("gasm")),
                "motion requires a .gasm source instance"
            );
        }
        let generated_target = |name: &str| {
            nodes.iter().any(|parent| {
                parent.mesh.is_some_and(|source| {
                    Path::new(&sources[source].path)
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("gasm"))
                        && name
                            .strip_prefix(&format!("{}/", parent.name))
                            .is_some_and(|body| !body.is_empty())
                })
            })
        };
        ensure!(
            document.skins.keys().all(|name| nodes
                .iter()
                .any(|n| &n.name == name && n.mesh.is_some())
                || generated_target(name)),
            "skin references an unknown or meshless node"
        );
        let mut tracks = BTreeMap::<String, Vec<Track>>::new();
        for (clip, node, time, value) in keys {
            let interpolation = *clips.get(&clip).context("key references unknown clip")?;
            let node = if let Some(index) = targets.iter().position(|name| name == &node) {
                index
            } else {
                ensure!(generated_target(&node), "key references unknown node");
                let index = targets.len();
                targets.push(node);
                index
            };
            let list = tracks.entry(clip).or_default();
            if let Some(track) = list
                .iter_mut()
                .find(|t| t.node == node && t.values.path() == value.path())
            {
                track.times.push(time);
                match (&mut track.values, value) {
                    (Values::Translation(a), Values::Translation(b))
                    | (Values::Scale(a), Values::Scale(b)) => a.extend(b),
                    (Values::Rotation(a), Values::Rotation(b)) => a.extend(b),
                    _ => unreachable!(),
                }
            } else {
                list.push(Track {
                    node,
                    times: vec![time],
                    values: value,
                    interpolation,
                });
            }
        }
        for (name, _) in clips {
            let clip = Clip {
                tracks: tracks.remove(&name).unwrap_or_default(),
                name,
            };
            clip.validate(targets.len())?;
            document.clips.push(clip);
        }
        document.clip_targets = targets;
        Ok(document)
    }

    fn scene_nodes(&self) -> Result<Vec<Node>> {
        if self.nodes.is_empty() {
            return Ok(self
                .sources
                .keys()
                .enumerate()
                .map(|(i, name)| Node {
                    name: name.clone(),
                    mesh: Some(i),
                    ..Default::default()
                })
                .collect());
        }
        let sources: BTreeMap<_, _> = self
            .sources
            .keys()
            .enumerate()
            .map(|(i, name)| (name.as_str(), i))
            .collect();
        let mut names = BTreeMap::new();
        for (i, node) in self.nodes.iter().enumerate() {
            ensure!(
                !node.name.trim().is_empty() && names.insert(node.name.as_str(), i).is_none(),
                "node names must be nonempty and unique"
            );
        }
        let nodes = self
            .nodes
            .iter()
            .map(|node| {
                Ok(Node {
                    name: node.name.clone(),
                    mesh: node
                        .source
                        .as_deref()
                        .map(|name| {
                            sources
                                .get(name)
                                .copied()
                                .context("node references unknown source")
                        })
                        .transpose()?,
                    parent: node
                        .parent
                        .as_deref()
                        .map(|name| {
                            names
                                .get(name)
                                .copied()
                                .context("node references unknown parent")
                        })
                        .transpose()?,
                    transform: node.transform,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            (0..sources.len()).all(|i| nodes.iter().any(|n| n.mesh == Some(i))),
            "each source requires at least one node instance when explicit nodes are used"
        );
        let scene = Scene {
            consumer:None,
            nodes,
            meshes: self
                .sources
                .keys()
                .map(|name| SceneMesh {
                    name: name.clone(),
                    primitives: Vec::new(),
                })
                .collect(),
            materials: Vec::new(),
            clips: Vec::new(),
        };
        scene.world_transforms()?;
        Ok(scene.nodes)
    }
}

fn tuple<const N: usize>(text: Option<&String>, default: [f64; N]) -> Result<[f64; N]> {
    let Some(text) = text else {
        return Ok(default);
    };
    let values = text
        .split(',')
        .map(str::parse::<f64>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    values
        .try_into()
        .map_err(|_| anyhow::anyhow!("transform requires {N} comma-separated values"))
}

fn positive(text: &str) -> Result<f64> {
    let value: f64 = text.parse()?;
    ensure!(
        value.is_finite() && value > 0.0,
        "size must be finite and positive"
    );
    Ok(value)
}

pub fn load(path: &Path, vars: &[(String, f64)]) -> Result<Asset> {
    load_internal(path, vars, None, false, &[])
}

/// Load the ordinary authored asset with guarded per-node source-face lineage.
/// Missing/ambiguous tessellation provenance fails instead of guessing ownership.
pub fn load_with_faces(path: &Path, vars: &[(String, f64)]) -> Result<Asset> {
    load_internal(path, vars, None, true, &[])
}

/// Explicit binding path: skip the requested node's old weight resource. Other
/// bindings that do not exist yet may be prepared later; existing resources must
/// still validate. Ordinary check/export require every declared skin.
pub fn load_for_binding(path: &Path, vars: &[(String, f64)], node: &str) -> Result<Asset> {
    load_for_binding_deferred(path, vars, node, &[])
}

/// Explicitly defer named peer skin resources while repairing a shared rig.
/// The returned preparation asset is not a fully validated export asset.
pub fn load_for_binding_deferred(path: &Path, vars: &[(String, f64)], node: &str, deferred: &[String]) -> Result<Asset> {
    load_internal(path, vars, Some(node), false, deferred)
}

fn load_internal(
    path: &Path,
    vars: &[(String, f64)],
    binding_node: Option<&str>,
    with_faces: bool,
    deferred: &[String],
) -> Result<Asset> {
    let bytes = crate::host::read(path)?;
    let document = Document::parse(std::str::from_utf8(&bytes)?)?;
    let mut deferred_names = std::collections::HashSet::new();
    for name in deferred {
        ensure!(binding_node.is_some() && binding_node != Some(name.as_str()), "defer-skin must name a peer of the binding target");
        ensure!(document.skins.contains_key(name), "deferred skin `{name}` is not declared");
        ensure!(deferred_names.insert(name.as_str()), "duplicate deferred skin `{name}`");
    }
    let with_faces = with_faces
        || !document.face_materials.is_empty()
        || !document.face_uvs.is_empty()
        || !document.face_weights.is_empty();
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let root = document.project.as_ref().map(|root| directory.join(root));
    let mut inputs = BTreeMap::new();
    let mut face_maps = BTreeMap::new();
    let mut face_sources = BTreeMap::new();
    let mut pending_motions = Vec::new();
    let mut scene = Scene {
        nodes: document.scene_nodes()?,
        consumer:document.consumer.clone(),
        clips: document.clips,
        ..Default::default()
    };
    let node_sources: Vec<_> = scene.nodes.iter().map(|n| n.mesh).collect();
    for (source_index, (name, source)) in document.sources.into_iter().enumerate() {
        let owners: Vec<_> = node_sources
            .iter()
            .enumerate()
            .filter_map(|(i, s)| (*s == Some(source_index)).then_some(i))
            .collect();
        let source_path = directory.join(&source.path);
        let (run, mut reads) = crate::model::run_path_recorded(&source_path, vars)?;
        let cad_reads = with_faces.then(|| reads.clone());
        let (textures, texture_reads) = crate::host::recording(|| {
            let mut remaining = 256 * 1024 * 1024;
            source
                .maps
                .iter()
                .map(|input| {
                    let loaded = super::texture_input::load(
                        input,
                        directory,
                        root.as_deref()
                            .context("explicit maps require project ROOT")?,
                        remaining,
                    )?;
                    remaining -= loaded.map.rgba.len();
                    Ok(loaded)
                })
                .collect::<Result<Vec<_>>>()
        });
        let textures = textures?;
        reads.merge(texture_reads);
        // Outer document recording observes shader inputs, but appearance-only edits
        // must not invalidate CAD/skin geometry binding provenance.
        let shader = source.shader.as_ref().map(|path| super::shader::load(
            path, directory, root.as_deref().context("shader requires project ROOT")?
        )).transpose()?;
        ensure!(shader.is_none() || source.atlas.is_none(), "shader with atlas baking is not yet supported");
        for (line, result) in &run.steps {
            if let Err(error) = result {
                bail!("source `{name}` line {}: {error:#}", line.number);
            }
        }
        let face_source = |body: &str| -> Result<Option<super::face_author::Source>> {
            let Some(reads) = &cad_reads else {
                return Ok(None);
            };
            let mut settings = source.clone();
            settings.body = body.to_string();
            capture_inputs(root.as_deref(), &source_path, &name, &settings, vars, reads)?
                .map(|inputs| super::face_author::evaluated(&run.model, inputs))
                .transpose()
        };
        if run.model.assembly {
            use monstertruck::modeling::{Matrix4, SquareMatrix, builder};
            let mut bodies = run.model.named_solids();
            bodies.sort_by(|a, b| a.0.cmp(&b.0));
            if source.body != "*" {
                bodies.retain(|(body, _)| body == &source.body);
            }
            ensure!(!bodies.is_empty(), "assembly source has no matching bodies");
            let selected = bodies.iter().map(|(name, _)| name.clone()).collect();
            let mut body_parents = BTreeMap::new();
            for &owner in &owners {
                ensure!(
                    Some(scene.nodes[owner].name.as_str()) != binding_node
                        && !document.skins.contains_key(&scene.nodes[owner].name),
                    "assembly source groups cannot be skinned; select INSTANCE/BODY"
                );
                scene.nodes[owner].mesh = None;
                body_parents.insert(
                    owner,
                    super::assembly_tree::append(
                        &mut scene,
                        owner,
                        &run.model.assembly_instances,
                        &selected,
                    )?,
                );
            }
            let mut remaining_budget = source
                .refinement
                .map(|(_, budget)| budget)
                .or_else(|| source.slicing.as_ref().map(|s| s.triangle_budget));
            for (body, solid) in bodies {
                let placement = run
                    .model
                    .placements
                    .get(&body)
                    .copied()
                    .unwrap_or_else(Matrix4::identity);
                let local = builder::transformed(
                    solid,
                    placement.invert().context("singular assembly placement")?,
                );
                let mut part_settings = source.clone();
                if let Some((levels, _)) = part_settings.refinement {
                    part_settings.refinement = Some((levels, remaining_budget.unwrap()));
                }
                if let Some(slicing) = &mut part_settings.slicing {
                    slicing.triangle_budget = remaining_budget.unwrap();
                }
                let (geometry, mut material, face_map) = prepare_mesh(
                    &run.model,
                    &body,
                    &local,
                    &part_settings,
                    &textures,
                    with_faces,
                )?;
                material.shader = shader.clone();
                if let Some(remaining) = &mut remaining_budget {
                    *remaining -= geometry.triangles.len();
                }
                // Exact complete-tuple/material equality only; no source-label or bounding-box guesses.
                let existing = scene.meshes.iter().position(|mesh| {
                    mesh.primitives.len() == 1
                        && mesh.primitives[0].geometry == geometry
                        && scene.materials[mesh.primitives[0].material] == material
                });
                let mut binding =
                    capture_inputs(root.as_deref(), &source_path, &name, &source, vars, &reads)?;
                if let Some(binding) = &mut binding {
                    binding.body = body.clone();
                    binding.instance = format!("{name}/{body}");
                    binding.generator.push_str("/assembly-1");
                }
                let index = if let Some(index) = existing {
                    index
                } else {
                    let index = scene.meshes.len();
                    scene.meshes.push(SceneMesh {
                        name: format!("{name}/{body}"),
                        primitives: vec![Primitive {
                            geometry,
                            material: scene.materials.len(),
                        }],
                    });
                    scene.materials.push(material);
                    index
                };
                let face_source = face_source(&body)?;
                for &owner in &owners {
                    let (parent, frame) = body_parents[&owner]
                        .get(&body)
                        .copied()
                        .unwrap_or((owner, Matrix4::identity()));
                    let transform = super::scene::Transform::from_matrix(
                        frame.invert().context("singular assembly parent frame")? * placement,
                    )?;
                    if let Some(map) = &face_map {
                        face_maps.insert(scene.nodes.len(), vec![map.clone()]);
                    }
                    if let Some(source) = &face_source {
                        face_sources.insert(scene.nodes.len(), source.clone());
                    }
                    if let Some(binding) = &binding {
                        inputs.insert(scene.nodes.len(), binding.clone());
                    }
                    scene.nodes.push(Node {
                        name: format!("{}/{body}", scene.nodes[owner].name),
                        parent: Some(parent),
                        mesh: Some(index),
                        transform,
                    });
                }
            }
            for motion in &document.motions {
                if owners
                    .iter()
                    .any(|&owner| scene.nodes[owner].name == motion.instance)
                {
                    pending_motions.push((motion.clone(), run.model.clone()));
                }
            }
            continue;
        }
        let solid = run.model.named_body(&source.body)?;
        let (geometry, mut material, face_map) = prepare_mesh(
            &run.model,
            &source.body,
            &solid,
            &source,
            &textures,
            with_faces,
        )?;
        material.shader = shader;
        let binding = capture_inputs(root.as_deref(), &source_path, &name, &source, vars, &reads)?;
        let face_source = face_source(&source.body)?;
        let index = scene.meshes.len();
        for owner in owners {
            if let Some(source) = &face_source {
                face_sources.insert(owner, source.clone());
            }
            if let Some(map) = &face_map {
                face_maps.insert(owner, vec![map.clone()]);
            }
            scene.nodes[owner].mesh = Some(index);
            if let Some(binding) = &binding {
                inputs.insert(owner, binding.clone());
            }
        }
        scene.meshes.push(SceneMesh {
            name: name.clone(),
            primitives: vec![Primitive {
                geometry,
                material: scene.materials.len(),
            }],
        });
        scene.materials.push(material);
    }
    if let Some(name) = binding_node {
        ensure!(
            scene
                .nodes
                .iter()
                .any(|n| n.name == name && n.mesh.is_some()),
            "binding target `{name}` must be an existing mesh node after CAD evaluation"
        );
    }
    for clip in &mut scene.clips {
        for track in &mut clip.tracks {
            let name = &document.clip_targets[track.node];
            track.node = scene
                .nodes
                .iter()
                .position(|n| &n.name == name)
                .with_context(|| {
                    format!("animation target `{name}` is absent after CAD evaluation")
                })?;
        }
    }
    for (motion, model) in pending_motions {
        let clip = motion.bake(&scene, &model)?;
        scene.clips.push(clip);
    }
    scene.validate()?;
    let skins = document
        .skins
        .iter()
        .filter(|(name, _)| Some(name.as_str()) != binding_node)
        .map(|(name, file)| {
            let node = scene
                .nodes
                .iter()
                .position(|n| &n.name == name)
                .with_context(|| format!("skin target `{name}` is absent after CAD evaluation"))?;
            scene.nodes[node]
                .mesh
                .context("skin requires a mesh node")?;
            let source = inputs
                .get(&node)
                .context("skin authoring requires `project ROOT` in the asset file")?;
            if deferred_names.contains(name.as_str()) { return Ok(None); }
            let bytes = match crate::host::read(&directory.join(file)) {
                Ok(bytes) => bytes,
                Err(error) if binding_node.is_some() && error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error).with_context(|| format!("reading weights for node `{name}`")),
            };
            super::skin::WeightFile::from_json(&bytes)?.bind_with_inputs(&scene, node, source).map(Some)
        })
        .collect::<Result<Vec<_>>>()?.into_iter().flatten().collect();
    let mut asset = Asset {
        scene,
        skins,
        inputs,
        face_maps,
        face_sources,
    };
    let mut regions: BTreeMap<usize, Vec<(super::faces::SelectionFile, SurfaceSettings)>> =
        BTreeMap::new();
    for declaration in document.face_materials {
        let node = asset
            .scene
            .nodes
            .iter()
            .position(|n| n.name == declaration.node)
            .with_context(|| format!("face_material target `{}` is absent", declaration.node))?;
        let bytes = crate::host::read(&directory.join(&declaration.selection))
            .with_context(|| format!("reading face selection for `{}`", declaration.node))?;
        let record = super::faces::SelectionFile::from_json(&bytes)?;
        regions
            .entry(node)
            .or_default()
            .push((record, declaration.settings));
    }
    // Skin files bind the pre-material mesh. Material partitioning deterministically
    // remaps their complete tuples after binding; rebinding returns that same base.
    for (node, regions) in regions {
        let refs: Vec<_> = regions
            .iter()
            .map(|(record, settings)| (record, settings))
            .collect();
        let assigned = super::face_material::assign_regions(&asset, node, &refs)?;
        if binding_node.is_none() {
            let original_mesh = asset.scene.nodes[node].mesh.unwrap();
            let derived_mesh = assigned.scene.nodes[node].mesh.unwrap();
            let source_maps = asset.primitive_face_maps(node)?;
            let maps = assigned
                .source_triangles
                .iter()
                .zip(&assigned.scene.meshes[derived_mesh].primitives)
                .zip(&assigned.source_primitives)
                .map(|((indices, primitive), &p)| {
                    source_maps[p].subset(
                        &asset.scene.meshes[original_mesh].primitives[p].geometry,
                        &primitive.geometry,
                        indices,
                    )
                })
                .collect::<Result<Vec<_>>>()?;
            asset.scene = assigned.scene;
            asset.skins = assigned.skins;
            asset.face_maps.insert(node, maps);
        }
    }
    let mut uv_regions: BTreeMap<usize, std::collections::BTreeSet<(usize, usize)>> =
        BTreeMap::new();
    // Material partitions are established first. UV edits retain triangle order,
    // allowing disjoint declarations to keep addressing the same primitive faces.
    for declaration in document.face_uvs {
        let node = asset
            .scene
            .nodes
            .iter()
            .position(|n| n.name == declaration.node)
            .with_context(|| format!("face_uv target `{}` is absent", declaration.node))?;
        let bytes = crate::host::read(&directory.join(&declaration.selection))
            .with_context(|| format!("reading UV face selection for `{}`", declaration.node))?;
        let record = super::faces::SelectionFile::from_json(&bytes)?;
        let selected = asset.resolve_face_primitives(node, &record)?;
        let occupied = uv_regions.entry(node).or_default();
        for (p, indices) in selected.iter().enumerate() {
            for &i in indices {
                ensure!(occupied.insert((p, i)), "face_uv selections overlap");
            }
        }
        let edited = super::uv_edit::edit_scene(&asset, node, &record, declaration.transform)?;
        if binding_node.is_none() {
            asset.scene = edited.scene;
            asset.skins = edited.skins;
            asset.face_maps.insert(node, edited.face_maps);
        }
    }
    // Weight edits run after material/UV preparation, independently of text order.
    let mut weight_regions: BTreeMap<usize, std::collections::BTreeSet<(usize, usize)>> =
        BTreeMap::new();
    for declaration in document.face_weights {
        ensure!(
            document.skins.contains_key(&declaration.node),
            "face_weights target needs a skin declaration"
        );
        let node = asset
            .scene
            .nodes
            .iter()
            .position(|n| n.name == declaration.node)
            .with_context(|| format!("face_weights target `{}` is absent", declaration.node))?;
        let bytes = crate::host::read(&directory.join(&declaration.selection))?;
        let record = super::faces::SelectionFile::from_json(&bytes)?;
        let selected = asset.resolve_face_primitives(node, &record)?;
        let occupied = weight_regions.entry(node).or_default();
        for (p, indices) in selected.iter().enumerate() {
            for &i in indices {
                ensure!(occupied.insert((p, i)), "face_weights selections overlap");
            }
        }
        for (name, _) in &declaration.influences {
            ensure!(
                asset.scene.nodes.iter().any(|n| &n.name == name),
                "face_weights joint node does not exist"
            );
        }
        // Binding deliberately has no target skin yet. Palette membership and the
        // final prepared boundary are checked on normal load after binding.
        if binding_node.is_some() {
            continue;
        }
        let influences: Vec<_> = declaration
            .influences
            .iter()
            .map(|(n, w)| (n.as_str(), *w))
            .collect();
        let edited = super::face_weights::edit_scene(
            &asset,
            node,
            &record,
            &influences,
            declaration.boundary,
        )?;
        asset.scene = edited.scene;
        asset.skins = edited.skins;
        asset.face_maps.insert(node, edited.face_maps);
    }
    if let Some(profile)=&asset.scene.consumer {profile.validate_skins(&asset.scene,&asset.skins)?;}
    Ok(asset)
}

fn prepare_mesh(
    model: &crate::model::Model,
    body: &str,
    solid: &monstertruck::modeling::Solid,
    source: &Source,
    textures: &[super::texture_input::Loaded],
    with_faces: bool,
) -> Result<(
    Mesh,
    super::material::Material,
    Option<super::faces::FaceMap>,
)> {
    let (mesh, mut face_map) = if with_faces {
        let (mesh, map) = super::faces::tessellate(solid, source.tolerance_mm)?;
        (mesh, Some(map))
    } else {
        (Mesh::from_solid(solid, source.tolerance_mm)?, None)
    };
    let mesh = match source.refinement {
        Some((levels, budget)) => {
            if let Some(map) = &face_map {
                let (mesh, map) = map.refined(&mesh, levels, budget)?;
                face_map = Some(map);
                mesh
            } else {
                mesh.refined(levels, budget)?
            }
        }
        None => mesh,
    };
    let mesh = match &source.slicing {
        Some(s) => {
            if let Some(map) = &face_map {
                let (mesh, map) = map.sliced(&mesh, s.axis, &s.planes_mm, s.triangle_budget)?;
                face_map = Some(map);
                mesh
            } else {
                mesh.sliced(s.axis, &s.planes_mm, s.triangle_budget)?
            }
        }
        None => mesh,
    };
    let mesh = match source.normal_crease_degrees {
        Some(angle) => {
            if let Some(map) = &face_map {
                let (mesh, map) = map.crease_normals(&mesh, angle)?;
                face_map = Some(map);
                mesh
            } else {
                mesh.crease_normals(angle)?
            }
        }
        None => mesh,
    };
    let mut finish = model
        .materials
        .get(body)
        .and_then(|m| m.finish.clone())
        .unwrap_or_default();
    if source.unwrap.is_some() {
        ensure!(!finish.texture.as_ref().is_some_and(|t| t.shown()),
            "unwrap needs newly painted maps; remove inherited CAD textures before unwrapping");
        finish.texture = None;
    } else {
        let texture = finish.texture.get_or_insert_default();
        texture.tile = Some(source.tile_mm);
        texture.turn = Some(source.rotation_degrees);
    }
    let colour = model.colours.get(body).copied().unwrap_or([0.7; 3]);
    // Model colours already include material defaults and later explicit overrides.
    // Do not let the retained library finish overwrite the evaluated body colour.
    finish.colour = Some(colour);
    let opacity = model.opacities.get(body).copied().unwrap_or(1.0);
    let (projected, mut material) = bake::finish(&mesh, &finish, colour, opacity)?;
    let projected = match source.unwrap {
        Some(settings) => super::unwrap::mesh(&projected, settings)?,
        None => projected,
    };
    let mesh = if let Some(map) = &face_map {
        let (mesh, map) = map.retain_triangles(&mesh, projected)?;
        face_map = Some(map);
        mesh
    } else {
        projected
    };
    for texture in textures {
        texture.apply(&mut material);
    }
    material.validate()?;
    let material = match &source.surface {
        Some(settings) => settings.apply(&material)?,
        None => material,
    };
    if let Some(settings) = source.atlas {
        let atlas = super::atlas::pack(
            &mesh,
            settings.width,
            settings.height,
            settings.padding,
            settings.density,
        )
        .with_context(|| format!("packing atlas for body `{body}`"))?;
        let material = if let Some(max_level) = settings.mips {
            if settings.mask_coverage {
                atlas
                    .rebake_mask_chain(&material, max_level)?
                    .into_material()
            } else {
                atlas.rebake_chain(&material, max_level)?.into_material()
            }
        } else {
            atlas.rebake(&material, settings.filter)?
        };
        let mesh = if let Some(map) = &face_map {
            let (mesh, map) = map.retain_triangles(&mesh, atlas.mesh)?;
            face_map = Some(map);
            mesh
        } else {
            atlas.mesh
        };
        Ok((mesh, material, face_map))
    } else {
        Ok((mesh, material, face_map))
    }
}

pub(crate) fn capture_inputs(
    root: Option<&Path>,
    path: &Path,
    name: &str,
    source: &Source,
    vars: &[(String, f64)],
    reads: &crate::host::Reads,
) -> Result<Option<super::provenance::Inputs>> {
    let Some(root) = root else {
        return Ok(None);
    };
    {
        let root = super::provenance::input_path(root)?;
        let path = super::provenance::input_path(path)?;
        let relative = path
            .strip_prefix(&root)
            .context("CAD source lies outside the project root")?;
        let relative = relative
            .to_str()
            .context("source path is not UTF-8")?
            .replace('\\', "/");
        let mut parameters = BTreeMap::new();
        for (name, value) in vars {
            ensure!(
                parameters.insert(name.clone(), *value).is_none(),
                "duplicate variable override"
            );
        }
        let mut inputs = super::provenance::Inputs {
            source: relative,
            body: source.body.clone(),
            instance: name.into(),
            parameters,
            dependencies: BTreeMap::new(),
            observations: BTreeMap::new(),
            tolerance_mm: source.tolerance_mm,
            generator: format!(
                "gcad-{}/asset-pipeline-2/box-uv-1/tile={}/rotation={}",
                env!("CARGO_PKG_VERSION"),
                source.tile_mm,
                source.rotation_degrees
            ),
        }
        .with_recorded_reads(&root, reads)?;
        if let Some(s) = source.unwrap {
            inputs.generator = format!("gcad-{}/asset-pipeline-2/xatlas-rs-v2-0.1.4/resolution={}/padding={}/normal-seams=1001/rotation=false",
                env!("CARGO_PKG_VERSION"), s.resolution, s.padding);
        }
        if let Some((levels, budget)) = source.refinement {
            inputs
                .generator
                .push_str(&format!("/refine-1/levels={levels}/budget={budget}"));
        }
        if let Some(angle) = source.normal_crease_degrees {
            inputs
                .generator
                .push_str(&format!("/crease-normals-1/angle={angle}"));
        }
        if let Some(s) = &source.slicing {
            inputs.generator.push_str(&format!(
                "/slice-1/axis={}/planes={:?}/budget={}",
                s.axis, s.planes_mm, s.triangle_budget
            ));
        }
        Ok(Some(inputs))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn first_binding_allows_unprepared_peers_but_rejects_invalid_existing_weights() {
        use super::*;
        use crate::asset::skin::{Assignment, Skin};
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(format!(
            "multi-binding-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("model.gasset");
        let text = "asset 1\nproject ../..\nsource plate ../../examples/parts/plate.gcad body=main\ntessellate plate tolerance=0.5\nuv plate projection=box tile=25\nnode first source=plate\nnode second source=plate translation=50,0,0\nnode bone\nskin first weights=first.json\nskin second weights=second.json\n";
        std::fs::write(&path, text).unwrap();
        assert!(load(&path, &[]).is_err(), "normal load must require both resources");
        let first = load_for_binding(&path, &[], "first").unwrap();
        assert!(first.skins.is_empty());
        let skin = Skin::assign(&first.scene, 0, vec![2], Assignment::Rigid).unwrap();
        let first_bytes = first.weight_file(&skin).unwrap().to_json().unwrap();
        std::fs::write(root.join("first.json"), &first_bytes).unwrap();
        let second = load_for_binding(&path, &[], "second").unwrap();
        assert_eq!(second.skins.len(), 1, "existing peer must still bind");
        assert!(load(&path, &[]).is_err(), "unfinished peer must not pass export loading");
        let skin = Skin::assign(&second.scene, 1, vec![2], Assignment::Rigid).unwrap();
        std::fs::write(root.join("second.json"), second.weight_file(&skin).unwrap().to_json().unwrap()).unwrap();
        let ready = load(&path, &[]).unwrap();
        assert_eq!(ready.skins.len(), 2);
        ready.encode_glb().unwrap();
        std::fs::write(root.join("first.json"), b"invalid JSON").unwrap();
        assert!(load_for_binding(&path, &[], "second").is_err(), "malformed peer must not be silently skipped");
        std::fs::write(root.join("first.json"), &first_bytes).unwrap();
        std::fs::write(&path, text.replace("node bone", "node bone translation=0,0,1")).unwrap();
        assert!(load_for_binding(&path, &[], "second").is_err(), "stale peer must still require explicit rebinding");
        let old_second = std::fs::read(root.join("second.json")).unwrap();
        for invalid in [vec!["absent".into()], vec!["second".into()], vec!["first".into(), "first".into()]] {
            assert!(load_for_binding_deferred(&path, &[], "second", &invalid).is_err());
        }
        let repair = load_for_binding_deferred(&path, &[], "second", &["first".into()]).unwrap();
        assert!(repair.skins.is_empty());
        let skin = Skin::assign(&repair.scene, 1, vec![2], Assignment::Rigid).unwrap();
        std::fs::write(root.join("second-v2.json"), repair.weight_file(&skin).unwrap().to_json().unwrap()).unwrap();
        let partially_repaired = text.replace("node bone", "node bone translation=0,0,1")
            .replace("second.json", "second-v2.json");
        std::fs::write(&path, &partially_repaired).unwrap();
        assert!(load(&path, &[]).is_err(), "deferred stale peer must still block ordinary loading");
        let repair = load_for_binding(&path, &[], "first").unwrap();
        assert_eq!(repair.skins.len(), 1);
        let skin = Skin::assign(&repair.scene, 0, vec![2], Assignment::Rigid).unwrap();
        std::fs::write(root.join("first-v2.json"), repair.weight_file(&skin).unwrap().to_json().unwrap()).unwrap();
        std::fs::write(&path, partially_repaired.replace("first.json", "first-v2.json")).unwrap();
        let repaired = load(&path, &[]).unwrap();
        assert_eq!(repaired.skins.len(), 2);
        repaired.encode_glb().unwrap();
        assert_eq!(std::fs::read(root.join("first.json")).unwrap(), first_bytes);
        assert_eq!(std::fs::read(root.join("second.json")).unwrap(), old_second);
        std::fs::write(&path, text.replace("skin first weights=first.json", "skin absent weights=absent.json")).unwrap();
        assert!(load_for_binding(&path, &[], "second").is_err(), "missing resource must not hide an invalid node reference");
        assert!(load_for_binding_deferred(&path, &[], "second", &["absent".into()]).is_err(), "deferral must not hide an invalid node reference");
        for file in ["model.gasset", "first.json", "second.json", "first-v2.json", "second-v2.json"] { std::fs::remove_file(root.join(file)).unwrap(); }
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn shader_authoring_tracks_description_without_changing_geometry_bindings() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let path=root.join("examples/game-assets/plate-shader.gasset");
        let (asset, reads)=crate::host::recording(|| super::load(&path,&[]));
        let asset=asset.unwrap();
        assert_eq!(asset.scene.materials.len(),1);
        let shader=asset.scene.materials[0].shader.as_ref().unwrap();
        assert_eq!(shader.at(2.).unwrap().offset,[0.25,0.]);
        assert!(reads.files().any(|path| path.ends_with("uv-scroll.shader.json")));
        assert!(!asset.inputs.is_empty());
        for inputs in asset.inputs.values() {
            assert!(inputs.dependencies.keys().all(|path| !path.ends_with("uv-scroll.shader.json")));
        }
        assert!(asset.encode_glb().unwrap_err().to_string().contains("shader export"));
        let text=include_str!("../../examples/game-assets/plate-shader.gasset");
        for invalid in [text.replace("shader plate", "shader absent"),
            format!("{text}\nshader plate other.json"),
            text.replace("uv-scroll.shader.json", "uv-scroll.shader.json unknown=true")] {
            assert!(super::Document::parse(&invalid).is_err());
        }
    }
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn authored_mip_chains_are_shared_guarded_and_not_silently_dropped() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/game-assets/atlas-mips.gasset");
        let asset = super::load(&path, &[]).unwrap();
        assert_eq!(asset.scene.materials.len(), 1);
        assert_eq!(asset.scene.meshes.len(), 1);
        assert_eq!(
            asset
                .scene
                .nodes
                .iter()
                .filter(|n| n.mesh == Some(0))
                .count(),
            2
        );
        let material = &asset.scene.materials[0];
        let chain = material.mip_chain.as_ref().unwrap();
        assert_eq!(chain.max_lod(), 2);
        assert_eq!(chain.levels().len(), 3);
        assert!(chain.levels().iter().all(|m| m.mip_chain.is_none()));
        let clone = asset.scene.clone();
        assert!(std::sync::Arc::ptr_eq(
            chain,
            clone.materials[0].mip_chain.as_ref().unwrap()
        ));
        assert!(
            asset
                .encode_glb()
                .unwrap_err()
                .to_string()
                .contains("does not yet transport explicit mip")
        );
        crate::view::asset::frame(&asset, None, 0.).unwrap();
        let images = crate::view::asset::mip_images(chain).unwrap();
        assert!(images.iter().all(|slot| slot.len() == 3));
        let mut changed = material.clone();
        changed.colour_map.as_mut().unwrap().rgba[0] ^= 1;
        assert!(
            changed
                .validate()
                .unwrap_err()
                .to_string()
                .contains("material changed")
        );
        changed = material.clone();
        changed.roughness = 0.5;
        assert!(changed.validate().is_err());
        let mut geometry = asset.scene.clone();
        geometry.meshes[0].primitives[0]
            .geometry
            .texcoords
            .as_mut()
            .unwrap()[0][0] += 0.0001;
        assert!(
            geometry
                .validate()
                .unwrap_err()
                .to_string()
                .contains("geometry changed")
        );
        asset.scene.validate().unwrap();
        let text = std::fs::read_to_string(path).unwrap();
        for bad in [
            text.replace("mips=2", "mips=0"),
            text.replace("mips=2", "mips=14"),
            text.replace("mips=2", "mips=-1"),
            text.replace("filter=linear mips=2", "filter=nearest mips=2"),
        ] {
            assert!(super::Document::parse(&bad).is_err());
        }
    }
    use super::*;
    #[test]
    fn authored_crease_normals_preserve_faces_and_guard_the_setting() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/game-assets/plate-normals.gasset");
        let asset = load(&path, &[]).unwrap();
        let get = |name: &str| {
            let node = asset
                .scene
                .nodes
                .iter()
                .position(|n| n.name == name)
                .unwrap();
            let geometry =
                &asset.scene.meshes[asset.scene.nodes[node].mesh.unwrap()].primitives[0].geometry;
            (node, geometry)
        };
        let (hard_node, hard) = get("hard");
        let (smooth_node, smooth) = get("smooth");
        assert_eq!(hard.triangles.len(), smooth.triangles.len());
        let mut smooth_corners = 0;
        for (a, b) in hard.triangles.iter().zip(&smooth.triangles) {
            let positions = a.map(|i| hard.positions[i as usize]);
            assert_eq!(positions, b.map(|i| smooth.positions[i as usize]));
            let n = super::super::cross(
                super::super::sub(positions[1], positions[0]),
                super::super::sub(positions[2], positions[0]),
            );
            let length = super::super::dot(n, n).sqrt();
            let n = n.map(|x| x / length);
            for k in 0..3 {
                assert!(super::super::dot(n, hard.normals[a[k] as usize]) > 1. - 1e-10);
                if super::super::dot(n, smooth.normals[b[k] as usize]) < 1. - 1e-6 {
                    smooth_corners += 1;
                }
            }
        }
        assert!(smooth_corners > 0);
        assert!(
            asset.inputs[&hard_node]
                .generator
                .contains("/crease-normals-1/angle=0")
        );
        assert!(
            asset.inputs[&smooth_node]
                .generator
                .contains("/crease-normals-1/angle=60")
        );
        asset.encode_glb().unwrap();
        let text = std::fs::read_to_string(path).unwrap();
        for invalid in [
            "normals hard crease=30",
            "normals missing crease=10",
            "normals hard angle=30",
        ] {
            assert!(Document::parse(&format!("{text}\n{invalid}")).is_err());
        }
    }
    #[test]
    fn explicit_maps_embed_provenance_and_guard_changed_texture_inputs() {
        use super::super::skin::{Assignment, Skin};
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = repo.join(format!("target/map-input-{nonce}"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir(root.join("maps")).unwrap();
        for name in ["rgba.png", "numeric.png"] {
            std::fs::copy(
                repo.join("examples/game-assets/maps").join(name),
                root.join("maps").join(name),
            )
            .unwrap();
        }
        let text = include_str!("../../examples/game-assets/plate-maps.gasset")
            .replace("../parts/plate.gcad", "../../examples/parts/plate.gcad")
            + "\nnode bone\nskin plate weights=plate.weights.json\n";
        let path = root.join("plate.gasset");
        std::fs::write(&path, &text).unwrap();
        let asset = load_for_binding(&path, &[], "plate").unwrap();
        let material = &asset.scene.materials[0];
        assert_eq!(material.texture_sources.len(), 6);
        assert_eq!(
            material.colour_map.as_ref().unwrap().rgba[..4],
            [255, 32, 16, 64]
        );
        assert_eq!(
            material.portable_normal().unwrap().unwrap().rgba[..4],
            [128, 191, 255, 255]
        );
        assert!(
            material
                .texture_sources
                .iter()
                .all(|r| r.project_path.starts_with("target/map-input-")
                    && r.license == "GPL-3.0-or-later")
        );
        let skin = Skin::assign(&asset.scene, 0, vec![1], Assignment::Rigid).unwrap();
        std::fs::write(
            root.join("plate.weights.json"),
            asset.weight_file(&skin).unwrap().to_json().unwrap(),
        )
        .unwrap();
        let loaded = load(&path, &[]).unwrap();
        let bytes = loaded.encode_glb().unwrap();
        let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + length]).unwrap();
        assert_eq!(json["images"].as_array().unwrap().len(), 5);
        assert_eq!(
            json["materials"][0]["extras"]["gcad_texture_source_inputs"]
                .as_array()
                .unwrap()
                .len(),
            6
        );
        let mut changed = material.colour_map.clone().unwrap();
        changed.rgba[0] = 254;
        std::fs::write(
            root.join("maps/rgba.png"),
            super::super::material::png(&changed).unwrap(),
        )
        .unwrap();
        assert!(
            load(&path, &[]).is_err(),
            "changed map bypassed source guard"
        );
        std::fs::remove_file(root.join("maps/rgba.png")).unwrap();
        assert!(
            load_for_binding(&path, &[], "plate").is_err(),
            "missing map was ignored"
        );
        for bad in [
            text.replace("base_colour maps/rgba.png", "unknown maps/rgba.png"),
            text.replace("map plate normal", "map missing normal"),
            text.replace("license=GPL-3.0-or-later", "unused=GPL-3.0-or-later"),
            format!(
                "{text}\nmap plate normal maps/numeric.png license=GPL-3.0-or-later origin=test"
            ),
        ] {
            assert!(Document::parse(&bad).is_err());
        }
        for file in ["maps/numeric.png", "plate.gasset", "plate.weights.json"] {
            std::fs::remove_file(root.join(file)).unwrap();
        }
        std::fs::remove_dir(root.join("maps")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn authored_surfaces_override_only_the_derived_source_materials() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let asset = load(
            &root.join("examples/game-assets/plate-surfaces.gasset"),
            &[],
        )
        .unwrap();
        let material = |name: &str| {
            let node = asset.scene.nodes.iter().find(|n| n.name == name).unwrap();
            &asset.scene.materials[asset.scene.meshes[node.mesh.unwrap()].primitives[0].material]
        };
        let masked = material("masked");
        let blended = material("blended");
        assert_eq!(masked.base_colour, [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(masked.metallic, 0.25);
        assert_eq!(masked.roughness, 0.75);
        assert_eq!(masked.emissive, [0.05, 0.1, 0.15]);
        assert_eq!(
            masked.alpha,
            super::super::material::AlphaMode::Mask { cutoff: 0.3 }
        );
        assert!(masked.double_sided && !blended.double_sided);
        assert_eq!(blended.alpha, super::super::material::AlphaMode::Blend);
        assert_eq!(blended.base_colour, [0.6, 0.4, 0.2, 0.5]);
        assert!(asset.encode_glb().is_ok());
        let text = include_str!("../../examples/game-assets/plate-surfaces.gasset");
        for bad in [
            text.replace("surface masked", "surface missing"),
            format!("{text}\nsurface masked alpha=opaque"),
            text.replace("alpha=blend", "alpha=blend alpha=opaque"),
        ] {
            assert!(Document::parse(&bad).is_err());
        }
    }
    #[test]
    fn sliced_cad_twist_fixture_survives_uvs_and_float32_export() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let asset = load_for_binding(
            &root.join("examples/game-assets/plate-twist-sliced.gasset"),
            &[],
            "plate",
        )
        .unwrap();
        let mesh = &asset.scene.meshes[0].primitives[0].geometry;
        assert!(mesh.triangles.len() > 1276 && mesh.triangles.len() <= 20416);
        assert!(mesh.texcoords.is_some() && mesh.tangents.is_some());
        assert!(asset.encode_glb().unwrap().starts_with(b"glTF"));
    }
    #[test]
    fn imported_part_track_resolves_after_expansion_and_leaves_other_instance_at_rest() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = root.join("examples/game-assets/linkage-part-motion.gasset");
        let asset = load(&path, &[]).unwrap();
        let first = asset
            .scene
            .nodes
            .iter()
            .position(|n| n.name == "first/crank")
            .unwrap();
        let second = asset
            .scene
            .nodes
            .iter()
            .position(|n| n.name == "second/crank")
            .unwrap();
        let clip = &asset.scene.clips[0];
        assert_eq!(clip.tracks[0].node, first);
        let start = clip.sample(&asset.scene, 0., false).unwrap();
        let end = clip.sample(&asset.scene, 1., false).unwrap();
        assert_eq!(end[first].translation[2] - start[first].translation[2], 10.);
        assert_eq!(end[second], asset.scene.nodes[second].transform);
        assert_eq!(
            end[first].rotation,
            asset.scene.nodes[first].transform.rotation
        );
        let bytes = asset.encode_glb().unwrap();
        let size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + size]).unwrap();
        assert_eq!(
            json["animations"][0]["channels"][0]["target"]["node"],
            first
        );
        let text = include_str!("../../examples/game-assets/linkage-part-motion.gasset");
        assert!(Document::parse(&text.replace("first/crank", "unknown/crank")).is_err());
        assert!(Document::parse(&text.replace("time=1", "time=0")).is_err());
        let missing = text.replace("first/crank", "first/absent").replace(
            "../assemblies/linkage.gasm",
            &format!(
                "\"{}\"",
                root.join("examples/assemblies/linkage.gasm")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        );
        let temporary = root
            .join("target")
            .join(format!("asset-missing-part-{}.gasset", std::process::id()));
        std::fs::write(&temporary, missing).unwrap();
        let error = load(&temporary, &[]).err().unwrap().to_string();
        std::fs::remove_file(temporary).unwrap();
        assert!(error.contains("absent after CAD evaluation"), "{error}");
    }
    #[test]
    fn assembly_sources_preserve_placements_materials_and_repeated_source_instances() {
        use monstertruck::modeling::{Point3, Transform as _};
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let asset = load(&root.join("examples/game-assets/linkage.gasset"), &[]).unwrap();
        let run = crate::model::run_path(&root.join("examples/assemblies/linkage.gasm"), &[], None)
            .unwrap();
        assert!(run.steps.iter().all(|(_, result)| result.is_ok()));
        let bodies = run.model.named_solids();
        assert_eq!(asset.scene.nodes.len(), 2 + bodies.len() * 2);
        assert!(asset.scene.nodes[..2].iter().all(|n| n.mesh.is_none()));
        let world = asset.scene.world_transforms().unwrap();
        let bounds = |points: Vec<[f64; 3]>| {
            let mut low = [f64::INFINITY; 3];
            let mut high = [f64::NEG_INFINITY; 3];
            for p in points {
                for k in 0..3 {
                    low[k] = low[k].min(p[k]);
                    high[k] = high[k].max(p[k]);
                }
            }
            (low, high)
        };
        for (name, solid) in bodies {
            let first = asset
                .scene
                .nodes
                .iter()
                .position(|n| n.name == format!("first/{name}"))
                .unwrap();
            let second = asset
                .scene
                .nodes
                .iter()
                .position(|n| n.name == format!("second/{name}"))
                .unwrap();
            assert_eq!(
                asset.scene.nodes[first].mesh,
                asset.scene.nodes[second].mesh
            );
            let primitive =
                &asset.scene.meshes[asset.scene.nodes[first].mesh.unwrap()].primitives[0];
            let (expected_low, expected_high) =
                bounds(Mesh::from_solid(solid, 0.1).unwrap().positions);
            for (node, offset) in [(first, 0.), (second, 100.)] {
                let (low, high) = bounds(
                    primitive
                        .geometry
                        .positions
                        .iter()
                        .map(|p| {
                            let p = world[node].transform_point(Point3::new(p[0], p[1], p[2]));
                            [p.x - offset, p.y, p.z]
                        })
                        .collect(),
                );
                for k in 0..3 {
                    assert!(
                        (low[k] - expected_low[k]).abs() < 0.11,
                        "{name} lower bound"
                    );
                    assert!(
                        (high[k] - expected_high[k]).abs() < 0.11,
                        "{name} upper bound"
                    );
                }
            }
            if name == "crank" {
                let colour = asset.scene.materials[primitive.material].base_colour;
                assert!(colour[0] > colour[1] * 2. && colour[0] > colour[2] * 2.);
            }
        }
        assert_eq!(asset.scene.clips[0].tracks[0].node, 0);
        assert!(asset.encode_glb().unwrap().starts_with(b"glTF"));
        let budget = asset
            .scene
            .meshes
            .iter()
            .flat_map(|m| &m.primitives)
            .map(|p| p.geometry.triangles.len())
            .sum::<usize>()
            * 4
            - 1;
        let path = root.join("target").join(format!(
            "asset-assembly-budget-{}.gasset",
            std::process::id()
        ));
        let text = include_str!("../../examples/game-assets/linkage.gasset").replace(
            "../assemblies/linkage.gasm",
            &format!(
                "\"{}\"",
                root.join("examples/assemblies/linkage.gasm")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        );
        for operation in [
            format!("refine linkage levels=1 triangle_budget={budget}"),
            format!(
                "slice linkage axis=y planes=1000000 triangle_budget={}",
                budget / 4
            ),
        ] {
            std::fs::write(&path, format!("{text}\n{operation}\n")).unwrap();
            let error = load(&path, &[]).err().unwrap().to_string();
            assert!(error.contains("triangle budget"), "{error}");
        }
        std::fs::remove_file(path).unwrap();
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn imported_part_skin_preserves_source_identity_for_shared_geometry() {
        use crate::asset::skin::{Assignment, Skin};
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("target/part-skin-{nonce}"));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("part.gasset");
        let text = "asset 1\nproject ../..\nsource a ../../examples/assemblies/linkage.gasm\nsource b ../../examples/assemblies/linkage.gasm\ntessellate a tolerance=0.1\ntessellate b tolerance=0.1\nuv a projection=box tile=25\nuv b projection=box tile=25\nnode first source=a\nnode second source=b translation=100,0,0\nnode bone\nskin second/crank weights=part.weights.json\n";
        std::fs::write(&path, text).unwrap();
        let asset = load_for_binding(&path, &[], "second/crank").unwrap();
        let node = asset
            .scene
            .nodes
            .iter()
            .position(|n| n.name == "second/crank")
            .unwrap();
        let first = asset
            .scene
            .nodes
            .iter()
            .position(|n| n.name == "first/crank")
            .unwrap();
        assert_eq!(asset.scene.nodes[node].mesh, asset.scene.nodes[first].mesh);
        assert_eq!(asset.inputs[&node].instance, "b/crank");
        assert_eq!(asset.inputs[&first].instance, "a/crank");
        let bone = asset
            .scene
            .nodes
            .iter()
            .position(|n| n.name == "bone")
            .unwrap();
        let skin = Skin::assign(&asset.scene, node, vec![bone], Assignment::Rigid).unwrap();
        std::fs::write(
            root.join("part.weights.json"),
            asset.weight_file(&skin).unwrap().to_json().unwrap(),
        )
        .unwrap();
        let loaded = load(&path, &[]).unwrap();
        assert_eq!(loaded.skins[0].mesh_node, node);
        let bytes = loaded.encode_glb().unwrap();
        let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + length]).unwrap();
        assert!(json["nodes"][first]["skin"].is_null());
        assert_eq!(json["nodes"][node]["skin"], 0);
        assert_ne!(json["nodes"][first]["mesh"], json["nodes"][node]["mesh"]);
        assert!(load(&path, &[("unused_override".into(), 1.0)]).is_err());
        // Rebinding skips only this resource; ordinary loads reject corrupt weights.
        std::fs::write(root.join("part.weights.json"), "invalid").unwrap();
        assert!(load(&path, &[]).is_err());
        assert!(load_for_binding(&path, &[], "second/crank").is_ok());
        assert!(load_for_binding(&path, &[], "second/missing").is_err());
        assert!(load_for_binding(&path, &[], "second").is_err());
        std::fs::write(
            &path,
            text.replace("skin second/crank", "skin second/missing"),
        )
        .unwrap();
        let error = load(&path, &[]).err().unwrap().to_string();
        assert!(
            error.contains("second/missing") && error.contains("after CAD evaluation"),
            "{error}"
        );
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(root.join("part.weights.json")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn authored_skin_loads_persisted_weights_and_rejects_regenerated_geometry() {
        use crate::asset::skin::{Skin, Weights};
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "target/asset-skin-author-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("plate.gasset");
        let source_bytes =
            std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/parts/plate.gcad"))
                .unwrap();
        std::fs::write(root.join("plate.gcad"), &source_bytes).unwrap();
        let text = "asset 1\nproject ../..\nsource plate plate.gcad\ntessellate plate tolerance=0.1\nuv plate projection=box tile=25\nnode plate source=plate\nnode bone\n";
        std::fs::write(&path, text).unwrap();
        let asset = load(&path, &[]).unwrap();
        let count = asset.scene.meshes[0].primitives[0].geometry.positions.len();
        let skin = Skin::bind(
            &asset.scene,
            0,
            vec![1],
            vec![vec![
                Weights {
                    joints: [0; 4],
                    weights: [1.0, 0.0, 0.0, 0.0]
                };
                count
            ]],
        )
        .unwrap();
        std::fs::write(
            root.join("plate.weights.json"),
            asset.weight_file(&skin).unwrap().to_json().unwrap(),
        )
        .unwrap();
        let text = format!("{text}skin plate weights=plate.weights.json\n");
        std::fs::write(&path, &text).unwrap();
        let loaded = load(&path, &[]).unwrap();
        assert_eq!(loaded.skins.len(), 1);
        let bytes = loaded.encode_glb().unwrap();
        let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + length]).unwrap();
        assert_eq!(json["skins"].as_array().unwrap().len(), 1);
        // Atlas remapping preserves complete vertex order and geometry, so existing
        // geometry/source guards and weights remain valid without rebinding.
        std::fs::write(
            &path,
            format!("{text}atlas plate width=256 height=256 padding=2 density=16 filter=linear\n"),
        )
        .unwrap();
        let atlased = load(&path, &[]).unwrap();
        let original = &loaded.scene.meshes[0].primitives[0].geometry;
        let mapped = &atlased.scene.meshes[0].primitives[0].geometry;
        assert_eq!(original.positions, mapped.positions);
        assert_eq!(original.normals, mapped.normals);
        assert_eq!(original.triangles, mapped.triangles);
        assert_ne!(original.texcoords, mapped.texcoords);
        assert_eq!(
            loaded
                .weight_file(&loaded.skins[0])
                .unwrap()
                .to_json()
                .unwrap(),
            atlased
                .weight_file(&atlased.skins[0])
                .unwrap()
                .to_json()
                .unwrap()
        );
        assert!(atlased.encode_glb().is_ok());
        std::fs::write(&path, &text).unwrap();
        let mut changed_bytes = source_bytes.clone();
        changed_bytes.push(b'\n');
        std::fs::write(root.join("plate.gcad"), changed_bytes).unwrap();
        let changed = load(&path, &[]).err().unwrap().to_string();
        assert!(changed.contains("CAD inputs changed"), "{changed}");
        std::fs::write(root.join("plate.gcad"), source_bytes).unwrap();
        assert!(load(&path, &[("unused_override".into(), 1.0)]).is_err());
        assert!(load(&path, &[("w".into(), 41.0)]).is_err());
        std::fs::write(
            &path,
            text.replace("node bone", "node bone translation=1,0,0"),
        )
        .unwrap();
        assert!(load(&path, &[]).is_err());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(root.join("plate.weights.json")).unwrap();
        std::fs::remove_file(root.join("plate.gcad")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn authored_clips_resolve_nodes_and_reject_bad_key_sequences() {
        let text = include_str!("../../examples/game-assets/plate-motion.gasset");
        let doc = Document::parse(text).unwrap();
        assert_eq!(doc.clips.len(), 2);
        assert_eq!(doc.clips[0].name, "slide");
        assert_eq!(doc.clips[0].tracks[0].node, 0);
        assert_eq!(doc.clips[1].tracks[0].node, 1);
        assert_eq!(doc.clips[1].duration(), 2.0);
        for bad in [
            text.replace("key swing pivot", "key unknown pivot"),
            text.replace("key swing pivot", "key swing unknown"),
            text.replace("time=2", "time=0"),
            text.replace("time=2", "time=-1"),
            text.replace("interpolation=linear", "interpolation=cubic"),
            format!("{text}\nclip empty interpolation=step"),
            text.replace("rotation=0,0,0,1", "rotation=0,0,0,0"),
        ] {
            assert!(Document::parse(&bad).is_err(), "accepted {bad}");
        }
    }
    #[test]
    fn explicit_nodes_share_meshes_resolve_forward_parents_and_reject_invalid_graphs() {
        let text = include_str!("../../examples/game-assets/plate-instances.gasset");
        let doc = Document::parse(text).unwrap();
        let nodes = doc.scene_nodes().unwrap();
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].mesh, nodes[1].mesh);
        assert_eq!(nodes[0].parent, Some(2));
        assert_eq!(nodes[0].transform.scale, [-2.0, 3.0, 4.0]);
        for bad in [
            text.replace("parent=assembly", "parent=unknown"),
            text.replace("source=plate", "source=unknown"),
            text.replace("node assembly", "node assembly parent=mirror"),
            text.replace("node copy", "node mirror"),
            text.replace("scale=-2,3,4", "scale=0,3,4"),
            text.replace("translation=50,0,0", "translation=50,0"),
            text.replace(
                "rotation=0,0,0.7071067811865476,0.7071067811865476",
                "rotation=0,0,0,0",
            ),
            text.replace("source=plate", ""),
        ] {
            assert!(Document::parse(&bad).is_err(), "accepted {bad}");
        }
    }
    #[test]
    fn strict_versioned_authoring_with_forward_settings_and_quoted_paths() {
        let text = "asset 1\nuv prop projection=box tile=250 rotation=90\nsource prop \"parts/a b.gcad\" body=main\ntessellate prop tolerance=0.1\n";
        let doc = Document::parse(text).unwrap();
        assert_eq!(doc.sources["prop"].path, "parts/a b.gcad");
        assert_eq!(doc.sources["prop"].rotation_degrees, 90.0);
        let atlas =
            format!("{text}atlas prop width=256 height=128 padding=2 density=16 filter=linear\n");
        let parsed = Document::parse(&atlas).unwrap().sources["prop"]
            .atlas
            .unwrap();
        assert_eq!(
            (parsed.width, parsed.height, parsed.padding, parsed.density),
            (256, 128, 2, 16.)
        );
        for bad in [
            atlas.replace("width=256", "width=0"),
            atlas.replace("padding=2", "padding=0"),
            atlas.replace("density=16", "density=NaN"),
            atlas.replace("filter=linear", "filter=cubic"),
            atlas.replace("atlas prop", "atlas missing"),
            format!("{atlas}atlas prop width=256 height=128 padding=2 density=16 filter=linear"),
            atlas.replace("height=128", "extra=128"),
        ] {
            assert!(Document::parse(&bad).is_err());
        }
        let refined = format!("{text}refine prop levels=2 triangle_budget=1000\n");
        let sliced = format!("{text}slice prop axis=y planes=-1,0,1 triangle_budget=1000\n");
        let settings = Document::parse(&sliced).unwrap().sources["prop"]
            .slicing
            .clone()
            .unwrap();
        assert_eq!(settings.axis, 1);
        assert_eq!(settings.planes_mm, [-1., 0., 1.]);
        for bad in [
            sliced.replace("axis=y", "axis=q"),
            sliced.replace("planes=-1,0,1", "planes=0,0"),
            sliced.replace("planes=-1,0,1", "planes=NaN"),
            sliced.replace("triangle_budget=1000", "triangle_budget=0"),
            sliced.replace("slice prop", "slice missing"),
            format!("{sliced}slice prop axis=y planes=0 triangle_budget=1000"),
            format!("{sliced}refine prop levels=1 triangle_budget=1000"),
        ] {
            assert!(Document::parse(&bad).is_err(), "accepted {bad}");
        }
        assert_eq!(
            Document::parse(&refined).unwrap().sources["prop"].refinement,
            Some((2, 1000))
        );
        for bad in [
            refined.replace("levels=2", "levels=0"),
            refined.replace("levels=2", "levels=9"),
            refined.replace("triangle_budget=1000", "triangle_budget=0"),
            refined.replace("refine prop", "refine missing"),
            format!("{refined}refine prop levels=1 triangle_budget=1000"),
        ] {
            assert!(Document::parse(&bad).is_err());
        }
        for bad in [
            text.replace("asset 1", "asset 2"),
            text.replace("tile=250", "tile=0"),
            text.replace("projection=box", "projection=sphere"),
            text.replace("tessellate prop", "tessellate missing"),
            text.replace("tolerance=0.1", "tolerance=0.1 extra=1"),
            format!("{text}uv prop projection=box tile=1"),
            text.replace("tile=250", "tile=250 tile=10"),
        ] {
            assert!(Document::parse(&bad).is_err(), "accepted {bad}");
        }
    }
}
