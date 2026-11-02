//! Guarded source-face association for a tessellation, not persistent CAD IDs.
//! Face fingerprints are deliberately sensitive to geometry and tessellation.
//! `SelectionFile` combines geometry checks with source provenance and selector intent.
use super::{BindingStamp, Mesh};
use anyhow::{Context, Result, ensure};
use monstertruck::{meshing::prelude::*, modeling::Solid};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub struct FaceMap {
    mesh_stamp: BindingStamp,
    triangle_faces: Vec<usize>,
    fingerprints: Vec<[u8; 32]>,
}

/// Geometry-only selection guard. Do not persist without source identity guards.
#[derive(Clone, Debug)]
pub struct Selection {
    fingerprints: Vec<[u8; 32]>,
}

/// Versioned source-bound face selection. Changed inputs require explicit capture;
/// runtime face indices and kernel IDs are never persisted.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionFile {
    version: u32,
    inputs: super::provenance::Inputs,
    selector: String,
    faces: Vec<[u8; 32]>,
}

#[derive(Debug)]
pub struct ResolvedSelection {
    pub mesh: Mesh,
    pub face_map: FaceMap,
    pub triangles: Vec<usize>,
}

impl SelectionFile {
    pub fn selected_face_count(&self) -> usize {
        self.faces.len()
    }

    fn validate_selector(selector: &str) -> Result<()> {
        ensure!(
            !selector.trim().is_empty()
                && selector.is_ascii()
                && selector.len() <= 4096
                && !selector.bytes().any(|b| b.is_ascii_control()),
            "face selector must be nonempty ASCII, without control characters, at most 4096 bytes"
        );
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        ensure!(self.version == 1, "unsupported face-selection version");
        self.inputs.validate()?;
        Self::validate_selector(&self.selector)?;
        ensure!(
            !self.faces.is_empty() && self.faces.windows(2).all(|p| p[0] < p[1]),
            "face fingerprints must be nonempty, unique and sorted"
        );
        Ok(())
    }

    /// Capture from the current solid, source observations and semantic selector.
    /// Inputs must describe the actual evaluated source, including dependencies.
    pub fn capture(
        inputs: super::provenance::Inputs,
        solid: &Solid,
        groups: &crate::select::Groups,
        selector: &str,
    ) -> Result<Self> {
        let inputs = inputs.canonical()?;
        Self::validate_selector(selector)?;
        let (mesh, map) = tessellate(solid, inputs.tolerance_mm)?;
        let faces = crate::select::select_faces(selector, solid, groups, inputs.tolerance_mm)?;
        let record = Self {
            version: 1,
            inputs,
            selector: selector.into(),
            faces: map.select(&mesh, &faces)?.fingerprints,
        };
        record.validate()?;
        Ok(record)
    }

    /// Re-evaluate the stored selector and require both its intent and geometry to
    /// agree. No update/rebind occurs on failure. Returned triangles address only
    /// the returned raw tessellation, before UV splitting/refinement/LOD.
    pub fn resolve(
        &self,
        inputs: &super::provenance::Inputs,
        solid: &Solid,
        groups: &crate::select::Groups,
    ) -> Result<ResolvedSelection> {
        self.validate()?;
        ensure!(
            self.inputs.clone().canonical()? == inputs.clone().canonical()?,
            "face-selection source inputs changed; rebind explicitly"
        );
        let (mesh, face_map) = tessellate(solid, inputs.tolerance_mm)?;
        let faces =
            crate::select::select_faces(&self.selector, solid, groups, inputs.tolerance_mm)?;
        let current = face_map.select(&mesh, &faces)?;
        ensure!(
            current.fingerprints == self.faces,
            "face selector resolves to changed geometry; rebind explicitly"
        );
        let triangles = face_map.resolve(&mesh, &current)?;
        Ok(ResolvedSelection {
            mesh,
            face_map,
            triangles,
        })
    }

    pub fn to_json(&self) -> Result<Vec<u8>> {
        self.validate()?;
        Ok(serde_json::to_vec_pretty(self)?)
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        let mut record: Self = serde_json::from_slice(bytes)?;
        record.validate()?;
        record.inputs = record.inputs.canonical()?;
        Ok(record)
    }
}

fn fingerprint(mesh: &Mesh, triangles: &[usize]) -> [u8; 32] {
    let mut tuples: Vec<[[u64; 6]; 3]> = triangles
        .iter()
        .map(|&index| {
            let vertices = mesh.triangles[index].map(|i| {
                let p = mesh.positions[i as usize];
                let n = mesh.normals[i as usize];
                [p[0], p[1], p[2], n[0], n[1], n[2]].map(|v| if v == 0.0 { 0 } else { v.to_bits() })
            });
            // Ignore cyclic corner rotation, but preserve winding and multiplicity.
            [
                vertices,
                [vertices[1], vertices[2], vertices[0]],
                [vertices[2], vertices[0], vertices[1]],
            ]
            .into_iter()
            .min()
            .unwrap()
        })
        .collect();
    tuples.sort_unstable();
    let mut hash = Sha256::new();
    hash.update(b"gcad-tessellated-face-v1");
    hash.update((tuples.len() as u64).to_le_bytes());
    for value in tuples.iter().flatten().flatten() {
        hash.update(value.to_le_bytes());
    }
    hash.finalize().into()
}

/// Tessellate with the same shell operation as ordinary asset conversion, retaining
/// source face order (the order used by `select::faces`). Missing face meshes fail
/// instead of yielding an apparently complete provenance map.
pub fn tessellate(solid: &Solid, tolerance_mm: f64) -> Result<(Mesh, FaceMap)> {
    ensure!(
        tolerance_mm.is_finite() && tolerance_mm > 0.,
        "tessellation tolerance must be finite and positive"
    );
    let mut polygon = PolygonMesh::default();
    let mut triangle_faces = Vec::new();
    let mut ranges = Vec::new();
    for shell in solid.boundaries() {
        let triangulated = shell.robust_triangulation(tolerance_mm);
        ensure!(
            shell.face_iter().count() == triangulated.face_iter().count(),
            "tessellation changed source face count"
        );
        for face in triangulated.face_iter() {
            let mut part = face
                .surface()
                .context("source face could not be tessellated")?;
            if !face.orientation() {
                part.invert();
            }
            let count = part.faces().tri_faces().len();
            ensure!(
                count > 0 && count == part.faces().len(),
                "source face requires nonempty triangle tessellation"
            );
            let start = triangle_faces.len();
            triangle_faces.extend(std::iter::repeat_n(ranges.len(), count));
            ranges.push(start..triangle_faces.len());
            polygon.merge(part);
        }
    }
    let mesh = Mesh::from_polygon(&polygon)?;
    ensure!(
        mesh.triangles.len() == triangle_faces.len(),
        "tessellation triangle order changed"
    );
    let fingerprints = ranges
        .into_iter()
        .map(|range| fingerprint(&mesh, &range.collect::<Vec<_>>()))
        .collect();
    let map = FaceMap {
        mesh_stamp: mesh.binding_stamp()?,
        triangle_faces,
        fingerprints,
    };
    Ok((mesh, map))
}

impl FaceMap {
    /// Preserve source-face lineage while editing UVs and splitting chart boundaries.
    pub fn transformed_uvs(
        &self,
        mesh: &Mesh,
        triangles: &[usize],
        transform: super::uv_edit::Transform,
    ) -> Result<(super::uv_edit::Edited, Self)> {
        self.triangle_faces(mesh)?;
        let mut edited = mesh.transformed_uvs(triangles, transform)?;
        let (mesh, map) = self.retain_triangles(mesh, edited.mesh)?;
        edited.mesh = mesh;
        Ok((edited, map))
    }
    /// Retain lineage for an explicit triangle subset with exact corner geometry.
    pub(crate) fn subset(
        &self,
        original: &Mesh,
        derived: &Mesh,
        indices: &[usize],
    ) -> Result<Self> {
        self.triangle_faces(original)?;
        derived.validate()?;
        ensure!(
            indices.len() == derived.triangles.len(),
            "face subset count changed"
        );
        let mut seen = std::collections::BTreeSet::new();
        for (&index, triangle) in indices.iter().zip(&derived.triangles) {
            ensure!(seen.insert(index), "face subset repeats a triangle");
            let source = original
                .triangles
                .as_slice()
                .get(index)
                .context("face subset index out of range")?;
            for (&a, &b) in source.iter().zip(triangle) {
                ensure!(
                    original.positions[a as usize] == derived.positions[b as usize]
                        && original.normals[a as usize] == derived.normals[b as usize],
                    "face subset changed corner geometry or winding"
                );
            }
        }
        Ok(Self {
            mesh_stamp: derived.binding_stamp()?,
            triangle_faces: indices.iter().map(|&i| self.triangle_faces[i]).collect(),
            fingerprints: self.fingerprints.clone(),
        })
    }
    pub(crate) fn retain_triangles(&self, original: &Mesh, derived: Mesh) -> Result<(Mesh, Self)> {
        self.triangle_faces(original)?;
        derived.validate()?;
        ensure!(
            original.triangles.len() == derived.triangles.len()
                && original
                    .triangles
                    .iter()
                    .zip(&derived.triangles)
                    .all(|(a, b)| a.map(|i| original.positions[i as usize])
                        == b.map(|i| derived.positions[i as usize])),
            "attribute operation changed triangle geometry/order; face lineage cannot be retained"
        );
        let map = Self {
            mesh_stamp: derived.binding_stamp()?,
            triangle_faces: self.triangle_faces.clone(),
            fingerprints: self.fingerprints.clone(),
        };
        Ok((derived, map))
    }

    /// Rebuild shading normals while retaining source-face lineage. Triangle
    /// corner positions/order must remain exact even when vertices split or join.
    pub fn crease_normals(&self, mesh: &Mesh, angle_degrees: f64) -> Result<(Mesh, Self)> {
        self.triangle_faces(mesh)?;
        self.retain_triangles(mesh, mesh.crease_normals(angle_degrees)?)
    }

    /// Box projection can split vertex tuples. Provenance follows triangles,
    /// with exact corner correspondence checked before accepting the new map.
    pub fn project_box(&self, mesh: &Mesh, tile_mm: f64) -> Result<(Mesh, Self)> {
        self.triangle_faces(mesh)?;
        self.retain_triangles(mesh, mesh.project_box(tile_mm)?)
    }

    /// Refine with explicit triangle lineage. Fingerprints continue to identify
    /// original CAD-face tessellations; the new mesh stamp guards derived geometry.
    /// Neither the original mesh/map nor captured selections are modified.
    pub fn refined(
        &self,
        mesh: &Mesh,
        levels: u32,
        triangle_budget: usize,
    ) -> Result<(Mesh, Self)> {
        let labels = self.triangle_faces(mesh)?;
        let (derived, labels) = mesh.refined_with_labels(levels, triangle_budget, Some(labels))?;
        let map = Self {
            mesh_stamp: derived.binding_stamp()?,
            triangle_faces: labels.context("refinement lost face labels")?,
            fingerprints: self.fingerprints.clone(),
        };
        Ok((derived, map))
    }

    pub fn face_count(&self) -> usize {
        self.fingerprints.len()
    }

    /// Split triangles at axis planes while retaining each child's source face.
    pub fn sliced(
        &self,
        mesh: &Mesh,
        axis: usize,
        planes_mm: &[f64],
        triangle_budget: usize,
    ) -> Result<(Mesh, Self)> {
        let labels = self.triangle_faces(mesh)?;
        let (derived, labels) =
            mesh.sliced_with_labels(axis, planes_mm, triangle_budget, Some(labels))?;
        let map = Self {
            mesh_stamp: derived.binding_stamp()?,
            triangle_faces: labels.context("slicing lost face labels")?,
            fingerprints: self.fingerprints.clone(),
        };
        Ok((derived, map))
    }

    /// Reduce while locking all vertices shared by different source faces, in
    /// addition to ordinary LOD boundaries and caller attachment locks. The target
    /// may be unreachable; inspect the returned LOD report. No face may disappear.
    pub fn simplified(
        &self,
        mesh: &Mesh,
        poses: &[Vec<[f64; 3]>],
        locked_vertices: &[usize],
        options: &super::lod::Options,
    ) -> Result<(super::lod::Lod, Self)> {
        let labels = self.triangle_faces(mesh)?;
        let mut owners = vec![None; mesh.positions.len()];
        let mut locked: std::collections::BTreeSet<_> = locked_vertices.iter().copied().collect();
        for (triangle, &face) in mesh.triangles.iter().zip(labels) {
            for &v in triangle {
                let v = v as usize;
                if owners[v].is_some_and(|owner| owner != face) {
                    locked.insert(v);
                } else {
                    owners[v] = Some(face);
                }
            }
        }
        let lod = super::lod::simplify(
            mesh,
            poses,
            &locked.into_iter().collect::<Vec<_>>(),
            options,
        )?;
        let triangle_faces: Vec<_> = lod.source_triangles.iter().map(|&i| labels[i]).collect();
        let before: std::collections::BTreeSet<_> = labels.iter().copied().collect();
        let after: std::collections::BTreeSet<_> = triangle_faces.iter().copied().collect();
        ensure!(before == after, "LOD removed a source face");
        let map = Self {
            mesh_stamp: lod.mesh.binding_stamp()?,
            triangle_faces,
            fingerprints: self.fingerprints.clone(),
        };
        Ok((lod, map))
    }

    /// UV/tangent edits that preserve the geometry stamp remain valid. Refinement,
    /// slicing and LOD require explicit provenance propagation. Use the FaceMap
    /// operation wrappers to retain lineage through refinement, slicing, LOD, normals and UVs.
    pub fn triangle_faces<'a>(&'a self, mesh: &Mesh) -> Result<&'a [usize]> {
        mesh.require_binding(self.mesh_stamp)?;
        Ok(&self.triangle_faces)
    }

    pub fn select(&self, mesh: &Mesh, faces: &[usize]) -> Result<Selection> {
        self.triangle_faces(mesh)?;
        ensure!(!faces.is_empty(), "face selection is empty");
        let mut fingerprints = Vec::new();
        for &face in faces {
            let value = *self
                .fingerprints
                .as_slice()
                .get(face)
                .context("selected face does not exist")?;
            ensure!(
                self.fingerprints.iter().filter(|&&v| v == value).count() == 1,
                "selected face geometry is ambiguous"
            );
            if !fingerprints.contains(&value) {
                fingerprints.push(value);
            }
        }
        fingerprints.sort_unstable();
        Ok(Selection { fingerprints })
    }

    /// Return current triangle indices only when every selected face has one exact
    /// geometry match. Reordering is allowed; split/merged/changed faces fail closed.
    pub fn resolve(&self, mesh: &Mesh, selection: &Selection) -> Result<Vec<usize>> {
        self.triangle_faces(mesh)?;
        let mut selected = std::collections::HashSet::new();
        for fingerprint in &selection.fingerprints {
            let matches: Vec<_> = self
                .fingerprints
                .iter()
                .enumerate()
                .filter_map(|(i, v)| (v == fingerprint).then_some(i))
                .collect();
            ensure!(
                matches.len() == 1,
                "selected face is missing, changed or ambiguous; rebind explicitly"
            );
            selected.insert(matches[0]);
        }
        Ok(self
            .triangle_faces
            .iter()
            .enumerate()
            .filter_map(|(i, face)| selected.contains(face).then_some(i))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lod_retains_shared_face_boundary_and_explicit_triangle_lineage() {
        let mut mesh = Mesh {
            positions: (0..7)
                .flat_map(|y| (0..7).map(move |x| [x as f64, y as f64, 0.]))
                .collect(),
            normals: vec![[0., 0., 1.]; 49],
            texcoords: None,
            tangents: None,
            triangles: Vec::new(),
        };
        let mut labels = Vec::new();
        for y in 0..6 {
            for x in 0..6 {
                let a = y * 7 + x;
                mesh.triangles
                    .extend([[a, a + 1, a + 8], [a, a + 8, a + 7]]);
                labels.extend([usize::from(x >= 3); 2]);
            }
        }
        let original = mesh.clone();
        let map = FaceMap {
            mesh_stamp: mesh.binding_stamp().unwrap(),
            triangle_faces: labels,
            fingerprints: vec![[1; 32], [2; 32]],
        };
        let selection = map.select(&mesh, &[0]).unwrap();
        let options = super::super::lod::Options {
            target_triangles: 40,
            max_error_mm: 2.,
            max_uv_error: 0.,
            max_angle_degrees: 30.,
            max_trials: 1000,
        };
        let poses = vec![
            mesh.positions
                .iter()
                .map(|p| [p[0], p[1], p[2] + 10.])
                .collect(),
        ];
        let (lod, derived) = map.simplified(&mesh, &poses, &[16], &options).unwrap();
        assert!(lod.mesh.triangles.len() < mesh.triangles.len());
        for v in (0..7).map(|y| y * 7 + 3).chain([16]) {
            assert_eq!(
                lod.mesh.positions[lod.original_to_lod[v] as usize],
                mesh.positions[v]
            );
        }
        let mut area = [0.; 2];
        for (i, triangle) in lod.mesh.triangles.iter().enumerate() {
            assert_eq!(
                *triangle,
                mesh.triangles[lod.source_triangles[i]].map(|v| lod.original_to_lod[v as usize])
            );
            let face = derived.triangle_faces[i];
            assert_eq!(face, map.triangle_faces[lod.source_triangles[i]]);
            let [a, b, c] = triangle.map(|v| lod.mesh.positions[v as usize]);
            assert!(
                [a, b, c]
                    .iter()
                    .all(|p| if face == 0 { p[0] <= 3. } else { p[0] >= 3. })
            );
            let cross = super::super::cross(super::super::sub(b, a), super::super::sub(c, a));
            assert!(cross[2] > 0.);
            area[face] += cross[2] / 2.;
        }
        assert_eq!(area, [18., 18.]);
        let selected = derived.resolve(&lod.mesh, &selection).unwrap();
        assert!(!selected.is_empty());
        assert!(selected.iter().all(|&i| derived.triangle_faces[i] == 0));
        assert!(map.triangle_faces(&lod.mesh).is_err());
        assert!(map.simplified(&mesh, &poses, &[49], &options).is_err());
        assert_eq!(mesh, original);
    }

    #[test]
    fn slicing_preserves_selected_surface_and_lineage_across_planes() {
        let model = model(30);
        let solid = model.solid.as_ref().unwrap();
        let (mesh, map) = tessellate(solid, 0.1).unwrap();
        let original = mesh.clone();
        let faces = crate::select::select_faces("base.end", solid, &model.groups, 0.1).unwrap();
        let selection = map.select(&mesh, &faces).unwrap();
        for planes in [&[-100., 100.][..], &[-5., 0., 5.][..]] {
            let (sliced, sliced_map) = map.sliced(&mesh, 0, planes, 1000).unwrap();
            assert_eq!(sliced, mesh.sliced(0, planes, 1000).unwrap());
            let mut area = 0.;
            let selected = sliced_map.resolve(&sliced, &selection).unwrap();
            for &index in &selected {
                let [a, b, c] = sliced.triangles[index].map(|v| sliced.positions[v as usize]);
                assert!([a, b, c].iter().all(|p| (p[2] - 5.).abs() < 1e-10));
                let cross = super::super::cross(super::super::sub(b, a), super::super::sub(c, a));
                assert!(cross[2] > 0.);
                area += cross[2] / 2.;
            }
            assert!((area - 600.).abs() < 1e-8);
            assert_eq!(
                sliced_map.triangle_faces(&sliced).unwrap().len(),
                sliced.triangles.len()
            );
            let (uv, uv_map) = sliced_map.project_box(&sliced, 25.).unwrap();
            assert_eq!(uv_map.resolve(&uv, &selection).unwrap(), selected);
            if sliced != mesh {
                assert!(map.sliced(&sliced, 0, &[0.], 1000).is_err());
                assert!(selected.len() > 2);
            }
        }
        assert!(map.sliced(&mesh, 0, &[0.], 12).is_err());
        assert!(mesh.sliced_with_labels(0, &[0.], 1000, Some(&[])).is_err());
        assert_eq!(mesh, original);
        assert_eq!(map.resolve(&mesh, &selection).unwrap().len(), 2);
    }

    #[test]
    fn refinement_propagates_source_faces_through_children_and_preserves_guards() {
        let model = model(30);
        let solid = model.solid.as_ref().unwrap();
        let (mesh, map) = tessellate(solid, 0.1).unwrap();
        let before = mesh.clone();
        let before_labels = map.triangle_faces(&mesh).unwrap().to_vec();
        let selected_faces =
            crate::select::select_faces("base.end", solid, &model.groups, 0.1).unwrap();
        let selection = map.select(&mesh, &selected_faces).unwrap();
        for levels in [0, 1, 2] {
            let (refined, propagated) = map.refined(&mesh, levels, 1000).unwrap();
            assert_eq!(refined, mesh.refined(levels, 1000).unwrap());
            let selected = propagated.resolve(&refined, &selection).unwrap();
            assert_eq!(selected.len(), 2 * 4usize.pow(levels));
            let mut area = 0.;
            for index in selected {
                let [a, b, c] = refined.triangles[index].map(|v| refined.positions[v as usize]);
                assert!([a, b, c].iter().all(|p| (p[2] - 5.).abs() < 1e-10));
                let normal = super::super::cross(super::super::sub(b, a), super::super::sub(c, a));
                assert!(normal[2] > 0.);
                area += normal[2] / 2.;
            }
            assert!((area - 600.).abs() < 1e-8);
            assert_eq!(
                propagated.triangle_faces(&refined).unwrap().len(),
                refined.triangles.len()
            );
            if levels > 0 {
                assert!(map.triangle_faces(&refined).is_err());
            }
            let mut stale = refined.clone();
            stale.positions[0][0] += 1.;
            assert!(propagated.triangle_faces(&stale).is_err());
        }
        assert!(map.refined(&mesh, 2, 191).is_err());
        assert!(map.refined(&mesh, 9, usize::MAX).is_err());
        assert_eq!(mesh, before);
        assert_eq!(map.triangle_faces(&mesh).unwrap(), before_labels);
        assert!(mesh.refined_with_labels(1, 1000, Some(&[])).is_err());
    }

    #[test]
    fn refinement_normals_and_uvs_keep_selection_and_reject_reordered_triangles() {
        let model = model(30);
        let solid = model.solid.as_ref().unwrap();
        let (mesh, map) = tessellate(solid, 0.1).unwrap();
        let faces = crate::select::select_faces("base.end", solid, &model.groups, 0.1).unwrap();
        let selection = map.select(&mesh, &faces).unwrap();
        let (refined, refined_map) = map.refined(&mesh, 1, 1000).unwrap();
        let (smooth, smooth_map) = refined_map.crease_normals(&refined, 120.).unwrap();
        assert_eq!(smooth, refined.crease_normals(120.).unwrap());
        assert_ne!(smooth.normals, refined.normals);
        let (uv, uv_map) = smooth_map.project_box(&smooth, 25.).unwrap();
        assert_eq!(uv, smooth.project_box(25.).unwrap());
        assert!(uv.texcoords.is_some() && uv.tangents.is_some());
        let selected = uv_map.resolve(&uv, &selection).unwrap();
        assert_eq!(selected.len(), 8);
        let (edited, edited_map) = uv_map
            .transformed_uvs(
                &uv,
                &selected,
                super::super::uv_edit::Transform {
                    scale: [-1., 2.],
                    rotation_degrees: 30.,
                    offset: [0.1, 0.2],
                },
            )
            .unwrap();
        assert_eq!(
            edited_map.resolve(&edited.mesh, &selection).unwrap(),
            selected
        );
        let mut stale_uv = edited.mesh.clone();
        stale_uv.positions[0][0] += 1.;
        assert!(
            edited_map
                .transformed_uvs(
                    &stale_uv,
                    &selected,
                    super::super::uv_edit::Transform::default()
                )
                .is_err()
        );
        for i in selected {
            for v in uv.triangles[i] {
                assert!((uv.positions[v as usize][2] - 5.).abs() < 1e-10);
            }
        }
        assert_eq!(
            uv_map.triangle_faces(&uv).unwrap(),
            refined_map.triangle_faces(&refined).unwrap()
        );
        assert!(refined_map.project_box(&smooth, 25.).is_err());
        let mut reordered = smooth.clone();
        let last = reordered.triangles.len() - 1;
        reordered.triangles.swap(0, last);
        assert!(smooth_map.retain_triangles(&smooth, reordered).is_err());
        assert!(smooth_map.project_box(&smooth, 0.).is_err());
        assert!(uv_map.refined(&uv, 1, 1000).is_err());
    }

    fn inputs() -> super::super::provenance::Inputs {
        use std::collections::BTreeMap;
        super::super::provenance::Inputs {
            source: "parts/box.gcad".into(),
            body: "main".into(),
            instance: "box".into(),
            parameters: BTreeMap::new(),
            dependencies: BTreeMap::from([("parts/box.gcad".into(), [7; 32])]),
            observations: BTreeMap::from([("parts/box.gcad".into(), true)]),
            tolerance_mm: 0.1,
            generator: "face-selection-test-v1".into(),
        }
    }

    #[test]
    fn persisted_selection_checks_source_and_selector_without_silent_rebinding() {
        let model = model(30);
        let solid = model.solid.as_ref().unwrap();
        let source = inputs();
        let record =
            SelectionFile::capture(source.clone(), solid, &model.groups, "base.end").unwrap();
        let bytes = record.to_json().unwrap();
        let loaded = SelectionFile::from_json(&bytes).unwrap();
        let resolved = loaded.resolve(&source, solid, &model.groups).unwrap();
        assert_eq!(resolved.triangles.len(), 2);
        assert_eq!(
            resolved
                .face_map
                .triangle_faces(&resolved.mesh)
                .unwrap()
                .len(),
            resolved.mesh.triangles.len()
        );
        let reordered = Solid::new_unchecked(
            solid
                .boundaries()
                .iter()
                .map(|s| s.face_iter().rev().cloned().collect())
                .collect(),
        );
        assert_eq!(
            loaded
                .resolve(&source, &reordered, &model.groups)
                .unwrap()
                .triangles
                .len(),
            2
        );
        let regenerated = self::model(30);
        assert_eq!(
            loaded
                .resolve(
                    &source,
                    regenerated.solid.as_ref().unwrap(),
                    &regenerated.groups
                )
                .unwrap()
                .triangles
                .len(),
            2
        );
        let mut changed = source.clone();
        changed
            .dependencies
            .insert("parts/box.gcad".into(), [8; 32]);
        assert!(loaded.resolve(&changed, solid, &model.groups).is_err());
        changed = source.clone();
        changed.instance = "other".into();
        assert!(loaded.resolve(&changed, solid, &model.groups).is_err());
        changed = source.clone();
        changed.observations.insert("optional.gcad".into(), false);
        assert!(loaded.resolve(&changed, solid, &model.groups).is_err());
        changed = source.clone();
        changed.tolerance_mm = 0.2;
        assert!(loaded.resolve(&changed, solid, &model.groups).is_err());
        let mut groups = model.groups.clone();
        for entry in &mut groups.0 {
            if entry.group == "end" {
                entry.group = "renamed".into();
            }
        }
        assert!(loaded.resolve(&source, solid, &groups).is_err());
        let changed_geometry = self::model(40);
        assert!(
            loaded
                .resolve(
                    &source,
                    changed_geometry.solid.as_ref().unwrap(),
                    &changed_geometry.groups
                )
                .is_err()
        );
        let mut edited: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        edited["selector"] = serde_json::json!(">X");
        let redirected = SelectionFile::from_json(&serde_json::to_vec(&edited).unwrap()).unwrap();
        assert!(
            redirected
                .resolve(&source, solid, &model.groups)
                .unwrap_err()
                .to_string()
                .contains("changed geometry")
        );
        assert_eq!(record.to_json().unwrap(), bytes);
    }

    #[test]
    fn persisted_selection_rejects_unknown_schema_and_invalid_guards() {
        let model = model(30);
        let solid = model.solid.as_ref().unwrap();
        let record = SelectionFile::capture(inputs(), solid, &model.groups, "base.end").unwrap();
        let original: serde_json::Value =
            serde_json::from_slice(&record.to_json().unwrap()).unwrap();
        for (field, value) in [
            ("version", serde_json::json!(2)),
            ("extra", serde_json::json!(true)),
            ("selector", serde_json::json!("é")),
            ("selector", serde_json::json!("")),
            ("faces", serde_json::json!([])),
            (
                "faces",
                serde_json::json!([original["faces"][0], original["faces"][0]]),
            ),
        ] {
            let mut edited = original.clone();
            edited[field] = value;
            assert!(SelectionFile::from_json(&serde_json::to_vec(&edited).unwrap()).is_err());
        }
        assert!(SelectionFile::capture(inputs(), solid, &model.groups, "é").is_err());
        assert!(SelectionFile::capture(inputs(), solid, &model.groups, "missing.end").is_err());
        let mut bad = inputs();
        bad.source = "../private.gcad".into();
        assert!(SelectionFile::capture(bad, solid, &model.groups, "base.end").is_err());
    }

    #[test]
    fn curved_plate_retains_every_face_and_matches_ordinary_tessellation() {
        let model = crate::model::run(
            &crate::parse::parse_program(include_str!("../../examples/parts/plate.gcad")).unwrap(),
        )
        .model;
        let solid = model.solid.as_ref().unwrap();
        let (mesh, map) = tessellate(solid, 0.1).unwrap();
        assert_eq!(mesh, Mesh::from_solid(solid, 0.1).unwrap());
        assert_eq!(map.face_count(), crate::select::faces(solid).len());
        let selected: Vec<_> = (0..map.face_count()).collect();
        let selection = map.select(&mesh, &selected).unwrap();
        assert_eq!(
            map.resolve(&mesh, &selection).unwrap(),
            (0..mesh.triangles.len()).collect::<Vec<_>>()
        );
        let mut changed = mesh.clone();
        changed.normals[0] = changed.normals[0].map(|v| -v);
        assert!(map.triangle_faces(&changed).is_err());
    }

    fn model(width: u32) -> crate::model::Model {
        crate::model::run(
            &crate::parse::parse_program(&format!("rect {width} 20\nbase: extrude 5\n")).unwrap(),
        )
        .model
    }

    #[test]
    fn face_mapping_matches_mesh_and_remaps_regeneration_and_face_order() {
        let model = model(30);
        let solid = model.solid.as_ref().unwrap();
        let (mesh, map) = tessellate(solid, 0.1).unwrap();
        assert_eq!(mesh, Mesh::from_solid(solid, 0.1).unwrap());
        assert_eq!(map.face_count(), 6);
        assert_eq!(
            map.triangle_faces(&mesh).unwrap().len(),
            mesh.triangles.len()
        );
        let end = crate::select::select_faces("base.end", solid, &model.groups, 0.1).unwrap();
        assert_eq!(end.len(), 1);
        let selection = map.select(&mesh, &end).unwrap();
        let selected = map.resolve(&mesh, &selection).unwrap();
        assert_eq!(selected.len(), 2);
        for i in &selected {
            for &v in &mesh.triangles[*i] {
                assert!((mesh.positions[v as usize][2] - 5.).abs() < 1e-10);
            }
        }
        let regenerated = self::model(30);
        let (again, again_map) = tessellate(regenerated.solid.as_ref().unwrap(), 0.1).unwrap();
        assert_eq!(again_map.resolve(&again, &selection).unwrap().len(), 2);
        let reordered = Solid::new_unchecked(
            solid
                .boundaries()
                .iter()
                .map(|shell| shell.face_iter().rev().cloned().collect())
                .collect(),
        );
        let (reversed, reversed_map) = tessellate(&reordered, 0.1).unwrap();
        let indices = reversed_map.resolve(&reversed, &selection).unwrap();
        assert_eq!(indices.len(), 2);
        for i in indices {
            for &v in &reversed.triangles[i] {
                assert!((reversed.positions[v as usize][2] - 5.).abs() < 1e-10);
            }
        }
        assert!(map.triangle_faces(&reversed).is_err());
        let changed = self::model(40);
        let (changed, changed_map) = tessellate(changed.solid.as_ref().unwrap(), 0.1).unwrap();
        assert!(changed_map.resolve(&changed, &selection).is_err());
        assert!(map.select(&mesh, &[]).is_err());
        assert!(map.select(&mesh, &[6]).is_err());
        assert!(map.triangle_faces(&mesh.refined(1, 1000).unwrap()).is_err());
    }

    #[test]
    fn duplicate_and_missing_faces_fail_without_changing_the_solid() {
        let model = model(30);
        let solid = model.solid.as_ref().unwrap();
        let (mesh, map) = tessellate(solid, 0.1).unwrap();
        let selection = map.select(&mesh, &[0]).unwrap();
        let duplicate = Solid::new_unchecked(vec![
            solid.boundaries()[0].clone(),
            solid.boundaries()[0].clone(),
        ]);
        let (d, dm) = tessellate(&duplicate, 0.1).unwrap();
        assert!(dm.select(&d, &[0]).is_err());
        assert!(dm.resolve(&d, &selection).is_err());
        let missing = Solid::new_unchecked(vec![
            solid.boundaries()[0].face_iter().skip(1).cloned().collect(),
        ]);
        let (m, mm) = tessellate(&missing, 0.1).unwrap();
        assert!(mm.resolve(&m, &selection).is_err());
        assert_eq!(mesh, Mesh::from_solid(solid, 0.1).unwrap());
        assert!(tessellate(solid, f64::NAN).is_err());
    }
}
