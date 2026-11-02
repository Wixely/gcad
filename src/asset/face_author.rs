//! Source-bound face capture/check loading, before render-mesh preparation.
use super::{
    faces::{ResolvedSelection, SelectionFile},
    provenance::Inputs,
};
use anyhow::{Context, Result, bail, ensure};
use monstertruck::modeling::{Matrix4, Solid, SquareMatrix, builder};
use std::path::Path;

#[derive(Clone)]
pub struct Source {
    solid: Solid,
    groups: crate::select::Groups,
    inputs: Inputs,
}

impl Source {
    pub fn capture(&self, selector: &str) -> Result<SelectionFile> {
        SelectionFile::capture(self.inputs.clone(), &self.solid, &self.groups, selector)
    }

    pub fn resolve(&self, selection: &SelectionFile) -> Result<ResolvedSelection> {
        selection.resolve(&self.inputs, &self.solid, &self.groups)
    }
}

/// Evaluate one declared CAD source and record the inputs actually consumed,
/// including cached imported parts. Assembly bodies use the same unplaced local
/// coordinates as ordinary asset export. Maps, skins and scene clips are not loaded.
pub fn load(path: &Path, vars: &[(String, f64)], name: &str, body: Option<&str>) -> Result<Source> {
    let bytes = crate::host::read(path)?;
    let document = super::author::Document::parse(std::str::from_utf8(&bytes)?)?;
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let root = directory.join(
        document
            .project
            .context("face selections require project ROOT")?,
    );
    let mut source = document
        .sources
        .get(name)
        .context("face source does not exist")?
        .clone();
    let body = body.unwrap_or(&source.body).to_string();
    ensure!(body != "*", "select one assembly body with --body");
    ensure!(
        source.body == "*" || source.body == body,
        "body is outside the declared source selection"
    );
    source.body = body.clone();
    let source_path = directory.join(&source.path);
    let (run, reads) = crate::model::run_path_recorded(&source_path, vars)?;
    for (line, result) in &run.steps {
        if let Err(error) = result {
            bail!("source `{name}` line {}: {error:#}", line.number);
        }
    }
    let inputs =
        super::author::capture_inputs(Some(&root), &source_path, name, &source, vars, &reads)?
            .context("face selections require project ROOT")?;
    evaluated(&run.model, inputs)
}

/// Construct from the very evaluation whose geometry is being prepared.
pub(crate) fn evaluated(model: &crate::model::Model, mut inputs: Inputs) -> Result<Source> {
    let body = inputs.body.clone();
    let mut solid = model.named_body(&body)?;
    let groups = if model.assembly {
        let placement = model
            .placements
            .get(&body)
            .copied()
            .unwrap_or_else(Matrix4::identity);
        let inverse = placement.invert().context("singular assembly placement")?;
        solid = builder::transformed(&solid, inverse);
        model
            .part_groups
            .get(&body)
            .context("assembly body has no face-group provenance")?
            .moved(inverse)
            .placed()
    } else {
        let mut groups = model.groups.clone();
        groups.0.retain(|entry| entry.body == body);
        groups
    };
    if model.assembly {
        inputs.instance = format!("{}/{body}", inputs.instance);
    }
    inputs.generator.push_str("/face-source-local-1");
    inputs.validate()?;
    Ok(Source {
        solid,
        groups,
        inputs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verify_scene_uv_edit(asset: &super::super::author::Asset, record: &SelectionFile) {
        verify_scene_weight_edit(asset, record);
        use super::super::uv_edit::{self, Transform};
        let before = asset.encode_glb().unwrap();
        let transform = Transform {
            scale: [-2., 3.],
            rotation_degrees: 30.,
            offset: [0.2, -0.1],
        };
        let selected = asset.resolve_face_primitives(0, record).unwrap();
        let edited = uv_edit::edit_scene(asset, 0, record, transform).unwrap();
        let original_mesh = asset.scene.nodes[0].mesh.unwrap();
        let output_mesh = edited.scene.nodes[0].mesh.unwrap();
        assert_eq!(edited.scene.materials, asset.scene.materials);
        assert_eq!(edited.scene.nodes[1].mesh, asset.scene.nodes[1].mesh);
        for (p, primitive) in edited.scene.meshes[output_mesh]
            .primitives
            .iter()
            .enumerate()
        {
            let source = &asset.scene.meshes[original_mesh].primitives[p];
            assert_eq!(primitive.material, source.material);
            edited.face_maps[p]
                .triangle_faces(&primitive.geometry)
                .unwrap();
            for (i, triangle) in primitive.geometry.triangles.iter().enumerate() {
                assert_eq!(
                    triangle.map(|v| edited.source_vertices[p][v as usize] as u32),
                    source.geometry.triangles[i]
                );
                for &v in triangle {
                    let v = v as usize;
                    let old = edited.source_vertices[p][v];
                    assert_eq!(
                        primitive.geometry.positions[v],
                        source.geometry.positions[old]
                    );
                    assert_eq!(primitive.geometry.normals[v], source.geometry.normals[old]);
                    let uv = source.geometry.texcoords.as_ref().unwrap()[old];
                    assert_eq!(
                        primitive.geometry.texcoords.as_ref().unwrap()[v],
                        if selected[p].contains(&i) {
                            transform.apply(uv)
                        } else {
                            uv
                        }
                    );
                    if !selected[p].contains(&i) {
                        assert_eq!(
                            primitive.geometry.tangents.as_ref().unwrap()[v],
                            source.geometry.tangents.as_ref().unwrap()[old]
                        );
                    }
                    assert_eq!(
                        edited.skins[0].weights[p][v].joints,
                        asset.skins[0].weights[p][old].joints
                    );
                    assert_eq!(
                        edited.skins[0].weights[p][v].weights,
                        asset.skins[0].weights[p][old].weights
                    );
                }
            }
        }
        for shift in [0., 2., 5.] {
            let mut pose: Vec<_> = asset.scene.nodes.iter().map(|n| n.transform).collect();
            pose.last_mut().unwrap().translation[0] += shift;
            let expected = asset.skins[0]
                .deform_positions(&asset.scene, &pose)
                .unwrap();
            let actual = edited.skins[0]
                .deform_positions(&edited.scene, &pose)
                .unwrap();
            for (p, vertices) in edited.source_vertices.iter().enumerate() {
                for (v, &old) in vertices.iter().enumerate() {
                    assert_eq!(actual[p][v], expected[p][old]);
                }
            }
            for i in 1..asset.skins.len() {
                assert_eq!(
                    asset.skins[i]
                        .deform_positions(&asset.scene, &pose)
                        .unwrap(),
                    edited.skins[i]
                        .deform_positions(&edited.scene, &pose)
                        .unwrap()
                );
            }
        }
        edited.encode_glb().unwrap();
        assert_eq!(asset.encode_glb().unwrap(), before);
    }

    fn verify_scene_weight_edit(asset: &super::super::author::Asset, record: &SelectionFile) {
        use super::super::{
            face_weights::{self, Boundary},
            skin::{Skin, Weights},
        };
        let before = asset.encode_glb().unwrap();
        let skin = &asset.skins[0];
        let joint = skin.joints[0];
        let name = asset.scene.nodes[joint].name.as_str();
        let selected = asset.resolve_face_primitives(0, record).unwrap();
        let edited =
            face_weights::edit_scene(asset, 0, record, &[(name, 1.)], Boundary::Split).unwrap();
        let original = &asset.scene.meshes[asset.scene.nodes[0].mesh.unwrap()];
        let derived = &edited.scene.meshes[edited.scene.nodes[0].mesh.unwrap()];
        assert_eq!(edited.scene.materials, asset.scene.materials);
        assert_eq!(edited.skins[0].inverse_bind, skin.inverse_bind);
        assert_eq!(edited.scene.nodes[1].mesh, asset.scene.nodes[1].mesh);
        for (p, primitive) in derived.primitives.iter().enumerate() {
            assert_eq!(primitive.material, original.primitives[p].material);
            edited.face_maps[p]
                .triangle_faces(&primitive.geometry)
                .unwrap();
            for (t, triangle) in primitive.geometry.triangles.iter().enumerate() {
                assert_eq!(
                    triangle.map(|v| edited.source_vertices[p][v as usize] as u32),
                    original.primitives[p].geometry.triangles[t]
                );
                for &v in triangle {
                    let v = v as usize;
                    let old = edited.source_vertices[p][v];
                    let geometry = &original.primitives[p].geometry;
                    assert_eq!(primitive.geometry.positions[v], geometry.positions[old]);
                    assert_eq!(primitive.geometry.normals[v], geometry.normals[old]);
                    assert_eq!(
                        primitive.geometry.texcoords.as_ref().unwrap()[v],
                        geometry.texcoords.as_ref().unwrap()[old]
                    );
                    assert_eq!(
                        primitive.geometry.tangents.as_ref().unwrap()[v],
                        geometry.tangents.as_ref().unwrap()[old]
                    );
                    let w = edited.skins[0].weights[p][v];
                    assert_eq!(edited.assigned_vertices[p][v], selected[p].contains(&t));
                    if selected[p].contains(&t) {
                        assert_eq!(w.joints, [0; 4]);
                        assert_eq!(w.weights, [1., 0., 0., 0.]);
                    } else {
                        assert_eq!(w.joints, skin.weights[p][old].joints);
                        assert_eq!(w.weights, skin.weights[p][old].weights);
                    }
                }
            }
        }
        let rigid = Skin::bind(
            &edited.scene,
            0,
            skin.joints.clone(),
            derived
                .primitives
                .iter()
                .map(|p| {
                    vec![
                        Weights {
                            joints: [0; 4],
                            weights: [1., 0., 0., 0.]
                        };
                        p.geometry.positions.len()
                    ]
                })
                .collect(),
        )
        .unwrap();
        for shift in [0., 2., 5.] {
            let mut pose: Vec<_> = asset.scene.nodes.iter().map(|n| n.transform).collect();
            pose[joint].translation[0] += shift;
            let old = skin.deform_positions(&asset.scene, &pose).unwrap();
            let actual = edited.skins[0]
                .deform_positions(&edited.scene, &pose)
                .unwrap();
            let expected = rigid.deform_positions(&edited.scene, &pose).unwrap();
            for (p, sources) in edited.source_vertices.iter().enumerate() {
                for (v, &source) in sources.iter().enumerate() {
                    assert_eq!(
                        actual[p][v],
                        if edited.assigned_vertices[p][v] {
                            expected[p][v]
                        } else {
                            old[p][source]
                        }
                    );
                }
            }
            for i in 1..asset.skins.len() {
                assert_eq!(
                    asset.skins[i]
                        .deform_positions(&asset.scene, &pose)
                        .unwrap(),
                    edited.skins[i]
                        .deform_positions(&edited.scene, &pose)
                        .unwrap()
                );
            }
        }
        for invalid in [
            vec![],
            vec![(name, 0.)],
            vec![(name, 0.5)],
            vec![(name, f64::NAN)],
            vec![(name, 0.5), (name, 0.5)],
            vec![("absent", 1.)],
        ] {
            assert!(face_weights::edit_scene(asset, 0, record, &invalid, Boundary::Split).is_err());
        }
        edited.encode_glb().unwrap();
        assert_eq!(asset.encode_glb().unwrap(), before);
    }

    fn fixture() -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::path::absolute(format!("target/face-source-{}-{nonce}", std::process::id()))
                .unwrap();
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("box.gcad"), "rect 30 20\nbase: extrude 5\n").unwrap();
        std::fs::write(
            root.join("box.gasset"),
            "asset 1\nproject .\nsource box box.gcad body=main\ntessellate box tolerance=0.1\nuv box projection=box tile=25\n",
        )
        .unwrap();
        root
    }

    #[test]
    fn authored_smooth_plate_splits_shared_weight_boundaries_and_isolates_instance() {
        use super::super::{
            author,
            face_weights::{self, Boundary},
            skin::{Assignment, Skin},
        };
        let root = fixture();
        let file = root.join("box.gasset");
        std::fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/parts/plate.gcad"),
            root.join("box.gcad"),
        )
        .unwrap();
        let base = format!(
            "{}normals box crease=60\nnode first source=box\nnode second source=box translation=100,0,0\nnode base\nnode tip parent=base translation=0,15,0\n",
            std::fs::read_to_string(&file).unwrap()
        );
        std::fs::write(&file, &base).unwrap();
        let source = load(&file, &[], "box", None).unwrap();
        let record = source.capture("base.end").unwrap();
        std::fs::write(root.join("end.json"), record.to_json().unwrap()).unwrap();
        let initial = author::load_with_faces(&file, &[]).unwrap();
        for node in [0, 1] {
            let skin = Skin::assign(
                &initial.scene,
                node,
                vec![2, 3],
                Assignment::Linear {
                    axis: 1,
                    start_mm: -15.,
                    end_mm: 15.,
                },
            )
            .unwrap();
            std::fs::write(
                root.join(format!("{node}.weights.json")),
                initial.weight_file(&skin).unwrap().to_json().unwrap(),
            )
            .unwrap();
        }
        let base = format!(
            "{base}skin first weights=0.weights.json\nskin second weights=1.weights.json\n"
        );
        std::fs::write(&file, &base).unwrap();
        let asset = author::load_with_faces(&file, &[]).unwrap();
        assert_eq!(asset.scene.nodes[0].mesh, asset.scene.nodes[1].mesh);
        assert!(
            face_weights::edit_scene(
                &asset,
                0,
                &record,
                &[("base", 0.25), ("tip", 0.75)],
                Boundary::RejectShared
            )
            .err()
            .unwrap()
            .to_string()
            .contains("boundary shares vertices")
        );
        let edited = face_weights::edit_scene(
            &asset,
            0,
            &record,
            &[("base", 0.25), ("tip", 0.75)],
            Boundary::Split,
        )
        .unwrap();
        assert!(
            edited.split_vertices > 0,
            "fixture must exercise actual shared-index splits"
        );
        assert_ne!(edited.scene.nodes[0].mesh, edited.scene.nodes[1].mesh);
        assert_eq!(edited.scene.nodes[1].mesh, asset.scene.nodes[1].mesh);
        verify_scene_weight_edit(&asset, &record);
        let declaration =
            "face_weights first end.json joints=base,tip weights=0.25,0.75 boundary=split\n";
        std::fs::write(&file, format!("{base}{declaration}")).unwrap();
        let authored = author::load(&file, &[]).unwrap();
        assert_eq!(authored.encode_glb().unwrap(), edited.encode_glb().unwrap());
        assert_eq!(
            authored.resolve_face_primitives(0, &record).unwrap(),
            asset.resolve_face_primitives(0, &record).unwrap()
        );
        std::fs::write(
            &file,
            format!(
                "{base}{}",
                declaration.replace("boundary=split", "boundary=reject-shared")
            ),
        )
        .unwrap();
        assert!(
            author::load(&file, &[])
                .err()
                .unwrap()
                .to_string()
                .contains("boundary shares vertices")
        );
    }

    #[test]
    fn persisted_face_weights_follow_uv_materials_and_preserve_base_binding() {
        use super::super::{
            author,
            face_weights::{self, Boundary},
            skin::{Assignment, Skin},
        };
        let root = fixture();
        let file = root.join("box.gasset");
        let base = format!(
            "{}node first source=box\nnode a\nnode b\nnode outside\nskin first weights=weights.json\n",
            std::fs::read_to_string(&file).unwrap()
        );
        std::fs::write(&file, &base).unwrap();
        let source = load(&file, &[], "box", None).unwrap();
        let end = source.capture("base.end").unwrap();
        let bottom = source.capture("<Z").unwrap();
        for (name, record) in [("end.json", &end), ("bottom.json", &bottom)] {
            std::fs::write(root.join(name), record.to_json().unwrap()).unwrap();
        }
        let binding = author::load_for_binding(&file, &[], "first").unwrap();
        let assign = |scene: &super::super::scene::Scene| {
            Skin::assign(
                scene,
                0,
                vec![1, 2],
                Assignment::Linear {
                    axis: 2,
                    start_mm: 0.,
                    end_mm: 5.,
                },
            )
            .unwrap()
        };
        let weights = binding
            .weight_file(&assign(&binding.scene))
            .unwrap()
            .to_json()
            .unwrap();
        std::fs::write(root.join("weights.json"), &weights).unwrap();
        let preparation =
            "face_material first end.json roughness=0.2\nface_uv first end.json offset=0.1,0.2\n";
        std::fs::write(&file, format!("{base}{preparation}")).unwrap();
        let original = author::load(&file, &[]).unwrap();
        let expected = face_weights::edit_scene(
            &original,
            0,
            &end,
            &[("a", 0.25), ("b", 0.75)],
            Boundary::RejectShared,
        )
        .unwrap();
        let declaration =
            "face_weights first end.json joints=a,b weights=0.25,0.75 boundary=reject-shared\n";
        std::fs::write(&file, format!("{base}{declaration}{preparation}")).unwrap();
        let authored = author::load(&file, &[]).unwrap();
        assert_eq!(
            authored.encode_glb().unwrap(),
            expected.encode_glb().unwrap()
        );
        assert_eq!(
            authored.resolve_face_primitives(0, &end).unwrap(),
            original.resolve_face_primitives(0, &end).unwrap()
        );
        let rebinding = author::load_for_binding(&file, &[], "first").unwrap();
        assert_eq!(
            rebinding
                .weight_file(&assign(&rebinding.scene))
                .unwrap()
                .to_json()
                .unwrap(),
            weights
        );
        let second = "face_weights first bottom.json joints=b,a weights=0.25,0.75 boundary=split\n";
        let expected = face_weights::edit_scene(
            &authored,
            0,
            &bottom,
            &[("b", 0.25), ("a", 0.75)],
            Boundary::Split,
        )
        .unwrap();
        std::fs::write(&file, format!("{base}{declaration}{preparation}{second}")).unwrap();
        assert_eq!(
            author::load(&file, &[]).unwrap().encode_glb().unwrap(),
            expected.encode_glb().unwrap()
        );
        for invalid in [
            declaration.to_string(),
            declaration.replace("end.json", "missing.json"),
            declaration.replace("joints=a,b", "joints=a,outside"),
            declaration.replace("first end", "absent end"),
        ] {
            std::fs::write(&file, format!("{base}{declaration}{preparation}{invalid}")).unwrap();
            assert!(author::load(&file, &[]).is_err());
        }
        // Palette membership is deliberately deferred by the binding loader.
        std::fs::write(
            &file,
            format!(
                "{base}{}",
                declaration.replace("joints=a,b", "joints=a,outside")
            ),
        )
        .unwrap();
        assert!(author::load_for_binding(&file, &[], "first").is_ok());
        assert!(
            author::load(&file, &[])
                .err()
                .unwrap()
                .to_string()
                .contains("palette")
        );
        for settings in [
            "joints=a weights=1",
            "joints=a weights=1 boundary=automatic",
            "joints=a,b weights=1 boundary=split",
            "joints=a,a weights=0.5,0.5 boundary=split",
            "joints=a weights=NaN boundary=split",
            "joints=a weights=0.5 boundary=split",
            "joints=a weights=1 boundary=split extra=true",
        ] {
            assert!(
                author::Document::parse(&format!("{base}face_weights first end.json {settings}\n"))
                    .is_err()
            );
        }
        assert_eq!(std::fs::read(root.join("weights.json")).unwrap(), weights);
    }

    #[test]
    fn persisted_face_materials_load_multiple_regions_and_preserve_weight_binding() {
        use super::super::{
            author, face_material,
            skin::{Assignment, Skin},
        };
        let root = fixture();
        let file = root.join("box.gasset");
        let base = format!(
            "{}refine box levels=1 triangle_budget=1000\nnode first source=box\nnode second source=box translation=100,0,0\nnode bone\n",
            std::fs::read_to_string(&file).unwrap()
        );
        std::fs::write(&file, &base).unwrap();
        let source = load(&file, &[], "box", None).unwrap();
        let end = source.capture("base.end").unwrap();
        let bottom = source.capture("<Z").unwrap();
        std::fs::write(root.join("end.json"), end.to_json().unwrap()).unwrap();
        std::fs::write(root.join("bottom.json"), bottom.to_json().unwrap()).unwrap();
        let initial = author::load_with_faces(&file, &[]).unwrap();
        let skin = Skin::assign(&initial.scene, 0, vec![2], Assignment::Rigid).unwrap();
        let weights = initial.weight_file(&skin).unwrap().to_json().unwrap();
        std::fs::write(root.join("weights.json"), &weights).unwrap();
        let base = format!("{base}skin first weights=weights.json\n");
        std::fs::write(&file, &base).unwrap();
        let original = author::load_with_faces(&file, &[]).unwrap();
        let red =
            author::SurfaceSettings::parse(&[("base_colour".into(), "1,0,0,1".into())].into())
                .unwrap();
        let blue =
            author::SurfaceSettings::parse(&[("base_colour".into(), "0,0,1,1".into())].into())
                .unwrap();
        let expected =
            face_material::assign_regions(&original, 0, &[(&end, &red), (&bottom, &blue)]).unwrap();
        let declarations = "face_material first end.json base_colour=1,0,0,1\nface_material first bottom.json base_colour=0,0,1,1\n";
        std::fs::write(&file, format!("{base}{declarations}")).unwrap();
        let mut authored = author::load(&file, &[]).unwrap();
        assert_eq!(
            authored.encode_glb().unwrap(),
            expected.encode_glb().unwrap()
        );
        let mesh = authored.scene.nodes[0].mesh.unwrap();
        assert_eq!(authored.scene.meshes[mesh].primitives.len(), 3);
        assert_eq!(authored.skins[0].weights.len(), 3);
        assert!(
            authored.face_map(0).is_err(),
            "pre-partition map must not escape"
        );
        assert_eq!(authored.primitive_face_maps(0).unwrap().len(), 3);
        let end_indices = authored.resolve_face_primitives(0, &end).unwrap();
        assert_eq!(
            end_indices.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![0, 8, 0]
        );
        assert_eq!(end_indices[1], (0..8).collect::<Vec<_>>());
        let bottom_indices = authored.resolve_face_primitives(0, &bottom).unwrap();
        assert_eq!(
            bottom_indices.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![0, 0, 8]
        );
        let all = source.capture("all").unwrap();
        verify_scene_uv_edit(&authored, &end);
        verify_scene_uv_edit(&authored, &all);
        let uv_transform = super::super::uv_edit::Transform {
            scale: [-2., 3.],
            rotation_degrees: 30.,
            offset: [0.2, -0.1],
        };
        let uv_expected =
            super::super::uv_edit::edit_scene(&authored, 0, &end, uv_transform).unwrap();
        let uv_declaration = "face_uv first end.json scale=-2,3 rotation=30 offset=0.2,-0.1\n";
        // Text order does not move UV editing before material partitioning.
        std::fs::write(&file, format!("{base}{uv_declaration}{declarations}")).unwrap();
        let uv_authored = author::load(&file, &[]).unwrap();
        assert_eq!(
            uv_authored.encode_glb().unwrap(),
            uv_expected.encode_glb().unwrap()
        );
        assert_eq!(
            uv_authored.resolve_face_primitives(0, &end).unwrap(),
            end_indices
        );
        let uv_rebind = author::load_for_binding(&file, &[], "first").unwrap();
        let uv_skin = Skin::assign(&uv_rebind.scene, 0, vec![2], Assignment::Rigid).unwrap();
        assert_eq!(
            uv_rebind.weight_file(&uv_skin).unwrap().to_json().unwrap(),
            weights
        );
        let bottom_transform = super::super::uv_edit::Transform {
            offset: [0.3, 0.4],
            ..Default::default()
        };
        let both_expected =
            super::super::uv_edit::edit_scene(&uv_authored, 0, &bottom, bottom_transform).unwrap();
        std::fs::write(
            &file,
            format!(
                "{base}{declarations}{uv_declaration}face_uv first bottom.json offset=0.3,0.4\n"
            ),
        )
        .unwrap();
        assert_eq!(
            author::load(&file, &[]).unwrap().encode_glb().unwrap(),
            both_expected.encode_glb().unwrap()
        );
        for invalid in [
            format!("{base}{declarations}{uv_declaration}{uv_declaration}"),
            format!("{base}face_uv absent end.json scale=2,2\n"),
            format!("{base}face_uv first missing.json scale=2,2\n"),
        ] {
            std::fs::write(&file, invalid).unwrap();
            assert!(author::load(&file, &[]).is_err());
        }
        for invalid in ["", "scale=0,1", "scale=2", "offset=NaN,0", "turn=30"] {
            assert!(
                author::Document::parse(&format!("{base}face_uv first end.json {invalid}\n"))
                    .is_err()
            );
        }
        std::fs::write(&file, format!("{base}{declarations}")).unwrap();
        let all_indices = authored.resolve_face_primitives(0, &all).unwrap();
        assert_eq!(
            all_indices.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![32, 8, 8]
        );
        for (indices, primitive) in all_indices
            .iter()
            .zip(&authored.scene.meshes[mesh].primitives)
        {
            assert_eq!(
                *indices,
                (0..primitive.geometry.triangles.len()).collect::<Vec<_>>()
            );
        }
        let before_edit = authored.encode_glb().unwrap();
        let matte =
            author::SurfaceSettings::parse(&[("roughness".into(), "0.25".into())].into()).unwrap();
        // One selection spans differently coloured primitives; inherit each colour.
        let edited = face_material::assign(&authored, 0, &all, &matte).unwrap();
        assert_eq!(edited.selected_triangles, 48);
        assert_eq!(edited.source_primitives, vec![0, 1, 2]);
        let edited_mesh = edited.scene.nodes[0].mesh.unwrap();
        for (p, primitive) in edited.scene.meshes[edited_mesh]
            .primitives
            .iter()
            .enumerate()
        {
            let prior = &authored.scene.meshes[mesh].primitives[p];
            assert_eq!(primitive.geometry, prior.geometry);
            assert_eq!(
                edited.scene.materials[primitive.material],
                matte
                    .apply(&authored.scene.materials[prior.material])
                    .unwrap()
            );
            assert_eq!(
                edited.skins[0].weights[p]
                    .iter()
                    .map(|w| (w.joints, w.weights))
                    .collect::<Vec<_>>(),
                authored.skins[0].weights[p]
                    .iter()
                    .map(|w| (w.joints, w.weights))
                    .collect::<Vec<_>>()
            );
        }
        for displacement in [0., 3., 8.] {
            let mut pose: Vec<_> = authored.scene.nodes.iter().map(|n| n.transform).collect();
            pose[2].translation[0] += displacement;
            assert_eq!(
                edited.skins[0]
                    .deform_positions(&edited.scene, &pose)
                    .unwrap(),
                authored.skins[0]
                    .deform_positions(&authored.scene, &pose)
                    .unwrap()
            );
        }
        let one = face_material::assign(&authored, 0, &end, &matte).unwrap();
        assert_eq!(one.selected_triangles, 8);
        let one_mesh = one.scene.nodes[0].mesh.unwrap();
        for p in [0, 2] {
            let prior = &authored.scene.meshes[mesh].primitives[p];
            let next = &one.scene.meshes[one_mesh].primitives[p];
            assert_eq!(next.geometry, prior.geometry);
            assert_eq!(next.material, prior.material);
        }
        assert!(
            face_material::assign_regions(&authored, 0, &[(&end, &matte), (&all, &matte)]).is_err()
        );
        edited.encode_glb().unwrap();
        assert_eq!(authored.encode_glb().unwrap(), before_edit);
        // Even damage in an unselected primitive invalidates the node's maps.
        authored.scene.meshes[mesh].primitives[2].geometry.positions[0][0] += 1.;
        assert!(authored.resolve_face_primitives(0, &end).is_err());
        assert!(authored.primitive_face_maps(0).is_err());
        let rebind = author::load_for_binding(&file, &[], "first").unwrap();
        let rebound = Skin::assign(&rebind.scene, 0, vec![2], Assignment::Rigid).unwrap();
        assert_eq!(
            rebind.weight_file(&rebound).unwrap().to_json().unwrap(),
            weights
        );
        assert_eq!(std::fs::read(root.join("weights.json")).unwrap(), weights);
        for invalid in [
            format!("{base}{declarations}face_material first end.json roughness=0.2\n"),
            format!("{base}face_material absent end.json roughness=0.2\n"),
            format!("{base}face_material first missing.json roughness=0.2\n"),
        ] {
            std::fs::write(&file, invalid).unwrap();
            assert!(author::load(&file, &[]).is_err());
        }
        assert!(author::Document::parse(&format!("{base}face_material first end.json\n")).is_err());
        std::fs::write(&file, format!("{base}{declarations}")).unwrap();
        std::fs::write(root.join("box.gcad"), "\nrect 30 20\nbase: extrude 5\n").unwrap();
        assert!(author::load(&file, &[]).is_err());
    }

    #[test]
    fn face_material_split_preserves_tuples_skinning_and_other_instances() {
        use super::super::{
            author, face_material,
            scene::{Node, Transform},
            skin::{Assignment, Skin},
        };
        let root = fixture();
        let file = root.join("box.gasset");
        let text = std::fs::read_to_string(&file).unwrap();
        std::fs::write(&file, format!("{text}refine box levels=1 triangle_budget=1000\nnode first source=box\nnode second source=box translation=100,0,0\n")).unwrap();
        let source = load(&file, &[], "box", None).unwrap();
        let record = source.capture("base.end").unwrap();
        let saved = record.to_json().unwrap();
        let mut asset = author::load_with_faces(&file, &[]).unwrap();
        asset.scene.nodes.extend([
            Node {
                name: "joint-a".into(),
                parent: None,
                mesh: None,
                transform: Transform::default(),
            },
            Node {
                name: "joint-b".into(),
                parent: None,
                mesh: None,
                transform: Transform {
                    translation: [0., 0., 5.],
                    ..Default::default()
                },
            },
        ]);
        for node in [0, 1] {
            asset.skins.push(
                Skin::assign(
                    &asset.scene,
                    node,
                    vec![2, 3],
                    Assignment::Linear {
                        axis: 2,
                        start_mm: 0.,
                        end_mm: 5.,
                    },
                )
                .unwrap(),
            );
        }
        let before = asset.encode_glb().unwrap();
        verify_scene_uv_edit(&asset, &record);
        let settings = author::SurfaceSettings::parse(
            &[
                ("base_colour".into(), "1,0,0,1".into()),
                ("roughness".into(), "0.2".into()),
            ]
            .into(),
        )
        .unwrap();
        let derived = face_material::assign(&asset, 0, &record, &settings).unwrap();
        assert_eq!(derived.selected_triangles, 8);
        assert_ne!(derived.scene.nodes[0].mesh, derived.scene.nodes[1].mesh);
        assert_eq!(derived.scene.nodes[1].mesh, asset.scene.nodes[1].mesh);
        let mesh = &derived.scene.meshes[derived.scene.nodes[0].mesh.unwrap()];
        assert_eq!(mesh.primitives.len(), 2);
        let original = &asset.scene.meshes[0].primitives[0];
        assert_eq!(
            derived.scene.meshes[0].primitives[0].geometry,
            original.geometry
        );
        let all: std::collections::BTreeSet<_> =
            derived.source_triangles.iter().flatten().copied().collect();
        assert_eq!(all, (0..original.geometry.triangles.len()).collect());
        assert_eq!(
            derived.source_triangles.iter().map(Vec::len).sum::<usize>(),
            all.len()
        );
        for (p, primitive) in mesh.primitives.iter().enumerate() {
            for (v, &source) in derived.source_vertices[p].iter().enumerate() {
                assert_eq!(
                    primitive.geometry.positions[v],
                    original.geometry.positions[source]
                );
                assert_eq!(
                    primitive.geometry.normals[v],
                    original.geometry.normals[source]
                );
                assert_eq!(
                    primitive.geometry.texcoords.as_ref().unwrap()[v],
                    original.geometry.texcoords.as_ref().unwrap()[source]
                );
                assert_eq!(
                    primitive.geometry.tangents.as_ref().unwrap()[v],
                    original.geometry.tangents.as_ref().unwrap()[source]
                );
            }
            for (i, &source) in derived.source_triangles[p].iter().enumerate() {
                assert_eq!(
                    primitive.geometry.triangles[i]
                        .map(|v| derived.source_vertices[p][v as usize] as u32),
                    original.geometry.triangles[source]
                );
            }
        }
        assert_eq!(
            derived.scene.materials[mesh.primitives[0].material],
            asset.scene.materials[original.material]
        );
        assert_eq!(
            derived.scene.materials[mesh.primitives[1].material],
            settings
                .apply(&asset.scene.materials[original.material])
                .unwrap()
        );
        for shift in [0., 2., 5.] {
            let mut pose: Vec<_> = asset.scene.nodes.iter().map(|n| n.transform).collect();
            pose[3].translation[0] += shift;
            let expected = asset.skins[0]
                .deform_positions(&asset.scene, &pose)
                .unwrap();
            let actual = derived.skins[0]
                .deform_positions(&derived.scene, &pose)
                .unwrap();
            for (p, vertices) in derived.source_vertices.iter().enumerate() {
                for (v, &source) in vertices.iter().enumerate() {
                    assert_eq!(actual[p][v], expected[0][source]);
                }
            }
            assert_eq!(
                derived.skins[1]
                    .deform_positions(&derived.scene, &pose)
                    .unwrap(),
                asset.skins[1]
                    .deform_positions(&asset.scene, &pose)
                    .unwrap()
            );
        }
        let encoded = derived.encode_glb().unwrap();
        let length = u32::from_le_bytes(encoded[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value = serde_json::from_slice(&encoded[20..20 + length]).unwrap();
        assert_eq!(json["skins"].as_array().unwrap().len(), 2);
        assert!(
            json["meshes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["primitives"].as_array().unwrap().len() == 2)
        );
        assert_eq!(asset.encode_glb().unwrap(), before);
        assert_eq!(record.to_json().unwrap(), saved);
        let mut bad = saved.clone();
        let mut json: serde_json::Value = serde_json::from_slice(&bad).unwrap();
        json["inputs"]["body"] = "other".into();
        bad = serde_json::to_vec(&json).unwrap();
        assert!(
            face_material::assign(
                &asset,
                0,
                &SelectionFile::from_json(&bad).unwrap(),
                &settings
            )
            .is_err()
        );
    }

    #[test]
    fn prepared_asset_faces_match_export_through_subdivision_uvs_and_atlas() {
        let root = fixture();
        std::fs::write(
            root.join("assembly.gasm"),
            "part first box.gcad\npart second box.gcad\nmove second 100,0,0\n",
        )
        .unwrap();
        let file = root.join("prepared.gasset");
        for preparation in [
            "",
            "refine boxes levels=1 triangle_budget=1000\n",
            "slice boxes axis=x planes=-5,0,5 triangle_budget=1000\n",
            "refine boxes levels=1 triangle_budget=1000\natlas boxes width=256 height=256 padding=2 density=16 filter=linear\n",
        ] {
            std::fs::write(&file, format!("asset 1\nproject .\nsource boxes assembly.gasm\ntessellate boxes tolerance=0.1\nnormals boxes crease=60\nuv boxes projection=box tile=25 rotation=30\n{preparation}")).unwrap();
            let plain = super::super::author::load(&file, &[]).unwrap();
            let mut prepared = super::super::author::load_with_faces(&file, &[]).unwrap();
            assert_eq!(plain.encode_glb().unwrap(), prepared.encode_glb().unwrap());
            assert_eq!(
                prepared.scene.meshes.len(),
                1,
                "repeated bodies still share geometry"
            );
            assert!(prepared.face_map(0).is_err(), "assembly group has no mesh");
            for body in ["first", "second"] {
                let source = load(&file, &[], "boxes", Some(body)).unwrap();
                let record = source.capture("base.end").unwrap();
                let raw = source.resolve(&record).unwrap();
                let labels = raw.face_map.triangle_faces(&raw.mesh).unwrap();
                let faces: Vec<_> = raw.triangles.iter().map(|&i| labels[i]).collect();
                let selection = raw.face_map.select(&raw.mesh, &faces).unwrap();
                let node = prepared
                    .scene
                    .nodes
                    .iter()
                    .position(|n| n.name == format!("boxes/{body}"))
                    .unwrap();
                assert!(plain.face_map(node).is_err());
                let mesh = &prepared.scene.meshes[prepared.scene.nodes[node].mesh.unwrap()]
                    .primitives[0]
                    .geometry;
                assert!(mesh.texcoords.is_some() && mesh.tangents.is_some());
                let selected = prepared
                    .face_map(node)
                    .unwrap()
                    .resolve(mesh, &selection)
                    .unwrap();
                let stored = SelectionFile::from_json(&record.to_json().unwrap()).unwrap();
                assert_eq!(prepared.resolve_faces(node, &stored).unwrap(), selected);
                let other = if body == "first" { "second" } else { "first" };
                let other_node = prepared
                    .scene
                    .nodes
                    .iter()
                    .position(|n| n.name == format!("boxes/{other}"))
                    .unwrap();
                assert!(
                    prepared.resolve_faces(other_node, &stored).is_err(),
                    "identical geometry cannot substitute another CAD body"
                );
                let mut area = 0.;
                for i in selected {
                    let [a, b, c] = mesh.triangles[i].map(|v| mesh.positions[v as usize]);
                    assert!([a, b, c].iter().all(|p| (p[2] - 5.).abs() < 1e-8));
                    let cross =
                        super::super::cross(super::super::sub(b, a), super::super::sub(c, a));
                    assert!(cross[2] > 0.);
                    area += cross[2] / 2.;
                }
                assert!((area - 600.).abs() < 1e-8);
            }
            prepared.scene.meshes[0].primitives[0].geometry.positions[0][0] += 1.;
            assert!(prepared.face_map(1).is_err());
            assert!(prepared.face_map(2).is_err());
            assert!(prepared.face_map(usize::MAX).is_err());
        }
    }

    #[test]
    fn actual_source_observations_guard_records_and_declared_body_scope() {
        let root = fixture();
        let file = root.join("box.gasset");
        let source = load(&file, &[], "box", None).unwrap();
        let record = source.capture("base.end").unwrap();
        let json = record.to_json().unwrap();
        assert!(
            !String::from_utf8_lossy(&json).contains(&root.to_string_lossy().replace('\\', "/"))
        );
        let stored = root.join("selection.json");
        std::fs::write(&stored, &json).unwrap();
        let loaded = SelectionFile::from_json(&crate::host::read(&stored).unwrap()).unwrap();
        assert_eq!(
            load(&file, &[], "box", None)
                .unwrap()
                .resolve(&loaded)
                .unwrap()
                .triangles
                .len(),
            2
        );
        assert!(load(&file, &[], "missing", None).is_err());
        assert!(load(&file, &[], "box", Some("other")).is_err());
        std::fs::write(root.join("box.gcad"), "\nrect 30 20\nbase: extrude 5\n").unwrap();
        assert!(
            load(&file, &[], "box", None)
                .unwrap()
                .resolve(&loaded)
                .is_err()
        );
        assert_eq!(std::fs::read(&stored).unwrap(), json);
        std::fs::write(&file, "asset 1\nsource box box.gcad body=main\ntessellate box tolerance=0.1\nuv box projection=box tile=25\n").unwrap();
        assert!(load(&file, &[], "box", None).is_err());
    }

    #[test]
    fn assembly_face_groups_and_geometry_use_local_coordinates_and_dependency_guards() {
        let root = fixture();
        std::fs::write(
            root.join("assembly.gasm"),
            "part item box.gcad\nmove item 100,200,300\nrotate item 90 axis=z\n",
        )
        .unwrap();
        let file = root.join("assembly.gasset");
        std::fs::write(&file, "asset 1\nproject .\nsource assembly assembly.gasm\ntessellate assembly tolerance=0.1\nuv assembly projection=box tile=25\n").unwrap();
        assert!(load(&file, &[], "assembly", None).is_err());
        let source = load(&file, &[], "assembly", Some("item")).unwrap();
        let record = source.capture("base.end").unwrap();
        let result = source.resolve(&record).unwrap();
        for i in result.triangles {
            for &v in &result.mesh.triangles[i] {
                assert!((result.mesh.positions[v as usize][2] - 5.).abs() < 1e-8);
            }
        }
        assert!(source.inputs.dependencies.contains_key("box.gcad"));
        assert!(source.inputs.dependencies.contains_key("assembly.gasm"));
        assert!(
            load(&file, &[], "assembly", Some("item"))
                .unwrap()
                .resolve(&record)
                .is_ok()
        );
        std::fs::write(root.join("box.gcad"), "rect 40 20\nbase: extrude 5\n").unwrap();
        assert!(
            load(&file, &[], "assembly", Some("item"))
                .unwrap()
                .resolve(&record)
                .is_err()
        );
    }
}
