# Game asset development plan

## Current direction

### Delivery texture density (2026-11-02)

Optional export normalization derives texture sizes from rest-pose world-space
surface area and UV coverage. GLB and FBX/PNG share the conversion; a background
preview toggle uses the same settings and preserves original source textures.
The asset-normalize-textures command writes a derived GLB and per-map density
report. Colour filtering uses linear light and premultiplied alpha; numeric maps
remain linear and normal vectors are renormalized.

Validation: two density tests and 36 non-GPU viewer tests pass (one native-helper
test skipped). Preview/export parity, source restoration, instance scale, filters
and output preservation are covered. Windows release build and default CAD check
pass. The launcher exports successfully to normalized GLB and FBX/PNG; at a target
of 128 pixels/metre its maps achieve approximately 125–131 pixels/metre. A skinned
fixture also exports. Source images remain unchanged.

Remaining: per-island repacking, alpha-mask coverage preservation, explicit mip
and UV-shader support, animated-scale density, Linux and engine import validation.
Next action (owner: user): compare source and normalized preview at the intended
game camera distance; choose a delivery density. Review: 2026-11-14.

### Viewer selection references (2026-11-02)

Game assets support click-to-select items and source faces, inspector selection
sync, item highlighting, face outlines and copyable JSON references. Named nodes
identify instances; primitive/face/triangle indices include a prepared-mesh hash.
Missing face lineage falls back explicitly to a triangle reference. Reloads and
preview LOD changes clear face references. Picking uses displayed geometry and
camera transforms; CAD selection is unchanged.

Windows checks: two picking tests and the 34-test non-GPU viewer suite pass;
one native-helper-dependent test remains skipped. Remaining: interactive review
of face outlines and target-platform validation outside Windows. Next action
(owner: user): select an item and face in the launcher viewer and paste a copied
reference into a modelling request. Review: 2026-11-14.

### Automatic UV unwrap and painting guides (2026-11-02)

Native game-asset builds now support `uv SOURCE projection=unwrap resolution=1024
padding=8`, using pinned xatlas bindings. The prepared mesh retains exact source
triangle positions and face lineage while seam vertices and UVs change. Existing
skin bindings require explicit regeneration after a UV change.

`asset-paint-guide` exports a blank canvas, labelled islands, a wireframe, six
colour-matched orthographic views and a geometry/UV fingerprint. `asset-paint-apply`
validates that fingerprint and image dimensions, masks the paint to the islands
and extends edge colours into padding. Both commands preserve existing outputs.
The skill workflow now prefers this path for bespoke surface detail. Image
generation remains a separate step, followed by inspection in the existing viewer.

Windows release CLI validation covers guide creation, padded diagnostic painting,
preview and GLB/FBX/PNG export on the curved plate fixture. Automatic unwrap tests
cover repeatability, geometry preservation, face lineage, invalid settings and
stale-guide rejection. Default CAD compilation also passes.
The 27 viewer tests pass, including unwrapped PNG live reload with stable UVs,
camera and clip state; the helper-dependent viewer test remains skipped. The CLI
FBX conversion was checked separately. The initial three viewer failures shared
a generated 0.5.2 skin-binding fixture, resolved by explicit regeneration for 0.5.3.
Native builds require
a C++ toolchain and libclang; Linux and engine-specific imports remain unverified.

Remaining: manual seam selection, shared atlases across multiple source meshes,
and evaluation of generated artwork across seams. Next action (owner: TBD): paint
a representative model from its guide, inspect seam continuity and validate its
GLB/FBX in a target engine. Review: 2026-11-14.

### Character playback performance (2026-11-02)

Playback now resolves world transforms once per pose and shares each skin's joint
matrices between position and normal deformation. Immutable rest geometry and skin
bindings are validated at load/reload, rather than twice per skin per frame;
sampled transforms and singular shading blends remain checked. Public one-shot
preview and deformation entry points retain validation. The MVP launcher uses a
release build, as does the locally configured skill runtime.

On the 75-surface character study, CPU frame preparation measured about 1536 ms
before the change, 152 ms after in debug, and 9-10 ms after in release. These are
three-sample CPU measurements, excluding loading and GPU rendering, not viewport
FPS. The eight skin tests and 26 release viewer tests pass (including deformation
parity, reload, LOD playback and weight display); the helper-dependent FBX test
was skipped. Logs: `target/animation-before.log`, `target/animation-after-debug.log`,
`target/animation-regression.log`, `target/animation-view-release-tests.log`.
Next action: assess interactive playback on the character study;
GPU buffer/texture reuse remains a potential further optimization. Review: 2026-11-14.

### Renderer fork (2026-11-02)

The renderer extensions now live in the [Wixely egui_bench fork](https://github.com/Wixely/egui_bench/tree/gcad-renderer).
Cargo pins `0c33aef02aa91dbb12ba1f556067dc806e225d16`; the inactive vendored copy
has been removed. All 28 renderer tests pass on Windows Direct3D 12 and Vulkan.
Future renderer changes belong in the fork; update the gcad Cargo revision and
lockfile together after validation. Next action:
exercise the game-asset viewer with the current character study. Linux and
external-engine acceptance remain unverified. Review: 2026-11-14.

### Viewer/export MVP scope (2026-11-02)

The immediate delivery scope is the existing viewer's game-asset mode with
live preview and GLB/FBX export. The wider plan remains deferred, not complete.
The viewer now has a collapsed export panel for saved `.gasset` sources, background
conversion, new-output protection and an explicit experimental FBX helper path.
Preview pose and visibility do not alter exported assets. The shell launcher and
[MVP guide](game-assets-mvp.md) provide the entry point.
Next action: verify the MVP against representative user assets before release.

The preceding full feature-enabled regression run finished with seven failures
across features, job_format, ops_doc and simulation; see
`target/game-assets-current-regression.log`. This is not a clean full-suite pass
and the difference from the earlier five-failure baseline remains to be triaged.
These results precede the new export panel.

### Multi-mesh binding preparation (2026-11-02)

`asset-bind` now permits not-yet-created peer weight files while preparing the
requested mesh. Existing peer bindings still validate, and invalid node references,
malformed/stale weights and non-NotFound I/O errors remain failures. Normal
check/export still requires every declared skin. This removes the temporary source
rewrite from `tools/game-assets-consumer-fixture.sh`. A fresh two-skin fixture was
bound sequentially through the rebuilt CLI and exported successfully; the focused
bootstrap/refusal regression passes in `target/multi-binding-tests.log`.
Authoring regression evidence: `target/multi-binding-author-regression.log`.
Explicit recovery now supports `asset-bind --defer-skin PEER` for named stale peer
skins. Deferral validates declared mesh references, rejects duplicates/target names,
and does not change ordinary loading. Repair one skin into a new resource, update
its declaration, then repair the remaining peers; old resources remain untouched.
The 14 authoring tests pass in `target/multi-rebinding-tests.log`, including partial
repair refusal, full two-skin recovery/export and original-resource preservation.
The rebuilt CLI also rejects the stale peer without deferral, then completes the
two-skin repair with explicit deferral and new files. The workflow is documented
in `game-assets.md`. Next action: continue broader engine acceptance;
Godot runtime validation requires an available executable.
Review: 2026-11-16.

### Godot acceptance preparation (2026-11-02)

`tools/game-assets-godot/check-import.sh` now prepares an isolated project for an
existing, explicitly versioned Godot 4 executable. The GDScript probe compares
active-scene mesh/surface/triangle and skin counts to independently parsed source
GLB metadata, checks skeleton binding and named clips, and writes a required pass
report. This is structural import preparation, not rendered/deformation acceptance
or a Godot consumer-role adapter. Shell syntax and existing-output refusal were
verified; engine execution/GDScript parsing remain unverified. No engine was
downloaded or installed. Next action: run the prepared gate with an available Godot executable, then extend sampled/rendered coverage. Other local implementation work continues.
Review: 2026-11-16.

### Explicit consumer roles and initial Unity adapter (2026-11-02)

The opt-in `consumer unity version=1` profile now declares named sockets, dedicated
collision mesh instances and ordered LOD groups/levels. Scene validation rejects
missing/conflicting roles, invalid thresholds, out-of-group members and unsupported
combinations. Socket references protect otherwise unused skin-palette joints.
Generic GLB geometry is unchanged; typed roles live in extras and survive managed
FBX packaging as a digest-bound `gcad-consumer-profile.json` receipt. Invalid
metadata refuses publication atomically. The full contract and limitations are in
[game-assets-consumer-profile.md](game-assets-consumer-profile.md).

The existing Unity material adapter preflights references before writing assets,
then adds native colliders/LODGroup and a `GcadSockets` lookup component. It requires
Preserve Hierarchy rather than silently changing source importer settings. Both
Unity 6000.4.7f1 and 6000.5.4f1 pass editor/standalone box-fixture checks for socket
coordinates, collision ray hits, disabled collision renderers, LOD membership and
thresholds. Corrupted socket positions, digest mismatches and role conflicts are
detected (`target/unity-consumer-profile-hierarchy-{VERSION}`). Additional ring
fixtures pass triangle/convex collision rays, readable owned collider meshes and
nonuniform reflected transforms in both editors and Windows standalone players
(`target/unity-consumer-collision-{VERSION}`). Rendered D3D11 checks also pass
automatic near/far LOD selection and culling, with forced-level controls and
matching editor/player images (`target/unity-consumer-lod-render-{VERSION}`).

The existing game-asset viewer has manual LOD and collision-source visibility
controls, socket mappings and role labels; CAD panels remain independent. Default
profile preview/snapshots show LOD 0 and hide collision source geometry. Frame
selection tests prove geometry selection and unchanged asset export, not interactive
GPU/UI acceptance. The 207-test library run passes in
`target/consumer-profile-lib-regression.log`; the native transport/refusal test passes
in `target/consumer-profile-native.log`. The subsequent explicit socket-pruning
regression passes in `target/consumer-profile-socket-pruning.log`.
The final focused consumer tests pass in `target/consumer-profile-unit-final.log`;
default and feature-enabled test/example builds also pass. A Unity 6000.5.4f1
editor/player case retains an authored root named `model` despite the synthetic
prefab wrapper sharing its name (`target/unity-consumer-profile-model-root-6000.5.4f1`).
Native/Unity validation also rejects LOD thresholds that collapse at float32 precision.

The two-mesh skinned LOD fixture also passes editor/Windows-player checks in both
Unity versions (`target/unity-consumer-skinned-refresh-{VERSION}`): shared-bone
translation, deformation of each mesh, automatic rest/posed LOD selection and
culling, and ten identical editor/player PNG pairs per version. The first probe
run correctly rejected unchanged rendered poses; enabling per-render skin-matrix
refresh for its synchronous captures resolved that test scheduling issue without
changing the prefab. This is manual bone-pose coverage, not Animator/controller
integration or a general deformation-accuracy guarantee. Preparation is reproducible
with `tools/game-assets-consumer-fixture.sh` and included in `prepare-fixtures`.

Focused native Windows UI checks verified LOD switching, collision-source display
and fitting the ring fixture in the existing viewer. A separate CAD document in
the same executable retained its normal CAD panels. Accessible names were added
to the animation/consumer LOD dropdowns after the UI harness detected omissions.
The live-document regression exercises control clicks and preserves selected
LOD, collision visibility and camera across source edits; invalid profiles retain
the last valid frame and recover after correction. This does not replace full
CAD regression or skinned interactive playback acceptance.
Both live-document tests (consumer roles and rig/binding edits) pass in
`target/consumer-viewer-live-tests.log`.

The existing asset viewer now has an independent **Box volumes (X-ray)** control.
Local mesh bounds follow the current animated world pose, contribute to explicit
camera fitting, and remain visible with collision source meshes hidden. Defaults
stay off; exports are unchanged. Native Windows visual inspection passed, along
with all 22 asset-view tests (`target/consumer-box-preview-tests.log`). Tests cover
rotation, reflected nonuniform scale, animation and overlay-state preservation on
reload. These boxes do not establish engine-specific shear semantics or represent
cooked convex hulls; the UI states the convex-source limitation explicitly.

Unity adaptation now captures synchronous native collider-cooking diagnostics and
refuses publication on warnings/errors. Exceptions after preflight remove only the
new output destination; existing output and orphan metadata are refused before
writes. Native collinear triangle and convex fixtures trigger actual PhysX/QuickHull
errors and verify cleanup, source preservation, retry and destination preservation.
Both Unity versions pass these editor recovery checks and the positive standalone
collider suite (`target/unity-consumer-recovery-both-{VERSION}`). The shell runner
requires the recovery report when `GCAD_CONSUMER_FAILURE=1`. This covers synchronous
reported cooking failures, not silent native failure, runtime recooking or process
termination; cleanup failures are surfaced alongside the original exception.

The skinned control regression now exercises Play/Pause, skeleton display, joint
weights and switching LOD while preserving sampled animation time and camera.
It independently checks rigid bone translation at the displayed time and verifies
the selected low-detail mesh's posed vertices. The joint-weight dropdown now has
an accessible name. The test passes in `target/consumer-skinned-ui-tests.log`;
the rebuilt native Windows viewer also passed playback, scrubbing to a bent pose
and skeleton-overlay inspection on the blended two-skin fixture.

Cooked convex transport remains unresolved: installed API documentation and Unity's
binding source expose a private native hull handle, not public vertices/faces.
The investigation and exact-hull requirement are recorded in
`game-assets-consumer-profile.md`; the source mesh is not substituted for a cooked
hull. Next action: establish a supported engine extraction route for cooked
convex preview transport and continue broader consumer acceptance. Other engines,
Linux/clean machines and production budgets remain open.
Review: 2026-11-16.

### Fractional and skinned root-motion acceptance (2026-11-01)

Fractional STEP keys exposed an exporter mismatch: animation accessors rounded
times to float32 while root-motion extras retained authored float64 values. At
the 0.3-second transition, the root advanced before its clip, producing about
0.0559 m error. GLB extras now use the same encoded key times and duration as the
clip. Native trajectory JSON retains authored precision. Receipt generation and
Unity reject older mismatched timelines with an instruction to regenerate the
root-motion GLB and package; they do not silently reinterpret them.

`plate-root-motion-fractional.gasset` exercises root translation/rotation, child
translation/scale and an independently animated root at fractional times, with a
1.7-second duration. `plate-root-motion-skinned.gasset` adds two-bone deformation
and fractional bone keys. The oracle now samples each source key immediately
before/at/after the boundary and again one loop later. Both fixtures and both
motion schemas pass 110 cases each in editors and standalone players on Unity
6000.4.7f1 and 6000.5.4f1. All eight combinations retain corruption, lifecycle,
invalid-data and publication-refusal controls. Maximum observed position error is
6.6641e-8 m. Evidence: `target/root-motion-fractional-summary.json` and the eight
`target/unity-root-motion-fixed-{fractional,skinned}-{translation,rigid}-{VERSION}`
directories. The skinned oracle uses direct GLB transforms on imported geometry
and Unity CPU BakeMesh; this does not establish rendered shading or arbitrary
skin/bind fidelity.

Sixteen focused Rust motion tests pass in
`target/root-motion-fractional-regression-final.log`. Native conversion validates
both motion schemas and refuses stale metadata without publishing output or
changing source files (`target/root-motion-fractional-native.log`). The historical
failing Unity import now also passes the explicit timeline refusal check in
`target/unity-root-motion-fractional-before-translation-6000.5.4f1`.

Next action: specify and implement versioned collision/socket/LOD consumer
conventions, starting with the installed Unity profile. Animator transitions/layers,
broader engine execution, Linux/clean-machine verification and representative
production budgets remain open. Review: 2026-11-14.

### Unity root-motion consumer (2026-11-01)

The material adapter serializes validated motion receipts into adapted prefabs.
Explicit `GcadRootMotionPlayer.Sample(seconds, looping)` playback supports both
motion schemas, negative seeks, clamping and accumulated loops. Receipt version 2
also retains exact in-place TRS keys for all animated nodes. The Unity player
samples these directly, preserving STEP boundaries and quaternion interpolation
instead of using FBX-imported curves. Old version 1 STEP receipts require re-export.
Receipt generation rejects malformed, ambiguous, nonfinite and oversized tracks.

Both STEP schemas pass 38 editor and standalone cases on Unity 6000.4.7f1 and
6000.5.4f1. LINEAR translation passes the same new oracle on 6000.5.4f1 and rigid
motion on 6000.4.7f1. Evidence: the four
`target/unity-root-motion-exact-step-{translation,rigid}-{VERSION}` directories and
`target/unity-root-motion-exact-linear-translation-6000.5.4f1` /
`target/unity-root-motion-exact-linear-rigid-6000.4.7f1`. Tests include samples
10 nanoseconds before/after keys, corruption, missing/nonfinite track data,
lifecycle, invalid-time and FBX-digest controls. The oracle independently reads
GLB transforms and applies them to imported mesh geometry; frozen expected
positions are serialized into the test player. These remain static-mesh CPU
checks, not skinned, continuous-time or rendered acceptance. Three receipt unit
tests and the native atomic-publication regression pass
(`target/root-motion-exact-unit.log`, `target/root-motion-exact-native.log`).
The editor control in
`target/unity-root-motion-exact-publication-controls-6000.5.4f1` confirms that
disabling exact tracks reproduces the STEP failure and malformed track data leaves
no adapted output directory or metadata. Motion targets/values are preflighted
against a temporary instance before creating materials or the prefab.

Historical failing imports are retained in
`target/unity-root-motion-step-no-resampling-rigid-6000.4.7f1`: Unity advances
discontinuities approximately 10 microseconds early even without resampling.

Next action: extend exact-track acceptance to skinned root motion and
fractional/non-root key boundaries. Animator transitions/layers, collision/socket/LOD
profiles, other engines, Linux and clean machines remain open.
Review: 2026-11-14.

### FBX root-motion metadata preservation (2026-11-01)

The engine-profile audit found that FBX conversion discarded the GLB's extracted
root-motion metadata. Managed package publication now adds `gcad-root-motion.json`
for either translation-only schema 1 or rigid schema 2. The versioned receipt binds
the exact FBX and original GLB hashes, resolves source/in-place clip names and the
active root-node name, and retains the metre/Y-up/right-handed trajectory unchanged.
Validation checks supported coordinate/composition conventions, unambiguous named
references, bounded key counts/times, axes, and consistent loop/anchor values.
Malformed or unsupported metadata rejects the entire unpublished package. The
shared receipt writer hashes FBX once and validates shader/motion receipts before
writing either. Ordinary exports with no root metadata have no motion receipt.

The native test also exposed rejection of untextured materials with no image-source
record. The helper now emits an empty provenance array for that case; textured
materials missing their source declaration remain rejected. Both motion schemas
pass native conversion/publication and invalid-schema rollback controls; the existing
shader receipt regression passes. A missing-texture-provenance control is refused.
The managed CLI publishes the receipt in `target/root-motion-receipt-cli-package`.
Evidence: `target/root-motion-receipt-native.log` and receipt unit tests in
`target/root-motion-receipt-tests.log`.

This preserves transport semantics, not engine playback acceptance. Direct helper
calls do not produce gcad-owned sidecars. The initial Unity consumer is described
above. Collision/socket/LOD profiles, other-engine testing, Linux and
clean-machine verification remain open. Review: 2026-11-14.


### Offline helper source bundle (2026-11-01)

`package-helper` now includes helper C++/build sources, the reviewed patch and
self-contained bare Git snapshots of the exact Assimp/ufbx commits. The snapshots
exclude checkout-local remotes, hooks, reflogs, untracked files and FETCH_HEAD;
no external object alternates remain. The bundle retains dependency notices and
checksums every distributed file. `rebuild-helper.sh` checks that manifest, clones
only those local snapshots, applies/verifies the patch and builds the static-runtime
helper without dependency downloads. Existing output directories are preserved.

A fresh Windows MSVC rebuild from `target/helper-source-bundle-verified` succeeds
in `target/helper-source-rebuild-verified`. The bundle has 89 files including its
manifest (160,509,986 bytes); all 88 manifest entries verify. Independent Git fsck
passes for both source repositories. An altered helper source control is refused
before creating build output, and an existing-output control is refused. Evidence:
`target/helper-source-rebuild-verified.log`, `target/helper-source-tamper-control.log`
and `target/helper-source-existing-output.log`.

The rebuilt executable passes Unicode/long-path relocation, conversion and both
inspection modes, plus the existing skinned package publication/material-corruption/
refusal regression (`target/helper-source-rebuilt-unicode.log` and
`target/helper-source-rebuilt-package.log`). Its revision, patch and schema match
the packaged executable. Binary hashes differ, so bit-for-bit reproducibility is
not claimed. This closes the local native source/build packaging gap; clean-machine
runtime, Linux, broader engines and representative production budgets remain open.
Next action: audit the remaining engine-profile and delivery requirements;
cross-engine execution still awaits the pending engine-path/download answer.
Review: 2026-11-14.


### Helper relocation and Unicode paths (2026-11-01)

Rebuilding and relocating the static-runtime helper exposed a Windows path bug:
ANSI process arguments and narrow file reads failed on non-ASCII paths. The
helper now explicitly converts UTF-16 arguments to UTF-8 and uses native filesystem
paths throughout conversion and material/skin inspection. The independent strict
readback and transactional publication checks remain enabled. Added an opt-in
native integration regression covering short and greater-than-260-character
Unicode paths for the executable, source and destination. Both conversions and
both inspection modes pass; source bytes remain unchanged. The existing skinned
package publication/corruption/refusal test also passes with the rebuilt bundle.

Evidence: `target/helper-unicode-long-regression.log`,
`target/helper-unicode-skinned-package-regression.log`, and
`target/helper-bundle-unicode-verified`. Fixed the checksum manifest's extra blank
line while retaining LF output; strict SHA256 verification passes for all 21 files.
PE dependency inspection lists only KERNEL32.dll in
`target/helper-bundle-unicode-dependencies.log`. This validates same-machine
relocation, not a clean-machine deployment. Complete corresponding-source
packaging, Linux runtime/recovery, additional engine coverage and production
asset budgets remain open. Review: 2026-11-14.

Engine inventory checked Windows PATH, uninstall registry entries, common install
and portable directories, plus the existing WSL distribution's PATH and /opt.
No Godot/Unreal executable was found. Runtime validation requires an available
engine installation; no new engine was installed during this inventory.
The complete current Windows release suites now finish: game-assets enabled
**605 passed, five failed, 19 ignored**; default **442 passed, five failed,
13 ignored**. All 199 enabled library tests pass. Both commands used
`cargo test --release --locked --no-fail-fast`, with `--features game-assets`
for the enabled run. The five failure names and observed causes match the recorded
baseline: three stored job-version mismatches, the operation-documentation test
inventory mismatch, and the curved-face simulation probe. Neither suite is green.
The Unicode/long-path and skinned package tests above were explicitly run apart
from these suites; ordinary runs correctly ignore native helper tests.

Evidence: `target/game-assets-current-full-regression.log`,
`target/game-assets-current-default-regression.log`, and
`target/game-assets-current-regression-summary.json`. This is current native
Windows regression evidence, not browser/Linux or interactive UI acceptance.
Next independent action: complete corresponding-source helper packaging
while awaiting an engine path/download decision for cross-engine validation.


### Static LOD and single-root FBX preservation (2026-11-01)

Added `plate-static-refined.gasset` and `GCAD_LOD_STATIC=1` to the LOD fixture
script. Static Unity checks now accept zero clips and measure ordinary mesh
renderers; animated fixtures retain their 21-pose requirement. Original and LOD
clip sets must match. Static reduction reaches 14,000 triangles in the selected
primitive (16,880 total, from 20,416), maximum sampled surface error 0.002975 mm,
shading angle 2.622 degrees, and zero difference in three sampled silhouettes.
GLB size falls from 849,016 to 721,720 bytes.

Found and fixed a helper failure: Assimp uses a sole authored node as its root,
which FBX export treats as an implicit container. Preparation now wraps that
node in an owned identity container with a unique name, preserving mesh and
transform. Strict readback checks remain. `plate-single-root.gasset` covers
translation, rotation, nonuniform scale and a container-name collision; its
native audit passes with maximum world error 3.165e-9 metres. The skinned LOD
native regression also passes. The final Unity harness passes rest plus all 21
animated poses in `target/unity-static-path-skinned-regression-6000.5.4f1`.

Unity 6000.4.7f1 and 6000.5.4f1 both pass static FBX fidelity and original-versus-LOD
rendering: four views and nine rejected controls per comparison, zero imported
position error, maximum LOD mean RGB error 1.397e-6, at most six changed pixels.
Evidence: `target/unity-lod-static-*`, `target/lod-static-report.json`,
`target/plate-single-root-audit.log` and `target/lod-static-fix-skinned-regression.log`.
These are editor D3D11 fixture checks, not static player/GPU performance acceptance.

Remaining: cross-engine coverage, production asset budgets, helper distribution,
full CAD regression and upstream integration. Next action: inventory
available Godot/Unreal/OpenGL consumers and prepare the next acceptance case.
Neither Godot nor Unreal was found on PATH or in a shallow Program Files check;
this does not establish that they are absent. Review: 2026-11-14.


### Paired original/LOD CPU deformation measurement (2026-11-01)

Added a reproducible graphics-free Unity editor cost mode with both original and
LOD adapters in the same process. Nine poses use four warmups and seven paired
batches of eight Refresh calls, alternating order; raw batch timings and medians
are retained. Matching import settings and independent owned adaptations preserve
the copied inputs. Counts fall from 12,155 to 10,387 vertices and 20,416 to 16,880
triangles across the full mesh.

Sequential runs pass on both installed versions, with lower LOD median CPU cost
at every pose. In 6000.4.7f1 the original medians span 7.29–7.94 ms and LOD medians
6.22–6.85 ms; in 6000.5.4f1 they span 7.27–8.23 ms and 6.21–6.97 ms. Evidence:
`target/unity-lod-cost-6000.4.7f1/lod-cost-report.json` and its 6000.5.4f1 counterpart;
both shell runs exit zero. The measurement covers CPU deformation/mesh publication,
excluding clip sampling, imports, GPU work and rendering. It does not establish
a production frame-time budget, and no performance threshold is hidden in `passed`.

The skinned fixture now has derived-error, rendered-quality, export-fidelity,
size and paired CPU-cost evidence. Representative production targets remain undefined; broader engine/platform coverage remains incomplete. Next action: audit the static LOD consumer path and the remaining cross-engine
acceptance matrix, using existing tools/installations before proposing any new
dependency. Review: 2026-11-14.

### Original-versus-LOD Unity rendering verified (2026-11-01)

Both Unity 6000.4.7f1 and 6000.5.4f1 pass the direct unoptimised-source versus
reduced-FBX rendered quality check: rest plus 21 poses, four views each (88 total),
with all 198 normal/UV/translation controls rejected. Maximum normalized mean
RGB error is 3.132e-6 and maximum changed-pixel fraction is 0.00019193 of visible
reference pixels. No pixels are excluded. The test uses the existing fixture
gates of 0.002 mean RGB and 1% changed visible pixels; these are not production
application budgets. Evidence: `target/unity-lod-quality-6000.4.7f1` and its
6000.5.4f1 counterpart, including `lod-quality-report.json` and images. Both shell
runs exit zero, and the normal reduced-GLB/FBX fidelity suites also pass.

The report explicitly marks changed topology: nearest-vertex distance is retained
as a diagnostic, while geometry acceptance remains in the Rust sampled surface
and silhouette report. This closes a fixture-level engine-rendered quality gap;
it does not prove runtime performance improvement. Next action: measure
original and LOD consumer costs in paired runs with identical conditions and
state the measured scope before proposing production budgets. Review: 2026-11-14.

### Posed-shading-aware LOD reaches the fixture target (2026-11-01)

The 1 mm correspondence experiment initially failed its final shading check
(3.060053° >3°), preserving the no-output invariant. Scene simplification now
checks sampled deformed normals/tangents when considering each collapse, while
retaining the final independent shading gate. Regression coverage demonstrates
that a statically valid collapse is rejected for posed normals or tangents and
that cached/uncached results agree; all six relevant tests pass.

With the same limits, `target/lod-posed-shading-report.json` reaches 14,000 triangles
from 17,536 for primitive 0 in 3,426 trials, reducing its vertices 10,576 to 8,808.
Across 54 sampled poses, maximum bidirectional sampled surface error is
0.003034 mm, maximum p95 is 0.00001997 mm, shading error is 2.927073°, and all
162 sampled silhouette views have zero difference. The GLB shrinks from
1,988,524 to 1,691,496 bytes. Release derivation/encoding took about 68 seconds
on this machine; it is not a runtime rendering benchmark.

The redundant original debug process was deliberately cancelled after the
completed release baseline established its no-reduction result. The accepted
derived asset converts to `target/fbx-lod-posed-package`; both editor source-reference
suites, standalone players and direct editor/player parity checks pass under
`target/unity-lod-posed-6000.4.7f1` and its 6000.5.4f1 counterpart. Both full shell
runs exit zero; each player passes 84 views and rejects all 105 controls.
Consumer checks compare FBX with the reduced GLB; the LOD report compares against
the retained unoptimised source. Production quality/performance budgets and direct
engine-rendered original/LOD quality comparison remain open. Next action:
measure original/LOD rendering and consumer
cost without conflating those metrics. Review: 2026-11-14.

### LOD rejection diagnostics and validated-check reuse (2026-11-01)

The release run under the original fixture limits finishes with no reduction:
17,536 triangles remain, target 14,000 is not reached, and all 5,000 candidates
are rejected. New report diagnostics identify 4,896 position and 104 normal
rejections. Evidence: `target/lod-rejection-diagnostic/lod-report.json`; its GLB
is byte-identical to the preceding release run. No useful LOD is accepted.

The reducer now reuses vertex/triangle checks only after an accepted mapping and
only while the relevant representatives remain unchanged. The first complete
validation pass and topology checks remain. All five relevant tests pass,
including exact cached/uncached output equivalence; a separate assertion checks
that accepted candidates plus classified rejections account for every trial.
This fixture accepts no collapse, so it does not demonstrate a cache speedup.
Its release derivation/encoding takes about 15 seconds; the original debug
process remains active and is not used as a production timing benchmark.

The next separate experiment (`target/lod-surface-gated*`) raises only the
internal correspondence allowance to 1 mm, retaining surface max 0.1 mm,
p95 0.05 mm, silhouette 0.01, UV 0.02 and shading 3 degrees. This distinguishes
endpoint distance from final sampled surface quality; acceptance is unproven
until the report passes. Next action: collect both live results, enforce
final quality gates and target attainment, then validate an actually reduced
asset in the consumer. Review: 2026-11-14.

### Skinned LOD consumer experiment in progress (2026-11-01)

Regenerated `target/lod-unoptimised.glb` from `plate-scene-refined.gasset` and
started a derived LOD for plate primitive 0, targeting 14,000 triangles from
17,536. Explicit fixture limits: correspondence 0.1 mm, UV 0.02, shading 3°,
sampled surface maximum 0.1 mm, p95 0.05 mm and silhouette difference 0.01;
17 uniform samples per clip plus authored keys/rest, with 5,000 candidate trials.
The debug process remains live at this entry; no acceptance result or engine
validation is claimed. Preserve and inspect its report before choosing another
run. `tools/game-assets-lod-fixture.sh NEW_DIRECTORY` records the reproducible
command and retains the unoptimised reference. Shell syntax passes.

Next action: collect the active derivation result, check target attainment
and every acceptance metric, then convert the accepted derived GLB and run the
independent consumer checks. Production quality/performance budgets and the
remaining milestones stay open. Review: 2026-11-14.

### M4 audit: UNORM16 consumer verification (2026-11-01)

The active v12 helper includes normalized integer weight decoding; the Unity
independent GLB reference now supports normalized unsigned-short VEC4 weights,
rejects unsupported weight layouts and checks weight bounds/sums. Native readback
passes UNORM16 and float fixtures, retaining malformed-input controls.

Quantizing the refined scene with explicit fixture limits 0.01 mm/0.1° and 17
uniform samples per clip (plus keys/rest) records maximum weight-only errors
0.000368723 mm and 0.000874278°. Weight payload halves from 197,536 to 98,768 bytes;
GLB size drops from 1,988,524 to 1,889,808 bytes. These are test limits, not
accepted production budgets or continuous error bounds.

Unity 6000.4.7f1 and 6000.5.4f1 pass rest/21-pose editor checks and 84 standalone
player views; all 105 player controls are rejected. Against the quantized GLB
reference, maximum player position error is 7.737e-9 m and mean RGB error 1.302e-6.
Direct editor/player parity passes (maximum mean RGB 3.790e-6). Both full shell
runs exit zero. Evidence: `target/unity-unorm16-6000.4.7f1`, its 6000.5.4f1
counterpart, `target/plate-scene-unorm16.glb` and `target/fbx-scene-unorm16-package`.

M4 remains incomplete: representative optimised LOD engine acceptance and
size/performance targets are not established. Palette pruning intentionally
preserves scene nodes; skeleton deletion is not added as a new requirement.
Next action: exercise a derived skinned LOD through independent consumer
checks, retaining its unoptimised reference and explicit error limits. Define application-specific production budgets. Review: 2026-11-14.

### Continuous Animator and shader-clock integration (2026-11-01)

Both installed Unity versions pass the shader-enabled lifecycle player over
54 frames. The single-state Animator advances geometry while an explicit
LateUpdate consumer follows its clamped state time for all displayed materials.
Five paused frames preserve geometry and scrolling; seek-back resets both and
resuming advances both. Existing disable/re-enable, reactivation, invalid-pose
last-good, recovery and cleanup checks also pass. Evidence:
`target/unity-shader-clock-6000.4.7f1/Player/player-report.json` and its 6000.5.4f1
counterpart. Initial graphics-free runs could not expose shader properties;
the same players pass on D3D11, and the runner now selects D3D11 for this case.

This validates a single one-second state with no transitions. Production shader
sampling remains explicit; arbitrary transition/layer/loop clock policies are
not inferred. The constrained template now has preview, reload, declared fallback,
tested Unity attachment, sampled rendered parity and this continuous integration
evidence. M5 still depends on M4 and broader acceptance requirements; do not mark
the full plan complete. Next action: audit remaining M4 optimisation
acceptance gaps against the actual implementation and recorded quality budgets,
then advance the highest-priority unverified operation. Review: 2026-11-14.

### Editor/player framing discrepancy resolved within render tolerance (2026-11-01)

The large mismatch came from camera fitting that included disabled imported
skinned-renderer bounds. Editor and player now frame the independently evaluated
reference vertices. Both complete source-reference/build/player suites pass
again, and a new decoded-image parity check passes all 84 views per version.
Maximum direct editor/player mean RGB error is 3.491e-6, with at most six pixels
whose RGB byte differences sum above six. Exact image equality is not achieved;
the check uses the existing source-render acceptance gate (changed pixels <=1%
of visible reference pixels and mean RGB <=0.002), without excluded pixels.

Evidence: `target/unity-shader-framing-6000.4.7f1` and its 6000.5.4f1 counterpart,
including `source-editor-player-report.json`, source/player reports and images.
The shell runner now requires this decoded parity gate after rendered player
checks. A separate exact-PNG script preserves the stronger diagnostic rather
than conflating tolerance-based parity with bit identity.
The new decoded gate rejects 48/84 views from the original framing-mismatch
dataset, confirming the regression control remains effective.
Next action: continuous Animator/shader clock integration, then review M5
against outstanding M4 quality dependencies and other engine/platform gates.
The overall plan remains incomplete. Review: 2026-11-14.

### Scrolling with skeletal deformation verified (2026-11-01)

`plate-scene-shader.gasset` combines the refined skinned fixture, three materials,
all six maps and frozen shader metadata. Both installed Unity versions pass rest
and 21 sampled bend/stretch/travel poses against independently deformed GLB
reference meshes with CPU-shifted UVs. The editor rejects 273 pose corruption
controls (plus rest controls), including wrong shader time for every view.
Maximum position distance is 8.343e-9 metres; maximum editor mean RGB error is
2.005e-6. Standalone Windows Mono/D3D11 players pass 84 views each and reject all
189 controls, with maximum mean RGB error 3.247e-6. Both builds/players exit zero.
Evidence: `target/unity-shader-deform-player-6000.4.7f1` and the corresponding
6000.5.4f1 project; separate editor runs are `target/unity-shader-deform-*`.

This proves sampled transfer/playback with the explicit adapters and shared
Unity lighting. The dense plate remains diagnostic topology, not accepted
deformable asset quality. Continuous Animator playback with scrolling, independent
lighting comparison and additional engine/platform gates remain open.
Direct editor/player image comparison is not yet passing: only 1/84 imported
PNGs are identical per version. Decoded RGB comparison gives maximum mean
error 0.02463666 and 24,336 changed pixels (sum of RGB byte differences >6).
Both source-reference suites passing does not resolve this discrepancy.
Next action: diagnose camera/framing versus runtime rendering differences,
then verify continuous Animator/shader clock integration and review M5 against
the remaining M4 dependencies. Review: 2026-11-14.

### Imported all-map shader coverage and late-renderer cleanup (2026-11-01)

`plate-shader-maps.gasset` exports all six texture slots with frozen UV metadata.
Both Unity versions pass 20 editor and 20 standalone D3D11 comparisons against
independently shifted mesh UVs, with zero RGB error and byte-identical editor/
player PNGs. Each imported map must exist and an omission control must visibly
change the image; all six controls pass. Evidence:
`target/unity-uv-maps-lifecycle-6000.4.7f1` and the corresponding 6000.5.4f1 project.
One runner was edited during execution and reported a shell parse error after
the player finished; its successful reports and final PNG gates were rechecked
separately. The current runner passes shell syntax validation.

Inspection found a real cleanup issue when a late-created deformation display
inherits an already-cloned material. UV sampling now tracks inherited clones,
and release scans current descendants as well as previously touched renderers.
Editor coverage tests the next-sample path; player coverage disables before a
further sample and confirms restoration plus deferred destruction. Both pass.
This synthetic lifecycle check does not establish animated deformation parity.

Next action: combine actual skinned/deformed imported geometry with shader
playback and verify its reference rendering. Independent engine lighting,
additional engines/platforms and remaining milestones stay open. Review: 2026-11-14.

### Standalone UV-scrolling player verified (2026-11-01)

The rendered FBX comparison now runs in Windows standalone Mono players built
with Unity 6000.4.7f1 and 6000.5.4f1, using the serialized adapted prefab and its
production shaders. Each player passes all 20 rendered comparisons (zero RGB
error), detects every wrong-time control and reproduces seek-back exactly.
All 20 PNGs are byte-identical between editor and player for each version.
Frame-based lifecycle checks confirm disable restores source materials, deferred
clone destruction completes, re-enable samples successfully, and invalid time
preserves the last-good sample. Both builds and players exit zero.

The test uses owned readable copies of imported static geometry for its CPU UV
reference; original FBX import settings are untouched. This is still the
base-colour plate fixture, not all-map/deforming shader acceptance. Evidence:
`target/unity-uv-player-fixed-6000.4.7f1` and the corresponding 6000.5.4f1 project,
including player reports, images and `uv-editor-player-report.json`.
Next action: extend imported shader scenes to all material maps and
deformation, then address the outstanding cross-engine and milestone gates.
M5 and the overall plan remain open. Review: 2026-11-14.

### Rendered FBX UV-scrolling comparison (2026-11-01)

The attached frozen plate prefab now has a real D3D11 rendered comparison in
both installed Unity versions (6000.4.7f1 and 6000.5.4f1). Four camera views and
five time samples compare production shader scrolling against independently
shifted UVs on cloned imported meshes. All 20 cases per editor have zero RGB
error; seek-back is pixel-exact and animation visibly changes. All deliberate
0.7-second timing errors are detected (minimum mean RGB error 0.00247176).
Evidence: `target/unity-uv-render-6000.4.7f1` and
`target/unity-uv-render-6000.5.4f1`, including reports, images and zero editor exit.

This covers the base-colour plate fixture with shared Unity lighting. Independent
gcad/Unity lighting, all-map imported scenes and player execution remain unproven.
Next action: standalone player verification of the attached shader.
Other engine/platform, performance and remaining milestone gates remain open;
M5 is not complete. Review: 2026-11-14.

### Digest-bound shader attachment (2026-11-01)

The Rust FBX publication path now includes a bounded `gcad-shaders.json` receipt
for frozen shader exports, binding source descriptions/material names to the
source GLB and published FBX digests. Unsupported metadata prevents publication;
plain exports require no receipt. The native helper alone does not generate it.
The unit test and real-helper publication/rejection test pass.

Unity's explicit adapter validates the receipt and material assignment before
creating assets, then saves a `GcadUvScrollAnimation` component on the prefab.
Explicit samples use owned per-instance materials and frozen-time deltas.
Disable/destroy restores source slots; invalid samples retain last-good state.
An initial edit-mode lifecycle failure was fixed with explicit editor execution.
Both installed editor versions pass attachment, two-instance isolation, seek,
last-good, restoration, digest/template/material rejection and source preservation
checks. Evidence: `target/unity-uv-attachment-validated-6000.5.4f1` and the
corresponding 6000.4.7f1 project, with `uv-attachment-report.json` and zero exit.

Next action: rendered FBX scene and standalone-player parity for the
attached shader. Broader engine/platform acceptance, performance and remaining
milestones stay open; this does not complete M5. Review: 2026-11-14.

### Unity UV-scrolling runtime and sampling validation (2026-11-01)

The Unity PBR shaders now share the template UV transform across all six map
slots. `Runtime/GcadUvScroll.cs` accepts explicit animation time, converts the
top-left UV convention for original meshes, and applies only a scroll delta to
UVs already baked by frozen FBX export. This avoids double-applying scale/offset.
Invalid input fails before material updates; the default shader path remains
disabled/unchanged. The API requires caller-owned material instances.

The Unity 6000.4.7f1 and 6000.5.4f1 D3D11 surface suites pass 120 independent UV-scrolling
cases (30,720 pixels) across original/frozen paths, five time samples, three
addressing modes and all material output channels. Maximum error is about
0.00000005960465; all 120 shifted-UV controls are rejected and invalid-input
last-good checks pass. Existing surface/raster regressions pass; editor exit zero.
Evidence and recipe: `target/unity-uv-scroll-verified-6000.5.4f1` and
`tools/game-assets-unity/README.md`; the corresponding 6000.4.7f1 project also
records identical numerical results and successful surface/raster regressions.

Next action: explicit attachment from retained
export metadata, then rendered FBX scene and standalone-player parity. The
numeric point-sampler test is not complete engine-template acceptance or M5.
Other engines, broader sampling/platforms and remaining milestones stay open.
Review: 2026-11-14.

### Explicit frozen shader export verified (2026-11-01)

Generic export now honors the shader's explicit reject/freeze fallback. Freeze
bakes top-left UVs at the declared time into an owned scene, rounds to portable
float32 coordinates and regenerates tangents. It preserves vertex/triangle order,
positions, normals, complete skin weights and inverse binds; derived skin guards
are rebuilt. Invalid or float32-collapsed UVs fail. Original assets are unchanged.

GLB extras and `asset-capabilities` record the original shader description, time,
lost shader animation and ordinary-PBR consumer requirement. Six shader tests
pass, including encoded UV readback, unchanged plain-material primitives, skin
preservation, rejection and numerical-collapse controls. At 0/1/2/4 seconds and
seek-back, the base-colour fixture's frozen mesh matches live raster appearance
with zero RGB byte error on both Direct3D 12 and Vulkan.

CLI export produces `target/plate-shader-frozen.glb`; the v12 helper converts it
to `target/fbx-shader-frozen-package` with one FBX and one separate PNG. Native
readback checks 1,276 ordered faces, 3,828 corners, scale and material connection;
eight altered material controls are rejected. The helper does not preserve GLB
fallback extras in FBX, so retain the capability report for FBX delivery. These
checks do not establish dynamic shader transfer or engine import/render parity.

Next action: implement the dynamic Unity template adapter and verify
matching time, UV conventions and texture sampling. Broader material/platform
coverage and remaining milestones stay open. Review: 2026-11-14.

### UV-scrolling raster preview and live recovery (2026-11-01)

The existing asset viewer now evaluates the selected shader at its explicit
preview time and passes a typed scale/offset through renderer material uniforms.
All raster texture slots share that transform; default CAD/PBR callers use the
identity. Shader-only assets have a ten-second preview timeline with play, pause,
loop and seek; a selected animation clip supplies its own duration and clock.
Shader reload retains time, and invalid descriptions retain the last valid frame.

The GPU test `uv_shader_gpu_matches_independent_uv_transform_and_seek` passes on
Windows Direct3D 12 and Vulkan. At times 0, 1, 2, 4 and a seek back to 0, its
textured fixture matches independently transformed reference UVs with zero RGB
byte error. The effect visibly changes pixels. Invalid/live repaired JSON tests
also pass, retaining the previous frame and explicit time. Existing extra-map
GPU tests, CAD/asset mode switching and the feature-disabled library check pass.

The fifth pinned renderer patch is `renderer-uv-transform.patch`; reproduction
from the original crate succeeds and all three changed source files match the
vendor copy modulo CRLF/LF. Raster draw stride grows from 256 to 512 bytes for
uniform alignment. Software/tracing explicitly reject nonidentity transforms;
browser/Linux runtime, all-map shader parity and performance remain unverified.
GLB/FBX export still rejects shaders until explicit fallback is implemented.
Next action: frozen UV export and dynamic Unity adapter parity, then
broader shader/engine/platform acceptance. M5 and the overall plan remain open.
Review: 2026-11-14.

### Shader authoring and dependency tracking connected (2026-11-01)

`.gasset` now accepts `shader SOURCE DESCRIPTION.json` under an explicit project
root. Source materials retain the validated description, including expanded CAD
assembly parts. Reads participate in live document dependency tracking but stay
outside geometry/skin binding provenance. Shader paths outside the project,
duplicate declarations, unknown source names and shader/atlas combinations fail.
`examples/game-assets/plate-shader.gasset` is the authoring fixture.

Verification: four shader/parser/authoring tests and three existing material
tests pass. Checks cover dependency observation, geometry-provenance separation,
invalid declarations, repaired description reads, project boundaries and export
refusal. GPU preview and live UI acceptance remain unverified for this effect.

Until evaluation is integrated, shader-bearing preview and GLB export explicitly
fail rather than silently losing the effect; FBX uses the same GLB gate. The
preview's existing last-good behavior remains. This is an interim implementation,
not M5 completion. Next action: add the typed UV transform to the existing
asset renderer using explicit preview time, then implement frozen export and the
dynamic engine adapter. Review: 2026-11-14.

### UV-scrolling template selected; initial contract implemented (2026-11-01)

UV-scrolling PBR is the first selected shader effect. The new
`asset::shader` library parses a version-1 JSON description with required scale,
offset, speed and explicit reject/freeze fallback. It evaluates top-left UVs as
`uv * scale + offset + seconds * speed` from a supplied clock. The description
is capped at 16 KiB; unknown fields/templates/versions, nonfinite or out-of-range
parameters and invalid fallbacks are rejected. Positive scale preserves tangent
orientation; bounds are recorded in the design document. Example:
`examples/game-assets/uv-scroll.shader.json`.

Verification: both `asset::shader::tests` pass, including deterministic sampling,
explicit freeze/reject behavior, unknown fallback fields and resource/numeric
limits. The independent native skin-inspection regression also passes with the
refined fixture, including sub-0.001 weights and invalid-input rejection.

This is the initial library contract only. `.gasset` attachment, renderer
evaluation, hot reload, frozen export with tangent/capability checks and the
dynamic engine adapter still need implementation. M5 remains incomplete.
Next action: connect the description to asset authoring and dependency
tracking, then the existing asset viewer's explicit clock and GPU material path.
Review: 2026-11-14.

### Complete FBX weights recovered for dense Unity assets (2026-11-01)

The explicit deformation adapter now reads original influences through native
`--inspect-skin-v1` and restores them into its owned mesh. It matches rest-world
positions and retained imported weights; missing or ambiguous matches fail.
The source FBX, importer settings and original mesh remain unchanged. Current
bounds are canonical FBX, one linear skin per mesh, one to four influences,
unique bone names per palette and world coordinates within 100 metres per axis.

The refined fixture restores 20 affected vertices. Both Unity 6000.4.7f1 and
6000.5.4f1 pass all 21 full-source editor poses and 84 standalone Windows
Mono/D3D11 views; all 105 player corruption controls are rejected. Maximum
position distance is 0.000000008344 metres, changed pixels per view 2, and mean
RGB error about 0.000001477. Editor/player processes exit zero. Recovery controls
reject shifted and ambiguous contracts and verify original-weight preservation.
Evidence: `target/unity-refined-recovered-*`; implementation/recipe and limits:
`tools/game-assets-unity/README.md`. Earlier failed pruning controls remain.

The reflected two-instance editor regression also passes all 21 poses, both
recovery controls and both deformation lifecycle checks on 6000.5.4f1
(`target/unity-reflected-recovered-6000.5.4f1`, exit zero). Neither instance needs
weight corrections; maximum position distance remains 0.00000001666 metres.

Next action: continue representative assets and remaining
engine/viewer/shader-authoring milestones, including broader skin-recovery
limits and consumer acceptance.
This does not accept the fixture's harsh deformation as production topology,
nor establish other pipelines, graphics APIs, platforms or engines.
Review: 2026-11-14.

### Refined fixture exposes Unity weight loss (2026-11-01)

The 20,416-triangle `plate-scene-refined.gasset` passes native v12 conversion
and readback, but fails four of 21 Unity 6000.5.4f1 editor position checks.
All 84 image comparisons and corruption controls pass. Maximum position error
is 0.000026813823 metres, above the unchanged 0.00001-metre gate; no player was
built. The copied Unity importer discards weights below 0.001. A zero-cutoff
request is reset to 0.001 after reimport and does not resolve the failure.

An independent diagnostic prunes and renormalizes only its source reference at
the effective importer cutoff. Its maximum distance falls to 0.000000008344
metres across all 21 poses, isolating weight pruning as the discrepancy.
This diagnostic is never substituted for full-source acceptance. Evidence:
`target/unity-refined-player-6000.5.4f1`,
`target/unity-refined-full-weights-6000.5.4f1` (historical experiment directory
name; weights were not preserved), and its `skin-weight-diagnostic.json`.

The geometry metric now uses an exact spatial search shared by editor/player
probes. Twenty-one independent oracle cases pass; two 100,000-point sets take
about 0.78 seconds in the initial editor check. The refined CPU deformation
probe takes about 7.39 ms per refresh for 12,155 imported vertices on this
machine; this is not a general runtime performance guarantee.

Next action: preserve the original FBX skin influences in the explicit
Unity adapter, then repeat full-source editor/player checks on both versions.
Other engine consumers, shader authoring and remaining milestones stay open.
Review: 2026-11-14.

### Reflected multi-instance Unity acceptance (2026-11-01)

The source-derived import gate now counts active mesh instances, skin palettes,
images and clip names from GLB, and deformation recovery checks cover every
adapter. The reflected fixture has 2,552 triangles and two skinned instances;
one is under parent scale (-1.25, 0.75, 1.5) and translation 0.1 metres.
Both Unity versions pass all 21 editor poses, 84 standalone D3D11 views and 105
player corruption controls using the existing v12/uncompressed/CPU-basis profile.
Maximum changed pixels per view is 67, mean RGB error about 0.000005815 and
vertex distance about 0.00000001666 metres. Editors and players exit zero.

Both mesh adapters pass last-good retention, recovery and source preservation.
The original single-instance import/material regression also passes. Evidence:
`target/unity-reflected-player-*`, `target/unity-source-count-regression-6000.5.4f1`;
recipe and scope: `tools/game-assets-unity/README.md`. This is two small instances,
not large-mesh performance or deformable-topology quality acceptance. Next action: scale the geometry comparison and validate a more highly refined asset.
Other engine consumers and the remaining plan milestones stay open.
Review: 2026-11-14.

### Rendered Unity players pass with explicit precision settings (2026-11-01)

Windows x64 Mono players on Unity 6000.4.7f1 and 6000.5.4f1 now pass 84 D3D11
image comparisons across 21 poses against direct-GLB reference mesh snapshots.
All 105 normal/translation corruptions are rejected. Maximum changed pixels per
view: 11; maximum mean normalized RGB error about 0.000002509; maximum vertex
distance about 0.000000007694 metres. Both editor and player processes exit zero.

The validated profile imports v12 normals/tangents, uses the explicit CPU basis
adapter and disables animation compression. Default keyframe reduction fails two
late-bend player views while still meeting the looser vertex-distance gate;
that failure is retained. No image thresholds were relaxed. Evidence:
`target/unity-player-render-uncompressed-*` and the failed
`target/unity-player-render-geometry-6000.5.4f1`; recipe:
`tools/game-assets-unity/README.md`. PNG output uses Unity's bundled
image-conversion module, with no registry dependencies or engine installation.

This checks sampled shipped-player geometry and shading using shared Unity
materials, not independent gcad-renderer equivalence or continuous GPU animation.
Larger/reflected fixtures, other rendering pipelines/platforms and other engines
remain open. Next action: broader fixture and consumer validation.
Review: 2026-11-14.

### Unity standalone lifecycle verification (2026-11-01)

The harness now builds and runs an isolated Windows x64 Mono test player using
the installed editor/player module. Both Unity 6000.4.7f1 and 6000.5.4f1 pass
automatic deformation initialization, Animator-driven vertex movement,
disable/re-enable, GameObject reactivation, singular-pose retention/recovery,
and component-removal cleanup/restoration in 38 frames. Both editor and player
processes exit zero. The preceding 21-pose shaded editor gate also passes.

The project enables only Unity's existing bundled animation and JSON modules;
there are no registry dependencies or engine installations. Player execution is
headless/no-graphics and does not automate the desktop. Evidence:
`target/unity-player-animator-*` (`player-build-report.json`,
`Player/player-report.json`); reproduction: add `GCAD_PLAYER_CHECK=1` to the
documented deformation-adapter command in `tools/game-assets-unity/README.md`.
This establishes CPU/runtime lifecycle for this fixture, not player GPU shading,
IL2CPP, Linux, large meshes or other engines. Next action: larger/reflected
fixture and rendered-player acceptance. Review: 2026-11-14.

### Explicit Unity deformation-basis path passes sampled shading (2026-11-01)

An opt-in CPU path in `GcadMaterialAdapter.Create(..., true)` now creates owned
readable meshes and a runtime `GcadDeformation` component while retaining imported
bones/clips. It uses weighted matrices, inverse-transpose normals and
orthogonalized tangents, matching gcad's deformation convention. Original assets
and metadata are preserved; normals/tangents must already be configured for
import from v12. The ordinary material-only adapter remains available.

Both installed Unity versions pass 21 poses / 84 shaded views and all 189
animated corruption controls. Maximum mean RGB error is about 0.000054214 and
maximum position error about 0.000006627 metres. The camera depth range now fits
the centimetre-scale fixture to remove folded-surface depth artifacts; acceptance
tolerances and full-frame coverage are unchanged. The ordinary path still fails
nine poses under the same fitted camera. Both adapted editor runs exit zero.

Singular/missing-bone rejection, last-good retention, recovery and rest-mesh
preservation pass. Measured explicit refresh cost is about 0.72–0.79 ms for this
1,169-vertex fixture (32 refreshes), not a general runtime performance guarantee.
Evidence: `target/unity-deformation-verified-*`; recipe/limits:
`tools/game-assets-unity/README.md`. Player lifecycle, larger/other fixtures,
URP/HDRP and other engines remain unverified. This does not complete M5 shader
authoring or the full plan. Next action: standalone player lifecycle and
broader fixture validation. Review: 2026-11-14.

### V12 corrects exported tangent handedness (2026-11-01)

Independent Unity rest-attribute matching identifies opposite tangent handedness
at all 1,129 unambiguous v11 imported vertices (40 seam vertices are ambiguous;
none unmatched). Direction and UV agree. Negating only tangent W makes all four
rest images exact. The native v12 patch fixes the importer cause: generating
bitangents with the corresponding sign change when glTF texture V is flipped.

V12 builds in Windows Release and passes mirrored-UV/reflected-instance native
audits plus the Rust helper package integration test. Both Unity versions pass
exact rest images with imported v12 tangents and no sign workaround. Active
helper identity, shell build/package and Unity pins now require v12. Patch hash:
`783bf12bee4103422f8e8d2f74b2f17ae4dd93109b25266cd67baa9838340bf9`.
Evidence and reproduction: `tools/game-assets-fbx/README.md` and
`tools/game-assets-unity/README.md`. Nine animated shaded poses still fail in
both versions; the tangent fix does not resolve the separate deformation-basis
convention. Next action: preserve the intended animated basis in the
engine adapter, then rerun the unchanged gate. Review: 2026-11-14.

### Animation and tangent import isolation (2026-11-01)

Three fixture-only Unity 6000.5.4f1 diagnostics now vary one import option at a
time. Disabling animation compression or resampling reduces maximum pose vertex
distance to about 7.509 nanometres but leaves the same nine shaded-pose failures.
Importing stored FBX tangents fails already at rest (13,490–16,900 changed pixels
per view, maximum mean RGB error about 0.007669) despite zero position error.
The production adapter remains unchanged; only the isolated copied fixture's
metadata is modified, before the adapter preservation snapshot.

A second diagnostic directly transforms both normal and tangent vectors without
orthogonalization. With compression off, stretch discrepancies fall to at most
15 pixels per view, but late bend discrepancies remain (up to 344). Neither
diagnostic changes the inverse-transpose source reference or acceptance gate.
All four runs exit 1 for their failed gates. Evidence and commands are recorded
under controlled import diagnostics in `tools/game-assets-unity/README.md`.
Next action: independent stored/imported tangent and handedness readback,
then an adapter path preserving the intended deformation basis. These modes are
not yet tested on the second Unity version. Review: 2026-11-14.

### Animated shaded comparison exposes remaining differences (2026-11-01)

The direct-GLB Unity reference now evaluates LINEAR translation/rotation/scale
tracks and skin weights, with gcad's inverse-transpose blended normal convention.
Both Unity 6000.4.7f1 and 6000.5.4f1 fail the unchanged shaded comparison:
3/7 bend, 2/7 stretch and 7/7 travel poses pass, each tested from four views.
All 21 poses pass the separate 0.00001-metre vertex-set distance gate (maximum
about 0.000006627 metres), and all 189 corruption controls are rejected. Rest
still passes. Both editors correctly exit 1; animated appearance is not accepted.

A diagnostic linear-matrix normal reference greatly reduces stretch differences
but does not eliminate all bend differences. This suggests a deformation-basis
mismatch, not a fully established root cause. The diagnostic does not affect
acceptance or change production behavior. Evidence:
`target/unity-source-animation-basis-*`; reproduction and limits:
`tools/game-assets-unity/README.md`. Next action: isolate imported
animation/tangent differences and preserve the intended deformation basis through
the adapter, then rerun the same gate. Review: 2026-11-14.

### Shaded source GLB versus FBX geometry (2026-11-01)

The Unity harness now builds a rest-geometry reference directly from the source
GLB, independently of Assimp/FBX, and compares four shaded views from above and
below against the adapted FBX. Both installed versions (6000.4.7f1, 6000.5.4f1)
pass on D3D11 and exit zero. Each view compares 262,144 pixels; changed counts are
0, 3, 1 and 14, with maximum full-frame mean RGB error about 0.000002469.
The gate allows changed pixels up to 1% of visible coverage and mean error up to
0.002; no silhouette exclusion is used. Shifted UVs and inverted normals in each
view plus a translated model all fail the same gate (nine controls).

Evidence: `target/unity-source-render-controls-*`; recipe and exact thresholds:
`tools/game-assets-unity/README.md`. The reference loader is fixture-scoped,
checks bind-neutral rest skinning and shares the independently audited adapter
materials with the imported model. This establishes bounded rest geometry and
attribute transfer, not independent shader equivalence, gcad-renderer parity,
animated shaded parity or other-engine acceptance. Next action: animated
source comparisons and broader engine fixtures. Review: 2026-11-14.

### Unity material calculation and raster checks (2026-11-01)

Both installed Unity versions pass a shared-production-surface-function probe:
144 cases / 36,864 pixels each, maximum RGBA error about 0.000080377 against
0.001 tolerance. The CPU reference checks sRGB colour decoding, linear numeric
channels, factors, scaled full-XYZ normals, controlled facing signs, all alpha
mode calculations and cutoff equality. Actual production shaders separately
pass 36 emission-only centre-pixel cases covering front/back triangle winding,
one-/two-sided culling and opaque/mask/blend states; all reported RGB errors are
zero against 0.002 tolerance. Evidence: `target/unity-surface-raster-*`; reproduce
with `GCAD_SURFACE_CHECK=1` in the Unity harness. These bounded checks do not prove
lit back-face tangent frames, shadows, transparent sorting, edge AA or complete
source-render parity. Next action: shaded source comparisons and broader
fixtures/engines. Review: 2026-11-14.

### Unity numerical sampler validation (2026-11-01)

The production PBR adapter and an offscreen probe now share `GcadSampling.cginc`.
An independent CPU address/weighted-sum reference validates 216 cases and 884,736
pixels per Unity version (6000.4.7f1, 6000.5.4f1), covering repeat/clamp/mirror,
both mag/all six min filters, LOD boundary/fractional/integer mip choices,
reversed UVs and mip-limit clamping. Both D3D11 runs pass with maximum channel
error about 0.000017494 against 0.0002 tolerance; a changed-colour control is
rejected. Evidence: `target/unity-sampler-6000.4.7f1` and
`target/unity-sampler-expanded-6000.5.4f1`. This checks explicitly supplied linear
float mip levels and the chosen LOD rule, not mip generation, sRGB decoding,
lighting, all API-specific sampling behavior or other engines. Recipe:
`GCAD_SAMPLER_CHECK=1` with the Unity harness. The material/raster checks above
add bounded colour/normal/alpha coverage. Next action: broader shaded
source-render comparisons.
Review: 2026-11-14.

### Explicit Unity material adapter (2026-11-01)

`GcadMaterialAdapter.Create` now reads the native FBX material contract and creates
new Built-In pipeline materials, per-slot PNG copies and a prefab. Original FBX,
PNG and import metadata bytes are preserved. Dedicated gcad shaders retain the
source G/B/R numeric channel layout, colour/emission factors, normal scale,
alpha/culling intent and separate min/mag filtering; no destructive repacking or
global import hook is used. Linear colour space is required. Both Unity
6000.4.7f1 and 6000.5.4f1 pass the source-derived settings/map/byte audit and D3D11
rendered feature-use controls for the masked reference fixture. All six controls
(normal, metallic, roughness, occlusion, alpha mask, emission) change rendered
pixels. Evidence: `target/unity-adapter-render-*`; recipe and limits:
`tools/game-assets-unity/README.md`. This does not establish source-render parity,
blend/back-face/shadow correctness, numerical sampler fidelity, URP/HDRP or other
engine acceptance. Next action: numerical shader/sampler validation and
source-render comparisons. Review: 2026-11-14.

### Direct FBX material contract for Unity (2026-11-01)

Unity 6000.5.4f1's opt-in MaterialDescription probe exposes none of eleven required
gcad properties across the three reference materials (33 missing checks). The
fixture-only observer is inactive outside its explicit audit mode. The native
helper now provides `--inspect-materials-v1 ABSOLUTE_MODEL.fbx`, reading schema-3
factors, alpha/culling intent, map paths, channels and sampler/UV settings directly
through ufbx. This preserves the requested FBX-plus-PNG delivery without requiring
a source GLB or extra metadata file. Windows Release and the native integration
test pass: extracted values match the source GLB, a damaged schema is rejected,
and input FBX bytes remain unchanged. Evidence: `target/fbx-material-contract.json`,
`target/unity-description-6000.5.4f1`. The explicit adapter described above now
consumes this command. Next action: broader rendered material comparisons.
Review: 2026-11-14.

### Unity source-material audit failures (2026-11-01)

The new source-GLB-derived `MaterialCheck.cs` fails identically on Unity
6000.4.7f1 and 6000.5.4f1. Both ordinary imports connect base/normal/emission
textures but lose red/blue base factors, metallic, alpha-mask settings, normal
scale, occlusion strength and emission factors; occlusion and metallic/roughness
maps are absent. Standard raster culling also fails the source double-sided
requirement. Reports contain 32 failed checks, including three deliberately
unverified metallic/roughness-repacking checks. These are settings/connection
findings, not rendered pixel differences or full shader acceptance. Both editors
terminate with code 1 for the failed gate. Evidence: `target/unity-material-*`;
reproduction: `tools/game-assets-unity/README.md`. Existing import/pose passes
do not establish material parity. Next action: explicit material adapter,
channel/factor conversion and rendered validation. Review: 2026-11-14.

### Live rig and binding regression (2026-11-01)

The Windows headless document test
`binding_and_rig_edits_update_live_pose_and_require_explicit_rebind` now passes.
It changes an external weight file without saving the `.gasset` and checks an
analytic 10 mm displacement in every displayed mesh corner. An animation edit
then produces 20 mm displacement and updates the skeleton marker. Changing the
rest pose rejects the stale weight file with the explicit-rebinding diagnostic
and retains the last valid frame; regenerating only the binding recovers to the
expected 18 mm displacement. Camera and named clip/time survive, and a weight
inspection selection clears when its joint leaves the skin palette. The fixture
uses a rigidly weighted mesh and real files/reload worker; blended deformation,
GPU rendering and integrated desktop acceptance remain separate checks.
Command: `cargo test --offline --lib --features game-assets
binding_and_rig_edits_update_live_pose -- --nocapture`.
Next action: continue integrated viewer acceptance and engine material
validation; this does not complete the remaining plan milestones. Review: 2026-11-14.

### Source-pixel texture inspection (2026-11-01)

The existing asset inspector now offers **Inspect source pixels** alongside its
thumbnail overview. It provides 1–16× texel zoom, drag panning, exact X/Y region
selection, transparency checkerboard and hovered raw RGBA/coordinate readout for
the selected material channel and mip. Uploads are bounded to the visible region
(at most 512 × 256 texels); switching maps or reloading invalidates that cache.
Source textures remain unchanged. A crop regression checks source texels beyond
thumbnail resolution and edge clamping. The opt-in Windows offscreen GPU test
`source_pixel_ui_renders_and_reloads_the_visible_region` passes, including a
rendered-change check after invalidation; its image was visually inspected at
`target/game-asset-source-pixels.png`. Pointer-event tests inside a scroll area
also pass at 100% and 200% scaling: 1-point drag increments accumulate at 16×
zoom, and completely transparent texels retain readable source RGBA values.
The offscreen test handles a native DPI transition to 2× and back, retaining the
source region and producing a 1200 × 920 image, visually checked at
`target/game-asset-source-pixels-2x.png`. The texture viewport now has an
accessibility label. These are component-level checks; integrated desktop
interaction and other platforms remain unverified. Next action: continue
rig/binding live reload coverage and integrated viewer acceptance. Review: 2026-11-14.

### Live document texture and playback regression (2026-11-01)

The headless document UI now has a passing Windows regression for texture-only
edits, incomplete PNG saves and automatic recovery. It verifies the actual
preview frame's texture bytes, the visible last-valid-model notice, retained
camera and named clip/time, time clamping when a clip is shortened, and rest-pose
fallback when that clip is removed. It uses real temporary files and the running
reload worker rather than injected events. Command:
`cargo test --offline --lib --features game-assets
live_document_recovers_texture_and_preserves_camera_and_clip -- --nocapture`.
This proves document state/frame updates, not GPU image correctness, interactive
responsiveness or full rig/weight/shader live-reload coverage. Next action: extend live-authoring coverage to rig/binding edits and complete visual
acceptance. Review: 2026-11-14.

### CAD and game-asset UI boundary (2026-11-01)

Keep one gcad application with distinct document modes. Normal CAD documents must
retain their intended CAD controls and workflow. Opening a `.gasset` activates
the game-asset viewing workspace; material, texture, rig, animation and LOD
controls belong there and must not crowd the normal CAD interface. Both modes
provide live feedback while sources are edited. Opening a CAD document returns
to the CAD interface. A separate viewer application is not required.

The current implementation routes both modes through the existing `App` and
gates game controls and help on the asset document. The headless existing-App
test passes on Windows: asset Fit model is visible while CAD measure/section
controls are absent; opening CAD restores those controls, its scene and watcher,
and removes Fit model. Each mode shows its own help; switching back to an asset
clears CAD loading/watching. Measure/section now expose accessibility labels.
`cargo test --offline --lib --features game-assets
game_asset_opens_and_switches_in_existing_app -- --nocapture` passes, as does
`cargo check --offline --lib --no-default-features`. This is automated UI/state
coverage, not desktop visual acceptance or full CAD regression coverage.
Next action: complete viewer visual acceptance and broader live-authoring
coverage. Review: 2026-11-14.

### FBX helper boundary in progress (2026-11-01)

Offscreen Unity Direct3D 11 material-use checks now capture baseline and disabled-
map controls. Both tested versions show 74,002 visible pixels, 66,400 base-map
changed pixels and 53,187 normal-map changed pixels. Default texture settings fail
the normal-import-type gate; the explicit `GcadNormalTextureImport.Configure`
adapter sets connected normal PNGs to NormalMap without changing image bytes or
installing a global postprocessor. With it both versions pass. Baselines before
and after settings normalization are byte-identical on this fixture, so this is
not evidence of visual correction or full source-material parity. Images were
visually checked; source alpha/PBR/AO/emission and wider engines still need tests.
Evidence: `target/unity-render-*`; recipe: `tools/game-assets-unity/README.md`.
Next action: wider material/render coverage. Review: 2026-11-14.

Unity 6000.4.7f1 and 6000.5.4f1 now pass a source-derived sampled pose check for
the textured reference fixture. `--unity-pose-reference` generates source world
positions from GLB animation/skin data; the Unity harness samples imported clips
and bakes skinning, then compares vertex sets in source metre coordinates. All
21 samples across bend/stretch/travel pass a 0.01 mm tolerance; the maximum
bidirectional nearest-vertex distance is about 0.007930061 mm. A translated
corruption is rejected. This does not prove triangle-interior, shading or
continuous-time equivalence. Evidence: `target/unity-pose-*`. Next action:
rendered material checks and broader engine/fixture coverage. Review: 2026-11-14.

The v11 writer patch fixes the first Unity consumer failures: one cluster per
joint now combines weights across material primitives (conflicting inverse binds
are refused), and standard normal/emission texture connections coexist with the
custom gcad connections. Both Unity 6000.4.7f1 and 6000.5.4f1 now pass the reference
headless import gate: two bones, zero duplicates, three normal maps, three base
maps, three clips and 1,276 triangles; the previous unweighted-vertex warnings
are absent. Independent reflected/shared-skin audits still pass, and a conflicting
bind mutation is rejected. The active Rust/package pin is now v11 SHA-256
`719d4a76baf866c62cb26eb20b1ee38191670738c94ccbe9b7ca10cdb55a6349`.
Evidence: `target/fbx-v11-*`, `target/unity-v11-*`. This is import acceptance for
one fixture, not sampled Unity pose or rendered material equivalence. Next action: engine pose/render checks and other consumers. Review: 2026-11-14.

First Unity consumer checks expose failures despite independent ufbx validation.
The ordinary 6000.4.7f1 import preserves 1,276 triangles, three clips/materials,
15 PNGs and three base-colour connections, but produces six bone entries with four
duplicate references, reassigns 168 vertices reported unweighted, and connects no
normal maps. 6000.5.4f1 initially reports the same warnings/map omissions and exits
abnormally after its report; its stricter rerun exits with code 1 and confirms the
same four duplicate bone references and zero normal maps. The headless check is in
`tools/game-assets-unity/`; this is failed import acceptance, not renderer support.
Next action: fix same-joint FBX cluster duplication and standard normal-map
connections, then rerun engine checks. Evidence: `target/unity-import-*`.
WSL is present but has no Rust/C++ build tools on PATH; no toolchains were installed.
Review: 2026-11-14.

Version-marked staging now uses shared OS file leases held by gcad and the native
helper for both the input snapshot and output directory. Recovery takes an
exclusive lease before deleting an abandoned stage. `asset-fbx-recover DIRECTORY`
scans only immediate marked stage children and preserves active leases, published
packages and older unmarked directories. Failed ordinary cleanup retains its
marker for recovery. The caller requires the new `leased-staging-v1` capability;
older experimental bundles must be rebuilt. Eight Rust tests and three native
integration tests pass on Windows, including recovery after the calling process closes its
lease while a helper remains live, then cleanup after helper exit. CLI recovery
also removes a marked abandoned stage and preserves an existing package. Evidence:
`target/fbx-lease-*`, `target/fbx-recovery-cli*`. Linux remains unverified. Next
action: validate deployment and engine consumers. Review: 2026-11-14.

`sh tools/game-assets.sh package-helper NEW_OUTPUT` now builds an experimental
Release helper bundle from the pinned checkouts in a separate build directory.
It includes the executable, handshake identity, reviewed source patch, usage notes,
dependency notices and SHA-256 inventory. Windows uses the static MSVC runtime;
the built executable imports only `KERNEL32.dll`. All 21 bundle-file checksums pass,
and gcad successfully used the bundled helper to publish 16 asset files. Evidence:
`target/fbx-bundle-*`. Clean-machine/Linux verification, signed distribution,
interrupted-process recovery and engine acceptance remain pending.
review: 2026-11-14.

Prepared conversion now checks ordered faces and per-face material assignments
after authored names are restored, retaining original source material provenance
through import reordering. Checks also cover corner positions/winding, normals,
UVs, tangent frames and skin weights/binds before any staged writes. The textured
fixture passes 1,276 faces and rejects five geometry/material corruptions; the
reflected fixture passes 2,552 faces and rejects ten. CLI conversion with this
gate enabled published the complete 16-file package. Evidence:
`target/fbx-restored*`. Next action: interrupted-process recovery and
versioned engine import/render acceptance; review: 2026-11-14.

`asset-fbx-experimental INPUT.glb OUTPUT_DIRECTORY --helper ABSOLUTE_PATH` now
exposes the staged package workflow, with a per-process timeout option. The caller
captures a bounded GLB snapshot in a separate temporary directory, passes only
that snapshot to the helper, and verifies its SHA-256 before publication. Windows
capture excludes concurrent writers; capture also checks file length and modified
time. The private snapshot is cleaned up separately from the published package.
Seven Rust tests pass, including source edits after capture. Windows CLI end-to-end
verification published 16 artifacts from the textured three-clip fixture and
refused the existing package on a second invocation. Evidence:
`target/fbx-snapshot-tests.*`, `target/fbx-cli-*`. Next action: complete
geometry/material-assignment validation on the restored scene, interrupted-process
recovery and engine acceptance. review: 2026-11-14.

The native helper now accepts experimental `--convert-v1 ABSOLUTE_INPUT
ABSOLUTE_STAGE`, requiring an existing empty staging directory. It runs shared
preparation and independent readback, writes `model.fbx` plus separate PNGs, checks
written bytes against memory, and returns protocol-1 JSON listing artifact paths
and lengths. Diagnostics use stderr. A Windows Debug run produced and verified
16 artifacts from the textured three-clip fixture; reusing the nonempty stage was
rejected (`target/fbx-convert-*`). The handshake advertises
`staged-conversion-v1-experimental` but still reports `conversion_ready: false`.
Rust `convert_experimental` now checks the helper identity/capability, runs staged
conversion through the bounded process runner, validates the protocol and exact
artifact set (portable paths, file types, sizes and signatures), checks cancellation
and invokes atomic publication. Six Rust boundary/package/manifest tests pass.
The Windows native end-to-end test passes for publication, preservation of an
existing package and cleanup after invalid input. It caught and fixed texture
filesystem separators under extended Windows paths; FBX references retain forward
slashes. Evidence: `target/fbx-connected-*`. Next action: expose
the experimental workflow in the CLI after that check and integrate stable input
snapshotting. This is not yet production engine support. Review: 2026-11-14.

`fbx_package::PackageStage` now reserves a sibling staging directory and requires
a validation callback before publishing the entire directory with a no-replace
rename. Existing destinations, including ones created during validation, must be
preserved. Dropping an unpublished stage attempts cleanup. This provides atomic
visibility, not crash/power-loss durability; interrupted-process recovery and
reporting cleanup failures still need integration. Three Windows tests pass for
complete publication, validation-failure cleanup, and preservation of concurrently
created empty/nonempty destinations (`target/fbx-package-tests.*`). The Linux
`renameat2(RENAME_NOREPLACE)` path is implemented but unverified. The
production conversion command does not use the publisher yet.

Texture preparation now builds an in-memory export manifest with stable relative
PNG paths and material-slot bindings, without changing the imported scene. It
decodes bounded PNG inputs for validation and retains their original encoded
bytes. Shared image references share one output entry; authored image filenames
cannot determine output paths. Native Debug tests pass for the RGBA and numeric
fixtures, including malformed references and truncated PNG rejection. This is
preparation only: no texture files or converted FBX are published yet.
The manifest can now be applied to a deep copy of the scene. Embedded images are
removed from that copy after material references are rebound; the source stays
unchanged. Independent ufbx readback of both fixture exports verifies authored
node/material names, relative texture paths, diffuse/normal connections and the
absence of embedded image content. These are in-memory fixture exports, not yet
the full GLB conversion pipeline or engine acceptance.
Name restoration now applies an import-identity map to nodes, meshes and materials,
including bone and node-animation target references. It rejects name collisions,
unrepresentable names and missing targets before applying identity changes. Native
tests cover those refusals, reference restoration and subsequent static FBX
readback. The synthetic rig references exercise renaming only, not deformed or
animated FBX equivalence. GLB identity-map construction is now shared with the
existing scene audit: it captures authored names before tagging JSON, including
Assimp's per-primitive mesh suffixes. The shared-skinned-instance animation audit
passes with this extraction (2,552 ordered faces and 551,232 sampled skin corners).
The original audit still retains temporary names through readback. Shared
`importScene`/`prepareScene` preparation now combines source validation, instance
isolation, provenance attachment, name restoration and external PNG preparation.
`gcad_fbx_probe --audit-prepared-scene INPUT.glb` exports that prepared scene in
memory and checks independent readback. It passes a shared-skin fixture and a
textured three-clip fixture (15 external PNG references, 865,128 sampled skin
corners). Physical units, authored node hierarchy and skin/animation playback are
checked after restoration. Evidence: `target/fbx-prepared-*` and the original
audit regression in `target/fbx-import-regression.*`. External custom-material
validation now checks each map's exact relative path, absence of embedded content,
samplers/UV sets, factors, channel conventions and source provenance. The textured
three-clip fixture passes 18 map connections and rejects 138 altered material
controls; the reflected fixture and original embedded-material audit also pass.
Evidence: `target/fbx-external-*` and `target/fbx-material-regression.*`.
Standard consumer-material acceptance, production request wiring and artifact
publication remain pending.
Next action: integrate canonical GLB import with provenance and skin/
animation handling, then add verified transactional publication.
Review: 2026-11-14.

The native helper now has a versioned handshake containing its actual Assimp
revision and full tracked-diff hash. The Rust caller checks the reviewed pins,
requires an explicit absolute executable path, bounds captured output and supports
timeout/cancellation with direct-child cleanup. An end-to-end Windows Debug check
through `asset-fbx-helper` accepts the reviewed v10 helper. Conversion is explicitly
reported unavailable; this is not a production exporter.
Validation: two Rust handshake tests and an opt-in native process integration test
pass, covering malformed JSON, oversized stdout/stderr, nonzero exit, deadline and
cancellation. The native helper builds and passes the gcad handshake check; a
negative build-identity check rejects mismatched source metadata. Evidence:
`target/fbx-helper-*`. Source identity is rechecked before helper compilation.
Next action: implement conversion requests preserving authored names,
separate PNG textures with relative references, independent readback and
transactional publication. Packaging and multi-engine acceptance remain required.
Review: 2026-11-14.

### Product direction

Use the repository's existing Rust/C++ implementation and shell-script conventions.
Build and validation entry points are in [development commands](../tools/game-assets.md).
The renderer is adopted and the optional native FBX helper approach is approved.
The target is portability across Unity, Unreal, Godot and other engines, rather
than selecting one engine as the product's scope. Unity delivery must include FBX
and separate PNG textures with relative references. OpenGL itself defines neither
an asset importer nor material conventions; additional engines need consumer tests.

Implement the helper's versioned boundary, transactional output, cancellation and
packaging, then validate each engine/version and representative assets. Standard
materials should import usefully without a custom adapter; identify features that
require one. Integration is still incomplete. Older dated entries below record
historical decisions and evidence, not current tooling requirements.

### Live game-asset viewing required (2026-11-01)

Game assets are a separate document mode within the existing gcad application,
not a separate viewer application. Opening a `.gasset` enables asset controls;
opening ordinary CAD documents retains their existing CAD interface and workflow.
The common window, open picker and drag-and-drop routes serve both modes. Keep
game-only controls out of ordinary CAD work. This supersedes the earlier standalone
`asset-preview` window; that command now routes to the same gcad app.

The user must be able to leave the gcad viewer open and watch the game asset
develop as the agent edits it, just as with ordinary CAD models. This is a core
authoring requirement, not an export-only workflow or a sequence of snapshots.

- Reload changes to the `.gasset` document and its dependencies, including CAD
  parts/assemblies, textures, material inputs, rigs, bindings and animation data.
  Handle file replacement, newly referenced files and recovery after failed loads.
- Keep navigation responsive, preserve the camera and compatible playback state,
  and coalesce edits so obsolete builds cannot replace newer results.
- Keep the last valid model visible during incomplete or invalid edits; clearly
  show that it is stale, explain the error, and recover automatically on correction.
- Provide inspection of geometry and instances, UVs and texture maps, materials
  and transparency, rigs and skin weights/deformation, rest poses, animation clips
  and root motion, and generated LODs. Include shader preview as shader authoring
  is introduced. Each authoring feature needs an appropriate viewer inspection
  mode; unsupported effects must be identified rather than silently approximated.
- Keep the ordinary CAD viewing workflow intact. Engine-specific appearance still
  requires engine validation; the gcad preview must explain known differences.

Implementation now connects `asset-preview` to a background dependency poller
with save coalescing, failed-load recovery and last-valid-scene retention. It keeps
the camera and matches clips by name across reloads. Polling uses file modification
times and sizes; writes that deliberately preserve both are not detected. The
existing navigation, material rendering, play/pause, looping and scrubbing remain.
Full inspection coverage and interactive acceptance are still incomplete.
Validation: 134 non-GPU asset library tests and two native input-recording tests
pass. The reload regression covers a CAD dependency edit, invalid-source recovery,
creation of a newly referenced missing dependency and retirement of old inputs.
Logs: `target/live-reload-regression.*` and `target/live-reload-inputs.*`.
Before integration, native Windows UI checks observed PNG reload, animation edits
at preserved paused time, retained camera zoom, invalid-source recovery, new rig
and binding loading, binding-error recovery, and return to rest pose when the
selected clip disappeared. Desktop automation was stopped by the user. These
checks do not prove the newly integrated app's visual acceptance. A headless
test of the actual gcad `App` passes: an asset loads, switching to a CAD part
rebuilds its scene and restores its watcher, and switching back activates asset
mode without leaving the CAD loader active. Evidence:
`target/integrated-viewer-test.*`. Full playback, rapid-save and remaining
inspection acceptance are still open. The **Fit model** action now adopts the
current pose's bounds; ordinary reloads preserve the user's view.
The integrated tree passes 135 asset library tests and the feature-disabled CLI
build check (`target/integrated-viewer-assets.*`, `target/integrated-viewer-default.*`).
Asset mode now has a read-only, toggleable scene inspector for node selection,
parent navigation, sampled local poses, mesh/primitive counts, material factors,
texture dimensions and skin joint navigation. Selection is retained by node name
across reloads. Primitive inspection now includes a fitted UV triangle layout
across all tiles, a 0–1 tile reference and raw-channel texture thumbnails with
explicit mip selection. Thumbnails are capped at 512 pixels on the longest side,
with source/preview dimensions labelled. Source-pixel navigation is now available
through the bounded region viewer described above. LOD comparison generates an in-memory derived scene using the
existing reducer, with source/LOD switching at the same camera and animation time.
The inspector and weight view follow the displayed scene. It reports target
attainment and sampled correspondence error; optional CLI acceptance gates and
vertex locks are not enabled by this preview. Reloads discard derived data and
detach old jobs, whose bounded computation may finish without being adopted.
Root-motion inspection now uses the existing exact
XYZ translation extractor for top-level nodes: an X-ray key trajectory, current
time marker and displacement in millimetres. STEP tracks display discontinuous
key markers; linear tracks connect their keys. Rotation extraction and multi-cycle
accumulation are not inferred by this view. Skinned assets now offer
a per-joint weight heatmap in the animated 3D preview, with blue=0/red=1 and grey
unskinned geometry. It replaces only preview materials/UVs and preserves authored
weights, deformation and export output. Joint selection survives reload by name;
removal returns to ordinary material appearance. The optional X-ray skeleton
overlay now draws joint/ancestor markers, parent connections and labels from the
same sampled world transforms as the displayed mesh. It stays aligned with the
retained mesh after failed updates; overlay visibility is independent of CAD mode.
Validation: 139 asset library tests pass, including a per-vertex comparison of
heatmap coordinates to persisted palette weights at an animated pose, identical
geometry/normals and unchanged GLB bytes (`target/asset-weights-preview-tests.*`).
Weight-mode desktop visual acceptance remains pending.
Skeleton validation: 140 asset library tests pass, including independently
expected joint positions under an animated parent translation, retained parent
links and restoration of the rest pose (`target/asset-rig-preview-tests.*`).
Desktop overlay acceptance remains pending.
Root-motion validation: 141 asset library tests pass. The trajectory regression
checks first-key anchoring, linear displacement against the original sampled root
position, and STEP values immediately before and at a jump
(`target/asset-motion-preview-tests.*`). Desktop trajectory acceptance remains pending.
LOD comparison validation: 142 asset library tests pass, including use of the
actual reducer, preparing the derived scene for preview, unchanged source GLB output and
rejection of an old worker result after invalidation
(`target/asset-lod-preview-tests.*`). Desktop comparison acceptance remains pending.
Validation: 138 asset library tests pass, including UV fitting for tiled,
degenerate and extreme finite coordinates, thumbnail RGBA preservation and bounded
thumbnail dimensions (`target/asset-visuals-tests.*`). Desktop visual acceptance
has not been resumed.
Next action: validate live reload interactively, then complete inspection
controls alongside each feature. Validate by editing geometry, a PNG,
a binding and a clip while the viewer stays open, including an invalid edit and
recovery, rapid successive saves and preservation of camera/playback state.
Review: 2026-11-14.

### Renderer adoption authorized (2026-11-01)

The renderer was adopted. The active Cargo patch
now selects `vendor/egui_bench-0.9.2`, preserving the pinned archive's licenses and
the four reviewed renderer patches. All three gcad adapters are applied. GPU tests
are opt-in through `gpu-tests`; regular asset validation remains GPU-independent.
The earlier pending renderer decision is superseded. FBX helper adoption, first
engine/version, explicit mip delivery and shader choices remain open.

Active validation passes: 140 asset tests on each of Direct3D 12 and Vulkan,
feature-disabled CLI check, feature-enabled Debug executable build and WASM asset
library check. All 130 vendored package files match the reproduced renderer; the
additional provenance document records licensing, pins and update instructions.
Evidence: `target/renderer-adoption-20261101`. Existing compiler warnings remain.
Next action: verify interactive controls and package adoption for review;
select the first engine and evaluate the FBX helper route.
Platform runtime and engine acceptance remain separate.
Review: 2026-11-14.

### Post-packaging platform and decision audit (2026-11-01)

All 39 review artifacts are frozen; the assembled tree reconciles all 531 active
repository paths (211 exact hashes and 320 text-only CRLF differences). A fresh
offline WASM library check of that snapshot passes in 11.02 seconds with existing
warnings. This does not verify the threaded browser application. Windows has only
the stable Rust toolchain; the browser build requires nightly/build-std and its
script can install wasm-bindgen, so it was not run. A PATH check in the existing
WSL distribution finds Clang but not Cargo, rustc or CMake; this is not an exhaustive
installation search or Linux validation. Evidence:
`target/game-assets-review/post-0039-platform-check.json`. No tools were installed.

Renderer adoption, the optional FBX helper, the first engine/version and
representative assets remained undecided in this snapshot. Next action: select
the production route, implement the integration and validate its consumer
contracts. No M0-M6 completion is inferred from packaging
or library compilation. Review: 2026-11-14.

### Documentation reconciliation 0039 (2026-11-01)

The user guide now distinguishes active guarded face-weight editing, the unpatched
Assimp limitations and the isolated v10 material/UNORM16 fixes. Design and review
records identify historical entries as snapshots rather than current acceptance.
The documentation review artifact follows the 38 implementation/tooling patches;
its manifest and verification evidence are kept outside the documentation to avoid
self-referential hashes.

Next action: finish documentation artifact verification and reconcile the
assembled tree, then audit remaining product requirements and available platform
capabilities. Renderer/FBX adoption, explicit mip transport, shader architecture
and selected-engine acceptance remain open; review packaging is not product
completion. The outstanding-decisions table lists the remaining choices. Review: 2026-11-14.

The dated progress records and older next-action lists below are historical;
use the newest status above together with the product-requirement audit and
outstanding-decisions table to determine remaining work.

### Pinned patch checkout correction 0038 (2026-11-01)

Native verification exposed CRLF conversion of embedded patch files in the
reconstructed checkout. Their normalized content matched, but exact hashes did
not, so the build refused them before compilation. Patch 0038 adds explicit LF
attributes for renderer and FBX patch files. All 17 pins now survive reconstruction
and an explicit Windows `autocrlf=true` checkout. Prior artifacts are unchanged.

Historical reconstruction requires command-local `core.autocrlf=false` on both
initial checkout and patch application, plus the documented attributes-first
sequence. No global configuration changes are needed. Details and failed-attempt
evidence are in `game-assets-review.md`.

The resumed native run now builds and passes in Windows Debug and Release. It
checks eight shared assembly instances across 97 poses, nested skin across 33
poses, and v10 FBX hierarchy/reflection plus 20,736 playback and adapter-basis
corners. Corruption controls pass and source/fixture hashes stay unchanged.
Evidence and the retained wrapper failures are documented in `game-assets-review.md`.

Next action: reconcile and package remaining general documentation.
These selected fixture checks do not establish production FBX or external-engine
acceptance. Review: 2026-11-14.

### FBX tooling review patch 0037 (2026-11-01)

Reconstruction exposed a packaging detail: patches 0036 and 0037 must apply their
`.gitattributes` change before their remaining content so Git accepts unified-diff
blank context lines. The verified two-step procedure is in `game-assets-review.md`;
all immutable patch contents and prior hashes are preserved.

Next action: build and exercise the packaged native probes, then package
the remaining general documentation. This change adds reconstruction evidence, not
native runtime or engine acceptance. Production integration, mip transport, shader
architecture and platform/engine gates remain incomplete. Review: 2026-11-14.

### Renderer tooling review patch 0036 (2026-11-01)

The packaged Debug CLI and both probe examples also build successfully. Three API
tests and all five added fixture checks pass. Both Vulkan and Direct3D 12 pass
2,000 RGBA comparisons, 100 unbounded LOD controls, 16,660 raster pixels and ten
raster sentinel controls each. Source/configuration hashes remain unchanged.
This verifies the diagnostic upload path, not viewer or engine acceptance.
A narrowly scoped
Git whitespace attribute preserves unified-diff blank context lines in the embedded
patches. See `game-assets-review.md` for the artifact hash and evidence paths.

Next action: package remaining FBX tooling, fixtures and documentation.
The normalized post-0036 inventory records 75 differing paths; no Rust source
differences remain against the packaged state. Production
adoption, mip transport, shaders and engine/platform gates remain open.
Review: 2026-11-14.

### Guarded mip review patch 0035 (2026-11-01)

The active mip generation, chain guards, authoring, sampled mask correction,
reporting and preview image accounting are now packaged as
`0035-guarded-atlas-mip-chains.patch` against the 34 frozen prerequisites. Required
fixtures, original PNG patterns, two debug entries and the scoped Windows/MSVC
Debug stack reserve accompany it. Renderer adoption and explicit-chain transport
are not included; unsupported consumers still refuse those chains.

The isolated state passes 133 Windows Release asset tests. Its Debug CLI builds,
validates both fixtures and emits all 12 mask-coverage records. Source/configuration
hashes remain unchanged; prerequisite/reverse patch checks pass. All 35 artifact
hashes verify. The assembled source now matches all 16 inventoried source paths
changed since 0032 (line endings normalized), while tooling/examples/docs remain
partially unpackaged. Details and limits are in `game-assets-review.md`; no engine,
interactive debugger, Linux or browser acceptance is inferred.

Next action: package renderer proposal tooling/probes and remaining
fixtures/debug configuration, followed by the isolated FBX work and current docs.
Production adoption, mip transport, shaders and engine/platform gates remain open.
Review: 2026-11-14.

### Asset hierarchy review patch 0034 (2026-11-01)

The asset hierarchy/motion layer is now frozen separately from evaluator metadata
as `0034-asset-hierarchy-and-motion.patch`. It depends on all 33 prior artifacts,
preserves nested ancestor groups and shared mesh identities, and computes motion
under the correct parent frames. The asset integration test withheld from 0033
is included here, where its implementation exists. Mip-related authoring changes
remain outside this artifact.

The fresh isolated Windows Release state passes 119 asset library tests and two
hierarchy integration tests, including sampled nested motion, reflected instances
and GLB node transforms. Source hashes are unchanged; strict prerequisite and
reverse application checks pass. `series-0034.sha256` verifies all 34 immutable
artifacts. Details and limitations are in `game-assets-review.md`. These tests do
not establish independent importer, engine, Linux/WASM or interactive acceptance.

Next action: package explicit atlas mip generation and its dependent
material/authoring/preview changes. Renderer/FBX adoption, mip transport, shaders
and engine/platform quality gates remain incomplete. Review: 2026-11-14.

### Reconstructed review series and patch 0033 (2026-11-01)

All 32 pinned review prerequisites now apply cleanly in a fresh checkout at the
recorded base commit. The resulting inventory records 137 differing current paths
with hashes, superseding the earlier retained-checkout estimate. The first new
artifact, `0033-evaluator-instance-hierarchy.patch`, contains only evaluator tree
metadata, placement/pattern handling, cache persistence and its focused tests.
The dependent asset-export test is reserved for the next artifact.

The isolated Windows Release candidate passes 49 assembly, one evaluator hierarchy,
one stored-mesh and one cache round-trip test; the last compiles with `game-assets`
enabled. Source hashes stay unchanged; strict patch apply/reverse checks pass.
`series-0033.sha256` adds the frozen artifact without altering any prior checksum.
Details, hash and evidence are in `game-assets-review.md`. This packages existing
implementation; it does not prove full CAD, Linux/WASM or engine acceptance.

Next action: prepare the dependent asset hierarchy/motion/export-test
artifact against this exact prerequisite, then continue the remaining packaging.
Production renderer/FBX adoption, mip transport, shader and engine/platform gates
remain open. Review: 2026-11-14.

### Milestone audit reconciliation (2026-11-01)

The product requirement audit below now distinguishes active code from isolated
renderer experiments. Source inspection confirms `src/main.rs` still restricts
asset export to GLB, `src/asset/glb.rs` refuses explicit mip chains, and the active
`src/view/asset.rs` refuses those chains and lacks the candidate alpha/extra-map
paths. `Cargo.toml` still uses the published renderer dependency. Candidate GPU
success therefore does not establish production preview or export support.

Rechecked the completed `gcad-extra-motion` and `renderer-extra-v3` verifier logs,
all 32 hashes in `target/game-assets-review/series-0032.sha256`, and all seven
renderer/adapter patch hashes against their README pins. The old review manifest
is intact, but does not package subsequent work. This reconciliation did not
rerun historical native FBX, CAD baseline, WASM or Linux checks; their earlier
results retain their original scopes. No milestone is marked complete.

Next action: package post-0032 source changes into additional immutable,
reviewable artifacts with prerequisite and validation records, preserving the
existing series. Outstanding decisions are listed in the audit below; unanswered
choices are not authorization to adopt dependencies or invent engine budgets.
Review: 2026-11-14.

### Five-map rigid and skinned animation (2026-11-01)

The extra-map candidate now uses `atlas-extra-motion.gasset` for both analytic
animation checks. It adds nonconstant occlusion/emissive maps and nonunit factors
to the existing motion fixture. Each of five slots must affect every one of five
poses at both 128 and 32 pixels. Tests preserve UVs, material factors and exact mip
bytes; both instances share slots while the second stays static. The skinned case
still builds a two-joint blended skin with nonidentity binds in memory and checks
the existing blended-affine shading-normal convention against a scalar reference.

All 140 candidate asset tests pass on Windows Direct3D 12 and Vulkan. Each rigid
and skinned case matches 87,040 rendered reference pixels exactly. Per-slot control
counts are ordered colour, normal, metallic/roughness, occlusion, emission:

| Case | Direct3D 12 controls | Vulkan controls |
| --- | --- | --- |
| Rigid | 7492, 6634, 6459, 7478, 7492 | 7492, 6610, 6439, 7482, 7492 |
| Skinned | 7125, 6246, 6107, 7116, 7125 | 7125, 6224, 6099, 7113, 7125 |

Motion changes 3,580 pixels in the rigid case on each backend and 2,607/2,606 in
the skinned case. The reference shares the renderer and texture preparation;
this proves the tested animation connection, not independent shading accuracy,
general CAD deformation quality or engine acceptance.

Evidence: `target/gcad-extra-motion-verified` and
`target/gcad-extra-motion-verify.*`. The candidate source hashes stay unchanged
during verification, and the refreshed patch pin is in the renderer README.
The active Debug CLI validates the new fixture; its VS Code configuration uses
that command, without a tested debugger UI session. Active dependencies and
Cargo.lock remain unchanged.

Next action: reconcile the older milestone audit with current artifacts
and prepare the remaining adoption/engine/backend decisions as concrete review
items. Renderer adoption, production FBX, mip transport, selected-engine quality
gates, shader support and platform/runtime acceptance remain incomplete.
Review: 2026-11-14.

### Authored five-map candidate adapter (2026-11-01)

The isolated gcad adapter now connects authored occlusion and emissive maps for
legacy images and explicit mip chains. It shares five slots across instances,
retains packed metallic/roughness, preserves all supplied extra-map bytes and
levels, and passes strength/emission factors separately in material keys. Explicit
occlusion is linear and emission is sRGB. Software snapshots refuse extra maps
before writing. Only the candidate removes the unsupported-map warning.

Image budget accounting includes both added slots. Tests check exact retained
payloads and image-count/CPU/GPU boundaries. For `atlas-extra-maps.gasset`, legacy
CPU accounting (including upload scratch) is 415,060 bytes and GPU texels 436,900;
explicit retained CPU is 757,760 and GPU texels 430,080. The unchanged per-frame
limits still exclude source assets, geometry, render targets, driver alignment
and overlapping/cached frames; these figures are not total process memory.

All 140 candidate asset tests pass on Direct3D 12 and Vulkan. The existing atlas
test now checks five shared slots and matches 17,472 constant-reference pixels
exactly on both backends. The new nonconstant fixture checks exact map bytes,
factors, encoding, budgets, source preservation and software refusal on both paths.
Occlusion controls change 2,869/2,867 legacy/explicit pixels on Direct3D 12 and
2,866/2,866 on Vulkan; emission changes 2,874 for each. Each control must affect
both tested image sizes. Existing alpha, mask and three-map rigid/skinned tests
remained green in that run; the subsequent five-map animation run is recorded above.

Evidence: `target/gcad-extra-v2-verified` and `target/gcad-extra-v2-verify.*`.
Reproduction and patch pin are in the renderer README. The active Debug CLI
validates the fixture, and its VS Code entry uses that command (debugger UI not
tested). Source hashes remain unchanged during verification; active dependencies
and Cargo.lock remain unchanged. The active preview still has its prior limits.

The five-map animation check above completes that follow-up. Renderer adoption,
explicit mip transport, software/tracing support and engine acceptance remain.
Review: 2026-11-14.

### Candidate occlusion and emission bindings (2026-11-01)

The isolated renderer proposal now supports occlusion and emission texture slots
alongside colour, normal and metallic/roughness. They participate in batching,
GPU binding and the shortest explicit mip-chain limit. Occlusion reads linear red
and attenuates ambient lighting by `1 - strength + strength * red`; direct light,
specular highlights and emission remain unaffected. Emission multiplies the
linear material factor by sRGB-decoded RGB. Texture alpha is ignored in both slots.
Missing maps preserve existing behavior. Mixed explicit/legacy bindings, invalid
extra-slot indices, encoding mismatches and invalid occlusion strength are refused.
The expanded draw uniform fits its existing 256-byte stride with a compile-time
guard. Software and tracing explicitly refuse these maps.

Windows Direct3D 12 and Vulkan pass the existing three API tests, three atlas/
integration tests and two alpha tests, plus two new extra-map tests. Each backend
compares 36,864 pixels per legacy/explicit path with map-free references across
nine occlusion value/strength combinations. Maximum channel error is zero except
Vulkan explicit images, where it is one. Occlusion and emission controls change
7,056 and 15,876 pixels respectively. A short occlusion chain bounds emission to
level zero; allowing its poisoned lower level changes 140 pixels on each backend,
with bounded-reference errors zero/one. Contract tests cover wrong encodings,
mixed paths, missing indices, strength limits and unsupported-mode refusal.

Evidence: `target/renderer-extra-v3-verified` and
`target/renderer-extra-v3-verify.*`. Preparation, verification and patch pin are in
`tools/game-assets-renderer/README.md`. Candidate source hashes remain unchanged
during checks; active dependencies and Cargo.lock are unchanged. This is not
engine PBR parity, indirect specular occlusion, software/tracing support or an
active gcad preview feature. The subsequent adapter work is recorded above.

The authored connection and budget checks are now covered by that candidate.
Renderer adoption, mip transport and engine acceptance remain pending.
Review: 2026-11-14.

### Supersampled mask reference limits (2026-11-01)

The candidate mask diagnostic now box-averages level-zero-only renders at 4x and
8x output dimensions into fractional occupancy. It checks the block mapping
against an asymmetric synthetic image and reports area-sum and spatial L1 errors
for low-resolution base/raw/corrected masks at all 24 views. The original binary
occupancy results are unchanged. Values below sum pixel-equivalents across views:

| Backend | 8x area error: base / raw / corrected | 8x spatial L1: raw / corrected | 4x-to-8x area / spatial drift | Improved / regressed / tied at either reference scale |
| --- | --- | --- | --- | --- |
| Direct3D 12 | 792.28125 / 785.28125 / 788.28125 | 1329.71875 / 1330.71875 | 96.21875 / 154.96875 | 0 / 3 / 21 |
| Vulkan | 783.515625 / 775.515625 / 780.515625 | 1327.109375 / 1328.140625 | 96.390625 / 153.796875 | 0 / 5 / 19 |

Reference drift exceeds the correction's 3/5 pixel-equivalent effect. Binary
low-resolution MSAA occupancy and fractional supersampled occupancy also use
different coverage conventions, as the base error exposes. The shaded RGB
contrast measurement therefore cannot establish fractional-area acceptance or
justify a production threshold. Keep correction opt-in; do not optimize it to
these diagnostic numbers. A proposed gate in `game-assets-design.md` requires
matching coverage conventions, a converged reference, per-view area and spatial
budgets, and selected engine/camera settings before acceptance.

All 139 candidate tests pass on both Windows backends; source/fixture hashes stay
unchanged during verification. Evidence: `target/gcad-mask-super-verified` and
`target/gcad-mask-super-verify.*`. The refreshed proposal patch is pinned in the
renderer README. Active dependencies and Cargo.lock remain unchanged.

Remaining: selected engine/assets/budgets, renderer adoption, mip transport
and engine acceptance. Next action: implement and verify the missing
occlusion/emissive material preview slots in the isolated renderer proposal.
Review: 2026-11-14.

### Projected mask occupancy characterization (2026-11-01)

The isolated alpha candidate now compares corrected and raw authored mask chains
at sizes 128, 64, 32 and 16, yaw angles 0.6, -0.6 and 2.4, and zooms 1 and 1.7.
The paired `atlas-mask-raw.gasset` fixture differs only by omitting `coverage=mask`.
Tests verify identical geometry, UVs, material factors, base bytes, colour RGB and
other maps. The reference clamps all maps to level zero; a lower-mip zero-alpha
control proves that the mip path is sampled. Fresh frames prevent cache aliasing.

Readback has opaque alpha, so occupancy means maximum RGB greater than 3 against
a verified black empty-scene background. Partial MSAA pixels count as occupied.
The base-only reference can alias; this is not fractional projected area or an
independent rasterizer oracle. Report both count error and spatial disagreements:

| Backend | Improved / regressed / tied views | Sum absolute count error, raw → corrected | Spatial disagreements, raw → corrected | Corrected / zero-alpha control changes |
| --- | --- | --- | --- | --- |
| Direct3D 12 | 2 / 1 / 21 | 7 → 6 | 9 → 10 | 3 / 1530 |
| Vulkan | 2 / 3 / 19 | 8 → 9 | 10 → 11 | 5 / 1715 |

All 139 candidate asset library tests pass on each backend. Source/fixture hashes
remain unchanged during verification; active dependencies and Cargo.lock remain
unchanged. Evidence: `target/gcad-mask-projected-verified/asset-{dx12,vulkan}.*`
and `target/gcad-mask-projected-verify.*`. The active Debug CLI validates the raw
fixture. A local VS Code entry uses that command; the debugger UI was not tested.

The sampled UV correction does not guarantee improvement in projected occupancy;
it remains opt-in. Renderer adoption, explicit mip transport, production backend
and engine acceptance are still open. The supersampled comparison and proposed
camera-space acceptance gate are now recorded above; neither selects production
thresholds. Review: 2026-11-14.

### Opt-in sampled mask coverage correction (2026-11-01)

`atlas ... mips=N coverage=mask` now enables per-chart correction for masked
materials with a base-colour map. Each lower level uses 1,024 deterministic
UV-area samples, bilinear alpha sampling and the authored factor/cutoff to match
the rebaked level-zero pass count as closely as a uniform alpha multiplier
permits. Only lower-level base-colour alpha and owned gutters change; RGB, other
maps, factors, geometry and the base level are preserved. Default baking is
unchanged. Work is preflighted at one Mi triangle references and 16 Mi sample
evaluations in addition to the existing bake budgets and footprint checks.

`asset-report` exposes per-material chart/level records with target, before and
after counts and applied scales. It retains residuals rather than claiming exact
coverage. Tests cover quantization, factor/cutoff boundaries, unattainable flat
coverage, source/map preservation, work limits, authoring validation and reporting.
All 133 active Windows Release asset library tests pass. The Debug CLI builds,
validates `atlas-mask-coverage.gasset` and emits its 12 records. One chart improves
from 611 to 614 against a target of 615 at level one, and from 598 to 615 at level
two. The analytic fixture improves 100/1,024 to its 102/1,024 target.
Evidence: `target/mask-coverage-tests.*`, `target/mask-coverage-debug-build.*`,
`target/mask-coverage-check.*` and `target/mask-coverage-asset-report.*`.
The refreshed isolated alpha adapter also passes all 138 tests on Direct3D 12
and Vulkan (`target/gcad-mask-coverage-verified` and
`target/gcad-mask-coverage-verify.*`). Existing GPU reference comparisons remain
green; this regression run does not characterize the new corrected mask fixture.

This metric is sampled UV area relative to the rebaked base, not original source
coverage, a projected silhouette, continuous area or trilinear coverage. Zero
support lost during filtering cannot be recreated, and equal coarse alpha values
can make targets unattainable. The projected occupancy characterization above
now records GPU behavior and regressions on both backends. Renderer adoption and
explicit mip transport remain pending. Next action: improve the projected
reference as described above.
Review: 2026-11-14.

### Candidate texture alpha and mask preview (2026-11-01)

Two additional unapplied patches extend the renderer and gcad adapter with
opaque, mask and blend modes plus a material alpha factor. Opaque ignores texture
and factor alpha. Masks compare their product with the cutoff and discard before
depth writes; equality survives. Blends multiply that product by look opacity
and use the transparent pass, including when the uniform opacity is one. The
adapter applies the material factor once and preserves a warning about batch
sorting. Software and tracing paths explicitly refuse material alpha.

Renderer tests pass on Direct3D 12 and Vulkan for legacy images and explicit
chains: cutoff boundaries, factor multiplication, opaque-ignore behavior,
foreground/background visibility and unsupported-mode refusal. The existing
three renderer integration tests and three API tests remain green. All 135
candidate asset tests pass per backend, including authored mode/factor controls
and the existing exact reference comparisons. `atlas-alpha.gasset` validates
with the active Debug CLI; its new VS Code entry uses that verified command.
The debugger UI was not launched. Evidence: `target/renderer-alpha-verified`,
`target/renderer-alpha-verify.*`, `target/gcad-alpha-verified`,
`target/gcad-alpha-verify.*` and `target/atlas-alpha-check.*`.

The additions are `renderer-alpha.patch` and `gcad-alpha-adapter.patch`, applied
after their existing proposal patches. Reproduction scripts and SHA256 pins are
in `tools/game-assets-renderer/README.md`. Active renderer dependencies and
Cargo.lock remain unchanged. This is candidate raster support, not adoption,
engine parity or order-independent transparency. Intersecting transparent
surfaces can sort incorrectly, and alpha-mask coverage is not preserved across
mip levels. Software/tracing alpha and occlusion/emissive-map preview remain open.
Next action: implement and verify coverage-aware masked-material mip
generation. Renderer adoption, FBX selection, transport, engine acceptance and
other plan requirements remain open. Review: 2026-11-14.

### Textured skinning and separate map controls (2026-11-01)

The isolated preview suite now binds the authored atlas-motion geometry to two
in-memory joints with nonidentity inverse binds. Every vertex blends both joints
with weights varying along Z. One joint rotates, translates and scales
nonuniformly while the other stays fixed; the second mesh instance remains
unskinned. Independent scalar calculations verify weighted positions and the
inverse transpose of the blended linear transform at five poses. UVs and all
prepared mip bytes must remain unchanged.

Rigid and skinned cases now neutralize colour, normal and packed metallic/roughness
maps separately. Every slot must affect every tested pose and image size. All
134 candidate asset tests pass on Direct3D 12 and Vulkan. Each case compares
87,040 pixels with its analytic-pose reference and has zero maximum byte error
on both backends. Skinned colour/normal/scalar controls change 7,125/6,385/6,166
pixels on Direct3D 12 and 7,125/6,381/6,176 on Vulkan; motion changes 2,605 on each.
These controls establish visible map use, not identical cross-backend rendering.
Evidence: `target/gcad-skin-textures-verified` and
`target/gcad-skin-textures-verify.*`. The patch and verifier pins are updated.

The new binding is constructed in the test, not persisted as a new weight file.
The reference uses the documented blended-affine shading-normal convention;
it is not a recomputed geometric normal or independent shading/baking oracle.
This small two-joint case does not establish general deformation quality or
continuous error bounds. Active dependencies and Cargo.lock remain unchanged.
Next action: address texture-alpha and masked-material preview in the
isolated renderer proposal, with explicit limits for software/tracing paths.
Renderer adoption, FBX selection, transport, engine acceptance and other plan
requirements remain open. Review: 2026-11-14.

### Nonconstant textures with rigid animation (2026-11-01)

`atlas-motion.gasset` uses spatially varying colour, negative-Y normal and packed
scalar maps on a three-level atlas. One instance translates, rotates 90 degrees
and scales nonuniformly; another shared instance stays fixed. The candidate test
compares five sampled poses against closed-form transformed rest geometry,
including inverse-transpose normals. UVs, map slots and mip bytes must remain
unchanged. A flattened-colour control and consecutive-pose comparisons require
visible texture and motion effects.

All 133 isolated asset tests pass independently on Direct3D 12 and Vulkan.
At 128 and 32 pixels square, all 87,040 animation-reference pixels match exactly
per backend. Colour flattening changes 7,492 pixels and motion changes 3,580 on
each backend. The three prepared base maps have 290, 277 and 141 distinct RGBA
values. Initial fixture controls caught invalid combined key syntax and too-low
atlas density; these were corrected without relaxing assertions. Evidence:
`target/gcad-animation-dense`, `target/gcad-animation-dense.*`; earlier failures
remain in `target/gcad-animation-verified` and `target/gcad-animation-corrected`.

The active Windows Debug CLI exposed a separate startup stack overflow, also
reproduced with `--help` and the existing mip fixture. The feature-enabled MSVC
Debug binary now reserves 8 MiB instead of 1 MiB; the linker setting applies only
to that configuration and reserves address space rather than committing it all.
The normal Debug build, help and both fixture checks pass; unsupported snapshots
still return an error without output. The new VS Code validation entry uses this
build command; the debugger UI was not launched. Evidence:
`target/atlas-motion-debug-stack-build.*`, `target/atlas-motion-fixed-*` and
`target/atlas-motion-refusal.*`.

The render reference independently verifies rigid poses, not shading mathematics
or texture baking, and does not establish skinning, alpha or general material
acceptance. Active renderer dependencies and Cargo.lock remain unchanged.
Next action: extend these controls to textured skinned deformation and
independent normal/scalar-map effects. Renderer adoption, FBX selection, transport,
engine validation and other plan gates remain open. Review: 2026-11-14.

### Explicit Direct3D 12 and Vulkan candidate verification (2026-11-01)

Both isolated test harnesses now restrict renderer creation to the requested
backend and assert the actual adapter backend. The Windows verifiers run separate
Direct3D 12 and Vulkan processes, check backend-specific success markers and
restore the caller's environment. Missing adapters or unexpected backends fail;
they cannot silently skip or substitute another API.

The candidate API passes its three tests. All three renderer integration tests
pass separately on each backend: 9,216 material-reference pixels and 17,472 real
atlas-reference pixels match exactly, with supplied-level and shortest-chain LOD
controls passing. The complete isolated gcad asset suite passes 132 tests on
each backend. Its 17,472 authored reference pixels match exactly on each; the
deliberately altered mip control changes 328 pixels on Direct3D 12 and 363 on
Vulkan. This is per-backend reference validation, not a claim of identical
cross-backend output for arbitrary images.

Evidence: `target/renderer-dual-verified`, `target/renderer-dual-verify.*`,
`target/gcad-preview-dual-verified` and `target/gcad-preview-dual-verify.*`.
The review patches and reproduction scripts are updated; active dependencies and
Cargo.lock remain unchanged. Windows headless readback is verified; Linux,
browser runtime, interactive controls and engine imports remain unverified.
Next action: add representative nonconstant textures and animated poses
to the candidate appearance checks. General material acceptance, frame/cache
lifetime budgets, transport and broader plan requirements remain open. Renderer
adoption and FBX backend selection remain open decisions.
Review: 2026-11-14.

### Preview image budgets and validation measurement (2026-11-01)

Preview preparation now preflights aggregate image payloads before conversion:
4,096 image slots, 256 MiB CPU payload and 256 MiB GPU texels per frame. CPU
accounting includes all explicit levels, compatibility base copies and peak
legacy mip-generation scratch. Legacy GPU estimates conservatively include a
full chain through 1x1. Materials count once per entry, independent of instance
count; metallic/roughness use one packed slot. Checked arithmetic and exact
boundary tests cover overflow, aggregate limits and NPOT dimensions without
allocating oversized images. The authored fixture's prepared images retain
454,656 bytes and upload 258,048 bytes across three shared slots.

All 130 active Windows Release asset library tests pass. The isolated adapter
passes 132 tests with the same 17,472 exact GPU reference pixels and 328 changed
supplied-level control pixels. A diagnostic measured 2,000 validations of the
196,608-byte fixture base in 3.7801 ms and 200 validations of a 16 MiB base in
62.8032 ms. These are single-run local microbenchmarks, not frame-rate guarantees.
A last-byte corruption control confirms validation still scans the complete
base image. Evidence: `target/preview-budget-tests.*`,
`target/gcad-preview-budget-verified` and `target/gcad-preview-budget-verify.*`.

Limits cover prepared RGBA payloads, not total memory: source assets, geometry,
driver alignment/staging, render targets and overlapping/cached frames remain
outside this accounting. The active renderer dependency and lockfile remain
unchanged. Next action: verify the candidate renderer and authored adapter
on Vulkan, then extend nonconstant/animated material coverage. Frame/cache lifetime
budgets and validation optimization remain open. Renderer adoption and FBX backend selection remain open. Review: 2026-11-14.

### Isolated authored-chain preview adapter (2026-11-01)

The reviewable `tools/game-assets-renderer/gcad-preview-adapter.patch` now connects
authored immutable chains to the proposed renderer. The adapter preserves every
prepared level, uses sRGB only for colour, shares three image slots between the
two fixture instances and validates the resulting scene. Unsupported software
snapshots return an error without creating output. GLB transport remains refused;
existing alpha, tangent, occlusion and emissive-map limitations remain explicit.
The active gcad renderer dependency and lockfile are unchanged.

All 129 Windows Release asset library tests pass in a freshly prepared isolated
copy. Direct3D 12 readback matches all 17,472 reference pixels exactly across
128/32/8 sizes. A deliberately green supplied lower-level control changes 328
pixels. That control initially exposed an ineffective saturated reference test:
the fixture has uniform emission=1. Lowering test exposure to 0.2 makes texture
differences observable without changing authored materials. The preparation
script also creates the local target directory required by an existing test.
Evidence: `target/gcad-preview-reproduced` and
`target/gcad-preview-reproduced.*`; earlier failed controls remain in
`target/gcad-preview-verified`. Reproduction and patch pin are documented in
`tools/game-assets-renderer/README.md`.

This verifies the isolated adapter, not active adoption or general material
acceptance. Next action: bound aggregate preview image memory and measure
validation cost before broadening candidate-renderer coverage to Vulkan and
nonconstant/animated materials. Renderer vendoring and FBX backend decisions remain open. Export transport, engine acceptance, alpha
coverage, normal variance and broader plan requirements remain open.
Review: 2026-11-14.

### Authored immutable mip-chain storage (2026-11-01)

`atlas ... filter=linear mips=N` now retains exactly levels 0..N on the derived
material, with N in 1..13. Omitted `mips` keeps existing behavior. The immutable
chain includes its baked geometry and base material; clones share it through Arc
and material equality includes the chain. Validation rejects changes to base
pixels, factors, sampler, conventions/provenance or prepared primitive geometry,
including UVs. This conservatively requires rebaking after incompatible edits.

`atlas-mips.gasset` checks successfully with two instances sharing one material,
mesh and three-level chain across six maps. All 127 Windows Release asset tests
pass, including sharing, changed-base/UV refusal and invalid authoring options.
The feature-enabled Release CLI builds. CLI check succeeds; GLB export and the
active preview refuse unsupported explicit chains and leave no output files.
This prevents silent loss or automatic regeneration before integration exists.
Evidence: `target/authored-mips-tests.*`, `target/authored-mips-build.*` and
`target/authored-mips-cli`.

This is authored storage and validation, not completed preview/export support.
The retained base and geometry copies are outside the RGBA bake budget; existing
texture reports still exclude mip payloads. Exact geometry guards may reject
otherwise safe derived edits until explicit remapping is implemented. Next action: prepare and test the isolated gcad preview adapter against these authored
chains and the candidate renderer. Renderer adoption remains undecided. Transport, general material/platform acceptance and broader plan gates
remain open. Review: 2026-11-14.

### Prepared atlas maps and MSAA edge correction (2026-11-01)

Added shared preview conversion for a material and every explicit mip level:
linear-light RGB tint, preserved alpha bytes, normal-convention conversion and
metallic/roughness packing. Scalar factors and normal scale stay separate. The
existing single-level preview uses this same helper. All 126 asset library tests
pass, including source/chain immutability and exact conversion at every level.

`game_asset_renderer_fixture` exports a bounded new-file-only binary fixture from
gcad's actual atlas packer, mip baker and preview conversion: two charts, three
map slots and four levels. The candidate renderer compares this with independent
constant-material chart references at 128, 32 and 8 pixels square. This exposed
an MSAA-edge regression: pixel-centre UV interpolation can extrapolate outside a
partially covered triangle, beyond its chart sampling footprint. The original
test failed with maximum byte error 18. The isolated renderer patch now provides
a separate centroid UV attribute for explicit materials, retaining legacy UVs.

With that correction all 17,472 fixture pixels match exactly on Direct3D 12.
The full candidate viewer3d suite passes 24 tests, including the supplied-level
and shared-LOD controls and existing viewer regressions. The fixture overwrite
guard also refuses an existing output without modifying it. Evidence:
`target/renderer-adapter-tests-final.*`, `target/renderer-atlas-diagnostic.*`
(failure), `target/renderer-centroid-verified` and
`target/renderer-centroid-regression.*`. The integration patch and preparation
script include the fix and test; gcad's active renderer dependency is unchanged.
Both VS Code example build commands succeed in Debug. Debug/Release fixture
bytes are identical, and the Debug standalone GPU checks pass on Vulkan and
Direct3D 12 (`target/renderer-atlas-debug-gpu`). The debugger UI itself was not
launched. Cargo.lock remains unchanged.

Next action: prepare authored-chain storage and the explicit-image preview
connection in an isolated gcad adapter patch. The vendoring decision remains open. General nonconstant/animated material acceptance, Vulkan
candidate-renderer coverage, export transport, alpha coverage, normal variance
and broader requirements remain open. Review: 2026-11-14.

### Isolated renderer scene and shader integration (2026-11-01)

The unapplied renderer proposal now includes scene-indexed explicit images,
base-pixel consistency checks, mixed-path and colour/numeric encoding refusal,
real GPU batch bindings and an sRGB shader path that avoids double decoding.
The shared sampler uses the shortest present map chain's safe LOD bound. Legacy
batches retain their path. Unsupported tracing/missing GPU is reported by the
interactive path; software rendering of explicit chains is explicitly refused.

The complete candidate compiles after ordinary Cargo resolution filled the cache
entry missing in the earlier offline attempt. Three API tests and two candidate
scene/renderer tests pass. On Direct3D 12, 9,216 pixels match the legacy constant
material with zero byte error, including 2,213 material pixels. Supplied green
lower levels change all 2,213 material pixels; shortening the normal map to its
base level forces LOD zero and restores the reference exactly. This proves that
the real renderer consumes supplied levels and honors the shared bound.

Next action: exercise a real atlas chain through the candidate renderer
and prepare the gcad preview adapter changes for review. Authoring, per-level
tint/packing, export transport, broader renderer/platform validation and the
remaining game-asset requirements are still open. Review: 2026-11-14.

### Reviewable explicit-image renderer API (2026-11-01)

Added `tools/game-assets-renderer/explicit-image-api.patch`, an unapplied public
API addition for pinned egui_bench 0.9.2. Its `ExplicitImage` owns validated
contiguous levels and uploads them with explicit sRGB/linear formats and a bounded
clamp/linear sampler. It does not change legacy renderer behavior. Preparation
verifies the cached archive hash, applies the patch in a temporary isolated tree
and checks that its module matches the source compiled by the GPU probe.

Two constructor tests pass. Both Vulkan and Direct3D 12 pass the existing compute
and raster checks using the proposed module's upload implementation and sampler.
Evidence: `target/renderer-image-api-tests.*`, `target/renderer-image-api-gpu` and
`target/renderer-image-api-candidate-final`. The complete candidate crate has not
been compiled: offline resolution stops at an uncached `ignore` dependency. That
limitation is distinct from the passing module/GPU checks. No network fetch or
active dependency/lockfile change was made.

Inspection also found that the viewer shader manually decodes sampled RGB, which
must be bypassed for explicit sRGB views. Its three material maps share one sampler;
mixed chains need a validated common LOD bound or separate samplers. Software
preview and tracing currently consume legacy images. These integration requirements
are recorded in the renderer proposal README. Next action: extend the
isolated proposal with scene/batch association and shader-path selection, then
verify the real renderer path. Adopting a vendored dependency remains undecided. Full candidate compilation, viewer integration, export transport
and broader acceptance gates remain open. Review: 2026-11-14.

### Rasterized adjacent-chart mip checks (2026-11-01)

Extended the GPU probe with two red/blue charts packed from disconnected UV
triangles into a 128-by-64 atlas. Four explicit levels pass the ownership guard;
the next level must fail specifically for overlapping chart footprints. The
fragment shader uses ordinary `textureSample` with implicit derivatives, and
renders each chart at 64, 8, 2 and 1 pixels square. With maximum LOD three and
anisotropy one, chart colours remain exact. Without the LOD bound, the ten pixels
at the two smallest viewport sizes expose poisoned green extra levels.

Windows Release Vulkan and Direct3D 12 both pass 16,660 raster readback pixels
per backend, alongside the existing 2,000 compute RGBA results and 100 explicit
LOD controls. The reproduction script now requires both sets of coverage.
Evidence: `target/atlas-mip-raster-final` and `target/atlas-mip-raster-vulkan.*`.
No renderer dependency, source CAD, production exporter or Cargo.lock change.

This establishes bounded implicit LOD behavior for these flat diagnostic charts,
not general asset appearance, oblique/anisotropic footprints, alpha coverage or
engine acceptance. Next action: prepare a minimal renderer image/sampler
API patch for review, using these checks as its acceptance tests. Applying a
vendored renderer remains an open decision. Material-authoring and
export transport integration and broader plan gates remain open. Review: 2026-11-14.

### Explicit mip GPU readback (2026-11-01)

Added `game_asset_mip_gpu`, using the existing wgpu dependency to upload three
explicit levels from a 31-by-17 atlas chain. A compute shader samples 100 chart
positions at LODs 0, 1, 1.5, 2 and 20. Readback checks 2,000 RGBA results against
a separate CPU interpolation reference over the baked bytes. Colour uses an sRGB
texture format; scalar data and alpha remain linear. A poisoned texture with
green extra levels stays correct with maximum LOD 2; an unbounded control exposes
the sentinel for all 100 excessive-LOD requests.

This verifies explicit-level GPU sampling of one chart, not automatic derivative
LOD selection, rasterized appearance, adjacent-chart GPU isolation, anisotropic
sampling or engine acceptance. The library's separate multi-chart CPU checks
remain relevant. Next action: extend the GPU fixture to adjacent charts
and a rasterized minification case while the renderer-vendoring decision remains open. Viewer integration, export transport and broader plan
gates remain open. Review: 2026-11-14.

### Explicit mip-chain storage and renderer contract (2026-11-01)

Added `Atlas::rebake_chain` and immutable-access `MipChain` storage for exactly
levels zero through an explicit maximum. Layout collisions reject the complete
request before any destination maps are baked. Aggregate RGBA output is limited
to 256 MiB across all levels, in addition to the existing per-level work budgets.
`max_lod()` exposes the required runtime bound; automatic levels and anisotropic
sampling are not interchangeable with this chain.

All 125 Windows Release asset tests pass. Added coverage for non-power-of-two
dimensions down to 1-by-1, level-zero-only storage, exact equality with independent
original-source level bakes, collision refusal and aggregate-memory refusal when
the base map alone would fit. Evidence: `target/atlas-mip-chain-tests.*`.

The locked viewer dependency's image type cannot carry explicit levels. Its GPU
code generates byte-averaged levels and uses one repeat/linear/aniso sampler.
Vendoring a pinned renderer to extend its image/sampler interface was raised with
the user as a dependency-maintenance decision; it is not approved or implemented.
No material authoring, viewer or export behavior changed. Next action:
verify explicit upload and bounded sampling with the existing wgpu dependency,
then connect the chosen renderer interface. Export transport, alpha coverage, normal variance, engine/platform
acceptance and the broader plan remain open. Review: 2026-11-14.

### Chart-isolated CPU mip levels (2026-11-01)

`Atlas::rebake_mip(material, level)` now bakes an explicit lower level directly
from the original maps, preserving UV transforms and linear-light area filtering.
Each chart reserves the complete rectangular bilinear sampling footprint at the
requested dimensions. A bounded per-pixel ownership map rejects collisions before
allocating output maps. Existing byte/work budgets apply. Levels outside the image
chain are rejected; rectangular dimensions stop shrinking at one pixel.

Windows Release: all 124 asset tests pass. Two new tests cover 40,804 bilinear
samples across separate red/blue chart bounds at safe levels, collision refusal,
invalid level requests, original-material preservation and stripe colour/alpha/
scalar filtering through a rectangular chain to 1-by-1. A control bypasses the
ownership check and turns a sample inside the red chart blue. Evidence:
`target/atlas-mips-test.*` and `target/atlas-mips-regression.*`. Cargo.lock is unchanged.

This is a CPU library step, not complete mip support: each result is a standalone
material with mip sampling disabled. Material level storage, export transport,
viewer upload/sampling and maximum-LOD handling remain. Rectangular footprints
are conservative and cover bilinear, not anisotropic sampling. Alpha-coverage,
normal-variance and engine/platform acceptance remain open. Existing CLI atlas
export still writes one level. Next action: inspect the renderer image and
sampler interfaces, then connect explicit level storage and bounded LOD sampling
without enabling an unsafe automatically generated chain. Review: 2026-11-14.

### Independent minified-atlas PNG verification (2026-11-01)

Added reusable source/atlas fixtures with six map slots, 25%-coverage stripes and
a flat normal map. The native reader independently decodes five exported PNGs and
checks all 20,480 RGBA pixels: linear-light colour/emissive RGB is 137, linear
alpha and scalar channels are 64, packed metallic/roughness channels remain
distinct, and normal pixels remain `(128,128,255,255)`. Six charts cover 1,702
pixels per map, including 720 gutter pixels; unused atlas pixels are also checked.
Geometry, winding, UV density and output sampler settings are checked separately.

Chart-safe mip generation, alpha-coverage preservation, normal-variance treatment,
engine appearance and broader production/platform gates remain open. Next action: inspect atlas mip storage/export contracts and add a failing chart-boundary
mip regression before implementing chart-safe levels. Production FBX backend and
consumer selection remain open decisions for the user. Review: 2026-11-14.

### Atlas minification footprint filtering (2026-11-01)

A new reduction regression exposed aliasing in atlas rebaking: point-centred
bilinear samples turned a 25%-coverage stripe texture entirely black. Linear
rebaking now averages source texel coverage over the destination footprint on
minified axes, with bilinear interpolation on other axes. Colour/emissive RGB
filter in linear light; alpha and numeric maps filter numerically. Wrapped chart
padding and fractional texel coverage use the same filter. Nearest behavior,
material factors and source maps remain unchanged; output mipmaps stay disabled.

Before destination allocation, filtering checks conservative per-axis estimates
up to 65,536 taps and total work up to 256 Mi source-pixel taps across charts/maps.
Excessive requests fail rather than silently undersampling. The existing 256 MiB
output-map budget remains separate from this work estimate.

Windows Release: 122 asset tests pass, including the formerly failing stripe case,
fractional clamp/repeat/mirror boundaries, both axes, 2D reduction, colour/alpha
handling and work-limit refusal. The feature-enabled executable builds and the
existing atlas fixture checks/exports successfully. Evidence:
`target/atlas-minify-red.*`, `atlas-minify-green.*`, `atlas-minify-regression.*`,
`atlas-minify-build.*` and `target/atlas-minify-cli`. The old failure is retained.

This improves single-level resizing; it does not supply chart-safe mip generation,
alpha-coverage preservation, normal-variance treatment or engine acceptance.
Next action: add a reusable minified-texture fixture and independently
decode its exported PNGs to verify area coverage, colour space and packed numeric
channels. Broader production/platform gates remain open. Review: 2026-11-14.

### Nested hierarchy and skin through the FBX candidate (2026-11-01)

The isolated v10 candidate passes the nested skin fixture in Debug/Release with
all 19 authored nodes and parent associations, four reflected rigid mesh instances,
36 raw/imported weighted corners, 96 ordered faces and 20,736 sampled position and
basis corners. Maximum position error is `4.30214e-9 m`; maximum adapter-basis
component error is `1.19465e-7`. These compare the independently loaded FBX against
the source GLB/imported scene, whose analytic GLB checks are recorded below.

The first audit stopped at a test assumption requiring two bones. The audit now
selects a skinned mesh independently of mesh order and supports a one-bone identity
corruption using an unrelated root node. All three weight/bind/identity controls
pass, as does a new parent-link control. The full reflected, mirrored-UV textured
scene regression remains passing in both builds. The v10 writer patch is unchanged;
original dependency checkouts are clean. No production FBX integration is claimed.

Next action: add a minification-sensitive atlas regression and improve
rebake filtering from its evidence; keep chart-safe mip generation explicit as a
separate unfinished requirement. Backend/consumer selection and broader production
gates remain open. Review: 2026-11-14.

### Nested-body binding and explicit rebinding (2026-11-01)

Added `nested-skinned.gasset`, binding only `first/left.a` to one translating joint
with a nonidentity rest transform. Its reflected repeated copy stays unskinned.
The preparation script creates the new weight resource only when absent.

The independent Assimp reader checks the original and rebound GLBs in both
Debug/Release: 33 analytic world poses, nonidentity inverse bind, complete rigid
weights and the unskinned reflected copy. Maximum position error is `2.0825e-9 m`
against a `1e-7 m` limit. Altered weight, inverse bind, parent and animation key
controls are rejected and restored. The earlier 97-pose mechanical hierarchy
fixture still passes both builds. Evidence: `target/nested-skin-rebound-final` and
`target/nested-skin-native`. Pinned native dependency checkouts remain clean.

This verifies rigid binding, hierarchy guards and explicit rebinding; it is not
blended deformation quality, engine shading, or cross-platform acceptance. Next
action: carry the nested hierarchy and skin fixture through the isolated
FBX candidate and check group association, inverse binds, world poses and the
unskinned reflected copy independently. Production backend/consumer selection and
the broader release gates remain open. Review: 2026-11-14.

### Independent nested assembly GLB import (2026-11-01)

Added reusable `nested-assembly.gasset` and its `nested/` CAD sources: a 2 by 3 by
4 mm block, single-body subassembly, hinged pair and repeated top-level pairs.
The fixture includes oblique placement, a reflected/nonuniform asset copy and a
mechanical hinge clip. CAD checks confirm 24 mm3 block volume, expected bounds,
no assembly overlaps and collision-free 13-step hinge sweeps. Software renders of
the part and all three assemblies were inspected; these are CAD views, not engine
material or animation acceptance.

The native GLB probe's `--nested-assembly` mode independently reads group parents,
shared mesh references and four 25-key tracks with Assimp. Expected world positions
come from analytic translations, Rodrigues rotation and hinge rotation rather
than gcad's transform implementation. Debug/Release pass eight body instances at
97 poses, with maximum position error `1.33881e-7 m` (about `0.000134 mm`). Limits
are `1e-7 m` at keys and `1e-6 m` between keys. Wrong parent, group placement,
reflection and animation-key controls are each rejected and restored.

Next action: exercise binding, stale-hierarchy refusal and explicit
rebinding for a nested body, then independently verify its skinned world poses.
Broader production and release gates remain open. Review: 2026-11-14.

### Nested assembly asset export and motion (2026-11-01)

Asset expansion now consumes the retained evaluator tree. Assembly ancestors
become meshless groups with trailing-slash paths such as `first/left/a/`; mesh
nodes retain existing `INSTANCE/BODY` names. Terminal single-body parts use their
existing mesh node. Selected-body imports retain ancestors and omit unrelated
branches. Group and mesh transforms are relative to their parent; mesh-local
geometry, sharing and face-map association remain intact.

Mechanical motion baking traverses descendant bodies and converts CAD poses into
their static parent's coordinates. Sampled error checks compose the parent frame
back into world coordinates. Explicit authored keys still use parent-local
coordinates, so keys for formerly flattened nested assemblies need review.
Changed skin hierarchy stamps require explicit rebinding rather than reuse.

Windows Release verification with `game-assets`: 119 asset library tests and both
hierarchy integration tests pass. The nested fixture checks repeated instances,
single-body renaming/selection, face maps, oblique assembly rotation, a reflected
nonuniform asset instance, and 49 mechanical poses. Key positions agree within
`1e-8 mm`; midpoint checks use the fixture's `0.01 mm` budget. JSON GLB readback
checks parent links and metre/Y-up world transforms within `1e-8 m`. This is not
independent-reader or engine evidence. Logs: `target/nested-asset-regression.*`,
`target/nested-asset-faces-test.*`, `target/nested-asset-oblique-test.*`.

Next action: add a reusable nested assembly fixture and independent native
GLB checks for hierarchy, shared geometry and animated world poses; exercise skin
rebinding on a nested body. Engine/FBX acceptance, platform coverage and broader
production gates remain open. Review: 2026-11-14.

### Retained CAD assembly instance metadata (2026-11-01)

The CAD evaluator now retains an explicit instance tree alongside its existing
flat body/placement maps. Each instance records its authored name, evaluated body
membership, nested children and frame in model coordinates. This preserves
ancestry even when a single-body import renames away its original path. Selected
body imports prune unrelated branches; repeated imports own separate trees.
Whole-member moves update frames while partial moves update only affected
descendants. Pattern copies record the copied body as a separate instance.

Stored parts serialize the tree with a new cache-key version. Old entries remain
on disk but are not read as the new layout. Geometry, CAD commands, body names and
existing asset output remain unchanged; asset hierarchy integration is still open.

Verified on Windows Release with `game-assets`: the new nested/selection/movement/
pattern integration test, 49 existing assembly tests, the available stored-cache
integration test, metadata serialization roundtrip, existing assembly asset test
and constrained-motion test all pass (54 tests total). A stale shared target
library initially caused missing-field compile errors; after preserving the old
executable and clearing gcad's Release artifacts, the current-source rebuild and
tests passed. Evidence: `target/assembly-hierarchy-clean-verify.*` and
`target/assembly-hierarchy-asset-regression.*`. No Linux/browser result is claimed.

Next action: consume the retained tree in the asset adapter, derive
parent-relative transforms, adapt mechanical motion tracks, and verify world-space
placement, shared meshes and binding identities through nested GLB export. Backend,
engine, rendering and wider production gates remain open. Review: 2026-11-14.

### Material-factor and transform preflight (2026-11-01)

The v10 native audit now validates core PBR factor types/ranges, alpha mode and
double-sided declarations, normal scale and occlusion strength before import.
Node checks cover numeric array shape, matrix/TRS conflicts, affine matrix rows,
unit quaternion tolerance and the prohibition on matrix-bearing animated nodes.
Float representability and squared quaternion norm tolerance `1e-4` are explicit
audit policies. Negative normal scale, alpha cutoff above one and reflected scale
remain permitted. Core rules were checked against the Khronos
[material schema](https://raw.githubusercontent.com/KhronosGroup/glTF/main/specification/2.0/schema/material.schema.json)
and [node schema](https://raw.githubusercontent.com/KhronosGroup/glTF/main/specification/2.0/schema/node.schema.json).

These guards do not prove matrix decomposability, nonsingular transforms,
animated quaternion normalization, material extensions or rendered parity.
Backend/consumer decisions and production gates remain open. Next action:
trace CAD assembly hierarchy retention and implement nested asset hierarchy from
authoritative evaluator metadata, preserving placement and existing binding rules.
Review: 2026-11-14.

### Texture-reference and UV-binding preflight (2026-11-01)

The experimental v10 audit now checks texture/image/sampler references, legal core
sampler enums, all five metallic-roughness texture-info locations and primitive
material references before import. Each material's requested texture-coordinate
set must exist on every primitive using it. Texture and texture-info extensions
are explicitly refused by this audit; that is not a product-scope exclusion.

This establishes reference and UV-binding validation, not nonzero UV-set export
fidelity, complete PBR-factor validity, image decoding or rendered acceptance.
Next action: validate finite/ranged PBR factors and transform declarations
before import, with targeted malformed-source controls. Backend/consumer selection
and the wider production/release gates remain open. Review: 2026-11-14.

### Embedded image-header and budget preflight (2026-11-01)

Added PNG/JPEG declaration and dimension inspection before import, using the
already-pinned stb_image header reader with filesystem I/O disabled. It checks
embedded unstrided views, MIME/signature agreement and positive dimensions.
Audit ceilings are 1,024 images, 16 MiB per encoded image, 64 MiB aggregate encoded
reads including repeated references, 8,192 pixels per dimension and 128 MiB
aggregate estimated decoded storage at eight bytes per pixel. These are not
production asset budgets or a bound on a real decoder's allocations.

The v10 compiler-input inventory now records 370 source/build inputs, 113
translation units and eleven Assimp contribution directories. The new stb header's
embedded MIT/Unlicense notice is mapped (`image-inventory-final`). No new server,
package installation, writer patch or Rust runtime change was made.

Pixels are not decoded here; full compressed-stream/checksum validity, colour
handling, decoder allocation behavior and rendered acceptance remain open. URI-
based images and non-PNG/JPEG formats are explicit audit limitations, not product
exclusions. Next action: validate texture/source/sampler references and
material texture-coordinate bindings before import. Backend/consumer selection
and the broader production/release gates remain open. Review: 2026-11-14.

### Normalized integer-weight importer candidate (2026-11-01)

Isolated candidate v10 extends the full v9 patch with numeric UNORM8/UNORM16
skin-weight decoding in Assimp's glTF importer. Typed packed values are extracted
with the existing vertex-remapping table, converted by their unsigned maxima,
and held in initialized float storage with owned lifetimes. The float path stays
available. Earlier patches and original pinned sources remain unchanged.

The audit now independently compares raw GLB weights to imported bone influences
by triangle corner, verifies position/remapping identity, and rejects an altered
imported weight before export. This closes the circular-oracle gap where a writer
could preserve already-corrupted imported weights. Each two-instance fixture
checks 7,656 raw/imported corners plus the existing 1,730,256 animated corners.

Debug/Release pass UNORM16, strided UNORM8 and float fixtures. UNORM16 maximum
animated position/basis errors are 1.95327e-8 m / 2.98456e-7 components; UNORM8
results are 2.01645e-8 m / 3.07963e-7. These compare each encoding to its own raw
source, not UNORM8 precision against the original float asset. The UNORM8 fixture
uses a sum-preserving largest-remainder conversion and retains an eight-byte
source stride to exercise strided extraction. Evidence: `v10-quantized` and
`v10-float` under `target/game-assets-fbx-scene-contract`.

### Binary value checks and quantized-import failure (2026-11-01)

Added bounded BIN value checks after byte-range and usage preflight. Float
accessors must be finite; primitive indices must address their vertex arrays and
avoid restart sentinels; skinned joint indices must fit each node's palette.
Weights must lie in [0,1] and sum across sets within 1e-4 of one (audit policy).
Animation times must be nonnegative and strictly increasing. Integer normalized
weights are decoded before testing. These checks do not cover every numeric
semantic such as unit quaternions, normal lengths or bind-matrix invertibility.

A valid UNORM16 export from the existing CLI passed raw value preflight but failed
the later native conversion audit in both builds. Release reported duplicate
source bone influences; Debug reported gained influences on unweighted corners.
Source inspection identifies the importer path: `BuildVertexWeightMapping`
requests four-float weights from `Accessor::ExtractData`, whose smaller-element
path copies raw UNORM16 bytes without conversion and leaves the rest of the
destination element uninitialised. No importer correction has yet been applied.
Evidence: `values-unorm16.glb` and `values-unorm16-verified` in the same directory.
The CLI's sampled quantization report was within the explicit test limits
(0.01 mm, 0.1 degrees); this does not establish FBX preservation.

Next action: create an isolated importer/writer candidate that converts
normalized integer skin weights correctly, preserves remapping, and independently
verifies UNORM16 and float routes. Do not mark the v9 quantized route accepted.
Decoded-image limits, production integration and broader acceptance gates remain open;
backend/consumer selection is still pending. Review: 2026-11-14.

### Primitive contracts and repeated-use budget (2026-11-01)

Added `scene_primitive_probe.h` before instance isolation/import. The canonical
triangle route checks attribute references/shapes/counts, normalization, joint and
weight pairing, skinned primitive requirements, index accessor types and triangle
corner counts. Custom underscore-prefixed attributes receive reference/count/
vector-shape checks. Non-triangle, morph-target and quantized geometric attributes
remain explicit unsupported audit cases, not product-scope exclusions.

The work estimate charges eight bytes per attribute scalar at the greater of
vertex/corner counts plus four bytes per corner. Repeated primitives are summed,
mesh references multiply the cost, and unreferenced meshes are charged once.
Per-mesh and aggregate audit ceilings are 128 MiB; these are not production
budgets or guarantees about real memory/CPU. The valid reflected fixture reports
1,638,384 estimated bytes and still passes the complete shading regression in
Debug/Release.

Next action: inspect actual index/joint/weight and animation-time values
in bounded BIN ranges before import. Decoded images, full schema validation,
transactional output and production integration remain open. Backend and first
consumer selection are still pending; the full implementation remains incomplete.
Review: 2026-11-14.

### Skin and animation accessor contracts (2026-11-01)

Added `scene_rig_contract_probe.h` before instance isolation/import. It checks
skin joint references and uniqueness, optional skeleton/bind references, float
MAT4 bind shape and sufficient bind count, and node mesh/skin references. For
the existing STEP/LINEAR TRS route it checks sampler/accessor/target references,
float scalar time data, float VEC3/VEC4 outputs, matching key counts and duplicate
target properties. Unsupported animation paths receive explicit diagnostics.

These are reference/shape contracts, not a full glTF validator. Actual index and
joint-weight values, animation time ordering/finite values, mesh attributes,
complete skin hierarchy semantics and importer expansion still need checks.
Next action: validate primitive attribute/index accessor contracts and
account for repeated primitive/instance expansion before import. Backend and
first-consumer selection remain pending; broader plan gates remain open.
Review: 2026-11-14.

### Embedded buffer/accessor preflight (2026-11-01)

Added `scene_buffer_probe.h` before importer work for the audit's canonical dense,
single-embedded-buffer route. It checks chunk bounds/order, declared BIN length,
view buffer references and byte ranges, accessor references, component types,
shapes, alignment, strides and occupied ranges. Range checks use subtraction and
bounded counts before multiplication; maximum-integer offsets are rejected.
Matrix-column padding and omitted final padding follow the linked glTF layout
reference in the harness documentation.

Audit limits are 1,000,000 elements per accessor and 128 MiB aggregate estimated
scalar storage, charged at eight bytes per scalar even for overlapping accessors.
These are not production budgets or a bound on all importer memory. Sparse
accessors, implicit zero-filled accessors without views, external buffers and
multiple buffers remain unsupported by this probe route; they are not declared
invalid glTF or removed from the wider product scope.

Next action: check mesh/skin/animation accessor references and their shape
contracts before import, then account for repeated primitive/instance expansion.
Decoded images, full schema validation, transactional output and production
integration remain open alongside the pending backend/consumer choices.
Review: 2026-11-14.

### Source hierarchy preflight (2026-11-01)

Added iterative node-graph validation before instance isolation/import. It checks
node/children types, child indices, duplicate edges, multiple parents, cycles
(including unreachable components), scene-root references and default-scene
indices. Audit ceilings are 65,536 nodes and 128 nodes along a root-to-leaf path;
they are not selected production asset budgets. A root may belong to more than
one scene, while a scene root with a parent is rejected.

This does not validate the full glTF schema or bound decoded images, accessor
expansion, every importer allocation or process lifetime. No production backend
was adopted and the v9 writer patch remains unchanged. Next action:
preflight buffer/view/accessor byte ranges and declared allocation sizes before
import. Backend/consumer selection and broader plan gates remain open.
Review: 2026-11-14.

### Bounded JSON preflight and refusal cases (2026-11-01)

The shared provenance reader now checks a 64 MiB whole-file ceiling before
allocation, retains its 16 MiB JSON-chunk ceiling, and uses an iterative SAX
preflight before constructing the DOM. It limits nesting to 64 containers and
counts at most 1,000,000 container starts, keys and scalar values combined.
These are explicit audit-only ceilings, not selected production asset budgets.

No writer patch, Rust runtime or production backend selection changed. This does
not establish complete hostile-input handling: graph cycles/depth, accessor/image
expansion, importer allocations, cancellation and transactional failures still
need coverage. Next action: validate source node-graph structure and bound
hierarchy traversal before importer/recursive audit work. Backend and first
consumer selection remain pending; broader acceptance gates stay open.
Review: 2026-11-14.

### Native compiler-input and notice inventory (2026-11-01)

Two runs produced identical manifests with no user name or absolute drive paths.
Evidence: `target/game-assets-fbx-scene-contract/dependency-inventory-final` and
`dependency-inventory-repeat`. The original pinned sources and writer patch were
not changed. This work was independent of the pending backend selection.

Next action: prepare bounded-input/refusal acceptance cases for conversion
before production integration. The backend and first consumer/version still need selection. Release notices/source bundle, runtime deployment, clean-build
reproducibility and remaining full-plan gates are open. Review: 2026-11-14.

### Production backend proposal and resource observation (2026-11-01)

The [FBX backend proposal](game-assets-fbx-backend-proposal.md) consolidates the
tested writer/consumer contract, alternatives, source pins, partial license
inventory, failure semantics, deployment gaps and concrete acceptance tasks.
It proposes an optional native helper but does not adopt that architecture.
Backend selection remains pending; no dependency or installation was
changed. The proposal explicitly retains browser, Linux, engine and broader
shader/optimisation gates.

### Explicit FBX basis adapter experiment (2026-11-01)

`scene_basis_probe.h` reconstructs a blended world-space matrix from independently
loaded/evaluated FBX skin weights and clusters. It inverse-transposes preserved
normals, transforms and orthogonalises tangents, and combines stored frame
orientation with the blend determinant for handedness. The reference uses source
joint animation, bind matrices and attributes through Assimp matrix operations;
the adapter uses ufbx matrix operations and loaded FBX attributes.

Debug/Release pass all 1,730,256 sampled corners of the mirrored/reflected fixture
with maximum normal/tangent component error 2.99752e-7 (limit 1e-5) and exact
handedness. Altered normal direction, tangent direction, bitangent orientation and
a singular blend are rejected. The singular control represents the average of
identity and an X reflection, whose individual matrices are invertible. The
untextured regression also passes both builds. Evidence:
`target/game-assets-fbx-scene-contract/basis-adapter` and `basis-untextured`.

This is an explicit consumer-adapter feasibility result, not a change to gcad's
runtime, writer patch or generic FBX evaluator. The prior regenerated-normal
discrepancy remains visible. The probe rejects absolute blend determinants at or
below 1e-10; production tolerance/conditioning policy is not established. No
rendered shader or engine has been validated against this adapter.
Next action: consolidate the tested writer/consumer contract and remaining
integration choices into a reviewable production-backend proposal, including
licensing, deployment, refusal behavior and resource costs. Backend selection,
consumer engine/version, rendered acceptance and broader plan gates remain open.
Review: 2026-11-14.

### Deformed normal convention evidence (2026-11-01)

The native audit now compares source inverse-transpose blended skin normals with
ufbx's recomputed deformed-surface normals as a separate diagnostic, never as a
passing shading-fidelity gate. Code inspection confirms these are different
algorithms: gcad ignores spatial weight derivatives, while the pinned ufbx path
regenerates normals from deformed triangles and has no evaluated tangent output.

Debug/Release agree on the mirrored/reflected fixture: 1,579,555 of 1,730,256
corners differ by more than 0.1 degrees, with maximum 180 degrees. Of 865,128
positive-determinant corners, 714,758 exceed the threshold. Differences already
exist at clip time zero without reflection: 9,204 of 11,484 corners, maximum
22.9719 degrees. Thus reflection alone does not explain the discrepancy; smoothing
and deformation conventions require separate investigation. Position, local tuple,
material and corruption-control checks still pass. Evidence:
`target/game-assets-fbx-scene-contract/normal-conventions-final`.

The harness requires diagnostic coverage matching all sampled position corners and
nonempty time-zero coverage. No writer or Rust runtime change was made. These
results explicitly contradict deformed-normal parity for this generic evaluator;
they do not establish a defect in either convention or acceptance in a game engine.
Next action: compare an explicit FBX adapter reconstruction of the preserved
normal/tangent basis against gcad's blended-matrix convention, including singular
blends and handedness controls. Production consumer/shader convention remains an
open decision; rendered engine acceptance and all broader plan gates remain open.
Review: 2026-11-14.

### Mirrored UV and reflected-instance audit (2026-11-01)

The FBX fixture places the separate skeleton/mesh instance under non-uniform scale
`-1.25,0.75,1.5`. Both builds verify one reflected mesh via world determinants and
reject a removed reflection. All four shading controls, 132 material controls,
18 map connections and 1,730,256 animated corners pass; maximum positional error
is 1.99329e-8 m. Evidence: `target/game-assets-fbx-scene-contract/shading-verified`.
Writer v9's patch is unchanged. The new authored fixture validates and exports with
the existing feature-enabled executable; no new Rust build was performed.

This completes the previous mirrored-chart/reflection data-contract check, not
rendered normal-map or deformed tangent-frame parity. Next action: audit
normal/tangent deformation conventions under the sampled non-uniform skin poses,
then expose any differences before claiming shading fidelity. Production backend,
consumer engine/version, rendering acceptance and resource budgets remain open.
Review: 2026-11-14.

### FBX tangent-frame candidate (2026-11-01)

Writer experiment v9 preserves direct per-polygon-corner tangent and binormal
layers under the existing opt-in export property. Merged primitives with incomplete
frames are refused. Earlier patches and the original pinned checkout remain
unchanged; this is still an isolated candidate, not production CLI support.

Debug and Release pass the combined six-map, three-clip, separately skinned
two-instance fixture, including normals, UV0, tangent directions and handedness.
Both reject four altered shading controls and a missing-primitive-frame export
control, in addition to 132 material, eight scene and two animation controls.
The 1,730,256 sampled animated corners retain maximum error 2.61536e-8 m.
The untextured regression also passes both builds. Source GLBs remain unchanged.
Evidence: `target/game-assets-fbx-scene-contract/v9-verified` and
`v9-untextured-verified`. No Rust source changed or fresh Rust build was required.

This resolves the v8 missing-layer gate below for these fixtures. It does not prove
rendered normal-map parity, MikkTSpace compatibility or engine acceptance.
Next action: extend the FBX shading audit to mirrored/rotated UV charts and
negative-scale instances, preserving direction and handedness through conversion.
Production backend selection and consumer engine/version remain user decisions;
rendering, platform coverage and resource budgets remain open.
Review: 2026-11-14.

### Combined textured animation/instance fixture (2026-11-01)

The scene audit now verifies each used material's factors, colour/alpha/sidedness,
normal/AO settings, packed channels, connected samplers and exact embedded image
bytes. The two-instance fixture initially passed Debug/Release with three materials,
18 custom map connections, 132 rejected material controls and 1,730,256 animated
corners (maximum skin error 2.61536e-8 m). Provenance and source-skin checks also pass.
These data-contract results are retained in `target/game-assets-fbx-scene-contract/first`.

Adding corner UV/normal and normal-map tangent-frame checks exposed a missing writer
capability: no tangent/binormal layers are exported. The combined fixture now fails
in both builds with `normal-mapped tangent changed or missing`; this supersedes any
whole-fixture acceptance inferred from the earlier partial checks. The untextured
regression passes including UV/normal checks. Failure evidence:
`target/game-assets-fbx-scene-contract/shading-{Release,Debug}.*`.

Next action: preserve tangent and bitangent layers across merged FBX geometry,
then rerun this exact combined fixture and corruption controls. Rendered/engine
acceptance, output-memory budgets and production integration remain open.
Review: 2026-11-14.

### Skinned mesh instance bridge (2026-11-01)

Two shared-mesh variants exposed loss in the pinned route: the second node lost its
skin with either a shared skeleton or a separate translated skeleton. The new
`--audit-scene-instance-skins` mode isolates repeated skinned mesh definitions in an
in-memory GLB before import, retaining the original buffer/accessor references.
This preserves the tested instances at the cost of separate imported/output mesh
storage; it does not promise FBX geometry sharing. JSON copy expansion is bounded.

The audit now verifies imported bone names against each source skin's joint list and
inverse binds against the source's dense float MAT4 buffer data. Wrong imported bones
and binds are rejected by two new controls. Without isolation, the separate-skeleton
case is rejected before output creation by the source-identity check. Sparse/external
bind buffers are outside this prototype's supported input contract.

Next action: combine textured material provenance, multiple animation clips
and separate skinned instances in one conversion acceptance fixture. Broader mesh
formats, output-memory budgets, consumer adapters and production integration remain
open. Review: 2026-11-14.

### Local rotation refinement candidate v8 (2026-11-01)

The version-8 writer candidate splits only intervals whose sampled Euler/quaternion
matrix error exceeds 2e-6, then recomputes the full angle sequence. It retains the
65,536-key limit and adds a 32-pass limit; nonfinite values and time collisions fail.
These remain sampled checks, not continuous bounds. Earlier patches are unchanged.

Debug/Release now pass all four rotation stress clips without a refusal allowance.
The formerly refused multi-interval clip uses 2,982 scalar FBX keys. Oblique keys
fall from 1,545 to 1,056; the antipodal case falls from 4,617 to 2,106; crossing stays
at 12. The independent node audit also samples exported key times and midpoints;
multi-interval node error is about 2.09e-6 against its 1e-5 comparison tolerance,
and sampled skin error is below 3.8e-8 m. LINEAR and STEP regressions pass both builds.

Exported-knot samples within two FBX ticks of a source key are aligned to the source
event time to avoid comparing opposite sides of STEP due solely to clock quantization.
Exact source times and the existing before/after samples remain. The initial stricter
timing failure is preserved in `v8-step-regression`; final evidence is under
`target/game-assets-fbx-rotation-stress/{v8-final,v8-step-final,v8-linear-final}`.
Input hashes and pinned source checkouts are unchanged. The tools README records
the candidate patch checksum and reproduction options.

Next action: verify shared skinned-mesh instances and either preserve their
bindings through conversion or reject unsupported sharing explicitly before export.
Representative clip budgets, general rotation fidelity, consumer acceptance and
production integration remain open. Review: 2026-11-14.

### Rotation stress and key-growth baseline (2026-11-01)

`plate-rotation-stress.gasset` adds an Euler-pitch crossing, quaternion sign changes
and a three-interval changing-axis track alongside the oblique clip. Debug/Release
pass crossing, oblique and actual antipodal GLB inputs. gcad canonicalizes exported
quaternion signs, so the reproducible harness injects an equivalent opposite-sign
key into a disposable GLB and verifies its dot product before testing. Source files
remain unchanged.

The multi-interval track fails the v7 bake budget in both builds: sampled matrix
error remains about 1.8e-5 against the 2e-6 target immediately after an interval
boundary. Do not count the baseline as broad rotation acceptance. Scalar FBX key
counts are 12 for the two-key crossing, 1,545 for the two-key oblique clip and 4,617
for the four-key sign-change clip, including default channels. These counts expose
uniform refinement cost; they are not accepted representative performance budgets.

Next action: refine failing rotation intervals locally under the same error
and key limits, retaining the multi-interval refusal as a regression. Production
backend, representative budgets, shared skin instances and consumer acceptance remain
open. Review: 2026-11-14.

### Oblique quaternion conversion candidate v7 (2026-11-01)

`plate-oblique-rotation.gasset` exposes the mismatch between LINEAR quaternion motion
and Euler-component interpolation: version 6 fails independent playback. The isolated
version-7 candidate doubles Euler bake density until endpoint/quarter-point rotation
matrix errors are at most 2e-6, with a 65,536-key cap and explicit failure on budget or
FBX-time-resolution exhaustion. This is sampled error control, not a continuous bound.
Double-precision conversion and orientation-preserving Euler alternatives avoid the
legacy helper's approximation across a whole degree around gimbal lock. STEP is unchanged.

Debug/Release pass the oblique fixture: 216 node transforms and 275,616 skinned corners,
maximum skin error 2.18288e-8 m. It exports 1,545 scalar animation keys (including the
three baked rotation curves and default channels). The audit adds 37 off-grid times
per clip to avoid checking only bake-grid points. LINEAR and STEP regressions pass
both builds, each with 865,128 sampled corners. Existing corruption/unit controls and
source-hash checks pass. Evidence: `target/game-assets-fbx-oblique/{verified,
linear-regression,step-regression}`; failed trials and version-6 evidence are retained.
Earlier writer patches remain unchanged; the tools README records the v7 patch hash.

Next action: stress antipodal quaternion signs, gimbal crossings and multiple
rotation intervals, and measure key growth against representative clip budgets.
General rotation fidelity, shared skinned instances, consumer acceptance and production
integration remain unproven. Review: 2026-11-14.

### STEP curve writer candidate v6 (2026-11-01)

The pinned importer retains STEP on keys, but the original writer always emits
LINEAR curve flags. The isolated version-6 candidate emits constant-previous flags
for STEP tracks under the existing export opt-in. Unsupported or mixed per-key
interpolation fails rather than silently becoming LINEAR. Material schema remains 3.
The audit compares imported modes with source GLB channel declarations, evaluates
STEP directly and samples immediately before/at/after keys as well as the prior grid
and midpoints. `plate-step-clips.gasset` covers rotation, translation and scale.

Debug/Release pass the three STEP clips: 345 node-transform comparisons and 440,220
skinned corners, maximum error 6.3629e-9 m. The LINEAR regression also passes both
builds on the expanded sample set. Deliberately changing a varying STEP curve to
LINEAR is rejected; the first attempted control used a constant-valued component
and was corrected because that cannot distinguish interpolation. Version 5 fails
the same STEP fixture, confirming the new gate detects the original loss.
Evidence: `target/game-assets-fbx-step-clips/{verified,linear-regression,v5-negative.*}`.
Source hashes and earlier patches are unchanged; patch details are in the tools README.

Next action: test arbitrary-axis LINEAR quaternion motion against the FBX
Euler conversion and establish a bounded conversion strategy if it diverges. Mixed
channel coverage, shared skinned instances, engine acceptance and production backend
integration remain open. Review: 2026-11-14.

### Multiple authored clips and scale-channel candidate v5 (2026-11-01)

The new `plate-multi-clip.gasset` has independent LINEAR rotation, translation and
nonuniform-scale clips with different durations and irregular intermediate key times.
It exposed a writer error: scale curves connected to `Lcl Scale` instead of the
existing `Lcl Scaling` transform property. An isolated full version-5 patch corrects
that connection when the consolidation option is enabled, retaining material schema
3 and earlier immutable patches. The source pose checker now includes every key time
and inter-key midpoint in addition to its uniform samples.

Debug/Release pass all three clips: 315 node transforms and 401,940 skinned corners,
maximum skin error 6.75314e-9 m. Physical-unit, scene-corruption and animation controls
pass; prior single-clip and multi-node fixtures also pass both builds. All 29 authored
fixtures validate using the existing feature-enabled executable. Source hashes remain
unchanged. Evidence: `target/game-assets-fbx-multi-clip/{verified,single-regression,
scene-regression}`; the earlier failing scale case remains in `initial`/`diagnostic`.
The tools README records the new patch hash and reproduction parameters.

Next action: preserve and independently verify STEP interpolation through the
GLB/import/writer route, including samples immediately around discontinuities. Arbitrary
rotation-track conversion, shared skin instances, engine acceptance and production
backend integration remain unproven. Review: 2026-11-14.

### Physical-unit verification for consolidated scenes (2026-11-01)

The consolidated-skin audit now declares GLB's metre coordinates as FBX
`UnitScaleFactor=100` / `OriginalUnitScaleFactor=100` (centimetres per source unit).
Independent readback multiplies world positions by the declared metre scale and
compares rest corners and sampled animation against the source. This changes the
audit bridge, not the version-4 writer patch or production export implementation.

Debug/Release pass the authored plate: 3,828 rest corners, physical spans
0.04/0.003/0.03 m and 126,324 animated corners with maximum error 6.75314e-9 m.
The multi-node fixture passes 11,484 rest corners with spans 0.14/0.003/0.03 m and
maximum error 3.72529e-9 m. Substituting centimetre metadata is rejected by both
rest and authored-animation checks. Existing scene/animation corruption controls
still pass and input hashes remain unchanged. Both harnesses require unit evidence
when invoked with `-WriterVersion 4`. Final evidence:
`target/game-assets-fbx-scene-provenance/physical-{animation,scene}-final`.

Next action: broaden authored clip coverage beyond the single LINEAR bend,
including multiple clips and translation/scale channels, then address STEP semantics.
Engine import, shared skinned instances, production backend selection and remaining
roadmap gates stay open. Review: 2026-11-14.

### Consolidated-skin writer experiment v4 (2026-11-01)

A separate, opt-in writer candidate now emits one Skin object per merged node's
geometry, retaining each primitive's clusters, vertex offsets and bind matrices.
It adjusts the deformer definition count and leaves the material contract at version
3. The version-4 patch is a full patch against the same pinned Assimp source; all
earlier patches and original pinned checkouts remain unchanged. No production
backend or dependency was added.

Debug/Release now pass the formerly failing authored bend clip: 99 sampled node
transforms and 126,324 skinned corners, maximum skin component error 6.75314e-9 in
source coordinate units. Eight scene controls and two animation controls are rejected
and restored. Both builds also pass the 3,828-face merged scene fixture. The source
hashes stay unchanged. Release opt-out reproduces the original playback failure;
the six-map material regression passes. Evidence:
`target/game-assets-fbx-scene-provenance/v4-{animation,scene}-verified` and
`v4-optout.{stdout,stderr}`. Build/contract details and patch hash are in the tools README.

Next action: apply/check metre metadata through the scene route and verify
physical bounds and animated positions; then broaden clip/interpolation coverage.
This fixture result does not establish shared skinned-mesh instancing, engine import,
STEP/CUBICSPLINE playback or production FBX support. Original production/consumer
choices and broader roadmap gates remain open. Review: 2026-11-14.

### Authored FBX playback gate exposes multiple-deformer gap (2026-11-01)

The new LINEAR animation audit compares clip names/duration and independently
interpolated source node transforms against ufbx at 33 times. The authored face-weight
plate passes 99 node-transform comparisons in Debug/Release (maximum component error
1.69434e-7). Changed duration and curve-value controls are rejected and restored.

Actual evaluated skin playback fails in both builds: the second primitive differs
by 0.00106013 source-coordinate units at sample 1 (the GLB uses metres). The pinned
ufbx evaluator uses `mesh->skin_deformers.data[0]`; the writer emits three disjoint
deformers for the merged plate. Data preservation and synthetic pose comparisons
therefore did not establish consumer playback compatibility. Keep this failing gate.
The scene audit does not yet set metre metadata; this mismatch compares numeric
source/output coordinates, not acceptance of physical FBX unit declarations.

Next action: prototype consolidation into one compatible skin deformer per
merged geometry, preserving corner influences/binds, then rerun the playback gate.
Production backend selection, engine acceptance and remaining roadmap gates are
still open. Review: 2026-11-14.

### Merged FBX skin parity audit (2026-11-01)

The scene audit now compares each ordered corner's bone influences, weights and
inverse bind matrices, then evaluates 13 synthetic affine bone poses. Bone poses
are stable by bone identity across corners with different influence sets. The
writer emits separate deformers with disjoint vertex ranges for merged primitives;
the checker selects the active deformer per corner and rejects overlapping active
deformers. This verifies the existing writer output without modifying its patch.

These synthetic poses prove sampled linear-skin parity, not exported clip playback
or consumer handling of multiple deformers. Next action: verify authored
animation timing/channels and sampled playback across the merged FBX route. Production
backend, consumer adapter/engine and remaining acceptance gates stay open.
Review: 2026-11-14.

### Merged FBX face/material association (2026-11-01)

The scene audit now checks node attachments and ordered face corners instead of
requiring one FBX geometry per imported primitive. Source inspection confirmed that
the writer merges each node's primitives and uses polygon material indices. In-memory
source node identities let ufbx readback verify every face's material/provenance,
corner positions and winding, plus the union of attached skin bone names. This
resolves the multi-primitive mesh-count failure recorded in the prior entry.

Next action: independently check merged skin weights, bind transforms and
sampled deformation; a matching bone-name set alone does not establish skin parity.
Consumer adapter, production backend and broader acceptance gates remain open.
Review: 2026-11-14.

### Scene provenance association audit (2026-11-01)

The new `--audit-scene-provenance-v3` experiment preserves distinct declarations
across referenced materials in a two-skinned-mesh fixture. Source inspection and a
failed broader fixture disproved index-based association: the pinned importer loads
objects lazily, omitting unused objects and changing material order. The audit now
tags only an in-memory GLB copy with source identities and checks imported primitive
material references before export. Original files remain untouched.

Release/Debug readback passes the two-mesh case; the existing six-map single-material
Release regression also passes. The scene audit compares the multiset of material,
face-count and skin-cluster-count tuples, plus exact provenance, not deformation or
node attachment. An active multi-primitive mesh currently fails the mesh-count gate
because the writer changes the layout. This is unresolved evidence, not acceptance.
Fixtures/build logs: `target/game-assets-fbx-scene-provenance` and
`target/game-assets-tools/scene-v3-*`. Next action: inspect merged FBX mesh
face-material assignments and implement independent association checks. Broader
production/engine/platform gates below remain open. Review: 2026-11-14.

### FBX source-provenance extension v3 (2026-11-01)

The isolated version-3 writer retains source texture declarations from GLB material
extras as versioned JSON on the FBX material. Paths, original source SHA-256 hashes,
license/origin declarations and unknown fields survive independent ufbx readback.
This preserves declarations; it does not validate licenses or identify transformed
FBX image payloads by the original source hash. No production backend is selected.

The provenance bridge currently requires one source mesh and one primitive, matching
the existing material audit fixture. Next action: establish checked material
association for multi-primitive/skinned scenes before evaluating a consumer adapter.
Production FBX choice, consumer engine/version, shaders, platform/GPU acceptance and
representative budgets remain open. Review: 2026-11-14.

### FBX material-semantics extension v2 (2026-11-01)

A separate version-2 writer experiment now preserves all six custom map connections,
normal scale, occlusion strength, alpha mode/cutoff/opacity, sidedness, base/emission
factors and per-slot wrap/filter/UV-set records. Its packed metallic/roughness/AO
channel identifiers remain explicit. Version 1 and the original pinned source stay
unchanged. The build script accepts `-Version 2`, verifies the complete patch diff,
and uses separate worktree/build paths. No production dependency is added.

Debug/Release pass the original MASK fixture and deliberately varied BLEND/OPAQUE
in-memory materials, covering all wrap modes and six minification filters. Six map
payloads stay byte-identical. Every case rejects and restores 38 altered imported
properties; these verify checker sensitivity, not engine rendering. Baseline and
version-1 writers fail the version-2 expectation; opt-out audits match the original,
the version-1 regression passes and the source GLB/pinned checkouts stay unchanged.
The portable patch checksum and intended consumer interpretation are documented in
`tools/game-assets-fbx/README.md`. Evidence:
`target/game-assets-tools/material-extension-v2-verify.{ps1,stdout,stderr}` and
`target/fbx-v2-verified-{Release,Debug}-{mask,blend,opaque}`.

Custom-property preservation does not make generic FBX PBR portable. The proposed
consumer must explicitly recognize this version and interpret factors, colour
spaces, packed channels and sampler enums; no renderer/engine adapter is tested.
Next action: preserve and verify source material provenance/licenses through
the conversion route, then evaluate the complete consumer contract. Engine/version
and production backend selection remain open. Review: 2026-11-14.

### Post-0032 audit and isolated FBX writer experiment (2026-11-01)

The requirements audit was reconciled with patches 0030-0032: guarded face lineage
now survives preparation and material/UV/weight authoring, with source-change
rejection, explicit boundaries and independent imports. Engine/platform acceptance,
production FBX, shaders and representative budgets remain unproven. Current exposed
MCPHub tools include browser control but no native desktop control; this did not
resolve the pending desktop-automation permission. No integration was installed.

An isolated extension of the existing Assimp writer now preserves previously lost
roughness/metallic/occlusion connections as explicit `gcad|...` properties, exact
embedded map bytes, packed G/B/R channel identifiers and roughness/metallic factors.
The caller must opt in with `gcad.extension.version=1`; legacy shading remains.
The portable patch and reproducible build script are under `tools/game-assets-fbx`,
with source revision and checksum documented there. Original pinned sources remain
clean; no production dependency or gcad export backend was added.

Debug/Release reports agree. The original writer fails the new extension assertion;
opting out of the patched writer reproduces the original audit. The GLB input hash
is unchanged and metre-scaled world extents remain correct. Evidence is in
`target/game-assets-tools/material-extension-{verify,release,debug}.*` and
`target/fbx-maps-extension-{release,debug}/{report,extension}.json`.
This preserves information for a future consumer but does not make it standard FBX
PBR: ufbx still sees legacy roughness and lacks standard metallic/roughness/AO map
connections. Normal scale, occlusion strength, alpha modes, samplers and material
provenance still need a complete explicit contract and verification.

Next action: test preservation of the remaining material semantics and
document the consumer contract before recommending production integration. First
engine/version and production backend selection remain open.
Review: 2026-11-14.

### Face-weight review patch 0032 (2026-11-01)

Selected-face weight editing, boundary policy, declarations, CLI, diagnostic fixture
and independent deformation checks are packaged as
`target/game-assets-review/0032-guarded-face-weight-authoring.patch`. Prerequisites
are the 31 checksum-verified canonical patches; `series-0032.sha256` records the
extended series. The detached `face-weights-worktree` passes 155 library tests,
two input-recording tests, WASM/default compilation and the feature-enabled release
build. Fresh preparation validates 28 fixtures. Three exports match the main tested
bytes exactly and pass Debug/Release independent weighted-pose imports. The library
suite includes the authored shared-boundary split and other-instance preservation.
Native probe sources match main; existing binaries are reused, with no separate
isolated CMake rebuild claimed. The lockfile is unchanged.

Artifact scope, limitations and evidence filenames are in `docs/game-assets-review.md`.
Prior patches remain immutable. No commit, push or external submission occurs.
Next action: audit remaining requirements against current implementation
and proceed with the highest-priority unblocked product gap. Production FBX, shaders,
engine/platform acceptance and representative quality budgets remain. Review: 2026-11-14.

### Independent weight import and real boundary splitting (2026-11-01)

The native paired importer now has `--face-weights NODE` for the plate end's
25% `base` / 75% `tip` assignment. It derives expected selected weights on the
reference side and compares candidate imported weights and 17 synthetic affine
bone poses. Geometry/shading tuples, unselected weights, inverse bind matrices,
hierarchy, clips, checked material factors/maps and other instances must survive.
The original and new expected motion must differ somewhere, avoiding a vacuous
deformation comparison. Direct and persisted plate exports pass Debug/Release with
65,076 posed corners and 4,320 changed expected positions. Unchanged-output and
corrupted selected/unselected weight, bind-matrix and winding controls fail. Existing
material, roughness and UV modes still pass.

A new authored smoothed-plate test exercises actual shared vertex indices across
source faces. `reject-shared` fails; `split` duplicates 68 boundary tuples, isolates
one of two shared mesh instances and retains source-face lineage. Declaration/API
bytes match. Tuple, weight and sampled deformation tests preserve the other instance.
The corresponding CLI export passes both native importer builds with 130,152 posed
corners; boundary refusal creates no output. All 155 library tests pass. This change
changes test/probe code, not production Rust behavior; it does not claim a new WASM
or full regression run, engine playback or continuous deformation bounds.

### Persisted face weight declarations (2026-11-01)

`face_weights NODE SELECTION.json joints=NAME,... weights=VALUE,... boundary=POLICY`
now participates in ordinary check, preview and export. Material edits precede UV
edits, which precede weight edits, regardless of declaration text order. Weight
regions must be disjoint. Joint lists and values receive the same strict validation
as the direct API, and normal loading checks palette membership and final boundary
sharing. Binding returns the original prepared mesh and validates syntax, joint-node
existence, selections and overlaps; palette and prepared-boundary checks are deferred
until the new skin resource is loaded normally. This limitation is documented.

All 154 library tests pass, including declaration/API byte parity after material/UV
preparation, disjoint regions, retained face indices, identical original binding
bytes, deferred palette checks and malformed/overlapping declaration rejection.
WASM/default compilation and release build pass. The new diagnostic
`plate-face-weights.gasset` reuses existing skin and face resources; all 28 fixtures
validate. Its GLB exactly matches the previously decoded direct CLI export, prepared
face resolution succeeds and the original weight-resource hash stays unchanged.
The bent software snapshot was inspected: the red end separates from the walls and
bottom as expected from different weights across existing separate tuples. This is
evidence of the diagnostic seam, not production deformation acceptance.

Next action: independently verify imported changed-weight deformation and
exercise an authored shared-boundary split case. Production FBX, shader implementation,
engine acceptance and representative quality budgets remain. Review: 2026-11-14.

### Selected face weight API and CLI (2026-11-01)

`face_weights::edit_scene` and `asset-face-weights` now replace selected corner
weights using one to four unique existing palette joints, positive finite values
and a unit sum. The caller must choose `reject-shared` or `split` for vertices
shared between selected/unselected triangles. Splitting duplicates complete tuples;
unselected corner weights stay exact. Existing separate tuples may also diverge
during deformation; this operation does not promise watertight animated surfaces.
Source selection, mesh and skin guards run first. Face lineage survives, inverse
bind matrices remain exact, and shared geometry is copied only when splits require
isolating the target instance. Materials and texture pixels stay unchanged.

Next action: add persisted declarations and independent import/deformation
verification, including an authored shared-boundary split case. Production FBX,
shaders and engine/quality acceptance remain. Review: 2026-11-14.

### UV review patch 0031 (2026-11-01)

The accumulated selected-face UV API, scene/skin remapping, persisted declarations,
CLI, textured fixture and independent importer checks are packaged as
`target/game-assets-review/0031-guarded-face-uv-authoring.patch`. Prerequisites are
the 30 checksum-verified canonical patches; the extended manifest is
`series-0031.sha256`. The detached `uv-worktree` passes 152 library tests, two
input-recording tests, WASM/default compilation and the feature-enabled release
build. All 27 fixtures validate from fresh resources. Five isolated exports match
the tested main GLBs exactly, and the byte-matched native probe sources' existing
Debug/Release binaries pass imported UV/tangent and sampled skin comparisons.
No separate isolated CMake rebuild or new full-regression run is claimed.

See `docs/game-assets-review.md` for artifact scope and evidence filenames. Existing
patches remain immutable; no commit, push or submission is performed. Next action: implement guarded face-bound weight editing, including shared-boundary
semantics and deformation verification. Production FBX, shader implementation,
engine acceptance and representative quality budgets remain. Review: 2026-11-14.

### Independent face UV import and textured example (2026-11-01)

Paired software snapshots were visually inspected: the end face changes texture
scale/orientation while the rim, walls and hole interiors retain their mapping.
The fixture uses existing licensed test pixels and prepared face records. All 27
authoring fixtures pass preparation. A VS Code snapshot configuration is supplied;
its JSON/task references are checked, but the debugger UI is untested. The initial
snapshot-baseline attempt used a forbidden absolute texture path; the corrected
relative path passes. Snapshot filtering does not certify engine sampler behavior.

Next action: package the accumulated UV implementation as the next immutable
review patch, with isolated build/test checks. Face-bound weight editing, production
FBX and engine/shader acceptance remain. Review: 2026-11-14.

### Persistent face UV declarations (2026-11-01)

`face_uv NODE SELECTION.json scale=U,V rotation=DEGREES offset=U,V` now participates
in normal `.gasset` loading. At least one transform setting is required; omitted
settings use identity defaults. Material assignments always precede UV edits,
regardless of text order. Saved selections resolve on the current prepared primitives;
disjoint UV regions preserve their lineage and overlapping regions fail. Weight
resources continue to bind the original prepared mesh, with explicit remapping after
material and UV operations. Rebinding validates declarations but returns original
geometry, preserving the existing weight-resource contract.

### Scene and CLI face UV editing (2026-11-01)

`uv_edit::edit_scene` and `asset-face-uv FILE NODE SELECTION OUTPUT.glb --scale U,V
--rotation DEGREES --offset U,V` now validate saved source selections and edit selected
UVs across material primitives. The selected instance is isolated from shared meshes;
materials, texture pixels, geometry and unselected UV/tangent tuples remain unchanged.
Skin joint/weight tuples remap exactly per source primitive and vertex; inverse bind
matrices must remain identical. The result includes updated companion face maps.
Transforms affect current sampling coordinates, including atlas UVs, without rebaking.

### Post-0030 selected UV transforms (2026-11-01)

`Mesh::transformed_uvs` now applies explicit scale/rotation/offset transforms to
selected triangles, splitting shared selected/unselected vertices and returning an
original-vertex remap. Positions, normals, triangle order and unselected UV/tangent
tuples stay exact. Selected tangents regenerate with the existing derivative method;
negative scales support mirrored charts. The `FaceMap` wrapper retains source-face
lineage after checking exact triangle correspondence. Existing UV/tangent data is
required; duplicate/invalid indices, zero scales, nonfinite inputs, overflow and
numerically collapsed charts fail. Identity transforms preserve the original mesh.

All 152 feature-enabled library tests pass, along with WASM compilation and formatting/
whitespace checks. Tests cover shared-boundary splits, complete source-index remapping,
selected/unselected attributes, reflection handedness, all-face and identity behavior,
invalid transforms, retained face resolution and stale-geometry rejection. The initial
test incorrectly expected a UV-only edit to invalidate a geometry stamp; correcting
the stale-map control to move a vertex matches the existing provenance contract.
Evidence: `target/game-assets-uv-region-verified-library.log` and
`target/game-assets-uv-region-wasm.log`. Patch 0030 remains unchanged. This is a library
foundation; scene skin remapping, source-record-driven CLI UV edits, persisted UV
declarations and face-bound weight editing remain. Next action: connect UV
edits to validated scene selections and exact skin remapping. Review date: 2026-11-14.

Latest review packaging (2026-11-01): patch 0030 in
`target/game-assets-review/0030-face-provenance-and-material-authoring.patch` extends
the immutable 0001–0029 series with guarded face provenance, prepared selections,
material assignments and examples. The isolated checkout passes 151 library tests,
two input-recording tests, WASM/default compilation and all 26 fixture checks.
Its exported face-material GLBs match main bytes and pass Debug/Release import probes.
The full feature-enabled regression run reports 555 passed, 5 failed and 13 ignored;
all five failures match the previously recorded baseline set. See
`0030-regression-summary.json` and `0030-isolated-full-regression.log` under the review
directory, and [the review record](game-assets-review.md). Packaging is local and does
not establish engine, production FBX, shader or platform-runtime acceptance. Next:
complete the remaining face-bound UV/weight authoring requirements and resolve the pending engine/converter/tooling choices. Review: 2026-11-14.

- Status: M0 decisions open; experimental GLB asset pipeline and isolated native FBX prototype implemented
- Created and reviewed: 2026-10-31
- Next review: 2026-11-14, or before implementation
- Latest platform progress (2026-10-31): browser virtual-file dependency recording
  and portable source guards are implemented; default and feature-enabled WASM
  library checks pass. Browser runtime and authoring UI remain unverified.
- Target date: None
- Upstream baseline: `33653aa19321ac6760b7f3aefa1f779030415e0a` (`0.5.2`)

## Outcome and requirements

The implemented experimental workflow is documented in [game-assets.md](game-assets.md).
That guide describes available commands; it does not mark the milestones below complete.

Extend gcad to author textured 3D game assets, create rigs and animation clips,
optimise assets without damaging deformation, and export game-ready files,
including FBX. Add shader authoring and engine integration after the asset pipeline
works. Preserve every existing CAD workflow and keep changes suitable for upstream
pull requests.

Confirmed scope comes from the project request on 2026-10-31. The architecture and
milestone order below are proposals until validated by prototypes and upstream review.

## Verified starting point

Implementation evidence, the proposed scene/schema contract and FBX candidate
comparison are in [game-assets-design.md](game-assets-design.md). The opt-in
`game-assets` feature includes an experimental GLB asset pipeline;
feature-enabled `check` and `export` accept experimental `.gasset` files
and GLB output. Existing CAD format dispatch is unchanged.

The baseline uses Rust, monstertruck for B-rep modelling, and egui/egui_bench with
wgpu for viewing. Preserve the upstream GPL-3.0-or-later license and notices.

Relevant source locations:

| Area | Existing implementation | Planning implication |
| --- | --- | --- |
| CAD evaluation | `src/parse.rs`, `src/model/`, `src/geometry.rs` | Keep exact geometry authoritative for manufacturing and simulation |
| Materials and textures | `src/model/materials.rs`, `src/model/texture.rs` | Extend existing finishes, image maps and procedural textures |
| Viewer appearance | `src/view/scene.rs`, `src/view/textures.rs` | Reuse PBR rendering and existing projected UV handling |
| Export | `src/export.rs` | Existing dispatch covers STEP, STL, OBJ, 3MF and SVG, with drawing and sheet-metal routes elsewhere; add game exports explicitly |
| Assemblies | `src/model/assembly.rs`, `src/model/mate.rs` | Reuse instance names, placements and joint relationships where appropriate |
| Regression coverage | `tests/`, `docs/ops.md` | Keep geometry, assembly, simulation, drawing and export tests intact |

Current viewer textures do not establish a portable textured-asset export pipeline.
The reviewed export dispatch has no glTF or FBX route. Mechanical joints do not by
themselves provide deformable skinning, bind poses or animation clips.

Source: the local checkout at the baseline above, verified 2026-10-31. See also
the [upstream repository](https://github.com/v0l/gcad).

## Compatibility and implementation boundaries

Game-specific authoring should use an optional, versioned asset description that
references existing `.gcad` and `.gasm` files. Decide its syntax and extension during
the first milestone. Preserve the line-oriented authoring model where practical.
Do not repurpose `.gsim`, `.gdwg`, existing operations or physical material values.

Keep the original CAD model separate from derived render meshes. UV unwrapping,
retopology, simplification and skinning must never change the solids used for STEP,
BOM, measurements, interference checks or simulation. Keep game dependencies optional
where practical, and avoid requiring a GPU for ordinary asset validation and export.

## Proposed asset pipeline

1. Evaluate the CAD sources and retain stable part, instance and face provenance.
2. Derive a scene with nodes, transforms, indexed meshes, material slots, UV sets,
   normals, tangents, textures, skeletons, skin weights, clips and export metadata.
3. Apply explicit asset operations and validate all references and attribute lengths.
4. Produce a derived optimised variant with an error report and reproducible settings.
5. Export through format-specific adapters and validate imports in target engines.

Define units, handedness, up axis, transform composition and tangent conventions in
one place. Existing CAD lengths are millimetres; each exporter must perform its own
documented conversion exactly once. Test mirrored instances and non-uniform scales.
Treat CAD regeneration as a possible topology change: detect invalid UV/weight/face
bindings and require rebinding instead of silently attaching them to different faces.

### Materials, textures and mesh preparation

Reuse existing finish values and texture loading. Add explicit UV authoring, seams,
projection choices and atlas generation as required. Separate texture colour space
from numeric maps; specify normal-map orientation, sampler settings and channel packing.
Support base colour, metallic/roughness, normals, occlusion, emissive, alpha and
double-sided materials with an explicit export capability report.

Experimental `surface SOURCE` settings now expose core factors, alpha/cutoff,
double-sided state, normal-Y/scale, occlusion strength and samplers in `.gasset`.
Independent Debug/Release imports verify authored MASK/BLEND fixtures. Explicit
`map SOURCE SLOT PATH license=... origin=...` inputs now cover all six map slots,
with project-relative provenance and original content hashes retained in GLB extras.
Native probes verify pixel packing and declarations; changed maps invalidate
persisted source bindings. Target-engine appearance acceptance remains open.

Bake procedural appearance into portable textures where needed. Include texture
files in export packages using relative paths, preserve provenance and licenses,
and diagnose missing resources. Add tessellation controls, smoothing and hard-edge
rules, tangent generation and high-to-low normal baking. Experimental
`normals SOURCE crease=DEGREES` now rebuilds area-weighted connected-face normals
before UVs and skin binding; omitted settings preserve CAD normals. The
`plate-normals.gasset` fixture compares 0-degree hard edges and 60-degree smoothing.
Exact shared-edge matching leaves gaps and point contacts separate; this does not
provide approximate welding. Independent Debug/Release import verifies crease
normals against full-precision geometry within `4.10e-8` normal-vector error;
the software comparison preview was inspected. GPU/engine appearance remains open.
This does not
provide arbitrary artist-marked edge selections or repair topology. Evaluate retopology for
deforming assets; CAD tessellation alone must pass deformation tests before acceptance.

### Rigging and animation

Start with rigid assembly animation, then add skeleton hierarchy, rest/bind poses,
inverse bind matrices, named bones, per-vertex weights and attachments. Provide
repeatable weight authoring and validation before adding automatic weighting.
Mechanical constraints and skeletal bones need an explicit conversion/baking step.

Support named clips, translation/rotation/scale tracks, interpolation, playback,
looping and root motion. Bake procedural or constrained motion at a declared sample
rate when a destination cannot represent it. Test quaternion continuity, clip bounds,
rest-pose recovery, influence limits and zero-weight vertices. Treat IK and morph
targets as later extensions whose priority is still open.

### Rig-aware optimisation

Make optimisation optional and reversible. Measure triangle and vertex counts,
material batches, texture memory, bone influences, clip size and processing time.
Generate LODs while preserving UV seams, hard edges, material boundaries and
attachment points. Remap skin weights and all vertex attributes together.

Judge skinned simplification in representative animated poses, including joint
extremes, rather than only in the bind pose. Specify maximum and percentile surface
error, silhouette error and animation sampling tolerances before accepting a result.
Preserve bones referenced by sockets or clips; prune only with an explicit remapping
and validation report. Key reduction and quantisation must also respect declared
motion error budgets. Keep an unoptimised reference for every comparison.

### Export formats and engine profiles

| Format | Proposed role | Required evidence |
| --- | --- | --- |
| glTF 2.0 / GLB | First complete game asset export | Materials, textures, hierarchy, skins and clips survive independent validation and engine import |
| FBX | Required interchange export | A chosen backend writes static and skinned meshes, bind poses, multiple clips, material references and correct units/axes |
| OBJ | Existing static mesh compatibility | Existing behaviour remains; any material-sidecar addition is opt-in |
| STEP, STL, 3MF, SVG, DXF, PDF | Existing CAD workflows | Baseline tests and representative output comparisons remain valid |

glTF defines meshes, materials, textures, skins and animation, making it a useful
first target for the shared scene representation. Follow the
[Khronos specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html)
(reviewed 2026-10-31).

Investigate FBX during the first milestone. Check actual writing capability, skin
and animation support, maintenance, native/WASM constraints, and license compatibility
before selecting a backend. Import-only support is insufficient. Compare a native
writer with an optional external converter; record installation, redistribution and
runtime costs. The user approved the native helper approach; the reviewed v12
Assimp/ufbx path is experimental, with production distribution/platform gates
still open. Do not silently remove FBX scope.

The isolated Assimp GLB-to-FBX audit now shows concrete limitations on authored
PBR maps: roughness/metallic/occlusion connections are lost and ufbx resolves a
different roughness. Raw conversion also shrinks world extents by 100; explicit
metre metadata corrects scale in the static fixture, but leaves material loss.
Debug/Release agree. Production selection therefore still requires a material
adapter/engine contract or another backend; the initial FBX feasibility success
does not establish a complete game-asset conversion route.

The pinned writer's fixed legacy material mapping and a Debug/Release custom-property
probe also show that a public `aiMaterial` override alone cannot repair these gaps.
A writer extension, a different native backend, or a tested external converter is
needed. MCPHub tools/recipes exposed no FBX converter and none was found on PATH;
no tooling was installed. The choice between native and external investigation
and the first engine/version remains pending.

Required import targets include Unity, Unreal Engine, Godot and other OpenGL-based
engines. Unity 6000.4.7f1 and 6000.5.4f1 have concrete fixture evidence; other
consumer versions remain unverified. Store a tested format/engine/version matrix;
report unsupported material and animation features instead of silently losing them.
Collision meshes, sockets, LOD naming and root-motion conventions belong in explicit
engine profiles, with generic export still available.

### Shader support

The installed renderer was inspected on 2026-10-31. `egui_bench` 0.9.2 has fixed
private raster/trace WGSL and public PBR parameters, with no public shader injection
hook. A concrete template-extension proposal, alternatives, fallback behavior and
verification gates are recorded in the final section of
[game-assets-design.md](game-assets-design.md#shader-integration-feasibility-and-proposed-first-slice-2026-10-31).
The user has selected the constrained UV-scrolling PBR template. The renderer is
already vendored for approved PBR/texture extensions; the initial shader-description
library now exists. Preview, reload, export fallback and the M5 engine adapter
remain outstanding. See the current-direction entry for implementation status.

After the material and animation pipeline is stable, add a versioned material/shader
description with parameters, texture bindings and supported rendering features.
Investigate a constrained graph or template system before arbitrary source code.
Preview through the existing renderer and isolate any required egui_bench changes.

Engine shader adapters are separate deliverables. FBX or glTF export must not imply
that arbitrary shader code transfers between engines. Supply a PBR fallback or baked
textures where possible, and report unsupported effects. Test compilation errors,
resource limits, hot reload and the selected desktop/browser targets.

## Milestones and acceptance

Scheduling depends on the feasibility results and engine/scope decisions.

| Milestone | Depends on | Exit condition |
| --- | --- | --- |
| M0: Baseline and feasibility | None | Record native baseline results and existing failures; choose first engine; prototype static and skinned animated FBX export; propose asset schema and dependency/license decisions |
| M1: Static textured assets | M0 | A CAD-derived prop exports to GLB with UVs, tangents and packed textures; imports at correct scale and appearance; old CAD checks pass |
| M2: Rigid motion and FBX | M1 | An articulated assembly exports with named clips to GLB and FBX and plays correctly in the chosen engine |
| M3: Deformable rigs | M2 | A weighted bending/twisting fixture exports with bind poses and multiple clips; sampled engine poses agree within declared tolerances |
| M4: Rig-aware optimisation | M3 | Static and skinned LODs meet recorded size/performance goals and deformation error budgets |
| M5: Shader support | M4 | A parameterised shader previews and has one tested engine adapter plus documented fallback behaviour |
| M6: Upstream integration | Throughout | Small reviewable changes, documentation, fixtures and regression evidence are prepared; maintainer acceptance remains external |

Prepare separate pull requests for the asset representation, static export, material
portability, animation, FBX adapter, optimisation and shaders as dependencies permit.
Keep unrelated fixes separate. Preserve upstream history and license notices, sync
regularly, and discuss extension boundaries before a broad API change. Sending an
upstream proposal or PR is a future action; no maintainer agreement is assumed.

Local [review preparation](game-assets-review.md), reviewed 2026-10-31, contains
29 local patch artifacts with scoped validation; 0024 is independently based on the baseline. No commits or external
submissions were made. The review document records exact prerequisites, hashes,
commands, logs and limitations; the current packaging status is:

| Patches | Packaged scope | Isolated validation |
| --- | --- | --- |
| 0001-0002 | Shared procedural generator and input recording | Three texture tests; two recording integration tests and three virtual-recorder tests; WASM checks |
| 0003-0006 | Mesh, portable materials/maps, scene/clips and mesh preparation | 37, 41, 46 and 56 library tests respectively; WASM checks |
| 0007-0008 | Source provenance/model helper and explicit skin binding | 65 and 72 library tests, plus two input-recording tests each; WASM checks |
| 0009-0011 | GLB encoding, procedural baking and atlas rebaking | 78, 82 and 86 library tests, plus two input-recording tests each; WASM checks |
| 0012 | Authoring, constrained motion and surface settings | 101 library and two input-recording tests; WASM check |
| 0013-0014 | Translation/rigid root motion and diagnostics | 109 and 113 library tests, plus two input-recording tests each; WASM checks |
| 0015 | Sampled clip reduction | 115 library and two input-recording tests; WASM check |
| 0016 | Sampled surface-distance and silhouette metrics | 118 library and two input-recording tests; WASM check |
| 0017 | High-to-low normal-map projection | 127 library and two input-recording tests; WASM check |
| 0018 | Pose-aware LOD and sampled acceptance gates | 131 library and two input-recording tests; WASM check |
| 0019 | Native asset preview adapter | 133 library and two input-recording tests; software snapshot inspected; WASM exclusion check |
| 0020 | Feature-gated CLI dispatch | Release build, feature-disabled CLI compile and smoke checks for all asset commands; details and limits in review record |
| 0021 | Rust fixture generators and matching debug entries | Release/Debug builds and execution; seven GLB modes; reference parity and overwrite rejection |
| 0022 | Remaining authored fixtures and preparation script | 25 fixtures checked on clean and repeated runs; nine generated weight files preserved on rerun |
| 0023 | Native import/FBX probes and debug entries | Existing Debug/Release builds; 14 GLB imports and two synthetic FBX audits; known production limitations remain |
| 0024 | Unix helper guards for stored-part tests | Independent baseline Windows build/test passes; existing Unix-only tests not run |
| 0025 | Documentation snapshot and source reconciliation | Ordered application, local links, whitespace and working-tree parity checks; checksum in external series manifest |
| 0026 | Opt-in UNORM16 skin weights | 135 isolated library tests, two input-recording tests, WASM compilation and CLI byte parity; native Assimp incompatibility documented |
| 0027 | Explicit palette pruning | 136 isolated library tests, two input-recording tests, WASM compilation, CLI remapping/keep/refusal controls |
| 0028 | Encoded clip costs and processing times | 137 isolated library tests, two input-recording tests, WASM compilation and structured-output smoke checks |
| 0029 | Updated evidence and documentation reconciliation | Ordered application, normalized content parity and local link/whitespace checks; no additional runtime claim |

Original module tests are retained. Isolated runs exposed temporary-directory
assumptions in provenance/authoring tests; fixes are also in the main tree. The
review record preserves initial failures and the discarded shared-cache result.
Patch 0012 includes the existing test-only progress-listener isolation correction.
These results cover the isolated library chains, not full integration, browser or
Linux runtime, engine appearance/playback, or continuous-quality guarantees.
The previously recorded full regression suite still has five baseline failures.

Next action: audit the
remaining product requirements against the assembled implementation. Resolve the pending engine, tooling and converter choices
and representative quality budgets. Production FBX, shader support, engine
validation and maintainer acceptance remain outstanding; no milestone is marked
complete by this packaging work.

## Product requirement audit after review packaging

Audited: 2026-11-01. Next review: 2026-11-14 or when engine/backend decisions arrive.
This audit
postdates the immutable documentation snapshots; do not replace their artifacts
or checksums to incorporate subsequent work. The assembled 39-patch snapshot
reconciles all 531 active repository paths and resolves local documentation links.
That proves packaging fidelity, not completion of M0-M6.

| Requirement | Current evidence and remaining gap | Next action |
| --- | --- | --- |
| Preserve CAD authority and existing workflows | Derived asset types are separate; baseline comparison retains five existing failures. Linux and representative cross-format runtime comparisons remain incomplete | Keep failures explicit; complete platform/regression evidence when toolchains are available |
| Stable source/part/instance/face provenance | Guarded source-face lineage now propagates through preparation and material/UV/weight edits. Saved selectors reject changed or ambiguous geometry. Exact bindings are not persistent topological names across arbitrary CAD edits. Native `.gasm` ancestry now survives evaluator caching, asset export and sampled native GLB/FBX checks; broader imported hierarchies and engine acceptance remain open | Preserve explicit recapture/rebinding semantics; extend representative hierarchy coverage |
| Portable materials and mesh preparation | Core PBR maps, factors, box UVs, atlas, crease normals, tangents and normal projection exist with native import evidence. Chart-safe explicit mip baking and opt-in sampled mask correction now exist in `src/asset/atlas.rs` and `atlas_coverage.rs`, with limits and footprint refusals. One UV set; arbitrary seam/edge tags and general retopology remain absent. Explicit chains are not transported by GLB | Retain refusal rather than lose mip semantics; select transport/consumer contract and representative appearance criteria |
| Rigging, clips, attachments and root motion | Four-influence skinning, explicit weights, bind matrices, STEP/LINEAR TRS, constrained-motion baking and both root-motion forms are implemented. Engine playback/attachment conventions and accepted tolerances remain unverified | Select engine/version, then implement/test its consumer conventions |
| Cost and performance reporting | `report::Report` counts geometry, materials, primitive instances, source RGBA bytes, animation keys/influences and sampled mask records. Capability reporting measures encoded clip buffer-view bytes. Preview preflight counts retained payloads, upload scratch and GPU texels, with the candidate accounting for all five slots. These exclude source storage, driver allocations, geometry, targets and overlapping frames; timing observations are not engine performance budgets | Set representative budgets and measure actual consumer costs; preserve the distinction between accounting and process/driver memory |
| Rig-aware optimisation | LOD, sampled surface/silhouette/shading gates and key reduction exist. Exact unused skin-palette pruning is now explicit, with remapping and protected animated/parent/keep nodes. An opt-in UNORM16 weight encoder now has sampled error gates; native validation exposes an Assimp UNORM16 incompatibility; target-engine acceptance and skeleton-node deletion remain; size/performance goals and representative camera/pose budgets remain unaccepted | Define explicit budgets; evaluate remaining operations with unoptimised references |
| GLB export | Actual encoder and independent fixture imports pass; `engine_verified` remains false. No tested engine/version matrix exists | Validate scale, maps, skinning and clips in selected engine |
| Production FBX | CLI explicitly permits GLB only. The pinned v10 native importer/writer experiment and custom material/basis contract are described in `game-assets-fbx-backend-proposal.md`; they are not a production converter. Handshake, transactional output, cancellation, release packaging and a selected consumer remain absent | Select the proposed helper route or an alternative, then implement its production boundary and consumer adapter |
| Engine-specific profiles | Unity root motion has static/skinned fractional STEP and loop/seek evidence. Explicit socket/collider/LOD roles now survive GLB/FBX transport and have initial Unity component/box-ray acceptance. Rendered LOD switching, mesh/convex cooking, skinned LODs, Animator controller integration and other engines remain open | Complete runtime/visual profile acceptance |
| Shader description, preview and adapter | UV-scrolling authoring, tracked reload, raster preview and explicit frozen export are implemented with Windows GPU/reference checks. Dynamic engine adapter, broader maps/platforms and M4 dependency acceptance remain | Implement dynamic consumer parity and remaining acceptance gates |
| Desktop/browser/platform support | Historical native CPU/WASM checks retain their scopes. The isolated Windows renderer now passes Direct3D 12/Vulkan alpha, explicit mips and five-map rigid/skinned tests: `target/renderer-extra-v3-verified` and `target/gcad-extra-motion-verified` (140 asset tests per backend). This is not active integration, interactive control verification, engine parity, browser runtime or Linux evidence | Decide renderer adoption; complete interactive and target-platform runtime validation with supported tooling |
| Review and upstream integration | The 39-patch series now includes renderer/FBX experiments and reconciled documentation; all 531 active paths match after text line-ending normalization. The 17 embedded patch pins remain exact. No commit, PR or maintainer agreement | Preserve the frozen series; package subsequent implementation separately. External submission still needs explicit instruction |

No milestone exit is proven by this audit. Encoded clip payloads and reporting-command timings are now implemented and verified. Engine-specific
implementation and production FBX selection must not silently assume unresolved choices. IK/morph targets remain later extensions, as originally scoped.

### Concrete outstanding decisions and executable follow-up

Reviewed: 2026-11-01. Review by 2026-11-14. The table records selected approaches,
remaining decisions and their validation requirements.

| Decision | Reviewable choice and consequence | Next evidence |
| --- | --- | --- |
| Engine coverage and representative budgets | The target engines are Unity, Unreal, Godot and other OpenGL-based engines; Unity delivery is FBX with separate PNGs. Unity 6000.4.7f1/6000.5.4f1 checks exist; broader engines and representative production budgets remain open | Validate consumers; define application-specific quality/performance targets |
| Renderer adoption — approved 2026-11-01 | The pinned project-local renderer contains the four renderer patches plus three gcad adapters. Active build/runtime checks accompany adoption; interactive/platform checks and software/tracing limitations remain explicit | Complete adoption verification and maintain the patched dependency |
| Production FBX route | The native helper approach is selected. The pinned Assimp/ufbx helper and explicit Unity adapter are implemented experimentally; production packaging and broader platform/consumer acceptance remain | Complete packaging and acceptance; no route reselection is required |
| Explicit mip delivery | Retain chain-export refusal until a selected texture container/format extension and consumer support preserve exact levels, colour space, maximum LOD and sampler restrictions. Dropping levels or generating arbitrary consumer mips is not equivalent | Define consumer constraints; prepare and validate a transport design |
| Shader architecture | The constrained UV-scrolling PBR template is selected; initial typed schema is implemented. M5 acceptance still requires preview, reload, fallback, engine parity and its dependency gates | Implement and verify the selected template; no arbitrary shader transfer is promised |

Independent follow-up work includes packaging post-0032 changes, reconciling source
and documentation prerequisites, and retaining evidence for review. Check platform
and tooling availability for each runtime validation task.

### Face-provenance investigation (2026-10-31)

The post-0029 audit identified an independent implementation path for the remaining
face-binding requirement. `select::Groups` already records body, operation label,
group and analytic surface, and `select_faces` resolves those expressions against
the current solid. `geometry::mesh` merges triangulated shells into a polygon mesh;
`asset::Mesh::from_solid` then retains complete vertex tuples but loses source-face
association. The existing topology `FaceId` is used as an in-process cache key;
it is not an established persisted cross-regeneration identifier.

Next: investigate retaining per-triangle source-face association through
tessellation and derived operations, with source/geometry guards and explicit
selector resolution. Preserve named selectors as author intent, reject changed or
ambiguous bindings, and do not present face-array positions or runtime `FaceId`s as
stable identities. Arbitrary topological naming across boolean splits/merges is
not yet solved. Validate regenerated, reordered, split and missing-face cases before
exposing face-bound material, UV or weight authoring. This work is independent of
the unanswered engine/FBX/shader decisions. Review date: 2026-11-14.

The initial `asset::faces` library now returns an ordinary mesh plus a guarded
triangle-to-face map. It retains per-face tessellation before merging and refuses
missing/non-triangle face meshes. Exact face fingerprints ignore triangle ordering
and cyclic corner rotation while retaining positions, normals, winding and
multiplicity. Selection resolution can remap reordered faces, but rejects changed,
missing or ambiguous geometry. The map's mesh stamp rejects stale derived meshes.
These are geometry guards, not persistent CAD feature identifiers or source identity.
The geometry-only API does not itself establish source identity. Named-selector
intent, source guards and explicit propagation through derived operations are
required before exposing authoring commands. The original `Mesh::from_solid` path
uses the same extracted polygon conversion helper and keeps its behavior.

All 140 feature-enabled library tests and WASM library compilation pass. Three
new tests cover named end-face selection, independent regeneration, reversed face
order, duplicate/missing/changed geometry, stale refinement/normal guards, and exact
ordinary-mesh parity for both a box and the curved plate fixture. Evidence:
`target/game-assets-faces-library.log` and `target/game-assets-faces-wasm.log`.
Split/merge propagation and source-bound persisted authoring still require work;
no continuous topological identity or engine behavior is certified. These additions
postdate the unchanged 0029 review snapshot.

`faces::SelectionFile` now provides a version-1 JSON record combining validated
`provenance::Inputs`, selector text and sorted unique face fingerprints. Capture and
resolve tessellate the supplied solid using the recorded tolerance. Resolve requires
identical canonical source observations/parameters/instance/generator, re-evaluates
the selector and requires the same selected geometry. Unknown fields/versions,
invalid portable input identities, empty or duplicate fingerprints and invalid
selectors fail. Selectors currently accept nonempty ASCII without control characters,
up to 4096 bytes. The returned triangle indices address the returned raw tessellation
only; they do not yet propagate through UV splitting, refinement, slicing or LOD.
No automatic update or rebinding occurs. Callers must supply observations of the
actual evaluated source, as with existing skin bindings.

Five focused face tests pass, including JSON roundtrip, source/instance/observation/
tolerance changes, selector rename/redirection, geometry changes and schema failures.
Derived-mesh propagation remains implementation work; this library record
is not yet an authored face-material/UV/weight operation.
The full feature-enabled library suite now passes 142 tests; WASM compilation also
passes. Evidence: `target/game-assets-face-selection-library.log` and
`target/game-assets-face-selection-wasm.log`. No full repository regression or engine
validation is inferred from these scoped checks.

`face_author::load` now connects the record to actual CAD evaluation observations,
including cached imported-part dependencies. It enforces the declared source/body
scope and project root, restores assembly solids and named face groups to the same
local coordinates, and leaves maps/skins/clips out of this raw-geometry operation.
`asset-face-bind` captures a new JSON record; `asset-face-check` reads and resolves
one without updating it. Both use declared source names, with an explicit `--body`
for wildcard assembly sources. Matching Windows debug entries are included.
Derived-mesh propagation and face-bound rendering/weight operations remain next; no automatic stable naming across arbitrary boolean edits is implied.

### Face lineage through mesh preparation (2026-11-01)

`FaceMap::refined` now propagates source-face labels at the point where each parent
triangle emits its four children. Multiple refinement levels retain the original
face fingerprints while updating the derived mesh stamp; ordinary `Mesh::refined`
does not allocate labels. Count, depth, triangle-budget and pre-UV requirements are
shared with ordinary refinement. Tests preserve selected-face area/winding, exact
ordinary-mesh parity and source immutability across zero, one and two levels, and
reject stale maps and invalid budgets.

`FaceMap::crease_normals` and `FaceMap::project_box` retain labels only after checking
exact triangle corner-position/order correspondence. This accommodates vertex
splitting or joining without guessing source ownership from proximity. Source-face
fingerprints remain lineage identifiers; a new stamp guards the prepared geometry.
The authored preparation
pipeline/CLI has not yet adopted these companion-map operations. The chained
refinement/normal/UV test preserves the end-face selection and rejects reordered
triangles and stale maps. All 146 feature-enabled library tests and WASM compilation
pass (`target/game-assets-face-preparation-library.log` and
`target/game-assets-face-preparation-wasm.log`). Next: connect the remaining
operations before adding face-bound material/UV/weight edits. Review date: 2026-11-14.

`FaceMap::sliced` now emits each child's source-face label alongside its triangle,
including unchanged triangles and repeated slicing planes. The ordinary mesh path
does not allocate labels. Tests check exact ordinary-mesh parity, selected end-face
area/winding, no-intersection planes, propagation into box UVs, source preservation,
stale-map rejection, invalid label counts and triangle budgets. All 147 feature-enabled
library tests and WASM compilation pass (`target/game-assets-face-slicing-library.log`
and `target/game-assets-face-slicing-wasm.log`). Authoring integration remains.

`Lod::source_triangles` now records original triangle indices in output order,
verified against the final vertex remap. `FaceMap::simplified` adds locks at every
vertex shared by different source faces, retains caller attachment locks and ordinary
LOD seam/crease locks, propagates surviving triangle ownership, and rejects loss of
an entire face. The triangle target may be unreachable under these constraints.
An adjacent coplanar-face test reduces geometry while preserving both face areas,
winding, the shared boundary, attachment positions, explicit lineage and selection
resolution with a supplied pose. Stale maps and invalid attachment indices fail;
the source stays unchanged. All 148 feature-enabled library tests and WASM compilation
pass (`target/game-assets-face-lod-library.log` and
`target/game-assets-face-lod-wasm.log`). This verifies library lineage, not engine
acceptance or face-aware authored exports. Review date: 2026-11-14.

### Prepared authored face maps (2026-11-01)

`author::load_with_faces` now carries companion maps through the ordinary authored
preparation pipeline: refinement or slicing, crease normals, baked/rotated box UVs
and atlas packing. Triangle corner correspondence is checked after UV/tangent and
atlas processing. `Asset::face_map(node)` checks current geometry before returning
the node's map. Maps belong to nodes rather than shared meshes, preserving distinct
CAD ownership when repeated assembly parts share identical render geometry. Ordinary
loading and explicit weight rebinding do not collect face maps.

The integration test evaluates two placed copies of a CAD box with four preparation
variants, checks end-face area/winding and source selection resolution, verifies stale
map rejection, and confirms identical GLB bytes versus ordinary loading with mesh
sharing retained. All 149 feature-enabled library tests and WASM compilation pass
(`target/game-assets-face-pipeline-library.log` and
`target/game-assets-face-pipeline-wasm.log`). Formatting and diff whitespace checks
pass. These maps provide geometry lineage; persisted source-record validation and
prepared-selection CLI integration still need connecting before face-bound edits.
The persisted-selection integration is recorded below.
Review date: 2026-11-14.

### Persisted selections on prepared nodes (2026-11-01)

`Asset::resolve_faces(node, record)` now validates the record's source inputs and
semantic selector using retained solids, face groups and CAD observations from the
same evaluation used to prepare the asset. Face observations are captured before
explicit render-map reads, matching geometry-only capture semantics; ordinary skin
provenance still includes its original texture observations. No second CAD evaluation
or on-disk recheck is hidden in the API: reload the asset to inspect later changes.
The prepared map's geometry stamp is checked before returning triangle indices.

`asset-face-resolve FILE.gasset NODE SELECTION.json` loads the full authored asset and
returns transient prepared primitive-0 triangle indices as JSON. It leaves the record
and asset unchanged. Generated assembly children use `INSTANCE/BODY`; a saved record
for another CAD body fails even when both bodies share identical render geometry.
The existing raw capture/check commands retain their behavior.

### Derived face-material assignment (2026-11-01)

`face_material::assign` and `asset-face-material FILE NODE SELECTION OUTPUT.glb
--surface KEY=VALUE` now apply existing surface overrides to validated saved faces.
The derived scene splits selected/unselected triangles into material primitives,
compacts complete vertex tuples without altering values, and remaps skin weights
exactly. Inverse bind matrices must remain identical. A shared mesh is copied for
the selected node; other instances retain their original mesh/material/skin data.
All-face assignment produces one primitive. Source assets, selection records and
weight resources remain unchanged, and the output must be new. This is a derived
export; persisted `.gasset` face-material declarations and UV/weight editing remain.

### Independent face-material import (2026-11-01)

The pinned Assimp probe now compares paired reference/assigned GLBs for a specified
node whose `base.end` faces lie on the maximum glTF-Y plane. It verifies per-face
red/roughness-0.2 assignment, retained metallic factors, embedded map bytes, complete
corner attributes, oriented triangle multiplicity, bone names/weights/inverse binds,
node hierarchy/transforms and clip keys. Seventeen synthetic affine bone poses
compare weighted positions across the primitive split. This is fixture-specific
import evidence, not engine playback, rendered shading or all-face assignment coverage.

Debug and Release both pass shared-box (96 total instance triangles, 8 selected),
six-map plate (1276/90) and skinned plate (1276/90) cases. The skin case checks 65,076
posed corners per build. Unmodified candidate controls fail; deliberate weight and
winding corruptions fail the semantic comparison in both builds. The corruption
script initially assumed mesh 0 was skinned; selecting an actual WEIGHTS_0 primitive
corrected that test assumption. Existing pinned dependency checkouts remain unchanged.
Evidence: `target/game-assets-face-material-probe-{release,debug}-build.log` and
`target/game-assets-face-material-import/` scripts, GLBs and stdout/stderr records.
Next action: add persisted face-material authoring declarations. Review date:
2026-11-14.

### Persistent face-material declarations (2026-11-01)

The `.gasset` operation `face_material NODE SELECTION.json KEY=VALUE ...` now uses
the existing surface settings and saved source-bound selections during ordinary
loading. Multiple disjoint regions on one node are resolved before partitioning;
overlaps fail even when settings agree. Missing targets/resources and stale records
fail loading. Assignment remains per instance, retaining other shared instances.
Skin resources bind the original prepared geometry, then weights remap through
material partitions. Explicit weight rebinding validates declarations but returns
the pre-material geometry, preserving the weight-file contract.

The initial test exposed that the common parser rejects hyphenated operation names;
the declaration uses `face_material` to follow its existing identifier rules, while
the CLI remains `asset-face-material`. All 151 feature-enabled library tests and
WASM compilation pass, including two disjoint regions on a skinned node, three
primitives, derived API/export parity, identical weight-file bytes after rebinding,
overlap/missing-target/missing-record rejection and stale-source failure. Evidence:
`target/game-assets-face-declarations-library.log` and
`target/game-assets-face-declarations-wasm.log`.

Current limitation: nodes partitioned by declarations discard their former
single-primitive face maps. Prepared face queries and additional derived assignment
reject those nodes; raw source capture/check remains available. Next action: extend guarded prepared selections to multiple material primitives. Review date:
2026-11-14.

### Prepared selections across material primitives (2026-11-01)

Material partitioning now retains one face map per output primitive. Explicit
source-triangle subsets must have valid unique indices and identical corner positions,
normals and winding before receiving a new geometry stamp. Each map retains original
source-face fingerprints. `Asset::primitive_face_maps` validates every primitive;
`resolve_face_primitives` returns one primitive-local triangle list per primitive,
including empty lists. Legacy single-primitive methods reject ambiguous multi-primitive
use. `asset-face-resolve` retains version-1 JSON for a single primitive and emits
version 2 with a `primitives` array for partitioned nodes.

All 151 library tests pass, including selected end/bottom faces, complete all-face
coverage across three primitives, exact local indices and rejection after changing
an unselected primitive. WASM and default-feature checks pass. Evidence:
`target/game-assets-face-primitives-{library,wasm,default}.log`. The prior query
limitation is resolved; additional derived face-material assignments on partitioned
nodes remain unsupported. Next action: generalize derived material assignment
across existing primitives without losing inherited per-primitive materials or skin
weights. Review date: 2026-11-14.

### Material edits across existing primitives (2026-11-01)

Derived face-material assignment now resolves selections across all current material
primitives. Overlap checks use primitive/triangle pairs. Each affected region inherits
its own source primitive's material, while unselected regions retain their current
material. Complete tuples and skin weights are copied from that same primitive;
`Assigned::source_primitives` disambiguates its vertex/triangle remaps. Prepared
provenance propagation consumes the explicit source-primitive mapping.

All 151 library tests pass. The expanded skinned test edits roughness across three
differently coloured regions, checks exact inherited material factors and weights,
compares three deformed poses, edits only the end region without changing the others,
rejects overlapping cross-primitive selections, and preserves source GLB bytes.
WASM/default-feature checks pass. Evidence:
`target/game-assets-material-multiprim-{library,wasm,default}.log`.
The prior additional-assignment limitation is resolved. Next action: extend
independent import checks to mixed-material edits on already partitioned skins and
adds reusable authored examples. Review date: 2026-11-14.

### Reusable face-material example and mixed-material import (2026-11-01)

The paired Assimp probe's `--face-roughness` mode checks a 0.25 whole-node roughness
edit on an already partitioned reference. Debug/Release preserve three primitive
regions, inherited colours/metallic factors, corner attributes, weights/inverse
binds, clips/hierarchy and 65,076 synthetic posed-corner results. Unchanged roughness
and wrong-colour controls fail in both builds. Evidence:
`target/game-assets-face-roughness-{release,debug}-build.log`,
`target/game-assets-face-roughness-import/`, and
`target/game-assets-face-example-{prepare,repeat}.{stdout,stderr}`.

Flat and time-0.5 software snapshots were inspected: red top, neutral walls and
open holes remain visible. The blue underside is hidden from this camera, so its
verification is the importer check. Images: `target/game-assets-face-example-flat.png`
and `target/game-assets-face-example-bent.png`. This retains the original diagnostic
skin deformation and does not establish GPU/engine or production quality acceptance.
All 31 debug configurations parse with existing task references; debugger interaction
remains unverified. Next action: review and package the post-0029 face-provenance
and material additions for isolated regression testing. Review date: 2026-11-14.

### Post-snapshot reporting progress (2026-10-31)

Capability JSON now lists each clip's actual encoded input/output buffer-view
payload bytes, excluding JSON/alignment/container overhead. The capability and
sampled-report CLI commands include cache-dependent evaluation and processing
seconds, excluding serialization, output I/O and engine work. The deterministic
library report itself does not acquire a native clock or depend on CLI timings.

Four focused capability tests pass, including known float32 translation/rotation/
scale payload sizes, empty clips and existing encoder failure handling. The release
CLI builds; actual `plate-motion.gasset` reports parse as JSON with 32-byte slide
and 40-byte swing payloads and finite nonnegative timing fields. An initial smoke
assertion assumed clip ordering; checking names resolved that test assumption
without changing implementation. Feature-enabled WASM library compilation passes.
Evidence: `target/game-assets-capability-cost-tests.log`,
`game-assets-cost-cli-build.log`, `game-assets-cost-capabilities.json`,
`game-assets-cost-report.json` and `game-assets-cost-wasm-check.log` in `target`.
No full regression rerun or engine benchmark is implied. The 25-patch review
snapshot remains unchanged and does not include this subsequent implementation.

### Derived-operation timing progress (2026-10-31)

Normal baking, LOD and key reduction now include evaluation, derive/encode and
open/write seconds in their diagnostic JSON. Root motion emits the same timing
shape as a separate stderr JSON line, preserving the strict trajectory record on
stdout. The measurements explicitly distinguish buffered writes from durable
storage and exclude engine work; root processing includes trajectory serialization.

### Explicit palette pruning progress (2026-10-31)

`Skin::prune_palette` and `asset-prune-joints` now remove unused palette entries
without deleting scene nodes or approximating weights. Positive influences,
animation references, parent nodes and explicit keep nodes retain their entries.
The command exports a new GLB and reports original/retained counts and an explicit
old-to-new mapping. This is palette compaction, not general skeleton deletion or
an accepted runtime optimisation profile.

Eight skin tests pass, including a new test forcing index remapping before weighted
joints, exact sampled position/shading preservation, source immutability, protected
animation/attachment/keep nodes, stale guards, idempotence and GLB node/clip retention.
A CLI fixture removes one of two joints and reduces encoded size from 183128 to
183064 bytes; a keep-list control retains both. Source weight hashes are unchanged,
existing-output overwrite is rejected, and an unknown keep node fails without an
output. Feature-enabled WASM library and feature-disabled CLI checks pass.
Evidence under `target`: `game-assets-palette-skin-tests.log`,
`game-assets-palette-build.log`, `game-assets-palette-wasm.log`,
`game-assets-palette-default.log` and `game-assets-palette-smoke/`.
Engine playback and performance acceptance remain unverified; no full regression
rerun is inferred. The original 25-patch snapshot is unchanged.

### Opt-in weight quantisation (2026-10-31)

`asset::quantize::encode` now derives UNORM16 weights and exports actual normalized
unsigned-short GLB accessors. Largest-remainder allocation preserves zero slots
and gives every vertex an exact integer sum of 65535. Source bindings remain
unchanged. Explicit position and shading-angle limits gate output using rest pose,
uniform clip samples, authored keys and STEP left limits. Reports include sample
times, measured maxima and weight-buffer payload bytes before/after. These bounds
cover sampled weight changes only, not other float32 encoding or engine errors.
The core encoding follows the [glTF skin specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#skins).

Quantisation is additionally packaged as patch 0026 in
[the review record](game-assets-review.md#patch-0026-opt-in-sampled-skin-weight-quantisation).
It applies after the original series and passes 135 isolated library tests, two
input-recording tests and WASM compilation; its CLI output matches the independently
checked GLB byte-for-byte. Palette pruning is separately packaged as 0027 (136
isolated library tests and two integration tests); encoded costs and processing
times are packaged as 0028 (137 library tests and two integration tests). Both
WASM checks and their scoped CLI smokes pass. Patch 0029 reconciles the updated
documentation and evidence with the assembled implementation; no external submission
has occurred. Next: resolve engine/version, production FBX route and shader architecture, then implement and validate the selected integrations.

## Verification

Run the documented native regression suite (`cargo test --release`) after checking
the dependency toolchain, and separate GPU-required tests from CPU coverage. Preserve
all currently supported operations, CLI paths, assemblies, simulations and drawing
exports; record existing failures so new regressions can be identified.

Add a textured prop with UV seams, a mirrored assembly, a rigid articulated mechanism,
a two-bone bending mesh, a twisting joint and an attachment-bearing rig. Check semantic
scene data as well as rendered appearance. Validate exports with an independent reader
and actual engine import; an exporter reading its own output is insufficient.
Record tool versions, fixtures, hashes, units, tolerances and measured results.

As of 2026-10-31, the CAD-derived `plate-attachments.gasset` adds bend/twist
clips and nested named attachment frames. Independent Debug/Release imports agree
with analytic skin and attachment transforms over 66 poses. Strict CPU shading
diagnostics pass, but measured twist shrinkage/stretch still requires deformation
quality review. These are ordinary scene nodes; engine socket conventions and
engine-pose acceptance remain unverified. Commands and measurements are recorded
in the design document.

Explicit piecewise weight authoring (`asset-bind --chain`) now supports ordered
joint knots with two neighbouring influences per vertex. A five-bone CAD twist
passes independent Debug/Release import checks. Radial shrinkage is reduced, but
triangle distortion remains severe; deformation-aligned retopology is the next
independent investigation. This is not M3/M4 quality acceptance.

The first aligned-subdivision experiment is now available as `slice` source
settings and `plate-twist-sliced.gasset`. It reduces this fixture from 20416 to
6104 triangles and improves sampled area ratios, with independent Debug/Release
checks of skin, attachments, plane boundaries and rest area/volume. Edge distortion
remains substantial. This does not replace general retopology or engine acceptance;
next evaluate continuous deformation-surface fidelity and topology choices.

Interior-point comparisons now expose error missed by vertex checks. A finer
33-plane fixture stays within the 20416-triangle budget and reduces sampled maximum
correspondence error from 0.557 to 0.384 mm versus uniform refinement; RMS improves
but p95 is slightly worse. Debug/Release probes agree. These are discrete samples
of a continuous analytic field, not a continuous bound or engine acceptance.
Deformation quality tolerances remain undefined; test the
selected engine and representative assets against those tolerances.

Run Windows and Linux checks, plus browser compatibility checks for shared changes.

Accumulated Windows release regression review on 2026-10-31 reproduces the default
baseline exactly: 435 passed, five known failures, 13 ignored. The full feature-enabled
run exposed a viewer-test global-progress listener collision with concurrent asset
tests; the test now uses the existing scoped listener and an unrelated-thread
regression control. The complete feature-enabled release library rerun passes 118
tests. Integration failures remain the baseline five. Detailed run scope and logs
are recorded in the design document; Linux/browser/GPU verification remains open.

Earlier accumulated full release runs (2026-10-31), after source-recording and crease
normal changes: default 438 passed / five failed / 13 ignored; feature-enabled
535 passed / five failed / 13 ignored. Exact failure names match the baseline in
both runs. The feature-enabled library contributes 133 passing tests. These are
regression comparisons, not claims that the full suite is green. Logs:
`target/game-assets-final-default-regression.log` and
`target/game-assets-final-enabled-regression.log`.

Latest complete Windows regression comparison (2026-10-31), after clip-size,
structured-timing and palette-pruning additions: default **440 passed, five failed,
13 ignored**; feature-enabled **539 passed, five failed, 13 ignored**. All 135
feature-enabled library tests pass. Both complete runs used `cargo test --release
--locked --no-fail-fast` (with `--features game-assets` for the enabled run).

Failure names exactly match the earlier baseline in both configurations:
`a_job_from_another_build_of_the_same_line_is_accepted`,
`a_static_job_packed_earlier_decodes_to_the_same_problem`,
`a_thermal_job_packed_earlier_decodes_to_the_same_problem`,
`every_operation_has_a_test_that_matches_its_status`, and
`probe_on_a_curved_face_snaps_to_the_mesh`. Both commands return failure status
because these failures remain; neither suite is green. Relative to the earlier
full runs, two input-recording integration tests now contribute in each build,
and two new feature-only tests cover encoded clip payloads and palette pruning.

Evidence: `target/game-assets-post-audit-default-regression.log`,
`target/game-assets-post-audit-enabled-regression.log` and
`target/game-assets-post-audit-regression-summary.json`. Cargo.lock is unchanged.
This supplies current native regression evidence; it does not establish Linux,
GPU/browser runtime, engine acceptance or production FBX/shader support.
List skipped or unavailable checks explicitly. See the feasibility document for
native build/test results and limitations. The prototype does not establish engine
or game-export support.

The WASM library now passes `cargo check` both with and without `game-assets` on
installed Rust 1.99.1. This caught and fixed a native-only texture-loader regression
by using the existing host-file abstraction; native texture/provenance tests still
pass. Full threaded-web/runtime checks remain unavailable without nightly/tooling
setup. Browser dependency recording and portable source guards are now implemented;
their logic has native tests, but end-to-end browser authoring remains unverified.
Native dependency recording now also retains conflicting observations from nested
and worker evaluations, so source guards reject inputs changed during evaluation
instead of retaining only their final observed version.
WSL is available but Cargo/rustc are not on its PATH; Linux builds remain unverified.
The standard WSL per-user Rust location was absent at that check.
Independent Debug/Release FBX audits now verify the authored two-bone plate's 66
sampled bend/twist poses and attachment positions after metre metadata correction.
Maximum position error is `5.25e-10 m`; the raw route fails from a 100-fold unit
mismatch. PBR losses remain, so this does not establish a production FBX backend.

## Risks, dependencies and open choices

| Risk or dependency | Response |
| --- | --- |
| FBX backend cannot meet feature or distribution requirements | Resolve in M0 with a skeletal animation fixture before committing to a backend |
| CAD topology changes invalidate asset bindings | Persist provenance; detect stale bindings; define rebuild and rebinding rules |
| CAD tessellation deforms poorly | Compare retopology and mesh authoring approaches on bending/twisting fixtures |
| Renderer changes require another repository | Identify egui_bench extension points and coordinate independently versioned changes |
| Engine import loses materials, bones or motion | Pin tested engine profiles and publish explicit capability reports |
| Scope slows upstream review | Deliver additive modules and small independently useful PRs |

Confirmed direction: target Unity, Unreal, Godot and other OpenGL-based engines;
Unity delivery uses FBX with separate PNGs. The native helper route and initial
UV-scrolling PBR template are approved. Unity 6000.4.7f1 and 6000.5.4f1 are the
current installed validation versions. Open questions remain around representative
production assets, polygon/texture/bone/animation budgets and upstream authoring
syntax. Engine installation/access for broader consumer execution remains pending.

## Next actions

- [x] M0: Record baseline commands, tests and native toolchain results.
- [x] M0: Record cross-engine scope, Unity FBX/PNG delivery, helper route and first shader.
- [ ] M0: Supply representative production assets and quality/performance budgets.
- [x] M0: Prove an FBX export of a textured mesh with a two-bone rig and two clips;
  document backend options and licenses. Native Assimp/ufbx probe
  passed on Windows; production backend selection and engine import remain open.
- [x] M0: Draft the scene/schema proposal and topology-rebinding rules for review.
- [ ] M1: Complete static textured GLB acceptance after M0 decisions.
  The CAD-box prototype passes independent import with UVs, tangents and packed
  PNGs, including core normal/occlusion/emissive/alpha/sampler settings. Engine
  appearance, general baking, CAD assembly/provenance and authoring integration
  remain. Scene hierarchy, shared meshes, multiple primitives and mirrored,
  nonuniform transforms now pass independent import checks.
  CPU baking of existing procedural tiles is implemented, with a wood fixture
  passing independent import. Atlas chart packing and six-map rebaking now pass
  library tests (reviewed 2026-10-31). Explicit per-source atlas authoring now
  connects packing/rebaking to export; CAD skin weights survive the UV remap.
  The textured CAD box now passes independent Debug/Release import and PNG pixel
  checks across six charts and five packed textures, including padding. General
  filtering quality and full chart-safe mip integration remain. CPU explicit-level
  baking now refuses overlapping bilinear chart footprints and passes boundary
  sampling controls; level storage, export and runtime LOD bounds remain open
  (reviewed 2026-11-01). A CPU high-to-low normal
  projection library now provides bounded bidirectional rays, a BVH and explicit
  miss coverage. Bounded padding and `asset-bake-normals` export now preserve
  shared instances and skin bindings. Independent Debug/Release checks verify
  analytic slope transfer, mirrored tangent space and padding to 0.227 degrees.
  Optional baking is now connected to LOD export with combined reporting and
  skinned-binding checks. The CAD fixture reaches 5000 from 5104 triangles with
  zero missed bake pixels. Independent reduced-CAD normal checks exposed and fixed
  coincident-edge normal selection; a 3000-triangle variant improves sampled normal
  error from 1.574 to 0.319 degrees in Debug/Release. Engine validation remains.
  Persisted source/mesh binding guards and a native evaluation-read adapter are
  implemented. `.gasset` check/GLB export and persisted skin weights are connected;
  version-2 weights enforce source-input binding with an explicit project root.
  Evaluated `.gasm` bodies now convert to placed children with per-part materials
  and shared meshes across repeated source instances. Native `.gasm` hierarchy now
  survives export with sampled native GLB/FBX evidence; broader imported hierarchy
  and engine acceptance remain. Explicit constraint-motion baking is connected;
  sampled world-space vertex error budgets are enforced when requested.
  Bounded adaptive uniform sampling now retries up to an explicit rate ceiling.
  The adaptive linkage passes independent Debug/Release checks at 33 keys and
  96 intermediate samples within its `0.001 mm` sampled budget.
  Broader mechanisms and continuous error bounds remain. Generated-child skin
  authoring and explicit binding now preserve per-node source provenance.
  Generated crank skin bind/check/export and independent Debug/Release sampling
  pass with `2.50766e-09 m` maximum position error; the repeated crank stays unskinned.
  Clips now resolve generated parts by `INSTANCE/BODY` after CAD evaluation.
  Independent Debug/Release import checks pass nine sampled crank poses, with
  maximum error `7.45058e-09 m` against the authored motion (`1e-6 m` limit).
  Explicit named instances and parent transforms now share source meshes.
- [ ] M2: Complete rigid animation and production FBX. Shared rigid
  clips, sampled STEP/LINEAR playback, authoring and GLB channels are implemented.
  Explicit joint-motion baking reuses the constraint solver. The linkage passes
  independent analytic checks at 25 keys and 24 midpoints, with maximum errors
  `1.59144e-08 m` and `9.42545e-07 m` respectively. Optional error budgets check
  every render vertex at keys and quarter intervals, including instance scales.
  Authored translation/rotation and LINEAR/STEP scale clips pass independent
  sampled import, including reflection jumps. Engine playback and production FBX
  remain.

- [ ] M3: Complete deformable rigs. CPU inverse-bind and explicit
  four-influence skinning and explicit GLB export are implemented with stale-binding
  guards. A textured two-bone ribbon with nonidentity binds, blended weights and
  bend/twist clips passes independent sampled import. Skin declarations load
  persisted geometry/rig-guarded weights. Explicit rigid and two-joint linear
  weight generation is available through `asset-bind`; CAD deformation quality,
  rendering and engine validation remain.
  Independent authored-plate sampling passes export accuracy, but exposes nearly
  collapsed triangles and 5.07 mm maximum centroid interpolation error. This
  fixture is not accepted for deformable use; retain it as the reference.
  Optional budgeted pre-binding refinement reduces sampled interpolation error
  to 0.317 mm at 16 times the triangle count, but near-collapse persists.
  A centred through-thickness bend avoids near-collapse in the sampled poses;
  narrowing its weight transition worsens interpolation error and area expansion.
  The wide-blend comparison is a candidate, not production quality acceptance.

Reusable JSON asset reporting now counts geometry, instances, source texture bytes,
animation keys and skin influences, and samples area/edge ratios and displacement.
It includes clip keys and STEP left limits. These diagnostics do not establish
rendering quality or continuous surface-error bounds.

CPU inverse-transpose normal and tangent deformation now handles nonuniform scale
and reflection and rejects singular blends; `asset-report --shading` exposes the
strict sampled check. Engine shading and rendered appearance remain unverified.

Native asset preview with playback controls and software snapshots is connected.
Flat/bent and textured plate software snapshots were visually inspected. Texture
adapter checks cover tint, normal convention, channel packing and source preservation.
Subsequent isolated Windows GPU checks cover five-map static, rigid and skinned
rendering on Direct3D 12 and Vulkan, as recorded at the top of this plan. The active
dependency and interactive controls have not gained that acceptance by inference.

Recommended next action: package the post-0032 source changes and candidate
adoption evidence; interactive preview controls remain a later runtime gate.
Native UI checks now run under the project's normal tooling; current acceptance
is recorded at the top of this plan. The evaluated linkage assembly passes independent GLB import with shared
meshes, placements, per-part colour and group animation. Next independent action:
validate the selected engine integration, and review representative
engine-camera budgets. As of 2026-10-31, `asset-capabilities`
provides an in-memory GLB dry run with destination material/texture, skin and
animation counts, encoder failures and explicit experimental/engine limitations.
FBX remains a prototype; encoding success does not certify engine compatibility.
The
endpoint-collapse library now preserves complete tuples and explicit weight remaps,
locks boundaries/attachments, and checks original-to-LOD displacement in supplied
poses. Scene/CLI export is connected and independently verifies a reduced skinned
CAD plate. Rejection-witness caching now reaches its 20000-triangle target in
215 evaluated trials, skipping 1110 unchanged rejections; independent Debug/Release
skin checks pass. Reports now measure bidirectional closest-triangle maximum,
p95 and RMS error at each pose using vertices, edge midpoints and centroids.
The flex-plate fixture's largest sampled bidirectional distances are about
`0.02393 mm` and `0.02254 mm`; this is not a continuous surface bound.
Reports include principal-axis orthographic coverage differences, and optional
surface maximum/p95 and silhouette limits gate export. Independent Debug/Release
scanline raster checks agree across all 54 fixture views, including nonempty-mask
and displacement controls. Engine-camera profiles and engine acceptance remain.
Skinned key reduction
passes independent Debug/Release sampling for a redundant constant-speed bend
(9 to 2 keys) and an accelerating bend (33 to 17 keys). Opt-in key reduction preserves an
unoptimised source and checks geometry, attachment and shading directions against
the original clip at declared samples; continuous bounds remain unproven.
The native helper route and multi-engine scope are approved. Continue validating
target-engine import and production integration. Game export,
engine validation, optimisation, shader support and upstream submission remain.

### Presentation and sprite export (2026-11-02)

Game-asset viewer lighting and model turntables now have independent angle, speed
and looping playback controls. GPU sprite export samples arbitrary camera, model,
lighting and animation settings into a transparent PNG sheet plus JSON rectangles.
The viewer panel and asset-sprites recipe command share the exporter. Direct3D 12
checks cover angle/light/pose differences, alpha recovery, bounds validation,
cancellation and preservation of existing output. Turntable wrap/navigation and
live-document reload checks pass. See [sprite workflow](game-asset-sprites.md).
Remaining: Linux GPU validation and target-engine sprite import.
Next action (owner: user): import a generated sheet with straight alpha and verify
cell rectangles/padding in the intended engine.

### Sprite relighting (2026-11-02)

Sprite recipes and the export panel now control key/fill colour and intensity,
ambient/exposure, and off/hard/soft traced shadows. Optional aligned albedo and
view-relative normal sheets support 2D runtime lighting. Normals include authored
surface normal maps; all sheets share raster coverage and two-texel transparent
RGB padding. Direct3D 12 tests cover material normal contribution, shadow occlusion,
light-invariant runtime sheets, unit normals, axis convention and alpha alignment.
Tracing material limitations are documented in game-asset-sprites.md.
Remaining: target-engine 2D-lighting import and Linux/Vulkan verification.
Next action (owner: user): import albedo.png with normals.png as linear normal data
and verify the selected green-channel convention in the game's 2D material.
