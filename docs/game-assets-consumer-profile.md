# Experimental consumer profile v1

Reviewed: 2026-11-02. Next review: 2026-11-16.

This opt-in profile declares engine roles without changing node names or CAD
geometry. Version 1 targets the explicit Unity adapter. Unreal, Godot and other
engine adapters remain required follow-up; these declarations do not make their
ordinary importers apply Unity components automatically.

```text
consumer unity version=1
socket handle node=grip
collider collision shape=box
lod_group detail root=model
lod_level detail height=0.6 nodes=detail_high
lod_level detail height=0.15 nodes=detail_low
```

Declare `consumer` once before its roles, and each `lod_group` before its levels.
Nodes may be declared elsewhere in the document; references resolve after CAD
assembly expansion. See `examples/game-assets/plate-consumer-profile.gasset`.

## Meaning and validation

- A socket has a unique public name and references an explicit transform node.
  Its existing parent and local transform define its pose, including bone motion.
  Named socket nodes are protected during skin-palette pruning; parent protection
  continues to preserve their ancestry.
- A collider references a dedicated unskinned leaf mesh instance. `box` uses its
  local mesh bounds; `mesh` uses its triangles; `convex` requests Unity convex
  cooking. The initial convex profile conservatively limits source meshes to 255
  triangles. This is an implementation restriction, not a general statement that
  every larger source cannot produce a valid hull. Box/convex volumes require
  nonzero extents in the Unity adapter. A collider's visual renderer is disabled
  only in the adapted prefab. No Rigidbody, trigger or physics material is inferred.
- A LOD group references a transform root, two to eight ordered levels and explicit
  mesh-node members. `height` is Unity's screen-relative transition height in
  `(0, 1]`, strictly decreasing even after float32 rounding; objects cull below the last threshold. Members must
  descend from their group root. The same node cannot serve multiple roles or
  levels. Nested LOD groups are refused. Cross-fading is disabled in v1.
- Ordinary nodes have no implicit role, regardless of names or prefixes. Missing,
  duplicate, inactive, contradictory or unsupported references reject export or
  adaptation. The Unity adapter currently requires renderer/MeshFilter components
  on the named nodes; additional FBX-generated primitive hierarchy shapes need
  further acceptance coverage.

## Export and Unity adaptation

GLB retains every node and mesh, with the profile under
`extras.gcad_consumer_profile`. Generic consumers can still display the geometry.
Managed experimental FBX packaging writes `gcad-consumer-profile.json`, containing
version/format, FBX and original GLB SHA-256 hashes, and the unchanged typed profile.
All consumer sidecars are validated before publication. Direct helper calls do not
write gcad-owned sidecars.

Enable **Preserve Hierarchy** on the Unity model importer before adaptation.
Otherwise Unity can replace a single model root with its prefab root, losing the
named reference. The adapter refuses this configuration instead of changing the
source importer itself. See Unity's
[Preserve Hierarchy API](https://docs.unity3d.com/ja/6000.0/ScriptReference/ModelImporter-preserveHierarchy.html).

The existing material adapter preflights the profile before creating output assets,
then adds a `GcadSockets` registry, native collider components and `LODGroup`.
`instance.GetComponent<GcadSockets>().Find("handle")` returns the socket Transform
or null for an absent name. Mesh/convex colliders own readable mesh assets in the
adapted output. Unity's
[MeshCollider reference](https://docs.unity.com/en-us/engine/6000.3/manual/physics-section/physics-overview/collision-section/collider-shapes/mesh-colliders/class-mesh-collider)
and [LODGroup API](https://docs.unity3d.com/cn/6000.0/ScriptReference/LODGroup.html)
describe the native components.

Collider attachment now treats synchronous Unity cooking warnings/errors as an
adaptation failure. The callback is scoped to attachment and always removed;
ordinary log messages are ignored. Unity can cook automatically when adding a
MeshCollider to a MeshFilter or assigning `sharedMesh`, as described in its
[mesh baking API](https://docs.unity3d.com/ja/6000.0/ScriptReference/Physics.BakeMesh.html).
The adapter uses attachment directly, avoiding version-specific explicit bake IDs.
It does not claim to detect a silent native failure or later runtime recooking.

After preflight, adaptation owns only its new destination. Any exception during
asset creation, collider attachment or prefab saving removes that destination
through the AssetDatabase. Existing destinations, trailing-slash aliases and
orphan destination metadata are refused before writes. Cleanup failure is reported
together with the original error instead of claiming a successful rollback.
This is exception recovery, not crash-safe publication: process termination or
power loss can still leave output requiring inspection.

Use `GCAD_CONSUMER_FAILURE=1` with the collision ring fixture to exercise native
triangle/convex cooking failures. The editor test temporarily substitutes a
collinear mesh with nonzero bounds, then verifies rejection, removal of partial
output, source FBX/import-metadata preservation, successful retry at the same
destination, and preservation of existing output and orphan metadata. The source
mesh reference is restored without saving the test substitution.
Both Unity 6000.4.7f1 and 6000.5.4f1 pass the two native failure cases and the
positive Windows-player collision suite in
`target/unity-consumer-recovery-both-{VERSION}` (2026-11-02).

## Existing gcad viewer

Profile controls appear only in the game-asset view. The initial preview shows LOD
0 and hides collision source meshes. **Consumer preview** selects a manual LOD
level (clamped separately for groups with fewer levels), all LODs, and collision
source visibility. Socket mappings and node-role labels appear in the inspector.
Selections persist across tracked reloads and do not modify source assets. A
software snapshot uses the same initial visibility. Collision source meshes are
not a visualization of the cooked convex hull or the box collider's effective
volume. The CAD UI remains independent.

**Box volumes (X-ray)** independently displays green, labelled wire boxes even
when collision source meshes are hidden. Bounds are computed across the source
mesh's primitives in local coordinates, then transformed by the current animated
world pose. They participate in **Fit model** bounds. This shows the declared
box volume for ordinary TRS transforms; engine handling of inherited shear may
differ. It does not claim to display convex cooked hulls. The overlay defaults
off, persists through live reload and leaves source/export data unchanged.
Software snapshots retain their existing default with overlays off.

Verification (2026-11-02): all 22 asset-view tests pass in
`target/consumer-box-preview-tests.log`, including local bounds with rotation,
reflected nonuniform scale and animation, visibility/fitting and live reload.
The rebuilt native Windows viewer was visually checked with the box outline
enabled and the source collider hidden. Convex hull transport/preview and
engine-specific shear acceptance remain open.

Cooked convex transport investigation (2026-11-02): the installed Unity 6000.5
PhysicsModule API documentation and Unity's
[geometry binding source](https://github.com/Unity-Technologies/UnityCsReference/blob/master/Modules/Physics/ScriptBindings/PhysicsGeometry.bindings.cs)
expose `ConvexMeshGeometry` scale/type and an opaque private native mesh handle,
not public hull vertices/faces. `MeshCollider.sharedMesh` remains the source mesh.
No supported exact-hull extraction route has been established. The viewer must
continue to identify convex source geometry as such; an independently computed
hull or ray-sampled surface would not prove the engine's actual cooked topology.
Next action: establish a versioned engine extraction route before
adding a cooked-hull receipt and tracked preview resource. Review: 2026-11-16.
The subsequent [Unity editor debug binding audit](https://github.com/Unity-Technologies/UnityCsReference/blob/master/Modules/PhysicsEditor/ScriptBindings/PhysicsDebug.bindings.cs)
found visualization/filter controls and query callbacks, but no public hull-mesh
export. This reinforces the unresolved extraction boundary; it is not evidence
that no native or future engine route could exist.

Native Windows UI acceptance (2026-11-02) verified LOD 0/1 selection, collision
source visibility and fitting the plate/ring fixture. Opening `plate.gcad` in the
same executable retained the CAD feature history, selection, measurement and
section controls without consumer panels. This is a focused UI check, not a full
CAD regression. The consumer dropdown and animation dropdown now have accessible
names. The automated live-document test exercises actual control clicks, source
translation reload, preservation of the selected LOD/collision visibility/camera,
last-valid-frame retention on an invalid profile, and recovery after correction.

## Verification and remaining acceptance

Run with installed Unity and the reviewed helper:

```sh
UNITY=/absolute/path/to/Unity UNITY_VERSION=6000.5.4f1 \
GCAD_FBX_HELPER=/absolute/path/to/gcad_fbx_helper \
GCAD_CONSUMER_CHECK=1 GCAD_CONSUMER_PLAYER=1 GCAD_CONSUMER_RENDER=1 \
  sh tools/game-assets-unity/check-import.sh FBX_PACKAGE NEW_PROJECT
```

The fixture passes on Unity 6000.4.7f1 and 6000.5.4f1 in editor and Windows
standalone players: socket coordinates/lookup, box-collider ray hit, hidden
collision renderer, LOD component membership/thresholds and corruption controls.
Editor controls also reject a mismatched FBX digest and conflicting LOD/collision
roles before creating an output directory. Evidence is in
`target/unity-consumer-profile-hierarchy-{VERSION}`.

The additional `plate-consumer-collision.gasset` fixture passes on both versions
in editor and Windows players (`target/unity-consumer-collision-{VERSION}`). Each
176-triangle ring collider owns a readable mesh separate from the render mesh.
Triangle colliders preserve the hole, while convex cooking fills it; rim and
outside rays distinguish correct geometry from a missing or oversized collider.
A nonuniform reflected triangle instance and displaced-collider controls pass.
This covers these valid shapes; the separate recovery checks above cover reported
native cooking failures for collinear triangle and convex inputs.

With `GCAD_CONSUMER_RENDER=1`, D3D11 editor/player frames show red LOD 0 at screen
height 0.9, green LOD 1 at 0.3 and culling at 0.05. Forced-level controls prove the
probe distinguishes levels and that the far camera can still see geometry.
All five PNGs match between editor and player on both versions; evidence is in
`target/unity-consumer-lod-render-{VERSION}`. Diagnostic flat colours test geometry
selection, not production PBR material appearance. Omit the render flag for CPU
checks. Cooked convex overlays and interactive skinned playback
still need acceptance. Next action: complete those runtime/visual
gates and cooking-failure recovery before claiming the profile production-ready.

The two-mesh skinned LOD fixture is `plate-consumer-skinned.gasset`. Prepare its
two explicit weight resources with `sh tools/game-assets-consumer-fixture.sh`
(or the normal `prepare-fixtures` command), then export/package it as above.
Existing resources are preserved and validated on repeated preparation.
The preparation script now binds the original document directly: missing peer
weight files are deferred only by the explicit binding loader, while normal
check/export still requires all of them. No temporary source rewrite is needed.
Add `GCAD_CONSUMER_SKIN=1` to the consumer check to require two skinned LOD renderers
sharing the base/tip bones. The probe checks uniform world-space bone translation
against every baked vertex and verifies tip rotation deforms both meshes. Render
mode additionally captures rest/posed LOD images and rejects unchanged poses;
player mode compares all ten images with the editor. These checks do not prove
general animation-controller behavior or arbitrary deformation accuracy.

Both Unity versions pass these skinned checks in editor and Windows standalone
players (`target/unity-consumer-skinned-refresh-{VERSION}`), including ten
identical editor/player PNG pairs per version and visibly changed poses at both
LOD levels. The maximum uniform-translation error is below `1e-6` metres, and
each mesh has a tip-rotation displacement above `0.001` metres. These are fixture
acceptance thresholds, not production budgets.

The synchronous rendering probe temporarily enables Unity's
[per-render skin-matrix recalculation](https://docs.unity.com/en-us/engine/6000.5/script-reference/unityengine/skinnedmeshrenderer/forcematrixrecalculationperrender)
because it captures multiple poses in one frame; it restores the original setting
after capture and does not change the adapted prefab.

Skinned gcad UI acceptance (2026-11-02): the automated control test passes in
`target/consumer-skinned-ui-tests.log`. It clicks animation selection, Play/Pause,
skeleton display, joint weights and consumer LOD selection, checks known rigid-bone
translation, and verifies that LOD switching keeps animation time, camera and
weight selection. It creates its own weight resources. Native Windows inspection
of `plate-consumer-skinned.gasset` additionally verified blended playback, time
scrubbing and the skeleton overlay. General multi-clip/large-rig usability and
other platforms still require acceptance.

The additional `target/unity-consumer-profile-model-root-6000.5.4f1` editor/player
check covers an authored root named `model`, matching Unity's synthetic wrapper.
Lookup excludes that wrapper rather than rejecting the valid authored name. A
float32-threshold collapse refusal check is recorded in
`target/unity-consumer-profile-hierarchy-6000.5.4f1/consumer-precision-refusal.json`.
