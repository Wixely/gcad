# Unity reference import check

Reviewed: 2026-11-01. Next review: 2026-11-14.

Explicit socket/collider/LOD adaptation is described in
[`game-assets-consumer-profile.md`](../../docs/game-assets-consumer-profile.md).
Use `GCAD_CONSUMER_CHECK=1` with the reviewed `GCAD_FBX_HELPER` and a package
containing `gcad-consumer-profile.json`; `GCAD_CONSUMER_PLAYER=1` adds the Windows
standalone check. The isolated fixture enables Preserve Hierarchy. Ordinary adapter
calls require this importer option already enabled when a consumer receipt exists.
The runner includes Unity's engine-bundled physics module for these checks.
Add `GCAD_CONSUMER_RENDER=1` for D3D11 camera-distance LOD selection/culling
checks and diagnostic PNGs. With the player enabled, the runner also compares
all five editor/player images. `plate-consumer-collision.gasset` covers ring
triangle/convex colliders, including a nonuniform reflected instance.
Add `GCAD_CONSUMER_FAILURE=1` on that ring fixture for native cooking-failure
recovery, source preservation and retry checks; it writes
`consumer-failure-report.json` and requires its final gate to pass.
For `plate-consumer-skinned.gasset`, first run
`sh tools/game-assets-consumer-fixture.sh`, export/package the fixture, and add
`GCAD_CONSUMER_SKIN=1`. It requires both LOD meshes to follow the shared skeleton;
render/player modes also check posed images and ten-image editor/player parity.

Root-motion mode uses `GCAD_ROOT_MOTION_CHECK=1`, `GCAD_ROOT_MOTION_REFERENCE`
(the original GLB) and `GCAD_FBX_HELPER` with an
exported package containing `gcad-root-motion.json`. Add
`GCAD_ROOT_MOTION_PLAYER=1` to build and run the Windows standalone CPU check.
The adapted prefab exposes `GcadRootMotionPlayer.Sample(seconds, looping)`;
the caller owns its clock. It requires an enabled Animator without a runtime
controller and does not integrate Animator transitions, layers or physics.
Disabling the component removes extracted motion from the last sampled pose.

Receipt version 2 includes exact in-place TRS channels and samples them directly;
the component still uses FBX geometry and hierarchy. Version 1 retains the imported
clip path for LINEAR motion but refuses STEP, requiring package re-export.
Both STEP motion schemas pass 38 editor/player cases in both installed Unity
versions (6000.4.7f1 and 6000.5.4f1), including times 10 nanoseconds before/after
keys. The oracle reads source GLB transforms independently and applies them to
imported mesh geometry; its frozen expected positions are included in the test
player. It does not compare against Unity-imported source animation curves.
Controls cover corrupted positions, missing/nonfinite exact tracks, seeks/loops,
invalid times and component lifecycle. Editor checks also reject a mismatched FBX
digest. The fractional and skinned root-motion fixtures extend the sampler with
each source key and its adjacent times, both directly and one loop later (110
cases for these fixtures). The skinned check uses Unity's CPU `BakeMesh`; this
validates posed positions, not rendered shading or Animator controller integration.
Both fixtures and both motion schemas pass editor/player checks on both Unity
versions; see `target/root-motion-fractional-summary.json` for the eight results.
Generate the skinned fixture's guarded weight resource before root-motion export:

```sh
gcad asset-bind examples/game-assets/plate-root-motion-skinned.gasset plate \
  target/root-motion-skinned.weights.json --joints base,tip --blend 'y,-15,15'
```

GLB motion metadata and clip accessors now share float32 key times/duration.
Older packages with unrounded metadata must regenerate their root-motion GLB
and FBX package. `CheckTimelineRefusal` verifies rejection of that older mismatch.
`GcadRootMotionCheck.Diagnose` records imported curves; `CheckStepRefusal` checks
refusal in a project containing an old version 1 STEP receipt.

This is a headless consumer check for the textured three-clip reference fixture
(`input-bounds-positive/reference.glb` in the local audit evidence), exported as
FBX plus PNGs. It is deliberately an ordinary Unity import without a custom
material postprocessor in its default mode. With a GLB reference, mesh-instance
triangle counts, skin palettes, image counts and clip names are derived from its
active scene. Without one, the original plate counts are required. Material and
sampled-pose assertions still target this fixture family, not arbitrary assets.

```sh
UNITY=/absolute/path/to/Unity UNITY_VERSION=6000.4.7f1 \
  sh tools/game-assets-unity/check-import.sh FBX_PACKAGE NEW_PROJECT
```

Use an already installed editor. The script creates an isolated project with
only Unity's bundled animation, JSON and image-conversion modules enabled, with no registry
dependencies or engine installations. It preserves existing project paths.
It uses Unity's documented [batch mode and executeMethod arguments](https://docs.unity.com/en-us/engine/6000.5/manual/unity-editor/command-line-arguments/editor).
Both a successful editor exit and `passed: true` in `import-report.json` are
required. `-nographics` makes this an import check, not rendering acceptance.

Historical v11 import result: **passed** on 6000.4.7f1 and 6000.5.4f1. Both import 1,276
triangles, three clips/materials, 15 PNGs, two bones with no duplicate references,
and three base-colour plus three normal maps. Both editor processes exit zero;
the prior unweighted-vertex warnings are absent. Independent shared-skin and
reflected ufbx checks also pass. Evidence: `target/unity-v11-*` and `target/fbx-v11-*`.
This does not establish sampled poses, rendered appearance or other-engine support.

## Source material audit

The separate opt-in `GCAD_DESCRIPTION_AUDIT=1` mode probes Unity's
[MaterialDescription callback](https://docs.unity3d.com/6000.0/Documentation/ScriptReference/AssetPostprocessor.OnPreprocessMaterialDescription.html).
Its test observer is gated on both `Assets/Fixture/model.fbx` and a marker in
the isolated project; it changes only that fixture's material import mode and
records exposed properties. It is inactive in all other harness modes.
Unity 6000.5.4f1 exposes none of the eleven required `gcad|...` property names
on any of the three materials (33 missing checks; editor exit 1). Evidence:
`target/unity-description-6000.5.4f1`. The adapter therefore needs direct FBX
metadata reading rather than relying on this callback. The native helper's
experimental `--inspect-materials-v1` command supplies that contract without a
GLB or sidecar in the delivered package. The explicit adapter below consumes it.

```sh
UNITY=/absolute/path/to/Unity UNITY_VERSION=6000.5.4f1 \
  GCAD_MATERIAL_REFERENCE=/absolute/path/to/reference.glb \
  sh tools/game-assets-unity/check-import.sh FBX_PACKAGE NEW_PROJECT
```

`MaterialCheck.cs` reads material expectations directly from the source GLB in
the isolated project, then compares ordinary FBX-imported material settings and
texture connections. It modifies no material or texture import settings. Run it
separately from pose/render checks. The reference is the three-material fixture;
this is not an arbitrary-asset validator. Shader equivalence, sampling, colour
space conversions and texture pixel repacking require further checks.

Both 6000.4.7f1 and 6000.5.4f1 fail with identical reports (32 failed checks):
red/blue base-colour factors become white, metallic becomes zero, alpha mask
becomes opaque, cutoff becomes 0.5, normal and occlusion strengths become one,
and emission colour becomes white. Base-colour/normal/emission maps connect;
occlusion and metallic/roughness maps do not. Default Standard shading also
does not implement the source's double-sided raster requirement. Three of the
32 failures are explicit unverified metallic/roughness-adaptation checks, not
measured pixel differences. Those checks remain failed until an adapter and
pixel validation exist. Logs show editor exit 1 as expected for this failed gate.
Evidence: `target/unity-material-6000.4.7f1` and `target/unity-material-6000.5.4f1`.

Unity's [Standard material documentation](https://docs.unity.com/en-us/engine/6000.3/manual/materials-and-shaders/built-in/shader-built-in-birp/shader-standard-shader/standard-shader-material-parameters)
describes smoothness in the metallic texture's alpha channel. Connecting the
source metallic/roughness PNG unchanged would therefore not establish parity.
The explicit adapter below addresses these settings through a dedicated shader.
Full source-to-engine rendered comparisons remain pending.

## Explicit Built-In pipeline adapter

`GcadMaterialAdapter.Create(absoluteHelper, "Assets/Package/model.fbx",
"Assets/NewAdaptedPackage")` creates new materials, per-slot PNG copies and a
prefab referencing the original FBX meshes/rig. Install `FbxMaterialContract.cs`
and `MaterialAdapter.cs` in the Unity project's Editor folder, install
`Runtime/GcadDeformation.cs` outside any Editor folder, and put the two
`GcadPbr*.shader` files plus `GcadPbr.cginc` and `GcadSampling.cginc` in its Assets. No automatic import
postprocessor is installed by these adapter files. The helper executable is
explicitly selected, identity-checked and run with output/deadline limits.

The adapter requires the Built-In render pipeline and Linear colour space. It
reads only the FBX contract and its PNGs; the source GLB is used solely by the test
harness. It preserves original files and import settings, creates a new output
directory, and refuses an existing destination. Failure after output creation may
leave partial generated assets; no automatic replacement or cleanup is attempted.

The shader reads roughness G, metallic B and occlusion R directly, applies linear
colour/emission factors, normal scale, alpha mask/blend and raster culling intent.
Normal maps remain uncompressed RGB data in the generated copies, preserving all
three components; this differs from the earlier Standard-shader normal adapter.
Per-map point storage plus shader filtering supports distinct min/mag selections.
Mipmapped filters use Unity-generated mip levels; explicit source mip transfer,
mip-generation equivalence and performance remain unverified. The numerical
sampling check below validates a bounded set of supplied mip levels. UV sets other
than zero are rejected. No URP/HDRP, Linux or other-engine support is claimed.

```sh
UNITY=/absolute/path/to/Unity UNITY_VERSION=6000.5.4f1 \
  GCAD_FBX_HELPER=/absolute/path/to/gcad_fbx_helper \
  GCAD_MATERIAL_REFERENCE=/absolute/path/to/reference.glb \
  GCAD_UNITY_ADAPTER_CHECK=1 GCAD_UNITY_ADAPTER_RENDER=1 \
  sh tools/game-assets-unity/check-import.sh FBX_PACKAGE NEW_PROJECT
```

Both tested versions (6000.4.7f1 and 6000.5.4f1) pass source-factor, map connection,
PNG-byte, import-setting and original-file-preservation checks: three materials
and eighteen per-slot copies. Direct3D11 rendered controls also pass with 37,452
visible pixels. Disabling normal, metallic, roughness, occlusion, alpha-mask and
emission behavior changes 28,722 / 22,885 / 27,594 / 6,340 / 52,528 / 36,649 pixels
respectively in both versions. Baseline visually inspected. Evidence:
`target/unity-adapter-render-*`. These are feature-use checks for one masked
fixture, not full appearance parity. The numerical checks below cover bounded
sampler, material and raster cases. Shaded back faces, shadows and wider fixtures
still need acceptance.
Next action: broader shader checks and source-render comparisons.

### Numerical sampler check

`GCAD_SAMPLER_CHECK=1` runs the shader's shared `GcadSampling.cginc` through a
small offscreen probe, compared with a separate CPU texel-addressing and weighted-
sum implementation. Use it alone with the same `UNITY`, `UNITY_VERSION`, package
and new-project arguments. The package is copied by the common harness but the
sampling test uses generated linear float texels and explicitly supplied mips.

Both Unity 6000.4.7f1 and 6000.5.4f1 pass 216 cases / 884,736 pixels each on D3D11:
three U/V wrap combinations covering repeat/clamp/mirror on both axes, both mag
filters, all six min filters, magnification, the LOD boundary, fractional/integer
mips, reversed UVs and mip-limit clamping. Maximum RGBA error is about
0.000017494 against a 0.0002 tolerance; a 0.1 red-channel corruption is rejected.
Evidence: `target/unity-sampler-6000.4.7f1/sampler-report.json` and
`target/unity-sampler-expanded-6000.5.4f1/sampler-report.json`.
This validates the chosen derivative-based LOD rule for those inputs, not all
API-specific filtering rules, anisotropy, automatic mip generation, sRGB texture
decoding, extreme UV ranges or shader lighting. Colour-space and material checks
are described next; broader lighting/source-render comparisons remain pending.

### Numerical material and raster checks

`GCAD_SURFACE_CHECK=1` runs `SurfaceCheck.cs` and then `RasterCheck.cs` in a fresh
project using the same harness arguments. The surface probe calls the production
`surf` function from `GcadPbr.cginc`, exposing its outputs before lighting.
A separate CPU reference checks sRGB decoding at dark/mid/white texels, linear
numeric channels, colour/emission factors (including emission above one),
metallic/roughness/occlusion, full-XYZ normal decoding and scaling, controlled
front/back normal signs, alpha values and cutoff equality. Float render targets
avoid screenshot quantization. A changed-colour control is rejected.

Both Unity versions pass 144 cases / 36,864 pixels each, with maximum channel
error about 0.000080377 against a 0.001 tolerance. This includes all three alpha
mode calculations, three normal scales, two facing signs, two cutoffs and four
output groups. It does not measure lighting or real back-face tangent frames.

The raster check uses the actual opaque/mask and blend shaders, a quad with both
triangle windings, one-/two-sided culling, alpha values below/equal/above cutoff,
and emission-only output. All 36 centre-pixel cases match their analytic RGB
values in both versions (reported maximum error zero; allowed tolerance 0.002).
Blend expectation combines emission with a measured empty background. This
checks real blend/cull/mask states, not overlapping-transparent sorting, shadows,
edge antialiasing or shaded back-face appearance. Evidence:
`target/unity-surface-raster-6000.4.7f1` and `target/unity-surface-raster-6000.5.4f1`,
each containing `surface-report.json` and `raster-report.json`.
Next action: shaded source-render comparisons and wider asset fixtures.

### Shaded source geometry comparison

`GCAD_SOURCE_RENDER_CHECK=1` adds direct GLB rest and animated geometry comparisons
after the material adapter audit. Without the explicit deformation adapter the
animated gate fails; use the basis-preserving mode below for the passing path:

```sh
UNITY=/absolute/path/to/Unity UNITY_VERSION=6000.5.4f1 \
  GCAD_MATERIAL_REFERENCE=/absolute/path/to/reference.glb \
  GCAD_FBX_HELPER=/absolute/path/to/gcad_fbx_helper \
  GCAD_UNITY_ADAPTER_CHECK=1 GCAD_SOURCE_RENDER_CHECK=1 \
  sh tools/game-assets-unity/check-import.sh FBX_PACKAGE NEW_PROJECT
```

`SourceMeshReference.cs` reconstructs the canonical fixture directly from GLB
accessors and scene transforms, independently of Assimp and the FBX importer.
It checks bind-neutral rest skinning and converts positions, normals, tangents,
UVs and winding to Unity's conventions. This restricted test loader is not a
general glTF importer: sparse accessors, morphs and arbitrary extensions are not
supported. It uses the same adapter materials as the FBX model, after those
materials pass the separate source-derived settings and texture-byte audit.

Four 512-square views from above and below compare shaded RGB images under
identical lighting. A changed pixel has summed absolute RGB byte differences
greater than six. Every view must have more than 1,000 visible pixels, changed
pixels at most 1% of visible coverage, and full-frame mean normalized RGB error
at most 0.002. No silhouette pixels are excluded. Deliberately shifted UVs and
inverted normals in each view, plus a translated model, must fail that same gate.

Both Unity 6000.4.7f1 and 6000.5.4f1 pass on D3D11 with zero editor exit codes:
changed pixel counts are 0, 3, 1 and 14; maximum mean RGB error is about
0.000002469. All nine corruption controls are rejected. Evidence:
`target/unity-source-render-controls-6000.4.7f1` and
`target/unity-source-render-controls-6000.5.4f1`, containing
`source-render-report.json` and `source-renders/` image pairs.

These rest results bound geometry/attribute transfer for one fixture using shared
Unity shading. They do not establish independent shader equivalence, parity
with gcad's renderer, URP/HDRP or other engines.

The harness now also samples seven times in each of bend/stretch/travel (21
poses, four views each). The restricted reader interpolates LINEAR TRS tracks,
using normalized quaternion slerp for rotations and weighted joint matrices for
skinning, following the [glTF animation and skinning specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html).
Normals use the inverse transpose of the blended matrix, matching gcad's current
CPU deformation convention; tangents are transformed and orthogonalized.
The source meshes are compared numerically with Unity BakeMesh vertex sets
(bidirectional nearest distance, 0.00001 metre tolerance), as well as shaded images.

Both installed versions give the same failed result: 3/7 bend, 2/7 stretch and
7/7 travel poses pass the unchanged pixel gate. Every pose passes the position
gate (maximum distance about 0.000006627 metres); all 189 attribute/translation
corruption controls are rejected. Rest still passes. Both editors exit 1.
Evidence: `target/unity-source-animation-basis-6000.4.7f1` and
`target/unity-source-animation-basis-6000.5.4f1`, with
`source-animation-report.json` and paired images under `source-renders/pose-N/`.

A diagnostic reference using the blended linear matrix directly for skin normals
greatly reduces stretch pixel differences, suggesting a skin-normal convention
mismatch. Some bend differences remain. This diagnostic does not replace or
relax acceptance. Vertex proximity alone does not prove exact silhouettes or
attribute interpolation, and the remaining error is not yet fully attributed.
Next action: isolate imported animation/tangent differences and determine
how to preserve the intended deformation basis in the engine adapter, then rerun
the unchanged gate. Broader fixtures and other engines remain pending.
Reviewed: 2026-11-01; next review: 2026-11-14.

### Controlled animation import diagnostics

With `GCAD_SOURCE_RENDER_CHECK=1`, optionally set `GCAD_IMPORT_DIAGNOSTIC` to
exactly one of `compression-off`, `source-tangents`, `source-tangents-uncompressed`,
`source-tangents-flip`, or `no-resampling`. The combined uncompressed mode imports
stored tangents and disables animation compression. The flip mode additionally
negates tangent W on temporary mesh
copies in the renderer, never on imported mesh assets. These
test-only modes change one setting on the isolated project's copied FBX before
the adapter's preservation snapshot; they do not change the delivery package,
install a global postprocessor or alter the production adapter. The before/after
settings are recorded in `import-diagnostic-report.json`.

On Unity 6000.5.4f1, disabling animation compression or curve resampling reduces
the maximum pose-position difference to about 0.000000007509 metres. Each still
fails the same nine animated shaded poses. This separates the default import's
curve approximation from the remaining deformation-basis discrepancy. Unity's
[resampling option](https://docs.unity3d.com/6000.0/Documentation/ScriptReference/ModelImporter-resampleCurves.html)
is conditional, so the result is specific to the tested FBX.

Importing stored tangents instead of the default calculated MikkTSpace tangents
fails even at rest: 16,900 / 16,493 / 13,490 / 16,856 changed pixels, maximum mean
RGB error about 0.007669. Position difference is zero. The run stops at the failed
rest gate; it does not produce animated results. This exposes an additional
stored-tangent transfer/import issue that needs independent attribute readback.

The renderer report also includes `linearBasisDiagnostic`: a separate reference
that directly transforms both normals and tangents without tangent
orthogonalization. With compression disabled, stretch changed-pixel counts fall
to at most 15 per view; some late bend views still differ by up to 344 pixels.
This is evidence about basis conventions, not a replacement acceptance profile.
The inverse-transpose/orthogonalized reference remains authoritative and fails.
Evidence: `target/unity-source-compression-off-6000.5.4f1`,
`target/unity-source-source-tangents-6000.5.4f1`,
`target/unity-source-no-resampling-6000.5.4f1`, and
`target/unity-source-linear-basis-6000.5.4f1`. All editor exits are 1 as expected
for these failed gates. Other Unity versions have not run these diagnostic modes.

The new rest attribute audit matches corners by material, position, normal and
UV, never tangent. With v11 stored tangents, all 1,129 unambiguous imported
vertices have opposite handedness, with zero tangent-direction or UV error;
40 vertices have ambiguous correspondence and are explicitly excluded. Negating
only tangent W eliminates every changed pixel in all four rest views.

The active v12 native patch fixes the corresponding glTF import issue: V is
flipped during import, so the generated bitangent must also be flipped. Both
Unity versions now pass exact rest images with stored v12 tangents and no mesh
sign workaround (zero changed pixels; zero unambiguous handedness mismatches).
Evidence: `target/unity-v12-tangent-6000.4.7f1` and `...6000.5.4f1`.
Both still fail nine animated poses and exit 1; this fixes rest handedness, not
the separate blended deformation-basis mismatch. The first 6000.5.4f1 run used
the v11 helper only to inspect unchanged material metadata from the v12 FBX;
6000.4.7f1 also validates the new v12 helper identity.

Next action: preserve the intended animated deformation basis through
the adapter and rerun unchanged shaded acceptance. Review: 2026-11-14.

### Explicit basis-preserving deformation adapter

Call `GcadMaterialAdapter.Create(helper, modelPath, newOutput, true)` to opt into
the experimental CPU deformation path. Configure the v12 FBX to **Import** both
normals and tangents first; the adapter requires these options and does not
modify them. The optional argument defaults to false, preserving the ordinary
material-only adapter. Install `Runtime/GcadDeformation.cs` outside Editor folders.
Use animation compression **Off** for the source-to-player acceptance profile
below; the adapter does not alter the caller's animation quality settings.

This path creates readable owned mesh assets and a `GcadDeformation` component
on each skinned renderer, retaining the imported bones and clips. It computes
weighted skin matrices, inverse-transpose normals and orthogonalized tangents in
`LateUpdate`, and displays an owned dynamic mesh. The original skinned renderer
is disabled while the component is active and restored when it is disabled.
Prefab Animators use AlwaysAnimate so bone updates are not culled with the
disabled skinned renderer. External scripts that modify bones later must call
`Refresh()` afterward. GameObject activation still controls visibility.

One to four influences and complete basis attributes are required; morph targets
are rejected. A singular blend or missing bone reports an error and retains the
last successfully published mesh, then recovers when the inputs become valid.
Original FBX/PNG/import metadata and owned rest-mesh data remain unchanged.
This is a CPU correctness fallback, not a performance claim for large crowds or
meshes. Per-frame bone access and mesh upload costs need broader profiling.

Reproduce with the source-render command above plus
`GCAD_DEFORMATION_ADAPTER=1 GCAD_IMPORT_DIAGNOSTIC=source-tangents`.
The latter configures only the harness's copied fixture before the preservation
snapshot. The source comparison now fits camera near/far planes to the fixture
bounds; the old fixed ten-metre depth range caused discrepancies on the small
folded model. Pixel and position tolerances are unchanged, no pixels are masked,
and the comparison measures the adapter's displayed CPU mesh positions.

Both Unity 6000.4.7f1 and 6000.5.4f1 pass all 21 poses / 84 views, all 189 animated
corruption controls and the four rest views. Maximum mean RGB error is about
0.000054214; maximum position distance is about 0.000006627 metres. Both editors
exit zero. With the same fitted camera and stored tangents, the ordinary path
still fails nine poses (6000.5.4f1 control run exits 1).

`DeformationCheck.cs` additionally verifies singular/missing-bone rejection,
last-valid-mesh retention, recovery and rest-mesh preservation. It records
32 explicit refreshes of the 1,169-vertex mesh: about 0.72–0.79 ms per refresh on
this machine/editor setup, excluding a standalone player's lifecycle/render loop.
Evidence: `target/unity-deformation-verified-6000.4.7f1`,
`target/unity-deformation-verified-6000.5.4f1` and
`target/unity-deformation-baseline-depth-6000.5.4f1`.

These are sampled editor renders and explicit Refresh calls. The player test
below additionally validates automatic lifecycle/Animator behavior; the reflected
fixture below covers one non-uniform reflected parent. Large meshes, other
transform combinations, blend shapes, and URP/HDRP remain unverified.
Next action: broader fixture validation.
Review: 2026-11-14.

### Standalone player lifecycle check

Add `GCAD_PLAYER_CHECK=1` to the passing deformation/source-render command. The
harness requires the installed Windows player module, builds a Windows x64 Mono
test player, then runs it with `-batchmode -nographics`. No desktop automation
is involved. This is an isolated consumer test, not a new gcad viewer application.
The test-only `PlayerTest/` scripts and `PlayerBuildCheck.cs` are not part of
the production adapter installation.

The test scene creates an Animator and bend controller for the adapted prefab
and uses a fixed simulation step. It checks automatic OnEnable initialization,
Animator-driven displayed vertex movement through LateUpdate, disabling and
re-enabling the component, whole-object deactivation/reactivation, rejection of
a deliberately singular pose, retention and recovery of the displayed mesh,
and restoration of the original renderer plus destruction of owned mesh/display
objects when the component is removed. The singular-pose diagnostic in the player
log is expected. A 45-second in-player watchdog reports lifecycle timeouts.

Both Unity 6000.4.7f1 and 6000.5.4f1 build successfully and pass all lifecycle
checks in 38 frames; both editors and players exit zero. The preceding 21-pose
shaded editor comparison also passes in each run. Evidence:
`target/unity-player-animator-6000.4.7f1` and
`target/unity-player-animator-6000.5.4f1`, containing `player-build-report.json`
and `Player/player-report.json`. Their package lockfiles contain only the two
engine-bundled modules. The runner requires successful build and player reports
as well as process exit codes; a successful editor import alone is insufficient.

The player uses no graphics device, so this proves runtime lifecycle and CPU
deformation, not standalone GPU appearance, IL2CPP, Linux, or another engine.
Next action: larger/reflected fixture and rendered-player acceptance.
Reviewed: 2026-11-01; next review: 2026-11-14.

### Rendered standalone player comparison

```sh
UNITY=/absolute/path/to/Unity UNITY_VERSION=6000.5.4f1 \
  GCAD_MATERIAL_REFERENCE=/absolute/path/to/reference.glb \
  GCAD_FBX_HELPER=/absolute/path/to/v12/gcad_fbx_helper \
  GCAD_UNITY_ADAPTER_CHECK=1 GCAD_DEFORMATION_ADAPTER=1 \
  GCAD_SOURCE_RENDER_CHECK=1 GCAD_PLAYER_CHECK=1 GCAD_PLAYER_RENDER=1 \
  GCAD_IMPORT_DIAGNOSTIC=source-tangents-uncompressed \
  sh tools/game-assets-unity/check-import.sh FBX_PACKAGE NEW_PROJECT
```

This selects the rendered test scene instead of the lifecycle-only scene and
runs the Windows x64 Mono player in batch mode with D3D11. The editor builds
reference mesh snapshots directly from the source GLB at all 21 sample times;
the player compares those with its adapted FBX geometry using the shared,
audited materials and shipped shaders. It renders 84 pairs of 512-square images,
checks bidirectional nearest-vertex distance at 0.00001 metres, and uses the same
pixel thresholds as the editor gate. Renderer visibility is toggled without
restarting the animated object's lifecycle. Inverted-normal controls in each
view and a translated reference in each pose must fail the image gate.

Both Unity 6000.4.7f1 and 6000.5.4f1 pass: all 84 views, all 105 corruptions
rejected, maximum 11 changed pixels in a view, maximum normalized mean RGB error
about 0.000002509, and maximum vertex distance about 0.000000007694 metres.
Both editors and players exit zero. Evidence:
`target/unity-player-render-uncompressed-6000.4.7f1` and
`target/unity-player-render-uncompressed-6000.5.4f1`, with
`Player/player-render-report.json` and `Player/player-renders/` image pairs.
The harness enables the engine-bundled image-conversion module to save PNGs;
the package lockfile still has no registry dependencies.

The default keyframe-reduced import fails two late-bend views in the player,
despite passing the separate 0.00001-metre position threshold. Its maximum vertex
distance is about 0.000006627 metres. Disabling compression resolves this tested
case without changing tolerances. Evidence:
`target/unity-player-render-geometry-6000.5.4f1`; this failed control remains
recorded rather than being treated as full player acceptance.

This verifies sampled player geometry/shading under shared Unity materials;
it does not prove independent shader equivalence with gcad, continuous animated
GPU motion, other graphics APIs, IL2CPP, URP/HDRP, Linux or other engines.
Next action: larger fixtures and broader consumer validation.
Reviewed: 2026-11-01; next review: 2026-11-14.

### Reflected two-instance fixture

The same uncompressed, basis-preserving profile also passes the existing native
scene harness's reflected reference:
`target/game-assets-fbx-scene-contract/normal-conventions-final/reflected/authored-animation.glb`.
Its second mesh/skeleton instance is translated 0.1 metres and parented under
scale (-1.25, 0.75, 1.5). The scene contains 2,552 triangles, two skinned instances,
four bone palette entries, three materials/clips and 15 PNGs. The v12 helper's
staged conversion produces `target/fbx-v12-reflected-package`; use that package
and this source GLB with the rendered-player command above and a fresh project.

`ImportCheck` now compares active source scene counts with displayed FBX mesh
instances rather than counting only unique mesh assets or assuming one skin.
`DeformationCheck` checks every adapter and writes a `meshes` array plus the
aggregate `passed` result. Both adapters pass invalid-pose retention, recovery
and source preservation. The original single-instance import/material test also
passes after this change (`target/unity-source-count-regression-6000.5.4f1`).

Both Unity versions pass all 21 editor poses and all 84 standalone D3D11 views;
all 105 player corruption controls are rejected. Maximum changed pixels per view
is 67, maximum mean normalized RGB error about 0.000005815, and maximum vertex
distance about 0.00000001666 metres. Editors and players exit zero. Evidence:
`target/unity-reflected-player-6000.4.7f1` and
`target/unity-reflected-player-6000.5.4f1`.

This covers two small instances and one reflected/non-uniform parent; it is not
a large-mesh performance test or an acceptance of the plate's deliberately harsh
deformation/topology quality. Larger meshes and additional engine consumers
remain open. Next action: scale the geometry comparison and validate a
more highly refined asset. Review: 2026-11-14.

## Sampled pose check

### Explicit-time UV-scrolling runtime API

`Runtime/GcadUvScroll.cs` drives the selected template through the production
PBR shaders' shared UV sampling path. It takes explicit seconds; it does not
sample Unity's wall clock. Apply it to an owned material instance using the
same time as your animation sampling. Default materials leave the transform
disabled and retain their previous behavior.

For original, untransformed mesh UVs, `GcadUvScroll.Original(scale, offset,
speed, seconds)` converts the top-left gcad convention to Unity sampling:
`(scale.x, scale.y, offset.x + seconds*speed.x,
1 - scale.y - offset.y - seconds*speed.y)`.

For FBX exported with a frozen shader fallback, the scale and offset are already
baked into mesh UVs. Use only the remaining scroll delta:

```csharp
// Parameters from the retained capability report for plate-shader-frozen.gasset.
GcadUvScroll.Apply(ownedMaterial,
    GcadUvScroll.Frozen(new Vector2(0.125f, 0.25f), 2.0, animationSeconds));
```

`Frozen` returns identity scale and `(seconds-frozenSeconds)*(speed.x,-speed.y)`.
Using `Original` on frozen UVs would apply scale/offset twice. Invalid parameters
are rejected before material properties change. These methods do not clone
materials, parse capability files, select renderer slots or bind an Animator;
the explicit editor attachment workflow below handles material ownership and
renderer slots. Callers still supply the animation time. Source FBX and PNG
assets are not modified.

When `gcad asset-fbx-experimental` converts a frozen-shader GLB, it now publishes
`gcad-shaders.json` alongside the FBX and PNG files. This bounded, versioned
receipt identifies materials and records the original description, source GLB
digest and final FBX digest. Direct invocation of the native helper alone does
not generate this receipt. Keep it beside `model.fbx` when importing into Unity.
Ordinary exports without shaders do not need a receipt.

`GcadMaterialAdapter.Create` checks the receipt against the FBX and material
assignments before creating assets, then attaches `GcadUvScrollAnimation` to the
adapted prefab. Call `instance.GetComponent<GcadUvScrollAnimation>().Sample(seconds)`
with the same explicit clock as animation playback. Each instance owns material
clones. Disable/destroy restores the original material slots and releases clones;
re-enable and sample to resume. Invalid time retains the last valid sample.
The component also works for explicit editor preview; it has no automatic clock.
Without sampling, the exported frozen appearance remains visible.

Run `GCAD_UV_ATTACHMENT_CHECK=1 GCAD_FBX_HELPER=/absolute/helper` with the usual
`UNITY`, `UNITY_VERSION` and `check-import.sh` arguments, using the frozen fixture
package and a fresh project. The attachment check covers two independent prefab
instances, seek/frozen-time identity, last-good state, disable/resample, source FBX
preservation, mismatched digest, unsupported template and absent material rejection.
Evidence: `target/unity-uv-attachment-validated-6000.5.4f1` and the corresponding
6000.4.7f1 project. This validates attachment and lifecycle, not rendered scene or
standalone player parity. Next action: exercise the attached prefab in
rendered scene/player comparisons. Review: 2026-11-14.

`GCAD_UV_RENDER_CHECK=1` runs the frozen plate FBX through the production PBR
shader on D3D11. Supply the same helper/package/editor arguments and a fresh
project as the attachment check. Twenty cases combine four camera views and
times 0, 0.5, 2, 4, then 0 again. The reference uses cloned imported meshes with
UVs shifted independently on the CPU and the shader transform disabled. A
deliberate 0.7-second timing error must produce a measurable difference for each
view. The report also requires visible animation and exact seek-back pixels.

Both 6000.4.7f1 and 6000.5.4f1 pass: zero RGB error in all 20 cases, with minimum
wrong-time mean RGB error 0.00247176 (normalized channel units). Reports and 20
rendered PNGs per editor are in `target/unity-uv-render-6000.4.7f1` and
`target/unity-uv-render-6000.5.4f1`. This tests actual imported geometry, prefab
attachment and production shader sampling for the base-colour plate fixture.
It shares Unity lighting between both paths; it does not establish independent
lighting equivalence with gcad, all-map imported scenes or standalone player
behavior. Next action: standalone player verification of this workflow.
Reviewed 2026-11-01; review again by 2026-11-14.

Add `GCAD_UV_PLAYER_CHECK=1` to `GCAD_UV_RENDER_CHECK=1` to build and execute the
same comparison in an installed Windows standalone Mono player. The runner
requires successful build, render and lifecycle reports, then compares all 20
editor/player PNGs byte-for-byte. The CPU reference uses owned readable mesh
copies, without changing the original FBX importer. Test-only player code stays
under `PlayerTest`; only the existing runtime component/shaders ship as adapter
code.

Both installed versions pass in `target/unity-uv-player-fixed-6000.4.7f1` and
`target/unity-uv-player-fixed-6000.5.4f1`: zero error for 20 rendered cases per
player, exact seek-back, all timing corruptions rejected and 20 byte-identical
editor/player images. The lifecycle check spans frames to verify material
restoration, deferred destruction, re-enable and invalid-time last-good state.
Build/editor/player exit codes are zero. This evidence covers Windows D3D11,
Mono and the static base-colour fixture. All-map/deforming shader scenes,
IL2CPP, other graphics APIs/platforms and other engines remain unverified.
Next action: all-map and deformation coverage for imported shader scenes.

For all-map coverage, export `examples/game-assets/plate-shader-maps.gasset` to
GLB and convert through `gcad asset-fbx-experimental`, then run its package with
`GCAD_UV_ALL_MAPS=1 GCAD_UV_RENDER_CHECK=1 GCAD_UV_PLAYER_CHECK=1`. The probe checks
all six imported slots and replaces each map individually with white as an
omission control. Every map must measurably affect at least one rendered view.
Both installed versions pass with zero reference error and 20 identical editor/
player PNGs: `target/unity-uv-maps-lifecycle-6000.4.7f1` and its 6000.5.4f1 counterpart.
This covers combined all-map rendering with per-map omission detection, not
independent lighting equivalence or all possible filter/mipmap combinations.

The UV component now restores materials on late-created child renderers that
inherit its material clones, including when disabled before another sample.
Editor and frame-based player regressions exercise that case. Actual animated
deformation with shader playback remains the next verification task.
Reviewed 2026-11-01; review by 2026-11-14.

### Paired CPU deformation cost

Set `GCAD_LOD_COST_BASELINE` to the unoptimised FBX package directory and
`GCAD_FBX_HELPER` to the reviewed helper, then invoke `check-import.sh` with the
reduced FBX package and a fresh project. Set the usual `UNITY`/`UNITY_VERSION`;
run this mode separately from render/player checks. It creates owned adaptations
with matching normals, tangents and uncompressed animation import settings.

`lod-cost-report.json` records nine poses (start/middle/end of three clips), four
warmups and seven paired batches of eight refreshes, alternating execution order.
It preserves raw batch times and medians. This is graphics-free Unity editor CPU
`GcadDeformation.Refresh` wall time including mesh publication. Import, clip
sampling, GPU work, rendering and allocations outside Refresh are excluded.
`passed` means the fixture measured successfully and geometry counts decreased;
it is not a production performance-budget gate.

Sequential runs in 6000.4.7f1 and 6000.5.4f1 both show lower LOD medians at every
pose. Original/LOD vertex counts are 12,155/10,387 and triangle counts are
20,416/16,880. Per-pose medians in 6000.4.7f1 span 7.29–7.94 ms original and
6.22–6.85 ms LOD; 6000.5.4f1 spans 7.27–8.23 ms and 6.21–6.97 ms respectively.
Evidence: `target/unity-lod-cost-6000.4.7f1` and its 6000.5.4f1 counterpart;
both shell runs exit zero. These are machine/fixture-specific measurements, not
GPU frame times or general engine performance claims. Reviewed 2026-11-01;
review by 2026-11-14.

### Original-versus-LOD rendered quality

For an accepted reduced fixture, use its GLB as `GCAD_MATERIAL_REFERENCE` and
its converted FBX package as the normal input. Set `GCAD_LOD_REFERENCE` to the
retained unoptimised GLB, alongside the usual source-render/deformation flags.
The runner preserves both references and requires `lod-quality-report.json`.
This compares rest plus 21 poses at four views each using the original's framing,
shared material interpretation, and the same full-image gate as source rendering:
changed pixels <=1% of visible pixels and normalized mean RGB error <=0.002.
Existing normal/UV/translation corruption controls must also fail the gate.

The report marks `topologyChanged=true`; its nearest-vertex distance is diagnostic,
not a geometric acceptance criterion for different vertex sets. The Rust LOD
report separately supplies the bidirectional sampled surface and silhouette
checks. Engine quality, export fidelity and production performance are distinct
claims. These fixture image limits are not application-specific production budgets.

Verified on 6000.4.7f1 and 6000.5.4f1 in `target/unity-lod-quality-*`: all 88 views
pass, with maximum mean RGB error 3.132e-6 and changed-pixel fraction 0.00019193.
All 198 corruption controls are rejected and both editor runs exit zero. Direct
rendered quality is now covered for this fixture; paired consumer performance
measurement is the next task. Reviewed 2026-11-01; review by 2026-11-14.

### Scrolling with imported skeletal animation

`plate-scene-shader.gasset` adds the frozen scrolling template to the refined
three-material skinned fixture. Run `tools/game-assets.sh prepare-fixtures`
first to create its shared face/weight bindings, then export it to GLB and convert
with `asset-fbx-experimental`. Use the standard source-render command with
`GCAD_SOURCE_SHADER_CHECK=1`, `GCAD_SOURCE_RENDER_CHECK=1`,
`GCAD_UNITY_ADAPTER_CHECK=1`, `GCAD_DEFORMATION_ADAPTER=1`,
`GCAD_IMPORT_DIAGNOSTIC=source-tangents-uncompressed`, and set
`GCAD_MATERIAL_REFERENCE` to this GLB. Add `GCAD_PLAYER_CHECK=1` and
`GCAD_PLAYER_RENDER=1` for the standalone player. Keep helper/editor paths and
the exact editor version explicit, and select a fresh project directory.

The source reference computes deformation from GLB and independently applies
the frozen-time UV delta on the CPU. Imported geometry uses the production
deformation and UV components, sampling both at the clip time. Rest plus 21 poses
exercise bend, nonuniform scale and translation; every camera view also tests
an intentionally wrong shader time. These fixture checks require three shader
bindings with the documented speed/frozen time and retain the existing geometric,
normal, UV and translation corruption gates. The shared lighting is Unity's;
this is not independent gcad/Unity lighting equivalence.

Verified on Unity 6000.4.7f1 and 6000.5.4f1: editor rest plus 21 poses pass;
all 273 pose corruption controls are rejected, with maximum position distance
8.343e-9 metres and maximum mean RGB error 2.005e-6. Standalone Windows Mono/D3D11
players pass all 84 views and reject 189 controls (including 84 wrong shader-time
controls), with maximum mean RGB error 3.247e-6. Evidence is under
`target/unity-shader-deform-player-6000.4.7f1` and its 6000.5.4f1 counterpart.
Build and player exits are zero. These are explicitly sampled clips; continuous
Animator/shader clock integration remains to verify. The refined plate is a
transfer diagnostic, not production deformation topology.
reviewed 2026-11-01, review by 2026-11-14.

Direct editor/player parity is unresolved: only 1/84 imported PNG pairs are
identical in each version. Decoded RGB comparisons give maximum mean error
0.02463666 and 24,336 changed pixels (RGB byte-sum difference >6). The separate
source-reference results above must not be interpreted as cross-runtime parity.
Next action: compare camera/framing and runtime rendering state to locate
the discrepancy before extending continuous playback acceptance.

The framing discrepancy above is resolved in `target/unity-shader-framing-*`.
Both consumers now fit cameras to independently evaluated reference vertices,
excluding disabled imported skin bounds. Both installed versions pass all 84
decoded editor/player image pairs with maximum mean RGB error 3.491e-6 and at
most six changed pixels (RGB byte-sum >6). Exact equality remains false.
`EditorPlayerParityCheck` reuses the source-render thresholds, reports exactness
separately, and includes every pixel. The runner invokes it after rendered
player checks and requires `source-editor-player-report.json` to pass; therefore
`GCAD_PLAYER_RENDER=1` also requires `GCAD_SOURCE_RENDER_CHECK=1`.
`check-render-parity.sh PROJECT` is an optional stricter PNG-byte diagnostic and
writes a separate `source-editor-player-exact-report.json`.
Next action: continuous Animator/shader clock integration. Review 2026-11-14.

### Continuous single-state Animator clock

With the shader fixture and `GCAD_PLAYER_CHECK=1`, omit `GCAD_PLAYER_RENDER` to
run the lifecycle player. Shader lifecycle tests now select D3D11; graphics-free
mode does not provide the shader properties this check needs. Both Unity versions
pass 54 frames in `target/unity-shader-clock-6000.4.7f1` and its 6000.5.4f1 counterpart.
The check follows Animator time in LateUpdate after deformation, validates each
displayed material's transform, pauses for five frames, seeks back and resumes.
It then verifies disable/re-enable, reactivation, invalid-pose last-good state,
recovery and cleanup. All four shader-clock report flags and the existing
lifecycle flags pass; the D3D11 players exit zero.

The fixture has a single one-second non-looping state with no transitions. Its
consumer code samples `GcadUvScrollAnimation.Sample` with the clamped state time;
the production UV component retains its explicit-time API. This verifies that
integration pattern, not automatic synchronization for arbitrary blend trees,
transitions, layered animation or looping clocks. Those consumers must choose
their time mapping explicitly. Reviewed 2026-11-01; review by 2026-11-14.

`GCAD_SURFACE_CHECK=1` now also runs `UvScrollCheck` and requires
`uv-scroll-report.json`. Its 120 D3D11 cases cover original/frozen UVs, five time
samples including seek-back, repeat/clamp/mirror, and base colour, roughness,
metallic, occlusion, emission and normalized normal output. The point-sampled
4-by-4 linear textures are compared with independent texel addressing, not a
second call to the runtime transform helper. Shifted-UV corruption controls and
invalid-input/last-good-state checks must pass. Existing surface and raster
tests continue to check the unchanged default path.

On 6000.4.7f1 and 6000.5.4f1, all 30,720 pixels pass with maximum channel error
0.00000005960465; all 120 corruptions are rejected and the editor exits zero.
Evidence: `target/unity-uv-scroll-verified-6000.4.7f1` and
`target/unity-uv-scroll-verified-6000.5.4f1`. Both editors exit zero. This is numeric surface
sampling, not a full FBX scene/standalone player, trilinear-mip parity or independent
gcad-versus-Unity lighting comparison. Next action: attach the effect
from retained export metadata to adapted prefab
materials, followed by scene/player checks. Review: 2026-11-14.

### Refined fixture and weight-cutoff diagnostic

`examples/game-assets/plate-scene-refined.gasset` refines the original plate twice
to 20,416 triangles while keeping its three materials and bend/stretch/travel
clips. Run `bash tools/game-assets.sh prepare-fixtures` to create its own face
selections and weight binding, export it to GLB, and convert that GLB into a new
empty package directory with the v12 helper. Use the resulting GLB/package with
the rendered-player recipe above and a fresh Unity project.

On 6000.5.4f1, `source-tangents-uncompressed` passes all 84 editor image views
but fails four bend position checks (maximum 0.000026813823 metres). The player
is intentionally not built after this failed acceptance gate. The explicit CPU
adapter passes recovery checks and averages about 7.39 ms over 32 refreshes of
12,155 imported vertices here. This remains a transfer diagnostic, not accepted
deformable topology or a general performance guarantee.

Unity's imported setting is `minBoneWeight=0.001`. The test-only profile
`source-tangents-uncompressed-zero-cutoff-request` requests Custom/four-bone
weights with a zero cutoff, but this editor restores 0.001 on reimport.
`import-diagnostic-report.json` records requested mode and effective before/after
settings; the profile name expresses a request, not successful preservation.
Unity's [importer reference UI](https://github.com/Unity-Technologies/UnityCsReference/blob/master/Modules/AssetPipelineEditor/ImportSettings/ModelImporterRigEditor.cs)
also describes discarding small weights and renormalizing those retained.

`SkinWeightDiagnostic.cs` compares full-source positions and a separate source
reference pruned at the effective cutoff. It runs before the render gate when
source rendering and the deformation adapter are enabled, or can be invoked
as `GcadSkinWeightDiagnostic.Run` in a prepared probe project. On this fixture,
the pruned comparison's maximum distance is 0.000000008344 metres across 21
poses, isolating the imported weight loss. It writes `skin-weight-diagnostic.json`
and never changes the acceptance source, thresholds, FBX or adapter meshes.
Evidence: `target/unity-refined-player-6000.5.4f1` and
`target/unity-refined-full-weights-6000.5.4f1`; the latter is a historical failed
experiment name, not a claim of full weight preservation.

The shared `GcadPositionMetric` uses exact kd-tree nearest-point searches, with
double-precision squared distances and final float rounding. Twenty-one cases
match an independent exhaustive oracle, including duplicate, collinear, planar
and closely spaced points. A 100,000-point-per-set analytic check passes in
about 0.78 seconds here. Reports are `position-metric-report.json`; these checks
validate the comparison implementation rather than asset fidelity.

The explicit deformation adapter now recovers complete weights through the
helper's `--inspect-skin-v1` command, independently of Unity's pruning. It matches
canonical FBX world positions (metres, reflected X for Unity) and the retained
imported influences, then writes recovered influences only into its owned mesh.
This currently supports one linear skin per mesh, one to four influences,
unique bone names within each palette, and world coordinates within 100 metres
per axis. Position matching uses 0.0000001 metres; candidates with different
complete weights are rejected as ambiguous. Missing bones/vertices also fail.
These are explicit supported bounds, not a general-purpose FBX importer.

`SkinWeightRecoveryCheck` verifies rejection of shifted and ambiguous contracts
and preservation of original mesh weights. The dense fixture changes 20 imported
vertices. On Unity 6000.4.7f1 and 6000.5.4f1 it passes all 21 full-source editor poses, all 84
rendered Windows Mono/D3D11 player views, and all 105 player corruptions. Maximum
position distance is 0.000000008344 metres, changed pixels per view 2, and mean
RGB error about 0.000001477. Editor/player exit zero. Evidence:
`target/unity-refined-recovered-6000.4.7f1` and
`target/unity-refined-recovered-6000.5.4f1`; recovery controls are recorded in
`skin-weight-recovery-report.json` (separate `recovery-controls.log` on 6000.5.4f1).
The earlier failed pruning controls remain intact. No tolerances were changed.

The reflected two-instance editor regression also passes all 21 poses, recovery
controls and deformation checks on 6000.5.4f1 (exit zero), without changing either
instance's original weights. Maximum position distance remains 0.00000001666
metres. Evidence: `target/unity-reflected-recovered-6000.5.4f1`.
Next action: broaden asset/consumer coverage and recovery limits.
Reviewed: 2026-11-01; next review: 2026-11-14.

### Native sampled positions

The probe's `--unity-pose-reference INPUT.glb NEW_REFERENCE.json` writes positions
computed from validated source GLB channels and raw skin weights, independently of
the FBX writer. Set `GCAD_POSE_REFERENCE` to that file when running `check-import.sh`
to enable `PoseCheck.cs` after the import gate. It samples seven times per clip,
instantiates a fresh model per sample, applies the imported clip, and uses Unity's
[BakeMesh](https://docs.unity3d.com/6000.0/Documentation/ScriptReference/SkinnedMeshRenderer.BakeMesh.html)
to collect deformed vertices in world space. The fixture conversion reflects X
back to the source right-handed coordinates before comparison.

Both 6000.4.7f1 and 6000.5.4f1 pass 21 samples across bend, stretch and travel.
Maximum bidirectional nearest-vertex distance is about 0.000007930061 metres,
below the declared 0.00001-metre tolerance. A translated corruption is rejected.
Evidence: `target/unity-pose-*`. This compares vertex sets, not triangle interiors,
normals, material appearance or continuous motion between samples. It is not a
general-purpose acceptance profile for arbitrary assets; reflected-instance and
broader fixtures still need engine checks.

## Offscreen material-use check

Set `GCAD_RENDER_CHECK=1` when invoking the runner to enable Direct3D 11 and
capture `renders/baseline.png`, `no-base-map.png` and `no-normal-map.png` with
[Camera.Render](https://docs.unity3d.com/6000.0/Documentation/ScriptReference/Camera.Render.html).
This mode is currently Windows-specific and overrides the pose mode. No desktop
UI is automated. The check requires visible model pixels, measurable changes when
each map is disabled, and correct normal-texture import settings.

The default import connects normal maps but leaves their TextureImporter type as
`Default`, so this gate fails. `GCAD_UNITY_NORMAL_ADAPTER=1` explicitly invokes
`GcadNormalTextureImport.Configure(modelPath)` before rendering. It sets connected
normal PNGs to NormalMap, disables sRGB/height conversion, and refuses PNGs shared
with other slots in that model. It installs no global postprocessor and does not
modify PNG bytes; Unity `.meta` import settings are changed in the test project.

With that adapter both tested Unity versions pass: 74,002 visible pixels, 66,400
pixels affected by base-map removal and 53,187 by normal-map removal. Baseline and
no-normal images were visually inspected. The default and configured baseline
PNGs are byte-identical for this fixture; the settings gate establishes explicit
normal import configuration, not an observed visual correction. It does not prove
source shading, alpha, metallic/roughness, AO or emission equivalence. Evidence:
`target/unity-render-*`. Next action: wider material fixtures and rendering
comparisons, plus other engine consumers.

Historical v10 result: **failed** on both versions. The fixture imports 1,276 triangles,
three clips/materials and all 15 PNG files. All three base-colour maps connect.
However, six bone entries contain four duplicate references; Unity warns that
168 vertices lack weights and assigns them to its first bone. None of the three
normal maps connects to the Standard shader. The 6000.5.4f1 initial run reports
the same skin warnings and missing normal maps; an initial weaker count-only
check was insufficient and has been tightened. That editor also exited abnormally
after writing its initial report, so it was never a successful process check.
Its stricter rerun exits with code 1 and confirms four duplicate bone references
and zero connected normal maps, matching 6000.4.7f1.

Evidence remains under ignored `target/unity-import-*`. Do not publish the raw
editor logs without reviewing machine/account details. Next action:
sample engine poses and compare rendered materials. Unreal, Godot and other
engine acceptance remain outstanding.

## Static LOD fixture

`plate-static-refined.gasset` retains the refined fixture's three materials and
six maps but has no skin or clips. Generate its source/LOD pair with
`GCAD_LOD_STATIC=1 GCAD=... sh tools/game-assets-lod-fixture.sh NEW_DIRECTORY`
after preparing the face bindings. Convert the LOD with the rebuilt reviewed
helper. Use `GCAD_UNITY_ADAPTER_CHECK=1`, `GCAD_SOURCE_RENDER_CHECK=1`,
`GCAD_IMPORT_DIAGNOSTIC=source-tangents-uncompressed`, `GCAD_MATERIAL_REFERENCE`
pointing to the LOD GLB, and `GCAD_LOD_REFERENCE` pointing to its original GLB.
Select `GCAD_FBX_HELPER`, `UNITY` and `UNITY_VERSION` as in the other recipes.
Leave deformation/player modes off: this case validates static editor rendering.

On 2026-11-01 both installed versions pass (`target/unity-lod-static-*`): four
views each for FBX fidelity and LOD quality, nine rejected controls in each,
zero imported position error, at most six changed LOD pixels and maximum mean
RGB error 1.397e-6. The helper needs the single-authored-root preservation fix in
`helper_scene_import.h`; the prior helper correctly refused this scene rather
than publishing an FBX with a missing mesh attachment.
