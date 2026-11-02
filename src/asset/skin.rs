//! Explicit four-influence linear skinning. Bindings address whole vertex tuples.
use super::{
    BindingStamp,
    scene::{Scene, Transform},
};
use anyhow::{Context, Result, ensure};
use monstertruck::modeling::{
    Matrix as _, Matrix3, Matrix4, Point3, SquareMatrix, Transform as _, Vector3,
};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weights {
    /// Indices into the skin's joint list, not scene node indices.
    pub joints: [u16; 4],
    pub weights: [f64; 4],
}

#[derive(Clone, Debug)]
pub struct Skin {
    pub(crate) mesh_node: usize,
    pub(crate) joints: Vec<usize>,
    pub(crate) inverse_bind: Vec<Matrix4>,
    pub(crate) weights: Vec<Vec<Weights>>,
    stamps: Vec<BindingStamp>,
    rest_world: Vec<Matrix4>,
    rest_hierarchy: Vec<(String, Option<usize>)>,
}

/// Explicit palette remap; scene node indices and hierarchy are never changed.
#[derive(Debug, serde::Serialize)]
pub struct PaletteReport {
    pub mesh_node: String,
    pub original_joints: usize,
    pub retained_joints: usize,
    pub old_to_new: Vec<Option<usize>>,
    /// Names removed from this palette only; the scene nodes still exist.
    pub removed_nodes: Vec<String>,
}

/// World-space shading basis under the per-vertex blended affine transform.
/// This ignores spatial weight derivatives and is not a recomputed surface normal.
#[derive(Clone, Debug)]
pub struct DeformedBasis {
    pub normals: Vec<[f64; 3]>,
    pub tangents: Option<Vec<[f64; 4]>>,
}

/// Versioned authoring resource. Edit weight values, retaining the captured
/// geometry/rig stamps; changed topology or rest pose requires explicit rebinding.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightFile {
    version: u32,
    joints: Vec<String>,
    mesh_stamps: Vec<[u8; 32]>,
    rig_stamp: [u8; 32],
    weights: Vec<Vec<Weights>>,
    source_bindings: Option<Vec<super::provenance::Binding>>,
}

#[derive(Clone, Copy, Debug)]
pub enum Assignment {
    Rigid,
    /// Blend joint 0 to joint 1 along a mesh-local CAD axis, clamping outside
    /// the declared millimetre interval. This is explicit projection, not auto-rigging.
    Linear {
        axis: usize,
        start_mm: f64,
        end_mm: f64,
    },
}

impl WeightFile {
    pub fn to_json(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        let value: Self = serde_json::from_slice(bytes)?;
        ensure!(
            value.version == 2,
            "unsupported skin weight version; explicitly rebind"
        );
        Ok(value)
    }
    pub fn bind(&self, scene: &Scene, node: usize) -> Result<Skin> {
        ensure!(
            self.version == 2,
            "unsupported skin weight version; explicitly rebind"
        );
        let joints = self
            .joints
            .iter()
            .map(|name| {
                scene
                    .nodes
                    .iter()
                    .position(|n| &n.name == name)
                    .context("weight file joint no longer exists")
            })
            .collect::<Result<_>>()?;
        let skin = Skin::bind(scene, node, joints, self.weights.clone())?;
        ensure!(
            skin.stamps.iter().map(|s| s.0).collect::<Vec<_>>() == self.mesh_stamps,
            "weight file mesh changed; explicit rebinding is required"
        );
        ensure!(
            skin.rig_stamp()? == self.rig_stamp,
            "weight file rig changed; explicit rebinding is required"
        );
        Ok(skin)
    }

    pub fn with_inputs(
        mut self,
        scene: &Scene,
        node: usize,
        inputs: &super::provenance::Inputs,
    ) -> Result<Self> {
        self.bind(scene, node)?;
        let mesh = scene.nodes[node].mesh.unwrap();
        self.source_bindings = Some(
            scene.meshes[mesh]
                .primitives
                .iter()
                .map(|p| super::provenance::Binding::capture(inputs.clone(), &p.geometry))
                .collect::<Result<_>>()?,
        );
        Ok(self)
    }

    pub fn bind_with_inputs(
        &self,
        scene: &Scene,
        node: usize,
        inputs: &super::provenance::Inputs,
    ) -> Result<Skin> {
        let skin = self.bind(scene, node)?;
        let bindings = self
            .source_bindings
            .as_ref()
            .context("weight file lacks source-input guards; explicitly rebind")?;
        let mesh = scene.nodes[node].mesh.unwrap();
        ensure!(
            bindings.len() == scene.meshes[mesh].primitives.len(),
            "source binding count changed; rebind"
        );
        for (binding, primitive) in bindings.iter().zip(&scene.meshes[mesh].primitives) {
            binding.require_current(inputs, &primitive.geometry)?;
        }
        Ok(skin)
    }
}

impl Skin {
    /// Remove zero-influence palette entries, preserving animated nodes, parents
    /// (including attachment parents), and explicitly protected scene nodes.
    /// No scene nodes, tracks, weights or inverse-bind matrices are approximated.
    pub fn prune_palette(&self, scene: &Scene, keep: &[usize]) -> Result<(Self, PaletteReport)> {
        self.validate(scene)?;
        ensure!(keep.iter().all(|&node| node < scene.nodes.len()), "protected joint node does not exist");
        let mut protected: HashSet<usize> = keep.iter().copied().collect();
        protected.extend(scene.nodes.iter().filter_map(|node| node.parent));
        protected.extend(scene.clips.iter().flat_map(|clip| clip.tracks.iter().map(|track| track.node)));
        if let Some(profile)=&scene.consumer {
            protected.extend(profile.sockets.iter().filter_map(|socket|scene.nodes.iter().position(|node|node.name==socket.node)));
        }
        let mut used: Vec<bool> = self.joints.iter().map(|node| protected.contains(node)).collect();
        for vertex in self.weights.iter().flatten() {
            for (&joint, &weight) in vertex.joints.iter().zip(&vertex.weights) {
                if weight > 0. { used[joint as usize] = true; }
            }
        }
        let retained: Vec<_> = used.iter().enumerate().filter_map(|(i, &keep)| keep.then_some(i)).collect();
        // Valid nonempty meshes must retain at least one positive influence.
        ensure!(!retained.is_empty(), "cannot produce an empty joint palette");
        let mut mapping = vec![None; self.joints.len()];
        for (new, &old) in retained.iter().enumerate() { mapping[old] = Some(new); }
        let mut derived = self.clone();
        derived.joints = retained.iter().map(|&i| self.joints[i]).collect();
        derived.inverse_bind = retained.iter().map(|&i| self.inverse_bind[i]).collect();
        derived.rest_world = std::iter::once(self.rest_world[0])
            .chain(retained.iter().map(|&i| self.rest_world[i + 1])).collect();
        for vertex in derived.weights.iter_mut().flatten() {
            for joint in &mut vertex.joints { *joint = mapping[*joint as usize].unwrap_or(0) as u16; }
        }
        derived.validate(scene)?;
        let report = PaletteReport {
            mesh_node: scene.nodes[self.mesh_node].name.clone(),
            original_joints: self.joints.len(),
            retained_joints: retained.len(),
            removed_nodes: self.joints.iter().enumerate().filter_map(|(i, &node)|
                (!used[i]).then(|| scene.nodes[node].name.clone())).collect(),
            old_to_new: mapping,
        };
        Ok((derived, report))
    }

    /// Piecewise-linear assignment between explicitly ordered joint knots in
    /// mesh-local millimetres. At most two adjacent joints influence a vertex.
    /// Knots do not infer joint positions, hierarchy or animation.
    pub fn assign_chain(
        scene: &Scene,
        node: usize,
        joints: Vec<usize>,
        axis: usize,
        knots_mm: &[f64],
    ) -> Result<Self> {
        scene.validate()?;
        ensure!(axis < 3, "chain axis must be x/y/z");
        ensure!(
            (2..=65536).contains(&joints.len()) && knots_mm.len() == joints.len(),
            "chain requires 2..65536 joints and one knot per joint"
        );
        ensure!(
            knots_mm.iter().all(|v| v.is_finite())
                && knots_mm
                    .windows(2)
                    .all(|w| w[1] > w[0] && (w[1] - w[0]).is_finite()),
            "chain knots must be finite and strictly increasing"
        );
        let mesh = scene
            .nodes
            .get(node)
            .and_then(|n| n.mesh)
            .context("assignment requires a mesh node")?;
        let weights = scene.meshes[mesh]
            .primitives
            .iter()
            .map(|primitive| {
                primitive
                    .geometry
                    .positions
                    .iter()
                    .map(|position| {
                        let coordinate = position[axis];
                        let last = knots_mm.len() - 1;
                        if coordinate <= knots_mm[0] || coordinate >= knots_mm[last] {
                            let joint = if coordinate <= knots_mm[0] { 0 } else { last };
                            return Ok(Weights {
                                joints: [joint as u16, 0, 0, 0],
                                weights: [1.0, 0.0, 0.0, 0.0],
                            });
                        }
                        let lower = knots_mm.partition_point(|k| *k <= coordinate) - 1;
                        let t = (coordinate - knots_mm[lower])
                            / (knots_mm[lower + 1] - knots_mm[lower]);
                        ensure!(t.is_finite(), "chain projection exceeds numeric range");
                        Ok(Weights {
                            joints: [lower as u16, (lower + 1) as u16, 0, 0],
                            weights: [1.0 - t, t, 0.0, 0.0],
                        })
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .collect::<Result<Vec<_>>>()?;
        Self::bind(scene, node, joints, weights)
    }

    pub fn assign(
        scene: &Scene,
        node: usize,
        joints: Vec<usize>,
        assignment: Assignment,
    ) -> Result<Self> {
        scene.validate()?;
        let mesh = scene
            .nodes
            .get(node)
            .and_then(|n| n.mesh)
            .context("assignment requires a mesh node")?;
        match assignment {
            Assignment::Rigid => ensure!(joints.len() == 1, "rigid assignment requires one joint"),
            Assignment::Linear {
                axis,
                start_mm,
                end_mm,
            } => {
                ensure!(
                    joints.len() == 2,
                    "linear assignment requires two ordered joints"
                );
                ensure!(
                    axis < 3
                        && start_mm.is_finite()
                        && end_mm.is_finite()
                        && end_mm > start_mm
                        && (end_mm - start_mm).is_finite(),
                    "blend interval must be finite and increasing on axis x/y/z"
                );
            }
        }
        let weights = scene.meshes[mesh]
            .primitives
            .iter()
            .map(|p| {
                p.geometry
                    .positions
                    .iter()
                    .map(|p| {
                        let t = match assignment {
                            Assignment::Rigid => 0.0,
                            Assignment::Linear {
                                axis,
                                start_mm,
                                end_mm,
                            } => {
                                let value = (p[axis] - start_mm) / (end_mm - start_mm);
                                ensure!(
                                    value.is_finite(),
                                    "blend projection exceeds numeric range"
                                );
                                value.clamp(0.0, 1.0)
                            }
                        };
                        Ok(Weights {
                            joints: if joints.len() == 1 {
                                [0; 4]
                            } else {
                                [0, 1, 0, 0]
                            },
                            weights: [1.0 - t, t, 0.0, 0.0],
                        })
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .collect::<Result<Vec<_>>>()?;
        Self::bind(scene, node, joints, weights)
    }
    /// Capture a portable authoring resource after explicit weight assignment.
    pub fn weight_file(&self) -> Result<WeightFile> {
        Ok(WeightFile {
            version: 2,
            joints: self
                .joints
                .iter()
                .map(|j| self.rest_hierarchy[*j].0.clone())
                .collect(),
            mesh_stamps: self.stamps.iter().map(|s| s.0).collect(),
            rig_stamp: self.rig_stamp()?,
            weights: self.weights.clone(),
            source_bindings: None,
        })
    }

    fn rig_stamp(&self) -> Result<[u8; 32]> {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(b"gcad-skin-rig-v1\0");
        hash.update(serde_json::to_vec(&(
            self.mesh_node,
            &self.joints,
            &self.rest_hierarchy,
        ))?);
        for matrix in &self.rest_world {
            let values: &[[f64; 4]; 4] = matrix.as_ref();
            for &v in values.iter().flatten() {
                hash.update(if v == 0.0 { 0.0_f64 } else { v }.to_le_bytes());
            }
        }
        Ok(hash.finalize().into())
    }
    /// Bind the current scene as rest pose. One weight array per mesh primitive.
    /// No automatic normalisation, truncation, welding or weight reassignment.
    pub fn bind(
        scene: &Scene,
        mesh_node: usize,
        joints: Vec<usize>,
        weights: Vec<Vec<Weights>>,
    ) -> Result<Self> {
        scene.validate()?;
        let mesh = scene
            .nodes
            .get(mesh_node)
            .and_then(|n| n.mesh)
            .context("skin requires a mesh node")?;
        ensure!(
            !joints.is_empty() && joints.len() <= 65536,
            "skin requires 1..65536 joints"
        );
        let mut unique = HashSet::new();
        ensure!(
            joints
                .iter()
                .all(|j| *j < scene.nodes.len() && unique.insert(*j)),
            "skin joints must be unique valid nodes"
        );
        let world = scene.world_transforms()?;
        let inverse_bind = joints
            .iter()
            .map(|&j| {
                Ok(world[j]
                    .invert()
                    .context("joint bind transform is singular")?
                    * world[mesh_node])
            })
            .collect::<Result<_>>()?;
        let stamps = scene.meshes[mesh]
            .primitives
            .iter()
            .map(|p| p.geometry.binding_stamp())
            .collect::<Result<_>>()?;
        let mut rest_world = vec![world[mesh_node]];
        rest_world.extend(joints.iter().map(|&j| world[j]));
        let skin = Self {
            mesh_node,
            joints,
            inverse_bind,
            weights,
            stamps,
            rest_world,
            rest_hierarchy: scene
                .nodes
                .iter()
                .map(|n| (n.name.clone(), n.parent))
                .collect(),
        };
        skin.validate(scene)?;
        Ok(skin)
    }

    pub fn validate(&self, scene: &Scene) -> Result<()> {
        scene.validate()?;
        ensure!(
            scene
                .nodes
                .iter()
                .map(|n| (n.name.clone(), n.parent))
                .collect::<Vec<_>>()
                == self.rest_hierarchy,
            "skin node identity or hierarchy changed; rebind"
        );
        let mesh = scene
            .nodes
            .get(self.mesh_node)
            .and_then(|n| n.mesh)
            .context("skin mesh node no longer exists")?;
        let primitives = &scene.meshes[mesh].primitives;
        ensure!(
            primitives.len() == self.weights.len() && primitives.len() == self.stamps.len(),
            "skin primitive count changed; rebind"
        );
        let world = scene.world_transforms()?;
        ensure!(
            world[self.mesh_node] == self.rest_world[0]
                && self
                    .joints
                    .iter()
                    .enumerate()
                    .all(|(i, j)| world.get(*j) == Some(&self.rest_world[i + 1])),
            "skin rest transforms changed; rebind"
        );
        for ((primitive, weights), stamp) in primitives.iter().zip(&self.weights).zip(&self.stamps)
        {
            primitive.geometry.require_binding(*stamp)?;
            ensure!(
                weights.len() == primitive.geometry.positions.len(),
                "weight count differs from vertex count"
            );
            for vertex in weights {
                ensure!(
                    vertex
                        .joints
                        .iter()
                        .all(|j| (*j as usize) < self.joints.len()),
                    "weight references invalid joint"
                );
                ensure!(
                    vertex
                        .weights
                        .iter()
                        .all(|w| w.is_finite() && (0.0..=1.0).contains(w)),
                    "weights must be finite and nonnegative"
                );
                ensure!(
                    (vertex.weights.iter().sum::<f64>() - 1.0).abs() < 1e-8,
                    "weights must sum to one; zero-weight vertices are invalid"
                );
                let mut used = HashSet::new();
                ensure!(
                    vertex
                        .joints
                        .iter()
                        .zip(vertex.weights)
                        .all(|(j, w)| w == 0.0 || used.insert(j)),
                    "duplicate nonzero joint influence"
                );
            }
        }
        for matrix in &self.inverse_bind {
            let values: &[[f64; 4]; 4] = matrix.as_ref();
            ensure!(
                values.iter().flatten().all(|v| v.is_finite()),
                "inverse bind matrix exceeds numeric range"
            );
        }
        Ok(())
    }

    fn pose_matrices(&self, scene: &Scene, pose: &[Transform]) -> Result<Vec<Matrix4>> {
        self.validate(scene)?;
        ensure!(
            pose.len() == scene.nodes.len(),
            "pose node count differs from scene"
        );
        let world = scene.posed_world_transforms(pose)?;
        Ok(self.matrices_from_world(&world))
    }

    // Playback callers validate the immutable rest asset once, before animation.
    pub(crate) fn matrices_from_world(&self, world: &[Matrix4]) -> Vec<Matrix4> {
        self
            .joints
            .iter()
            .zip(&self.inverse_bind)
            .map(|(joint, bind)| world[*joint] * bind)
            .collect()
    }

    /// Inverse-transpose the blended linear matrix for normals, transform and
    /// orthogonalise tangents, and flip handedness for reflected matrices.
    /// Singular blends fail explicitly even if individual joints are invertible.
    pub fn deform_basis(&self, scene: &Scene, pose: &[Transform]) -> Result<Vec<DeformedBasis>> {
        let matrices = self.pose_matrices(scene, pose)?;
        self.basis_from_matrices(scene, &matrices)
    }

    pub(crate) fn basis_from_matrices(&self, scene: &Scene, matrices: &[Matrix4]) -> Result<Vec<DeformedBasis>> {
        let mesh = &scene.meshes[scene.nodes[self.mesh_node].mesh.unwrap()];
        mesh.primitives
            .iter()
            .zip(&self.weights)
            .map(|(primitive, weights)| {
                let geometry = &primitive.geometry;
                let mut result = DeformedBasis {
                    normals: Vec::with_capacity(weights.len()),
                    tangents: geometry
                        .tangents
                        .as_ref()
                        .map(|_| Vec::with_capacity(weights.len())),
                };
                for (i, weights) in weights.iter().enumerate() {
                    let mut linear = Matrix3::from_value(0.);
                    for (&joint, &weight) in weights.joints.iter().zip(&weights.weights) {
                        if weight == 0. {
                            continue;
                        }
                        let m = matrices[joint as usize];
                        linear = linear
                            + Matrix3::from_cols(m.x.truncate(), m.y.truncate(), m.z.truncate())
                                * weight;
                    }
                    let determinant = linear.determinant();
                    ensure!(
                        determinant.is_finite() && determinant != 0.,
                        "singular blended shading transform at vertex {i}"
                    );
                    let inverse = linear
                        .invert()
                        .context("singular blended shading transform")?;
                    let n = geometry.normals[i];
                    let normal = normalise(inverse.transpose() * Vector3::new(n[0], n[1], n[2]))?;
                    result.normals.push(normal);
                    if let (Some(source), Some(target)) = (&geometry.tangents, &mut result.tangents)
                    {
                        let t = source[i];
                        let v = linear * Vector3::new(t[0], t[1], t[2]);
                        let projection = normal[0] * v.x + normal[1] * v.y + normal[2] * v.z;
                        let tangent = normalise(
                            v - Vector3::new(normal[0], normal[1], normal[2]) * projection,
                        )?;
                        target.push([
                            tangent[0],
                            tangent[1],
                            tangent[2],
                            t[3] * determinant.signum(),
                        ]);
                    }
                }
                Ok(result)
            })
            .collect()
    }

    /// Return world-space deformed positions in mm for each primitive.
    pub fn deform_positions(
        &self,
        scene: &Scene,
        pose: &[Transform],
    ) -> Result<Vec<Vec<[f64; 3]>>> {
        let matrices = self.pose_matrices(scene, pose)?;
        self.positions_from_matrices(scene, &matrices)
    }

    pub(crate) fn positions_from_matrices(&self, scene: &Scene, matrices: &[Matrix4]) -> Result<Vec<Vec<[f64; 3]>>> {
        let mesh = scene.nodes[self.mesh_node].mesh.unwrap();
        scene.meshes[mesh]
            .primitives
            .iter()
            .zip(&self.weights)
            .map(|(primitive, weights)| {
                primitive
                    .geometry
                    .positions
                    .iter()
                    .zip(weights)
                    .map(|(point, weights)| {
                        let mut result = [0.0; 3];
                        for (&joint, &weight) in weights.joints.iter().zip(&weights.weights) {
                            if weight == 0.0 {
                                continue;
                            }
                            let p = matrices[joint as usize]
                                .transform_point(Point3::new(point[0], point[1], point[2]));
                            for (value, p) in result.iter_mut().zip([p.x, p.y, p.z]) {
                                *value += p * weight;
                            }
                        }
                        ensure!(
                            result.iter().all(|v| v.is_finite()),
                            "deformation exceeds numeric range"
                        );
                        Ok(result)
                    })
                    .collect()
            })
            .collect()
    }
}

fn normalise(v: Vector3) -> Result<[f64; 3]> {
    let length = v.x.hypot(v.y).hypot(v.z);
    ensure!(
        length.is_finite() && length > 0.,
        "invalid deformed shading direction"
    );
    Ok([v.x / length, v.y / length, v.z / length])
}

#[cfg(test)]
pub(crate) mod tests {
    #[test]
    fn palette_pruning_preserves_deformation_guards_and_protected_nodes() {
        use crate::asset::animation::{Clip, Interpolation, Track, Values};
        let (mut scene, mut weights) = fixture();
        for name in ["unused", "animated", "socket_parent", "socket", "explicit"] {
            scene.nodes.push(Node { name: name.into(), ..Default::default() });
        }
        scene.nodes[6].parent = Some(5);
        scene.clips.push(Clip { name: "unused-animation".into(), tracks: vec![Track {
            node: 4, times: vec![0., 1.], interpolation: Interpolation::Linear,
            values: Values::Translation(vec![[0.; 3], [10.; 3]]),
        }] });
        for vertex in weights.iter_mut().flatten() {
            for joint in &mut vertex.joints { *joint += 1; }
            vertex.joints[3] = 0;
        }
        let skin = Skin::bind(&scene, 0, vec![3, 1, 2, 4, 5, 7], weights).unwrap();
        let before = skin.weight_file().unwrap().to_json().unwrap();
        let source_scene = format!("{scene:?}");
        let (pruned, report) = skin.prune_palette(&scene, &[7]).unwrap();
        assert_eq!(report.old_to_new, vec![None, Some(0), Some(1), Some(2), Some(3), Some(4)]);
        assert_eq!(report.removed_nodes, vec!["unused"]);
        let mut attached=scene.clone();
        attached.consumer=Some(crate::asset::consumer::Profile{sockets:vec![crate::asset::consumer::Socket{name:"attachment".into(),node:"unused".into()}],..Default::default()});
        assert!(skin.prune_palette(&attached,&[7]).unwrap().1.removed_nodes.is_empty());
        for angle in [0.0_f64, 0.3, 1.2] {
            let mut pose: Vec<_> = scene.nodes.iter().map(|node| node.transform).collect();
            pose[2].rotation = [0., 0., (angle / 2.).sin(), (angle / 2.).cos()];
            assert_eq!(skin.deform_positions(&scene, &pose).unwrap(), pruned.deform_positions(&scene, &pose).unwrap());
            let a = skin.deform_basis(&scene, &pose).unwrap();
            let b = pruned.deform_basis(&scene, &pose).unwrap();
            assert_eq!(a[0].normals, b[0].normals);
            assert_eq!(a[0].tangents, b[0].tangents);
        }
        assert_eq!(skin.weight_file().unwrap().to_json().unwrap(), before);
        assert_eq!(format!("{scene:?}"), source_scene);
        pruned.weight_file().unwrap().bind(&scene, 0).unwrap();
        let bytes = crate::asset::glb::encode_skinned_scene(&scene, &[pruned.clone()]).unwrap();
        let size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let doc: serde_json::Value = serde_json::from_slice(&bytes[20..20 + size]).unwrap();
        assert_eq!(doc["nodes"].as_array().unwrap().len(), scene.nodes.len());
        assert_eq!(doc["skins"][0]["joints"].as_array().unwrap().len(), 5);
        assert_eq!(doc["animations"].as_array().unwrap().len(), 1);
        assert!(pruned.prune_palette(&scene, &[7]).unwrap().1.removed_nodes.is_empty());
        assert!(skin.prune_palette(&scene, &[999]).is_err());
        scene.nodes[1].transform.translation[0] += 1.;
        assert!(skin.prune_palette(&scene, &[]).is_err());
    }

    #[test]
    fn shading_basis_handles_nonuniform_reflection_and_rejects_singular_blends() {
        let (mut scene, weights) = fixture();
        let q = std::f64::consts::FRAC_1_SQRT_2;
        scene.meshes[0].primitives[0].geometry.normals = vec![[q, 0., q]; 3];
        scene.meshes[0].primitives[0].geometry.tangents = Some(vec![[q, 0., -q, 1.]; 3]);
        let skin = Skin::assign(&scene, 0, vec![1], Assignment::Rigid).unwrap();
        let mut pose: Vec<_> = scene.nodes.iter().map(|n| n.transform).collect();
        pose[1].scale = [-2., 3., 4.];
        let basis = skin.deform_basis(&scene, &pose).unwrap();
        let n = basis[0].normals[0];
        let t = basis[0].tangents.as_ref().unwrap()[0];
        let expected = [-2. / 5f64.sqrt(), 0., 1. / 5f64.sqrt()];
        for k in 0..3 {
            assert!((n[k] - expected[k]).abs() < 1e-12);
        }
        assert!((t[0] + 1. / 5f64.sqrt()).abs() < 1e-12);
        assert!((t[2] + 2. / 5f64.sqrt()).abs() < 1e-12);
        assert_eq!(t[3], -1.);
        assert!((n[0] * t[0] + n[2] * t[2]).abs() < 1e-12);
        let skin = Skin::bind(&scene, 0, vec![1, 2], weights).unwrap();
        let mut pose: Vec<_> = scene.nodes.iter().map(|n| n.transform).collect();
        pose[2].rotation = [0., 0., 1., 0.];
        assert!(skin.deform_positions(&scene, &pose).is_ok());
        assert!(
            skin.deform_basis(&scene, &pose)
                .unwrap_err()
                .to_string()
                .contains("singular")
        );
    }
    use super::*;
    use crate::asset::scene::{Node, Primitive, SceneMesh};
    #[test]
    fn chain_assignment_preserves_rest_pose_and_nonuniform_knots() {
        let (mut scene, _) = fixture();
        scene.nodes.push(Node {
            name: "third".into(),
            ..Default::default()
        });
        let skin = Skin::assign_chain(&scene, 0, vec![1, 2, 3], 0, &[-500., 500., 2500.]).unwrap();
        assert_eq!(skin.weights[0][0].joints, [0, 1, 0, 0]);
        assert_eq!(skin.weights[0][0].weights, [0.5, 0.5, 0., 0.]);
        assert_eq!(skin.weights[0][1].joints, [1, 2, 0, 0]);
        assert_eq!(skin.weights[0][1].weights, [0.75, 0.25, 0., 0.]);
        let pose: Vec<_> = scene.nodes.iter().map(|n| n.transform).collect();
        let deformed = skin.deform_positions(&scene, &pose).unwrap();
        for (p, q) in deformed[0]
            .iter()
            .zip(&scene.meshes[0].primitives[0].geometry.positions)
        {
            assert!((p[0] - q[0] - 100.).abs() < 1e-10);
            assert!((p[1] - q[1] - 200.).abs() < 1e-10);
            assert!((p[2] - q[2]).abs() < 1e-10);
        }
        let bytes = skin.weight_file().unwrap().to_json().unwrap();
        let again = Skin::assign_chain(&scene, 0, vec![1, 2, 3], 0, &[-500., 500., 2500.]).unwrap();
        assert_eq!(bytes, again.weight_file().unwrap().to_json().unwrap());
        WeightFile::from_json(&bytes)
            .unwrap()
            .bind(&scene, 0)
            .unwrap();
        let clamp = Skin::assign_chain(&scene, 0, vec![1, 2, 3], 0, &[0., 500., 1000.]).unwrap();
        assert_eq!(clamp.weights[0][0].joints[0], 0);
        assert_eq!(clamp.weights[0][1].joints[0], 2);
        assert_eq!(clamp.weights[0][1].weights, [1., 0., 0., 0.]);
    }

    #[test]
    fn chain_assignment_rejects_invalid_knots_and_joints() {
        let (scene, _) = fixture();
        for knots in [
            vec![],
            vec![0.],
            vec![0., 0.],
            vec![1., 0.],
            vec![0., f64::NAN],
            vec![-f64::MAX, f64::MAX],
        ] {
            assert!(Skin::assign_chain(&scene, 0, vec![1, 2], 0, &knots).is_err());
        }
        assert!(Skin::assign_chain(&scene, 0, vec![1, 2], 3, &[0., 1.]).is_err());
        assert!(Skin::assign_chain(&scene, 0, vec![1, 1], 0, &[0., 1.]).is_err());
        assert!(Skin::assign_chain(&scene, 0, vec![1, 99], 0, &[0., 1.]).is_err());
        assert!(Skin::assign_chain(&scene, 1, vec![1, 2], 0, &[0., 1.]).is_err());
    }

    #[test]
    fn explicit_assignment_is_repeatable_clamped_and_validates_joint_counts() {
        let (scene, _) = fixture();
        let rule = Assignment::Linear {
            axis: 0,
            start_mm: 0.0,
            end_mm: 2000.0,
        };
        let skin = Skin::assign(&scene, 0, vec![1, 2], rule).unwrap();
        assert_eq!(skin.weights[0][0].weights, [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(skin.weights[0][1].weights, [0.5, 0.5, 0.0, 0.0]);
        assert_eq!(
            skin.weight_file().unwrap().to_json().unwrap(),
            Skin::assign(&scene, 0, vec![1, 2], rule)
                .unwrap()
                .weight_file()
                .unwrap()
                .to_json()
                .unwrap()
        );
        let clamped = Skin::assign(
            &scene,
            0,
            vec![1, 2],
            Assignment::Linear {
                axis: 0,
                start_mm: 250.0,
                end_mm: 750.0,
            },
        )
        .unwrap();
        assert_eq!(clamped.weights[0][0].weights, [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(clamped.weights[0][1].weights, [0.0, 1.0, 0.0, 0.0]);
        assert!(Skin::assign(&scene, 0, vec![1, 2], Assignment::Rigid).is_err());
        assert!(Skin::assign(&scene, 0, vec![1], rule).is_err());
        assert!(
            Skin::assign(
                &scene,
                0,
                vec![1, 2],
                Assignment::Linear {
                    axis: 0,
                    start_mm: 1.0,
                    end_mm: 1.0
                }
            )
            .is_err()
        );
        let rigid = Skin::assign(&scene, 0, vec![2], Assignment::Rigid).unwrap();
        assert!(
            rigid.weights[0]
                .iter()
                .all(|w| w.weights == [1.0, 0.0, 0.0, 0.0])
        );
    }
    #[test]
    fn persisted_weights_round_trip_and_reject_same_count_geometry_and_rig_changes() {
        let (scene, weights) = fixture();
        let skin = Skin::bind(&scene, 0, vec![1, 2], weights).unwrap();
        let bytes = skin.weight_file().unwrap().to_json().unwrap();
        let resource = WeightFile::from_json(&bytes).unwrap();
        resource.bind(&scene, 0).unwrap();
        assert_eq!(resource.to_json().unwrap(), bytes);
        let mut changed = scene.clone();
        changed.meshes[0].primitives[0].geometry.positions[1][0] += 1.0;
        assert!(
            resource
                .bind(&changed, 0)
                .unwrap_err()
                .to_string()
                .contains("mesh changed")
        );
        let mut changed = scene;
        changed.nodes[2].transform.translation[1] += 1.0;
        assert!(
            resource
                .bind(&changed, 0)
                .unwrap_err()
                .to_string()
                .contains("rig changed")
        );
        let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        json["version"] = 3.into();
        assert!(WeightFile::from_json(&serde_json::to_vec(&json).unwrap()).is_err());
        json["version"] = 2.into();
        json["unexpected"] = true.into();
        assert!(WeightFile::from_json(&serde_json::to_vec(&json).unwrap()).is_err());
    }
    pub(crate) fn fixture() -> (Scene, Vec<Vec<Weights>>) {
        let mut mesh = crate::asset::tests::triangle();
        mesh.positions[2][1] = 2000.0;
        let scene = Scene {
            nodes: vec![
                Node {
                    name: "mesh".into(),
                    mesh: Some(0),
                    transform: Transform {
                        translation: [100.0, 200.0, 0.0],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Node {
                    name: "base".into(),
                    transform: Transform {
                        translation: [100.0, 200.0, 0.0],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Node {
                    name: "tip".into(),
                    parent: Some(1),
                    transform: Transform {
                        translation: [0.0, 1000.0, 0.0],
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            meshes: vec![SceneMesh {
                name: "triangle".into(),
                primitives: vec![Primitive {
                    geometry: mesh,
                    material: 0,
                }],
            }],
            materials: vec![Default::default()],
            ..Default::default()
        };
        let weights = vec![vec![
            Weights {
                joints: [0; 4],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
            Weights {
                joints: [0, 1, 0, 0],
                weights: [0.5, 0.5, 0.0, 0.0],
            },
            Weights {
                joints: [1, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
        ]];
        (scene, weights)
    }
    #[test]
    fn nonidentity_bind_pose_recovers_geometry_and_blended_bend_is_analytic() {
        let (scene, weights) = fixture();
        let skin = Skin::bind(&scene, 0, vec![1, 2], weights).unwrap();
        let mut pose: Vec<_> = scene.nodes.iter().map(|n| n.transform).collect();
        let rest = skin.deform_positions(&scene, &pose).unwrap();
        assert_eq!(
            rest[0],
            vec![
                [100.0, 200.0, 0.0],
                [1100.0, 200.0, 0.0],
                [100.0, 2200.0, 0.0]
            ]
        );
        let q = std::f64::consts::FRAC_1_SQRT_2;
        pose[2].rotation = [0.0, 0.0, q, q];
        let bent = skin.deform_positions(&scene, &pose).unwrap();
        for (actual, expected) in bent[0].iter().zip([
            [100.0, 200.0, 0.0],
            [1100.0, 1200.0, 0.0],
            [-900.0, 1200.0, 0.0],
        ]) {
            for k in 0..3 {
                assert!((actual[k] - expected[k]).abs() < 1e-9);
            }
        }
        pose[2] = scene.nodes[2].transform;
        assert_eq!(skin.deform_positions(&scene, &pose).unwrap(), rest);
    }
    #[test]
    fn rejects_bad_weights_and_stale_geometry_or_bind_transforms() {
        let (scene, weights) = fixture();
        for change in [
            |w: &mut Vec<Vec<Weights>>| w[0][0].weights = [0.0; 4],
            |w: &mut Vec<Vec<Weights>>| w[0][0].weights = [f64::NAN, 0.0, 0.0, 0.0],
            |w: &mut Vec<Vec<Weights>>| w[0][0].joints[0] = 2,
            |w: &mut Vec<Vec<Weights>>| w[0][1].joints = [0; 4],
            |w: &mut Vec<Vec<Weights>>| {
                w[0].pop();
            },
        ] {
            let mut bad = weights.clone();
            change(&mut bad);
            assert!(Skin::bind(&scene, 0, vec![1, 2], bad).is_err());
        }
        assert!(Skin::bind(&scene, 0, vec![1, 1], weights.clone()).is_err());
        let skin = Skin::bind(&scene, 0, vec![1, 2], weights).unwrap();
        let mut changed = scene.clone();
        changed.meshes[0].primitives[0].geometry.positions[1][0] += 1.0;
        assert!(skin.validate(&changed).is_err());
        let mut changed = scene.clone();
        changed.nodes[2].name = "replacement".into();
        assert!(skin.validate(&changed).is_err());
        let mut changed = scene;
        changed.nodes[2].transform.translation[1] += 1.0;
        assert!(skin.validate(&changed).is_err());
    }
}
