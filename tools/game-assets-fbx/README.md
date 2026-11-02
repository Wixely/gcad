# Isolated FBX feasibility probe

## Active v12 tangent handedness correction (2026-11-01)

The active helper pin now uses `experimental-material-properties-v12.patch`,
SHA-256 `783bf12bee4103422f8e8d2f74b2f17ae4dd93109b25266cd67baa9838340bf9`.
The pinned glTF importer flips texture V but previously generated bitangents
using the original tangent sign. V12 negates that bitangent when UV0 is present.
The change covers the base tangent stream used by the supported canonical
fixtures; morph-target tangent conversion remains outside the supported contract.

Windows Release builds and staged conversion pass. Mirrored-UV and reflected
instance audits pass, including 865,128 and 1,730,256 sampled skin corners and
explicit basis checks. Evidence: `target/game-assets-fbx-scene-contract/v12-mirrored`
and `v12-reflected`. The Rust helper package integration test passes with v12.
Rust, Unity material inspection, shell build/package commands and bundle pins
now require v12; historical versions remain available only for reproduction.

Independent raw-GLB-derived Unity comparisons now match four rest images exactly
when importing stored tangents on 6000.4.7f1 and 6000.5.4f1. Tangent direction,
UV and handedness agree at 1,129 unambiguous matched imported vertices; 40 seam
vertices are ambiguous in the numerical audit. The pixel test includes the
complete surface. Animated shading still fails for nine of 21 poses; this is
not full engine acceptance. Next action: animated basis preservation,
then packaging/platform acceptance. Review: 2026-11-14.

## Experimental material inspection (2026-11-01)

### Complete skin influence inspection

Native regression: `skin_inspection_preserves_small_weights_and_rejects_invalid_inputs`
in `tests/fbx_helper_process.rs` passes with the refined GLB/FBX fixture. Set
`GCAD_FBX_HELPER`, `GCAD_FBX_INPUT` (GLB) and `GCAD_FBX_MODEL` (FBX) to absolute
paths, then run `cargo test --features game-assets --test fbx_helper_process
skin_inspection_preserves_small_weights_and_rejects_invalid_inputs -- --ignored --exact`.
It compares distinct complete weight values against the raw source accessor in
both directions, including values below 0.001; validates palette references and
normalization; checks byte preservation; and rejects malformed, oversized,
missing and relative-path input without partial stdout. Per-vertex association
is covered separately by full-source Unity pose comparisons, not this scalar
weight-set check.

The helper also advertises `skin-inspection-v1-experimental` and accepts
`--inspect-skin-v1 ABSOLUTE_MODEL.fbx`. Its protocol-1 JSON contains skinned mesh
node names, per-skin bone names, complete normalized influences and bind/rest
world positions in the FBX coordinate system, measured in metres. It reads via
ufbx without external files, animation evaluation or writes. Input and each
ufbx allocator are capped at 256 MiB, output at 64 MiB, and total vertices at
one million. It rejects non-linear/multiple skin deformers, invalid or missing
influences, more than four influences, nonfinite positions and duplicate bone
names within a skin. This is for the canonical gcad FBX contract, not arbitrary
FBX coordinate conventions or a general importer.

The Unity deformation adapter uses this readback to restore small weights lost
at Unity's 0.001 import cutoff. It rejects unmatched/ambiguous recovery and
preserves the imported source mesh. Dense full-source editor and Windows
Mono/D3D11 player checks pass on Unity 6000.4.7f1 and 6000.5.4f1; detailed evidence
and remaining bounds are in `../game-assets-unity/README.md`. Linux, Unicode paths
and broader helper process containment remain unverified. Review: 2026-11-14.

### Material properties

The rebuilt v12 helper accepts `--inspect-materials-v1 ABSOLUTE_MODEL.fbx` and
returns protocol-1/material-schema-3 JSON on stdout. It reads the gcad properties
and external map connections directly with ufbx, including colour/scalar factors,
alpha/culling intent, channel indices, sampler settings and UV sets. No external
images are opened and no files are written. Unattached default materials are
excluded; attached materials without the schema or unique names are rejected.
Only canonical package PNG paths are accepted. Input is capped at 256 MiB,
ufbx temporary/result allocators at 256 MiB each, and JSON output at 4 MiB.
These bounds are not a complete process sandbox. Unicode CLI paths and Linux
execution remain unverified.

Windows Release build and the opt-in `converts_and_publishes_complete_package`
integration test pass with the textured three-material source. The test compares
factors, alpha flags, all six map connections, sampler values and packed channel
indices against the original GLB; a corrupted schema property is rejected with
empty stdout. Inspection preserves the FBX bytes. This is metadata extraction,
not engine material/render acceptance. Next action: consume this contract
in the explicit Unity adapter and validate its converted textures and shaders.

The C++ probes exercise Assimp import/export and independent ufbx readback.
Build them with `sh tools/game-assets.sh build-probes Release base` or the `v10`
variant. See [development commands](../game-assets.md) for prerequisites, Debug
builds, fixture preparation and direct audit commands. The optional production
helper approach is approved. The separate experimental helper now performs staged
conversion; `sh tools/game-assets.sh package-helper NEW_OUTPUT` builds a Release
developer bundle. See [bundle usage and limitations](HELPER-BUNDLE.md). Production
engine acceptance remains incomplete.

The dated findings below retain their original test scope; the consolidated shell
entry point does not reproduce all retired one-off mutation campaigns.

Reviewed 2026-10-31; next review: 2026-11-14 or before backend adoption.

## Reproduce on Windows

### UNORM16 weight probe

The separate GPU mip check uses the existing Rust/wgpu dependencies. Build it with
`cargo build --locked --release --features game-assets --example game_asset_mip_gpu`,
then run the historical validation harness. Both Vulkan and
Direct3D 12 must pass; an unavailable backend is a failure rather than a skip.
The direct executable accepts `vulkan` or `dx12`. It uploads three explicit NPOT
atlas levels and checks 2,000 RGBA readbacks across integer, fractional and excessive
LODs against CPU interpolation of the uploaded bytes. A maximum-LOD sampler blocks
poisoned extra levels; 100 unbounded controls must expose them. A rasterized
two-chart fixture additionally checks 16,660 pixels with implicit derivative LOD
at four viewport sizes. Its ten unbounded over-minified pixels must expose green
sentinels while bounded chart colours remain exact. These are diagnostic GPU
checks, not viewer integration, general asset appearance or engine acceptance.

For the atlas reduction fixture, run `the historical validation harness
-OutputDirectory NEW_DIRECTORY` after building both native configurations and the
feature-enabled gcad executable. The direct reader command is
`gcad_glb_probe SOURCE.glb ATLAS.glb --atlas-minify`.
It independently decodes five PNGs and checks all 20,480 RGBA pixels, including
linear-light colour, linear alpha, packed scalar channels, flat normals, chart
gutters and unused space. Six charts contain 1,702 covered pixels per map, of
which 720 are padding. Four pixel-corruption controls must fail. Fixture and GLB
hashes must remain unchanged. This checks one atlas level, not mip safety or
engine rendering. The old bilinear-only executable fails this pixel oracle.

For nested assembly GLB export, run `the historical validation harness -OutputDirectory
NEW_DIRECTORY` after building both configurations. The `--nested-assembly` reader
mode checks the reusable `nested-assembly.gasset` fixture against analytic CAD
translations, Rodrigues rotation about `(1,2,3)`, a hinge rotation and the explicit
asset reflection/nonuniform scale. Eight body instances share geometry across two
asset owners; four bodies have 25-key motion tracks. It checks 97 poses with
`1e-7 m` key and `1e-6 m` between-key position limits, plus parent, placement,
reflection and key corruption controls. This verifies independent native import,
not engine appearance, FBX conversion or deformable skin quality.

the historical validation harness binds the nested skin
fixture, changes only a joint's parent identity while retaining its rest-world
placement, requires stale-weight refusal with no output, and explicitly rebinds
to a new weight file. Joint names, geometry stamps, weights and source guards
must remain equal while the rig stamp changes. `--nested-skin` independently
checks 33 analytic world poses, a nonidentity inverse bind and the isolated rigid
copy in both configurations; four weight/bind/parent/key controls must fail.
Original input hashes must remain unchanged. This is a rigid one-joint binding
test, not blended-skin quality or engine acceptance.

After exporting `nested-skinned.gasset`, run `the historical validation harness -InputGlb
SOURCE.glb -OutputDirectory NEW_DIRECTORY` with both v10 candidate builds. It
requires all 19 authored nodes and their parent associations, 36 raw/imported
weight corners, four reflected mesh instances, three skin corruption controls,
20,736 animated position/basis corners and 96 ordered faces. The scene audit now
checks tagged parent links explicitly. Skin controls select a skinned mesh even
when it is not the first mesh, and use an unrelated root node as the identity
control when the palette contains only one bone. No writer patch change is needed.
FBX geometry storage sharing and engine acceptance remain separate checks.

After building gcad with `game-assets` and preparing the authored fixture weights:

```sh
gcad export examples/game-assets/plate-flex.gasset target/flex-reference.glb
gcad asset-quantize-weights examples/game-assets/plate-flex.gasset target/flex-unorm16.glb --max-error 0.01 --max-angle 0.1
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/flex-reference.glb target/flex-unorm16.glb --quantized-weights 0.001 0.01
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/flex-reference.glb target/flex-unorm16.glb --quantized-raw 0.001 0.01
```

The `--quantized-weights` command currently **fails** with the pinned Assimp:
`BuildVertexWeightMapping` extracts into four floats, while `Accessor::ExtractData`
copies eight UNORM16 bytes without conversion. This is recorded incompatibility,
not a passing importer test. Dependencies are unchanged.

`--quantized-raw` independently decodes joint/weight bytes, checks normalized
unsigned-short accessors and exact 65535 sums, then uses imported geometry and
rig transforms to compare against the original float-weight export. It checks
unchanged vertex attributes, triangles, binds and rotation keys, zero influences,
quantisation steps, and position/normal/tangent differences at 65 poses. This
mode is specific to the two-joint flex plate. It does not repair or certify Assimp.

Matching Windows VS Code configurations export the float reference and quantised
candidate, then run the raw probe with the native Debug build. Run fixture preparation
first. The quantised export requires a new destination; preserve or explicitly move
an earlier `target/flex-unorm16.glb` before the next run. The configurations reuse
the build tasks above; debugger interaction itself has not been tested.

Debug and Release agree over 802490 vertex/pose comparisons: maximum position
change 0.000158527 mm and normal/tangent change 0.000874278 degrees. Zero limits
reject; using a float-weight candidate rejects the encoding check. Total GLB size
changes from 1975268 to 1876516 bytes. These diagnostic thresholds are not accepted
engine budgets. Evidence: `target/game-assets-quantize-smoke/` and native build logs
`target/game-assets-quantize-native-{build,debug-build}.log`.

### Native tool build

Skip cloning if those directories already exist; preserve unrelated checkouts.
The build script rejects a different revision or modified tracked dependency files.
It builds static libraries locally, enabling Assimp's FBX exporter and glTF importer; it does
not install them. CMake handles dependency-provided C/C++ sources, including zlib.
All third-party source, generated build files and outputs stay under ignored `target/`.

The build also produces `gcad_glb_probe`, an independent importer for the Rust
static GLB fixture. Run `cargo run --release --locked --features game-assets
--example game_asset_glb -- target/game-asset-prop.glb`, then
`target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-prop.glb`.
It verifies metre/Y-up bounds, geometry counts, winding, normals/tangents, PBR
factors and embedded PNG references. This is semantic import validation, not a
renderer or a complete glTF conformance validator.

Append `--rich` to both the Rust sample and import-probe commands to exercise
five textures, normal scale, occlusion strength, emission, alpha Mask/cutoff,
double-sided rendering, and nearest sampling with clamp/mirror wrapping. The
sample's two modes use the same CAD geometry. Opaque, Mask and Blend encoding,
normal-map convention conversion and invalid material inputs also have Rust tests.

Alternatively append `--scene` to both commands to check a rotated parent, two
instances of one shared two-material mesh, and mirrored/nonuniform scale. The
probe checks analytic world bounds within `1e-5 m`, hierarchy, mesh sharing,
material slots, transformed normal/tangent orientation and determinant-aware
winding. The sample reports source mesh counts; this variant stores two
primitives with 48 vertices total and renders 12 triangles per instance.

Append `--baked` to both commands to exercise CPU wood-pattern baking at a
250 mm tile size and 90-degree rotation. The importer checks two connected
256x256 PNG colour/normal tiles, linear finish tint, every vertex's UVs against
analytic projection, geometry bounds and tangent validity. It reads PNG header
dimensions but does not decode baked pixels or establish visual appearance.

Use the repository's **FBX feasibility probe (Windows)** VS Code launch configuration
with the C/C++ debugging extension already installed. It builds Debug first and
passes an output directory. No extension is installed automatically.

## What is checked

For independent crease-normal validation, export `plate-normals.gasset`, then
generate a geometry-only full-precision reference and run:

```sh
cargo run --locked --features game-assets --example game_asset_normals_reference -- examples/game-assets/plate-normals.gasset target/game-asset-plate-normals-reference.json
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-plate-normals.glb target/game-asset-plate-normals-reference.json --crease-normals
```

The reference writer refuses an existing destination. Its JSON contains only
positions, triangles and coordinate metadata, with no expected normals or smoothing
groups. The probe verifies that each reference corner converts to the actual GLB
corner, independently traverses adjacent face fans at 60 degrees, and checks the
imported area-weighted normals, flat normals, UVs and tangent orthogonality.
Float32 GLB positions alone are insufficient: quantization can collapse distinct
CAD points and invent smooth connections. Full-precision adjacency is retained.
Debug/Release agree within `4.10e-8` normal-vector error across 1276 triangles per
plate. The comparison fixture's software preview was inspected; GPU/engine shading
remains unverified. Use **Crease reference geometry (Windows)** to debug the
reference writer, with a fresh destination for each run.

`gcad_fbx_probe --audit-skin INPUT.glb NEW_DIRECTORY` and
`--audit-skin-metres` audit the existing `plate-attachments.gasset` GLB through
Assimp's FBX writer and independent ufbx evaluation. The second mode supplies
`UnitScaleFactor` and `OriginalUnitScaleFactor` of 100 (centimetres per source
metre); it does not rescale vertices or animation keys manually. Both modes refuse
an existing output directory and write `converted.fbx` plus `report.json`.
Exit zero means the audit completed; inspect `sampled_geometry_pass` for its result.

```sh
target/game-assets-tools/build/Release/gcad_fbx_probe.exe --audit-skin target/game-asset-plate-attachments.glb target/fbx-skin-raw
target/game-assets-tools/build/Release/gcad_fbx_probe.exe --audit-skin-metres target/game-asset-plate-attachments.glb target/fbx-skin-metres
```

The fixture must first be generated as described for the attachment GLB probe
below. Its expected geometry is 12346 vertices and 20416 triangles, with two bones,
two one-second bend/twist clips, and named nested attachments. The audit checks
control-point correspondence, hierarchy and clip duration, then compares 66 poses
(4042368 face-corner samples) with an analytic deformation in metres. Attachment
origins are checked separately. It does not verify attachment orientation, shading,
material preservation, arbitrary clips or continuous error bounds.

Debug and Release agree: the raw route has maximum skin error `0.02390996999 m`
and attachment error `0.0118075918399 m`, reflecting the 100-fold unit mismatch.
The metre-metadata route has maximum skin error `5.24774693874e-10 m` and
attachment error `2.61627059775e-10 m`, passing this fixture's `1e-6 m` sampled
threshold. Both reports retain `production_ready: false`; prior PBR losses remain.
The VS Code **FBX authored skin unit audit (Windows)** configuration uses Debug;
prepare the GLB first and choose a fresh output directory before repeating it.

`gcad_fbx_probe --audit-maps INPUT.glb NEW_DIRECTORY` audits the authored
`plate-maps.gasset` GLB through Assimp's raw FBX conversion and ufbx import.
`--audit-maps-metres` additionally sets explicit metre unit metadata. Both create
an FBX plus JSON report and refuse an existing output directory. Success means
the audit ran, not production acceptance. The raw route loses PBR map connections
and shrinks world extents; metre metadata fixes the tested static scale but not
material loss. See the design document for measured Debug/Release results.

`--audit-maps-api INPUT.glb NEW_DIRECTORY` additionally tests a custom material
property through Assimp's public API. It verifies that the property exists before
writing and reports whether ufbx finds it afterward. The pinned writer drops it,
so public material overrides alone are not a general PBR export extension point.

LOD exports also support sampled acceptance gates: `--max-surface-error MM`,
`--max-surface-p95 MM` and `--max-silhouette-difference FRACTION`. CLI verification
on 2026-10-31 accepted the flex-plate fixture at `0.03 mm`, `0.000001 mm` and `0.01`
respectively, and rejected a `0.01 mm` maximum before creating an output file.
The Rust report contains three shared orthographic 256x256 coverage comparisons
per pose. Verify these independently from the original and LOD GLBs with:

```sh
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-plate-flex.glb target/game-asset-plate-flex-lod-gated.glb --silhouette
```

Windows Debug/Release passed on 2026-10-31: scanline rasterisation of imported skin
poses agrees across all 54 views, with zero differing pixels and at least 4526
occupied pixels per mask. Square/retriangulation and missing-half calibrations pass,
and a 1 mm displacement produces a difference. The method is independent of the
Rust triangle-edge raster but has the same orthographic, opaque, 256x256 scope;
it does not establish subpixel or arbitrary-camera contour accuracy.

For an exported skinned LOD:

```sh
target/release/gcad.exe asset-lod examples/game-assets/plate-flex.gasset plate target/game-asset-plate-flex-lod20000.glb --triangles 20000 --max-error 2 --max-uv-error 0.1 --max-angle 20 --samples 17 --max-trials 1000
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-plate-flex-lod20000.glb --plate-flex
```

On 2026-10-31 the trial-limited result contains 12179 vertices and 20082 triangles;
the 20000-triangle target remains unmet and the CLI report says so. Debug/Release
import checks pass weights, inverse binds and 17 analytic deformation poses with
maximum vertex error `6.2023e-09 m`. This verifies retained skin behavior, not
closest-surface LOD error or silhouette quality. The original asset and weights
remain untouched, and the output path must be new.

After rejection-witness caching, the same settings reach 20000 triangles and
12138 vertices in 215 evaluated trials, skipping 1110 unchanged rejections.
Debug/Release probes pass the retained skin at 17 poses with the same
`6.2023e-09 m` maximum analytic vertex error. This follow-up was verified on
2026-10-31; the earlier trial-limited result remains a useful work-budget reference.

For key reduction, run:

```sh
target/release/gcad.exe asset-reduce examples/game-assets/linkage-adaptive.gasset target/game-asset-linkage-reduced.glb --clip drive --max-error 0.01 --max-angle 0.01
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-linkage-reduced.glb --linkage-reduced
```

The output path must be new. Windows Debug/Release checks passed on 2026-10-31:
40 property keys, reduced from 396, evaluated at 129 poses against analytic
four-bar motion. Maximum position error is `8.68383e-06 m`; the `1.1e-5 m`
fixture allowance covers the bake, reduction and export contributions. This is
sampled rigid-motion evidence, not independent skinned or engine validation.

Skinned reduction fixtures reuse the guarded weights generated for `plate-flex.gasset`:

```sh
target/release/gcad.exe asset-reduce examples/game-assets/plate-flex-dense.gasset target/game-asset-plate-flex-reduced.glb --clip bend --max-error 0.001 --max-angle 0.001
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-plate-flex-reduced.glb --plate-flex
target/release/gcad.exe asset-reduce examples/game-assets/plate-flex-eased.gasset target/game-asset-plate-flex-eased-reduced.glb --clip bend --max-error 0.05 --max-angle 0.1
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-plate-flex-eased-reduced.glb --plate-flex-eased
```

On 2026-10-31, Windows Debug/Release passed both: constant-speed rotation reduces
9 to 2 keys (17 poses, maximum vertex error `6.2023e-09 m`); quadratic accelerating
rotation reduces 33 to 17 keys (129 poses, maximum vertex error `2.31259e-05 m`).
The eased probe compares against `90 * time^2` degrees and allows `6e-5 m` for
source-key interpolation, reduction and export rounding. Both check two-bone
weights and inverse binds; neither verifies rendered shading or engine behavior.

For constrained mechanical motion, export
`examples/game-assets/linkage-motion.gasset`, then run `gcad_glb_probe OUTPUT.glb
--linkage-motion`. The probe independently solves the four-bar's upper
circle-intersection branch and compares all vertices at 25 keys and 24 midpoints.
Windows Debug/Release passed on 2026-10-31: maximum key error `1.59144e-08 m`
(limit `1e-6 m`), midpoint error `9.42545e-07 m` (fixture limit `1e-5 m`).
These are sampled fixture bounds, not a continuous constraint guarantee.

Export `examples/game-assets/linkage-adaptive.gasset` and use `--linkage-adaptive`
to validate automatic rate increases within the declared ceiling. The fixture
starts at two intervals, accepts 32, and exports 33 keys. Windows Debug/Release
passed on 2026-10-31 at all keys and 96 intermediate samples: key error
`1.58119e-08 m`, between-key error `5.30855e-07 m` (limit `1e-6 m`). The probe
also checks quarter-interval samples for the fixed-rate `--linkage-motion` mode.

For generated-part skinning, generate weights and export the assembly fixture:

```sh
target/release/gcad.exe asset-bind examples/game-assets/linkage-part-skin.gasset first/crank target/game-asset-linkage-part.weights.json --joints bone
target/release/gcad.exe export examples/game-assets/linkage-part-skin.gasset target/game-asset-linkage-skin.glb
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-linkage-skin.glb --linkage-skin
```

`asset-bind` requires a new output path. The probe checks complete rigid weights,
inverse bind placement, a separate unskinned repeated crank, and nine analytically
known poses of the 10 mm lift. Windows Debug/Release passed on 2026-10-31 with
maximum position error `2.50766e-09 m`, below `1e-6 m`. This verifies rigid skin
export and instance isolation, not engine behavior or bent CAD surface quality.

Export `examples/game-assets/linkage.gasset` and run `gcad_glb_probe OUTPUT.glb
--linkage` to validate evaluated CAD assembly conversion. It checks two source
groups, four shared meshes/eight child bodies, the per-part red crank override,
100 mm separation at every vertex and the first group's 10 mm lift channel.
Windows Release passed with maximum instance-offset error `7.45058e-09 m`, below
`1e-6 m`. This does not reconstruct nested CAD source hierarchy or bake constraints.

Export `examples/game-assets/linkage-part-motion.gasset` and use `--linkage-part`
to check the generated `first/crank` animation target. In addition to the shared
assembly checks, nine poses compare every crank vertex with its analytic 10 mm
lift while retaining its rest rotation. Windows Debug and Release passed on
2026-10-31 with maximum error `7.45058e-09 m` (limit `1e-6 m`).

Generate the Rust example with `--skin`, then run `gcad_glb_probe OUTPUT.glb
--skin` for the six-vertex, two-bone textured ribbon. The check covers nonidentity
mesh/joint bind placements, rigid endpoint and 50/50 middle weights, an unskinned
copy, and bend/twist clips. Nine samples per clip compare all vertices against
analytic world positions within `1e-6 m`. This establishes numeric deformation,
not rendered skin normals/tangents or suitability of CAD tessellation for rigs.

For the authored CAD skin, generate weights and export `plate-skinned.gasset` as
described in [the design record](../../docs/game-assets-design.md), then run
`gcad_glb_probe OUTPUT.glb --plate-skin`. It validates imported joint weights,
hierarchy and one-second bend timing, and compares all 1240 vertices at 17 poses
against analytic deformation (limit `1e-6 m`). It also reports triangle area
ratios, maximum edge stretch, and maximum/p95 centroid interpolation errors.
The surface reference is the continuous explicit linear weight field evaluated
at each rest triangle centroid. Percentiles weight triangle/pose samples equally.
These diagnostics do not establish surface-wide, silhouette or rendering quality.
Windows Debug/Release passed with `7.67988e-09 m` vertex error, but exposed near
triangle collapse and `5.07022 mm` maximum centroid error: this plate is not an
accepted deformable asset. Keep the reference while evaluating topology/weights.
The same `--plate-skin` probe accepts `plate-refined.gasset` output: two uniform
refinement levels produce 20416 triangles and reduce maximum centroid error to
`0.316888 mm`, but still show near-collapse. See the design record for commands
and the complete comparison. Refinement is not a quality acceptance shortcut.

`plate-flex.gasset` changes the refined reference to a centred through-thickness
bend with the original wide y blend; validate it with `--plate-flex`.
`plate-flex-narrow.gasset` uses the same joint and motion with a -7.5 to 7.5 mm
blend; validate it with `--plate-flex-narrow`. Each mode checks its own analytic
motion and weight field, including rejection when the wrong profile is used.
Both pass Windows Debug/Release numeric import. The wide blend avoids near-collapse
in sampled poses; the narrow blend increases area expansion and interpolation
error. These are diagnostic candidates, not visual or production acceptance.

For authored rigid animation, export `examples/game-assets/plate-motion.gasset`
and run `gcad_glb_probe OUTPUT.glb --plate-motion`. The independent Assimp reader
must retain both two-second clips and their target nodes. The probe interpolates
the imported keys and compares a point transformed through the parent/child chain
against analytic rotation/translation at eleven times per clip (limit `1e-6 m`).
Export `plate-scale.gasset` and use `--plate-scale` to check LINEAR nonuniform
scaling and STEP reflection changes, including both sides of a discontinuity.
The scale probe checks imported interpolation modes and world determinant signs.
These fixtures do not establish engine playback or skinning support.

For authored hierarchy validation, export
`examples/game-assets/plate-instances.gasset` through the feature-enabled gcad CLI,
then run `gcad_glb_probe OUTPUT.glb --plate-scene`. It checks the three named nodes,
one shared plate mesh, reflection and analytic bounds within `1e-6 m`, plus the
presence of normals, tangents and UVs. This checks the real authoring/export route.

The original fixture is a six-vertex, four-triangle ribbon with normals, UVs and an
original generated 2x2 checker BMP. Static and skinned binary FBX files reference
the texture by relative filename. Internal fixture dimensions are centimetres;
the ribbon is 100 cm long. This is deliberately independent of gcad's millimetre
conversion, which still needs its own adapter tests.

The skinned fixture has Base and Tip bones, nonidentity rest translations and
inverse binds, rigid endpoint weights and 50/50 middle weights. Two one-second
clips rotate Tip by 90 degrees around Z (Bend) and Y (Twist). Each has three keys.

ufbx verifies geometry and attribute values, connected material texture, units,
hierarchy, weights, clip names/durations and evaluated skin positions at nine
times per clip, including between keys and the bind pose. Expected positions are
computed analytically, not by Assimp. The maximum Euclidean vertex error must be
at most 0.001 cm (0.01 mm).

To verify copied output without rewriting it:

```sh
target/game-assets-tools/build/Release/gcad_fbx_probe.exe target/game-assets-tools/relocated --verify
```

The specified directory must contain `static.fbx`, `skinned.fbx` and `checker.bmp`.
A missing texture, malformed file or failed semantic check returns nonzero.
Normal mode writes these three filenames; use a dedicated output directory.

## Evidence and limits

Windows x64 MSVC, CMake 4.2.3-msvc3; Release prototype passed on 2026-10-31.
Maximum sampled error: `2.00972e-14 cm`. See
[the feasibility record](../../docs/game-assets-design.md) for the native baseline
and remaining acceptance work. FBX headers may include generation timestamps;
artifact hashes identify a run, not byte-for-byte reproducibility across runs.

This does not test engine import, renderer appearance, PBR packing, morphs,
constraints, arbitrary Euler rotation sequences, negative/nonuniform scale,
skinned instancing, Linux, macOS or WASM. Assimp expects bind-pose input and its
skinned-instancing assumptions need explicit handling before a backend is adopted.
The BMP proves texture linkage; the production pipeline still needs portable PNG
or JPEG packaging and material capability reporting.

## Sources and licenses

`gcad_glb_probe FILE.glb --rigid-motion` verifies schema-2 rigid root motion using
Assimp animation import, separately parsed metadata and an analytic turning path.
Debug/Release check 17 poses and ten forward/reverse loop times, with maximum
recomposition error 1.666e-8 m and analytic error 6.67441e-8 m. Reversing composition
order creates 0.0433692 m error. The in-place root is frozen and the unrelated shared
instance remains unchanged. This linear unskinned fixture does not establish engine,
STEP, animated-scale or skinned-root-motion import support.

`gcad_glb_probe FILE.glb --root-motion` checks the translation root-motion CAD
fixture. RapidJSON reads the GLB extras separately from Assimp's scene importer.
Debug/Release validate 17 clip poses plus eight forward/reverse loop samples,
preserved vertical/rotational motion and the unrelated shared instance. Maximum
recomposition error is 5.96046e-8 m; analytic pose error is 1.49012e-8 m. Omitting
root displacement produces 0.111803 m error. This fixture uses LINEAR tracks;
engine root-motion handling and independent STEP/skinned import remain unverified.

`gcad_glb_probe SOURCE.glb LOD.glb --lod-normals` checks the refined plate source
(5104 triangles) against its corrected 5000- or 3000-triangle normal-baked LOD.
Independent rasterization and plane/barycentric projection check all 1349 covered
pixels. Float32 imports use a fixed 0.02-micrometre nearest-hit coincidence band.
Debug/Release agree that the 3000-triangle fixture's maximum normal error changes
from 1.57433 to 0.318367 degrees. The old pre-alignment bake fails at 45.3187 degrees,
providing a real regression control. This is sampled, rest-pose verification; it
does not certify engine rendering or animated normal maps. Reproduction commands
and artifact hashes are in `docs/game-assets-design.md`.

`gcad_glb_probe NORMAL.glb --normal-detail` checks the example's `--normal-bake`
fixture against its analytic two-axis slope. Debug/Release reconstruct 1152
projected and 392 padded normals using imported tangent frames, including a mirrored
UV chart. Maximum error is 0.226419 degrees; a flipped-green control differs by at
least 32.6438 degrees. It also checks 504 neutral outside texels, unchanged flat
geometry and clamp/no-mipmap sampling. This verifies analytic detail transfer and
encoding, not high-poly performance, mipmaps or engine shading.

The GLB probe additionally accepts `SOURCE.glb ATLAS.glb --atlas` for the CAD-box
example's `--rich` and `--atlas` variants. It uses the pinned Assimp checkout's
`contrib/stb/stb_image.h` to decode PNGs independently of Rust. Debug and Release
verify 35760 exact RGBA samples, including 8320 padding samples, across six charts
and five packed textures. Source nearest-filter ties (12030 samples) are excluded
because float32 UV rounding can select either texel. This is not engine rendering
or general minification validation. See the game asset design document for commands.

- Assimp: https://github.com/assimp/assimp at the pinned revision above; BSD-style
  redistribution conditions in its `LICENSE`, plus bundled dependency notices.
- ufbx: https://github.com/ufbx/ufbx at the pinned revision above; MIT or public
  domain dedication, as specified in its `LICENSE`. The probe can use MIT.
- stb_image: bundled in the pinned Assimp checkout, dual MIT/public-domain terms
  retained at the end of `contrib/stb/stb_image.h`; the probe can use MIT.
- RapidJSON: headers bundled in the pinned Assimp checkout; MIT with dependency
  notices retained in `contrib/rapidjson/license.txt`. The probe does not use its
  `bin/jsonchecker` test data.
- The fixture and probe source are project-owned under gcad's GPL-3.0-or-later.

`gcad_glb_probe FILE.glb --attachments` checks the authored
`plate-attachments.gasset` fixture: two bone weights and inverse binds, bend and
twist clips, and two nested named attachment transforms. It compares 66 poses
against analytic rotations, including each attachment origin and three basis
endpoints. These are generic scene nodes, not an engine-specific socket contract.
Generate weights and GLB with the commands in `docs/game-assets-design.md` before
using the repository-local debug configuration. No new dependency is required.

`gcad_glb_probe FILE.glb --twist-chain` independently checks the five-bone
`plate-twist-chain.gasset` fixture over 33 poses, including piecewise weights,
analytic twist positions, nested attachment frames and the radial shrink bound.
The design document records CLI preparation and the still-poor triangle distortion;
this probe establishes export fidelity, not production deformation quality.

`gcad_glb_probe REFERENCE.glb SLICED.glb --twist-sliced` compares the uniform
five-bone reference with `plate-twist-sliced.gasset`. Besides analytic skin and
attachment checks, it verifies triangle boundaries at all 17 slicing planes and
compares rest area/signed volume. The design document records the smaller mesh
and improved area ratios, as well as remaining edge distortion and limitations.

The twist modes additionally sample 13 non-vertex barycentric points per face at
33 poses against the analytic continuous twist field. They report maximum, nearest-rank
p95 and RMS correspondence error; samples are not area-weighted and shared edges
are repeated. `REFERENCE.glb FINE.glb --twist-sliced-fine` checks the 33-plane fixture.
See the design document for the coarse/fine/uniform comparison and its limits.

`gcad_glb_probe FILE.glb --surfaces` verifies `plate-surfaces.gasset` after ordinary
CLI export: separately authored MASK/BLEND materials, linear RGBA, metallic,
roughness, emission, cutoff and sidedness. This checks imported semantics, not
engine compositing or visual appearance. The design document lists preparation.

`gcad_glb_probe FILE.glb --maps` verifies the `plate-maps.gasset` export, including
all 80 embedded RGBA bytes, roughness/metallic packing, negative-Y conversion,
samplers and six declared provenance records. Expected source hashes were computed
independently of Rust. The two original fixture PNGs and their pixel/license notes
live in `examples/game-assets/maps`; no external artwork is used.

`gcad_glb_probe REFERENCE.glb ASSIGNED.glb --face-material NODE` checks the
`base.end` material-assignment fixtures: the selected face must be on the maximum
glTF-Y plane, with red base colour and roughness 0.2. It compares triangle-corner
multisets (including winding and multiplicity), normals, UVs, tangents/bitangents,
bone names/weights/inverse binds, node hierarchy/transforms and animation keys.
Selected/unselected base colour, roughness and metallic factors are checked, and
embedded map bytes must remain identical. Seventeen synthetic affine bone poses
compare weighted positions; this is not an engine playback or rendered-shading test.
The reference must have one primitive per mesh node and the changed node two.
All-face assignment is outside this probe's fixture contract.

`gcad_glb_probe REFERENCE.glb EDITED.glb --face-roughness NODE` checks a whole-node
roughness override to 0.25 on an already partitioned reference. Primitive count/order
must remain the same, and each primitive's inherited base colour and metallic factor
must survive. It retains the paired attribute, texture, skin, hierarchy, clip and
17-pose checks. `plate-face-materials.gasset` passes in Debug/Release with three
material regions and 65,076 posed corners. Unchanged-roughness and changed-colour
controls fail. Evidence: the historical validation harness.

Shared box, six-map plate and skinned plate pass Debug/Release. The skinned fixture
checks 65,076 posed triangle corners per build. Unmodified-output controls and
deliberately changed weights/winding are rejected. Local reproduction scripts and
evidence are in the historical validation harness and
the historical validation harness; the original source fixtures and exported inputs are preserved.

Dependency license files remain with their source checkouts. No third-party
binary or source is vendored or distributed in this change. Packaging a production
backend requires an explicit notice inventory and dependency selection.

### Isolated material-property writer experiment

`experimental-material-properties.patch` extends the pinned Assimp writer only
when a material supplies the explicit `gcad.extension.version=1` property. It emits
custom roughness/metallic factors, roughness/metallic/occlusion texture connections,
and their glTF packed channel identifiers (G/B/R). It retains legacy shading.
The extension is specific to the GLB route; it is not a general Assimp material
conversion and is not wired into gcad's exporter. Assimp's original BSD license
and notices remain in the pinned source checkout; this patch derives from that
revision. Patch SHA-256:
`37EF939F2AB1328D0CFBB25B2B9D5E6C9B5D20630A245F1678968E98FA5FC5AB`.

With the existing pinned checkouts available, run from the repository root:

The build script creates a detached local Assimp worktree if missing, applies the
patch and requires its entire tracked diff to match before building. It downloads
and installs nothing; the original pinned sources and ordinary probe build remain
separate. Use a new output directory for each audit. Debug/Release agree: custom
connections retain exact embedded image bytes, factors and channel identifiers;
metre-scaled world extents remain correct. An unmodified-writer control fails the
extension assertion, and opt-out reports match the original writer's report.

This does **not** fix generic FBX PBR interpretation: ufbx still reports the legacy
roughness and lacks standard roughness/metallic/AO map connections. A consumer
adapter must interpret the custom properties and channels. Normal scale, occlusion
strength, alpha modes, samplers, full material provenance and engine acceptance
are outside this experiment's proof. Production backend selection remains open.
Reports: `target/fbx-maps-extension-{release,debug}/{report,extension}.json`;
controls: the historical validation harness.

#### Version 2 material contract experiment

The separate `experimental-material-properties-v2.patch` retains version 1 unchanged.
Select it with the historical validation harness; its isolated worktree and
build directory use the `-v2` suffix. It still targets the same pinned Assimp source.
SHA-256: `550D07D7D0B222504DE4E999450460DEA54EA293008A96ACB64AB541631BE6E9`.
Use `material-extension-v2-build/Release/gcad_fbx_probe.exe` with
`--audit-maps-extension-v2`, `--audit-maps-extension-v2-blend` or
`--audit-maps-extension-v2-opaque`. The latter two deliberately vary the imported
in-memory material before export, including different filters per map; they do not
alter the source GLB. All audit destinations must be new directories.

An experimental consumer must explicitly recognize `gcad|PbrVersion=2`. These
properties override legacy material interpretation for that consumer; generic FBX
applications are not expected to recognize them. The intended contract is:

| Properties / texture connections | Interpretation |
| --- | --- |
| `gcad|BaseColour`, `BaseAlpha` | Linear RGB factor and linear alpha factor; the BaseColour texture is sRGB RGB with linear alpha |
| `gcad|Roughness`, `Metallic` | Scalar factors multiplied by the numeric map channels identified by `RoughnessChannel=1` and `MetallicChannel=2` (zero-based G/B) |
| `gcad|Normal`, `NormalScale` | Linear tangent-space normal texture in the exported GLB convention; scale decoded XY before normalisation |
| `gcad|Occlusion`, `OcclusionChannel=0`, `OcclusionStrength` | Numeric R occlusion map; strength blends from unoccluded 1 toward the sampled value for ambient lighting; the legacy Occlusion scalar is not strength |
| `gcad|Emission` | Linear RGB factor multiplied by sRGB-decoded emission texture RGB |
| `gcad|AlphaMode`, `AlphaCutoff`, `DoubleSided` | OPAQUE ignores alpha for coverage; MASK compares combined alpha with cutoff; BLEND uses combined alpha; sidedness is explicit |
| `gcad|SLOT|WrapU`, `WrapV` | glTF numeric modes: 10497 repeat, 33071 clamp, 33648 mirror |
| `gcad|SLOT|MagFilter`, `MinFilter` | glTF filter enums (9728/9729 and 9984-9987 where applicable); -1 means unspecified and requires a consumer default |
| `gcad|SLOT|UvSet` | UV coordinate-set index retained from the importer |

`SLOT` is BaseColour, Roughness, Metallic, Normal, Occlusion or Emission. Sampler
records exist only for connected maps. Material factors remain explicit even when
a map is absent. Unsupported Assimp wrap modes fail; this is specifically a GLB
conversion contract. It does not claim arbitrary shader/texture-transform support.

Debug/Release verify all six custom map connections and exact bytes, normal/AO/alpha
settings, sidedness, colour factors and sampler records for MASK, BLEND and OPAQUE.
Each case rejects 38 deliberately modified imported properties and restores them;
these are in-memory verifier-sensitivity controls, not corrupt-file or engine tests.
Original/version-1 writers reject the version-2 expectation, version-1 behavior is
retained and opt-out matches the original audit. Reproduction/evidence:
the historical validation harness and
`target/fbx-v2-verified-{Release,Debug}-{mask,blend,opaque}`.
The contract is a proposal with data-roundtrip evidence, not a tested renderer.
Version 2 does not carry source provenance; a consuming engine adapter remains
unimplemented.

#### Version 3 source-provenance experiment

`experimental-material-properties-v3.patch` combines the version-2 contract with
source declarations; versions 1 and 2 remain unchanged. Its SHA-256 is
`8B85B4A2FA96F2B6D74CD5FDA45A537C7F9B5237B06C690A784CF3EE45E7025D`.
Build with the historical validation harness (or
Debug), then run `material-extension-v3-build/Release/gcad_fbx_probe.exe` under
`target/game-assets-tools` with `--audit-maps-extension-v3 INPUT.glb NEW_OUTPUT_DIR`.
The source/build paths have a separate `-v3` suffix. The writer opt-in marker and
exported `gcad|PbrVersion` are 3; all version-2 map semantics still apply.

The probe reads `materials[].extras.gcad_texture_source_inputs` directly from the
source GLB because the importer does not retain these records. Its current checked
association requires one source mesh with one primitive and an explicit material
reference. It passes UTF-8 JSON through an Assimp binary material property, avoiding
the bounded `aiString` representation. The writer stores it on the FBX material as
`gcad|TextureSourceInputsJson`, with `gcad|TextureSourceInputsVersion=1`.
`provenance.json` is an audit copy; the FBX itself carries the records.

Each record requires `slot`, `project_path`, `sha256`, `license` and `origin` strings.
Slots use gcad's six source-map names. Paths must be relative portable paths without
empty, dot or parent components; hashes contain 64 hexadecimal characters; license
and origin declarations must be nonblank. Duplicate record fields fail. Unknown
fields are retained. The reader bounds the GLB JSON chunk at 16 MiB, the record array
at 4096 entries and serialized provenance at 1 MiB. JSON whitespace is normalized;
record values, array order and unknown fields survive. These are original source
hashes and authored declarations, not hashes of packed/transformed output images or
verification of license validity.

`--audit-scene-provenance-v3 INPUT.glb NEW_OUTPUT_DIR` extends the provenance audit
to several referenced materials and meshes. It tags material/mesh names in an
in-memory GLB copy because the pinned importer loads referenced objects lazily:
source indices cannot be assumed to equal imported indices. Every source material
must carry the extras array (an explicit empty array is valid). Imported primitive
references are checked against source identities. Original input bytes are not edited.
The FBX writer emits unnamed geometry and merges primitives attached to each node.
The audit also tags source nodes in memory and uses their identities to check output
mesh attachments. It compares ordered faces and corners, polygon material indices,
exact provenance, local vertex positions/winding and attached bone-name sets. It
checks for extra faces or mesh attachments. This verifies the pinned writer's ordered
conversion, not arbitrary face reordering or rendering.

Release/Debug pass 3,828 faces in a three-primitive merged fixture and 2,552 in the
prior two-skinned-mesh case. Deliberate imported-scene changes to a material index,
position, winding, mesh attachment and provenance length are rejected and restored.
The single-material-per-mesh case omits the material-index change. Reproduce the
merged case after building version 3 in both configurations:

The harness requires the existing authored shared-reference fixture and a new output
directory. It creates synthetic, distinct material declarations and duplicates a
primitive to exercise merging; these are diagnostic declarations, not real image
license claims. It requires the expected face/control counts and checks the original
input hash. Initial face-association evidence is under
`target/game-assets-fbx-scene-provenance/reproduced`.
The earlier mesh-count failures are retained there as historical diagnostic evidence
in the parent directory. No writer patch or source pin changed.

The current audit additionally checks per-corner bone identities, weights and
inverse binds through ufbx, plus 13 synthetic affine bone poses. Comparisons use
absolute tolerance 1e-6. The writer emits separate skin deformers with disjoint
vertex ranges for merged primitives: exactly one active deformer is required for
each weighted corner, and overlapping active deformers fail. Unweighted corners
must remain unweighted. Pose parameters are stable by bone identity across corners.
This checks linear skin data independently of vertex and cluster ordering; it does
not evaluate authored animation clips or engine treatment of multiple deformers.

Debug/Release pass the merged and prior two-mesh fixtures. Changed skin weight,
inverse bind and bone assignment controls bring the totals to eight and seven
respectively, including the five/four earlier scene controls. Every control restores
the imported scene, which must pass again. The harness now requires the expanded
coverage; final evidence is `target/game-assets-fbx-scene-provenance/skin-final`.
Authored animation timing/channel playback and engine acceptance remain pending.

The scene audit now checks authored LINEAR clips at 33 times: clip name/duration,
independently interpolated node world transforms and ufbx-evaluated skinned corners.
Transform tolerance is 1e-5; position tolerance is 1e-6 in source coordinate units.
STEP/CUBICSPLINE are refused explicitly. Duration and curve-value mutations test
transform-checker sensitivity before skin playback. This does not establish engine
acceptance or physical unit correctness; the scene audit has not set metre metadata.

Reproduce the authored plate with `the historical validation harness -InputGlb
target/game-assets-face-weight-declarations-smoke/authored.glb -OutputDirectory
target/animation-audit-new`. It adds explicit empty provenance only to untextured
materials lacking records, preserves geometry/animation bytes and checks input hashes
on success. Debug/Release pass 99 node-transform comparisons (maximum error 1.69434e-7)
and both animation controls, then **fail skin playback** at sample 1, primitive 1:
error 0.00106013. The pinned ufbx evaluator selects only the first skin deformer while
the writer emits three disjoint deformers on the merged geometry. This is an open
consumer-compatibility gate, not a passing animation test. Evidence is under
`target/game-assets-fbx-scene-provenance/animation-gate-{Release,Debug}.*`.
Next experiment: consolidate compatible skin deformers and rerun this exact gate.

#### Version 4 consolidated-skin experiment

`experimental-material-properties-v4.patch` combines the unchanged version-3
material contract with opt-in skin consolidation. SHA-256:
`6E785A500DEF4BB1EC03D13F4346329FEE1502B534E88E16F157A38C86B0D016`.
Build using the historical validation harness and
again with Debug. Separate `-v4` source/build paths retain the earlier experiments.
The caller passes the boolean export property `GCAD_FBX_MERGE_SKINS=true`;
`--audit-scene-merged-skin INPUT.glb NEW_OUTPUT_DIR` does so and requires at most one
skin deformer per output geometry. Material `gcad|PbrVersion` remains 3: the build
experiment version is not a new material schema.

One Skin object is emitted per merged node geometry. Each primitive's original
clusters, geometry vertex offsets and bind matrices survive, including repeated
bone identities across clusters with disjoint vertex ranges. Deformer definition
counts account for consolidation. This does not add support for arbitrary shared
skinned-mesh instancing or other unsupported source layouts.

Both harnesses accept `-WriterVersion 4`. The authored-animation harness now passes
in Debug/Release: 99 node transforms, 126,324 evaluated corners, maximum skin component
error 6.75314e-9, eight scene controls and two animation controls. The nonanimated
merged fixture also passes both builds. Evidence:
`target/game-assets-fbx-scene-provenance/v4-animation-verified` and
`v4-scene-verified`. Release without the option reproduces the earlier playback
failure (`v4-optout.stdout/stderr`); the six-map contract regression also passes.
Input hashes and earlier patch hashes are unchanged, and original pinned sources
remain clean. Physical metre metadata, additional interpolation/clip cases and
engine acceptance remain separate open gates.

The consolidated-skin scene mode now sets `UnitScaleFactor` and
`OriginalUnitScaleFactor` to 100, declaring metre source coordinates. Rest world
corners and animation translations/skinned positions are checked after applying
ufbx's declared `unit_meters`; the output declaration must be one metre per unit.
Changing that declaration to 0.01 is rejected by the rest checker and, for animated
fixtures, the animation checker. This is a bridge change, not a writer-patch change.

Both configurations pass the authored plate's 40 x 3 x 30 mm bounds and 126,324
animated corners (maximum error 6.75314e-9 m). The multi-node fixture passes 11,484
rest corners, 140 x 3 x 30 mm bounds and maximum error 3.72529e-9 m. Harnesses require
these unit checks for `-WriterVersion 4`; final evidence is under
`target/game-assets-fbx-scene-provenance/physical-animation-final` and
`physical-scene-final`. Earlier numeric-only results above remain historical.
Additional clips, translation/scale animation, STEP semantics and engine acceptance
remain open.

#### Version 5 scale-animation connection experiment

`experimental-material-properties-v5.patch` adds the scale-channel connection fix
to version 4. SHA-256:
`5345B4EE0C3310AE6F76587A618E39AE713824B747ED414B058D9FF0BD0CCEC0`.
Build with the historical validation harness for Release and Debug; use
`-WriterVersion 5` with either verification harness. The existing
`GCAD_FBX_MERGE_SKINS` export opt-in also selects the corrected `Lcl Scaling`
connection. Opt-out retains the original `Lcl Scale` connection; material schema
remains 3. Earlier patch files and source pins are unchanged.

Export `examples/game-assets/plate-multi-clip.gasset` using the feature-enabled gcad
executable, then pass that GLB to the historical validation harness. This fixture exercises
three separate LINEAR clips: rotation, translation and nonuniform scale with distinct
durations and intermediate times. The checker now samples every imported key time
and interval midpoint as well as the uniform grid. It still refuses STEP/CUBICSPLINE.

Debug/Release pass 315 node transforms and 401,940 evaluated skin corners with maximum
error 6.75314e-9 m; controls, physical scale and source immutability also pass. The
single-clip and multi-node regressions pass both builds. Evidence:
`target/game-assets-fbx-multi-clip/{verified,single-regression,scene-regression}`.
Version 4's failed scale case remains under `initial`/`diagnostic`. These results do
not establish arbitrary quaternion-to-Euler interpolation fidelity or engine support.

#### Version 6 STEP interpolation experiment

`experimental-material-properties-v6.patch` includes version 5 and writes FBX
constant-previous flags for imported STEP keys. SHA-256:
`9A9316832A3D5BCC14448DE8BAF1E496235D4E157DC7717EE7FBE41D305E5FBE`.
Build with `-Version 6` and run the harness with `-WriterVersion 6`. The existing
export opt-in selects this behavior; material schema stays 3. Every key in a curve
must agree on STEP or LINEAR; other/mixed interpolation modes fail. CUBICSPLINE
remains unsupported. Earlier patches are unchanged.

The checker compares imported interpolation with the raw GLB sampler declarations
and samples keys, midpoints, a uniform grid, and offsets of
`max(1e-7 seconds, duration * 1e-7)` on either side of each key within the clip.
Export `plate-step-clips.gasset` and use the historical validation harness on its GLB.
Debug/Release pass three STEP clips, 345 node transforms and 440,220 skin corners
(maximum error 6.3629e-9 m). The three-clip LINEAR regression passes the same expanded
sample count. A varying STEP segment changed to LINEAR is rejected, adding a third
animation control; constant components cannot exercise that control.
Version 5 fails the new STEP fixture. Evidence:
`target/game-assets-fbx-step-clips/{verified,linear-regression,v5-negative.*}`.
This establishes the tested discontinuities, not continuous bounds, arbitrary
rotation conversion fidelity or consumer-engine acceptance.

#### Version 7 sampled rotation-bake experiment

`experimental-material-properties-v7.patch` adds opt-in quaternion-to-Euler baking
to version 6. SHA-256:
`209D32279E15D2A190431F1697C17AA492D47268B3A2FCCCB9213DD890B48AE3`.
Build with `-Version 7` and verify with `-WriterVersion 7`. Material schema remains 3;
the same export opt-in selects the bake. STEP tracks are unchanged.

Each LINEAR rotation interval is subdivided uniformly; density doubles until sampled
rotation-matrix component errors are at most 2e-6 at endpoints and quarter points.
Conversion uses double precision internally, stores float Euler curves, and selects
equivalent Euler representations without the legacy helper's one-degree gimbal-lock
approximation. The bake fails if it cannot meet that sampled threshold within
65,536 keys or if times collide at FBX resolution. This is not a continuous bound;
large or difficult tracks may be refused and key-count growth needs budgeting.

The audit adds 37 deterministic off-grid samples per clip. The authored oblique
fixture passes Debug/Release with 216 node transforms, 275,616 skinned corners and
maximum skin error 2.18288e-8 m. Its exported scalar curves contain 1,545 keys total.
The prior writer fails this fixture. LINEAR and STEP regressions each pass 865,128
corners in both builds. Evidence is under `target/game-assets-fbx-oblique` in
`verified`, `linear-regression` and `step-regression`; earlier failed bake trials
remain as diagnostic evidence. Antipodal signs, gimbal crossings, multiple intervals,
representative key budgets and consumer-engine acceptance still need broader testing.

The version-7 stress baseline is now reproducible with `the historical validation harness
-InputGlb INPUT.glb -OutputDirectory NEW_DIR -AllowMultiArcRefusal`, where INPUT is
the exported `plate-rotation-stress.gasset`. Each clip runs separately in Debug and
Release. `results.json` records source quaternion key counts, total scalar FBX keys,
FBX sizes, elapsed milliseconds and explicit status. The optional refusal flag
allows only the known multi-interval bake-budget rejection; it is not a passing
rotation result. Without that flag, the current candidate fails the harness.

gcad canonicalizes quaternion signs. To exercise an actual antipodal input, the
harness changes only one quaternion's sign in its temporary GLB and asserts the
middle pair's dot product is at most -0.999999. The original source hash is checked.
Debug/Release pass that case, the gimbal crossing and the oblique clip. Their total
scalar key counts are respectively 4,617, 12 and 1,545; repeated output clusters and
default curves are included. The multi-interval track exceeds the 65,536-key bake
budget while its sampled error remains about 1.8e-5. Evidence:
`target/game-assets-fbx-rotation-stress/verified-baseline/results.json` and adjacent
logs. Local refinement around failing intervals is the next experiment; these
measurements do not establish representative performance budgets.

#### Version 8 local rotation-refinement experiment

`experimental-material-properties-v8.patch` replaces whole-track density doubling
with midpoint insertion only in failing intervals. SHA-256:
`5E91636154F027BF458DF2D46382C7BCC0660ACD987C5FFD1CADA9FBF5734F97`.
Build with `-Version 8`; all harnesses accept `-WriterVersion 8`. The material schema
and export opt-in stay unchanged. Limits are 65,536 rotation keys and 32 refinement
passes, with the same 2e-6 sampled matrix-error target. The candidate rejects
nonfinite errors and exhausted floating-point or FBX time resolution. No continuous
error guarantee or representative production key budget is established.

Run the historical validation harness without a refusal allowance. Debug/Release pass all
four clips. Total scalar FBX keys compare as follows (including default channels):

| Clip | Version 7 | Version 8 |
| --- | ---: | ---: |
| crossing | 12 | 12 |
| multi_arc | budget refusal | 2,982 |
| oblique | 1,545 | 1,056 |
| sign_flip | 4,617 | 2,106 |

The independent transform checker additionally samples exported key times and
midpoints, without skin-evaluating those extra poses. It aligns exported times
within two FBX clock ticks (`2/46186158000` seconds) to source keys, avoiding false
STEP comparisons across the quantized boundary. Exact source times and nearby
before/after samples are retained. The multi-interval maximum matrix-component
error is about 2.09e-6 (checker tolerance 1e-5), with sampled skin error below 3.8e-8 m.
This differs from the writer's internal sampled metric/reference precision.

Final evidence: `target/game-assets-fbx-rotation-stress/v8-final/results.json` and
adjacent per-clip logs; `v8-step-final` and `v8-linear-final` preserve both-build
regressions. The initial quantization-related audit failure remains in
`v8-step-regression`. Source hashes, earlier patch files and original pinned
checkouts remain unchanged. Shared skin instances, broader consumer testing and
production integration remain separate gates.

#### Isolated skinned-instance bridge

`--audit-scene-instance-skins INPUT.glb NEW_OUTPUT_DIR` combines the version-8 audit
with per-node isolation of repeated skinned mesh definitions before Assimp import.
The temporary JSON copies continue to reference the same source binary accessors;
the source file is unchanged. JSON copy expansion is limited to 16 MiB. Imported
meshes and FBX geometry may be duplicated: this preserves tested skin semantics,
not geometry-sharing efficiency. No writer patch changed for this bridge.

Imported bones must belong to their source skin's declared joint list, and inverse
binds must match its embedded dense float MAT4 accessor (tolerance 1e-6). Missing
inverse binds mean identity; sparse/external bind buffers are refused by the audit.
Wrong imported bind matrices and bone identities are deliberately rejected/restored.
The separate-skeleton fixture without isolation now fails before creating output.

the historical validation harness accepts `-InstanceMode SharedSkin` or `SeparateSkin`
plus `-IsolateInstances`. These diagnostic variants require the authored three-node
plate fixture. SharedSkin adds a node using the same mesh and skeleton; skinning
continues to use that skeleton's world transforms. SeparateSkin duplicates the
skeleton under a translated root, with copied animation channels and a separate
skin referencing the shared mesh. Both must preserve their source GLB semantics.

Debug/Release pass 551,232 animated corners per variant, with maximum error below
1.7e-8 m. Existing corruption/unit controls and new source-skin controls pass; source
hashes stay unchanged. Evidence: `target/game-assets-fbx-instances/SharedSkin-final`,
`SeparateSkin-final`, `single-regression`, and `source-guard-{Release,Debug}.*`.
The original second-instance skin loss is retained in `SharedSkin-v8` and
`SeparateSkin-v8`. Broader instancing layouts, shared output-storage efficiency and
engine acceptance remain unproven.

#### Resource observations and production proposal

the historical validation harness
checks core material factors and node transform declarations in both v10 builds.
The preflight validates factor ranges/types, alpha modes, booleans, transform
array sizes, matrix/TRS exclusivity, affine matrix rows and animated-node matrix
exclusion. Float representability and squared quaternion norm tolerance `1e-4`
are audit policies. Negative normal scale, alpha cutoff above one and reflected
node scale remain permitted. Three boundary cases intentionally remove provenance
to prove they pass this preflight and reach the later provenance refusal; they do
not demonstrate successful conversion. Matrix decomposability, singular transforms,
animated quaternion normalization and rendered material behavior remain separate.
Core declarations follow the Khronos glTF 2.0
[material schema](https://raw.githubusercontent.com/KhronosGroup/glTF/main/specification/2.0/schema/material.schema.json)
and [node schema](https://raw.githubusercontent.com/KhronosGroup/glTF/main/specification/2.0/schema/node.schema.json)
(reviewed 2026-11-01).

the historical validation harness
exercises the v10 pre-import texture checks with the three-clip textured fixture.
`scene_texture_probe.h` validates core texture/image/sampler references, sampler
enums, the five metallic-roughness texture-info locations, material references and
availability of each requested `TEXCOORD_n` on its primitive. Texture and texture-info
extensions are explicitly unsupported by this audit. These checks do not establish
nonzero UV-set export fidelity, complete PBR-factor validity or rendered parity.

the historical validation harness tests embedded PNG/JPEG
declarations, header dimensions and audit budgets against both v10 builds. It uses
the repository PNG fixture and a tiny JPEG generated locally with Windows
System.Drawing. Header-only dimension fixtures intentionally retain the old pixel
stream/CRC; passing those fixtures means only the header preflight reached the
expected later schema check, not that they are valid complete images.

`scene_image_probe.h` uses the already-pinned stb_image 2.30 header inspector,
compiled for PNG/JPEG with filesystem I/O disabled. It does not decode pixels.
Audit limits: 1,024 image declarations, 16 MiB per encoded image, 64 MiB aggregate
encoded reads (including repeated references), 8,192 pixels per dimension and
128 MiB aggregate estimated decoded storage at eight bytes per pixel. MIME/signature
agreement and unstrided, embedded views are required. URI-based images and other
formats remain unsupported audit cases. These checks do not validate complete
compressed streams, checksums, colours or decoder allocation behavior.

The FBX probe now compiles the pinned stb contribution; its MIT/Unlicense notices
are embedded in `contrib/stb/stb_image.h`. The inventory accepts `-WriterVersion
10` (default remains 9), records that notice source and includes the new compiler
input. This does not complete the production distribution-license audit.

Candidate v10 is the full v9 writer patch plus a glTF importer correction for
normalized integer skin weights. Build with `-Version 10`; the animation and
shading harnesses accept `-WriterVersion 10`. Material schema remains 3.
`the historical validation harness -InputGlb UNORM16_SCENE_CONTRACT.glb -OutputDirectory
NEW_DIRECTORY` checks both UNORM16 and a synthetic sum-preserving UNORM8 variant
with an eight-byte weight stride, in Debug/Release. It does not claim equal
precision between those encodings.

The source-weight checker now decodes raw GLB weights independently, maps them
through original triangle corners and joint names, and compares imported weights
within 1e-6 before export. Position equality verifies corner remapping. An altered
imported weight must fail. Each tested two-instance fixture checks 7,656 source
corners and 1,730,256 animated corners. Both integer routes and the float regression
pass; evidence is `v10-quantized` and `v10-float` under the scene-contract evidence
root. Generic regenerated-normal differences remain separate and unresolved.
Full v10 patch SHA-256:
`DDC3EBA640BAB3D0FFEAB7A81B1580E78B23478745C07CFEBE5E5100586069BB`.
This resolves the v9 normalized-weight failure described below for these fixtures;
mixed multi-set layouts and production integration are not verified.

`the historical validation harness -InputGlb THREE_CLIP_TWO_JOINT.glb -OutputDirectory
NEW_DIRECTORY` alters only BIN bytes for ten finite-value, index, palette, weight
and time checks. It expects the fixture's uint32 indices, uint16 joints and float
weights. Both builds refuse all cases without output or source changes. The
preflight decodes bounded dense accessors, checks finite floats, index ranges,
joint palettes, weight ranges/sums (1e-4 sum tolerance), and increasing nonnegative
times. This is not full numeric glTF validation. Float-weight positive evidence:
`target/game-assets-fbx-scene-contract/values-positive`.

The valid `values-unorm16.glb` generated by `asset-quantize-weights` passes raw
value checks but **fails** native conversion in both v9 builds. The pinned glTF
importer's weight mapping requests float vectors through an extractor that copies
integer bytes without normalization/conversion. Release/Debug failure diagnostics
differ; neither result is accepted. Evidence: `values-unorm16-verified` under the
same evidence root. An importer correction and independent quantized-weight
readback are required; do not infer FBX support from successful GLB quantization.

`the historical validation harness -InputGlb THREE_CLIP_TWO_JOINT.glb -OutputDirectory
NEW_DIRECTORY` checks primitive accessor contracts and repeated-use budgets.
It follows the skinned node's mesh reference rather than assuming source mesh 0.
The preflight requires the audit's triangle topology, validates standard attribute
shapes/normalization, equal vertex counts, paired joint/weight sets and unsigned
scalar index accessors. Skinned primitives require JOINTS_0 and WEIGHTS_0. Custom
attributes with underscore prefixes receive reference/count/vector-shape checks.
Non-triangle/morph-target and quantized geometric attributes are unsupported by
this canonical audit route, not removed from the wider product scope.

An audit estimate charges each primitive for eight bytes per attribute scalar
at the larger of vertex/corner counts, plus four bytes per corner. Mesh references
multiply that charge; unreferenced meshes are charged once. Per-mesh and aggregate
ceilings are 128 MiB. This accounts for repeated source references conservatively
but is not a bound on actual importer memory, images, allocator overhead or CPU.
Actual index values, joint indices and weights still require binary validation.

`the historical validation harness -InputGlb THREE_CLIP_TWO_JOINT.glb -OutputDirectory
NEW_DIRECTORY` mutates the existing scene-contract fixture in 16 ways while
preserving its BIN payload. `scene_rig_contract_probe.h` validates skin joints,
bind references/shapes/counts, node mesh/skin references, animation sampler and
target references, float time/output shapes, STEP/LINEAR key-count equality and
duplicate targets before instance isolation/import. The audit requires TRS
animation; morph-weight/extension-driven targets remain unsupported here.
Each mutation must exit cleanly with its expected diagnostic, leave its source
unchanged and create no output directory. This does not validate actual index,
joint-weight or animation-time values in the BIN data, mesh attribute contracts,
or complete skin hierarchy semantics.

the historical validation harness tests 22 buffer/accessor
cases in both v9 builds. The dense embedded route validates chunk/view/accessor
ranges before import, including stride, component alignment and matrix-column
padding. Layout rules were checked against the
[Khronos glTF specification, Data Alignment](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#data-alignment)
on 2026-11-01. The matrix fixture accepts omitted padding after the final column;
truncating actual column data fails. Exact-range and matrix-layout cases reach
the later missing-material check, while malformed cases fail in preflight.

The audit permits one embedded buffer and dense accessors with explicit views.
Sparse/external/multiple buffers and implicit zero-filled accessors are unsupported
here, not invalidated as glTF features. Limits are one million elements per
accessor and 128 MiB estimated scalar storage at eight bytes per scalar, summing
even overlapping accessors. These do not bound primitive duplication, decoded
images or every importer allocation. Source files remain unchanged and no refusal
creates a destination. Evidence: `buffer-bounds` and the full shading regression
`buffer-positive` under `target/game-assets-fbx-scene-contract`.

the historical validation harness exercises 21 graph cases
against both v9 builds. `scene_graph_probe.h` iteratively validates all source
nodes before importer recursion, including unreachable cycles. It rejects invalid
types/indices, duplicate children, multiple parents, invalid scene roots and bad
default-scene references. Limits are 65,536 nodes and 128 root-to-leaf nodes.
Exact boundaries and a root shared across scenes pass graph preflight and reach
the expected later missing-material check; malformed graphs fail earlier. Source
hashes must remain unchanged and no refusal may create a destination directory.
The complete reflected shading fixture also passes with this preflight enabled.
Evidence: `target/game-assets-fbx-scene-contract/{graph-validation,graph-positive}`.
This is a node-forest check, not a complete glTF validator or importer-memory bound.

the historical validation harness exercises ten malformed
or over-limit inputs and two exact structural boundaries in both v9 builds.
The shared provenance reader has audit ceilings of 64 MiB for the GLB, 16 MiB for
its JSON chunk, 64 nested containers and 1,000,000 combined container/key/scalar
events. An iterative SAX pass checks JSON encoding/structure before DOM creation.
The boundary fixtures intentionally lack scene materials: reaching that specific
schema refusal proves structural preflight accepted them. Every refusal must exit
with code 1, retain its input hash and leave the destination nonexistent.
Evidence: `target/game-assets-fbx-scene-contract/input-bounds-final`; the complete
shading regression passes under these guards in `input-bounds-positive`.
These limits do not bound image expansion, graph traversal or all importer memory;
they are not production asset budgets or a complete untrusted-input guarantee.

the historical validation harness inventories the
existing v9 Release build from the four native target projects and their MSVC
compiler read logs. It checks project-reference closure, hashes source/build
inputs and mapped notice files, and records project/log/executable hashes.
An unmapped contribution directory or unknown source root fails explicitly.
The result uses neutral relative paths; toolchain/Windows reads are counted only.
The current build has 113 declared translation units and 363 observed source/build
inputs spanning ten Assimp contribution directories. This does not establish final
linker retention, clean-build reproducibility, or complete license compliance.

See [the production-backend proposal](../../docs/game-assets-fbx-backend-proposal.md)
for the tested contract, packaging alternatives and remaining acceptance gates.
the historical validation harness runs the
Release v9 instance audit and records source/probe hashes, file sizes, elapsed time
and the maximum OS peak working-set value observed while live (50 ms interval).
The input must already contain the intended scene/instance fixture; this script
does not generate additional instances. It preserves the source and requires a
new output directory. Its timing includes all probe validation and corruption
checks and is not conversion-only throughput or a production resource limit.

#### Version 9 tangent-frame preservation

`experimental-material-properties-v9.patch` adds direct `ByPolygonVertex`
`LayerElementTangent` and `LayerElementBinormal` arrays and layer references.
The existing `GCAD_FBX_MERGE_SKINS` opt-in enables this behavior; material schema
remains 3. A merged geometry with only some primitive frames is refused rather
than emitting misaligned attributes. No frame is regenerated by this writer.

Build both configurations with the historical validation harness, then
use `the historical validation harness -WriterVersion 9 -InstanceMode SeparateSkin
-IsolateInstances -ExpectedMaterialMaps 18` for the combined fixture.
Debug/Release pass all corner shading checks, 132 material controls and 1,730,256
animated corners (maximum error 2.61536e-8 m). Four mutations cover normal, UV,
tangent and reversed bitangent; a separate export control removes one primitive's
frame and verifies the explicit refusal. The untextured regression also passes,
including checks of any frames present. Legacy writers may omit untextured frames,
but normal-mapped materials always require them.

Evidence: `target/game-assets-fbx-scene-contract/v9-verified` and
`v9-untextured-verified`. This supersedes v8's missing-layer failure for these
fixtures only. Rendered/engine parity and mirrored-chart coverage remain open.
Patch SHA-256:
`B6A00A9391159E70EB863D161FD7A224ACDC610B54144029A4F35A09D17F1ADF`.

#### Combined textured/animated/instanced scene gate

For mirrored shading and reflected-instance coverage, run
the historical validation harness after preparing examples
and building both configurations of the GLB probe and v9 FBX probe. The script
exports `plate-scene-contract` and `plate-scene-mirrored-uv`, verifies the authored
UV affine transform and geometry-derived tangent handedness with the independent
GLB probe, and rejects unchanged source UVs. It then invokes the scene harness with
`-ReflectInstance`, which requires `-InstanceMode SeparateSkin` and adds negative,
non-uniform scale to the instance root. World determinants and the reflected mesh
count are checked explicitly; removing the reflection must fail the audit.
These checks cover local shading tuples, node transforms and sampled skinned
positions. They do not evaluate rendered or deformed tangent-frame parity.

The animation audit now emits a separate **normal convention diagnostic**, not a
parity gate. It derives an inverse-transpose normal from the source's weighted
joint matrices and compares its direction against ufbx's evaluated normal. The
pinned ufbx evaluator recomputes normals from deformed triangles (including winding),
whereas gcad's `Skin::deform_basis` uses the inverse transpose of the blended linear
matrix and ignores spatial weight derivatives. The diagnostic counts angular
differences above 0.1 degrees and separates reflected from positive determinants.
Neither convention is substituted for the other. ufbx does not expose evaluated
skin tangents, so passing position/local-tuple checks still cannot establish
deformed tangent-frame or rendered normal-map parity.

`scene_basis_probe.h` is a separate explicit adapter experiment. It obtains each
corner's blended skin matrix from the evaluated FBX scene, inverse-transposes the
preserved normal, transforms/orthogonalises the tangent, and derives handedness
from the preserved normal/tangent/bitangent orientation and blend determinant.
Source-side expected values use independently sampled source joints, weights and
binds. Both builds compare 1,730,256 mirrored/reflected corners with maximum
component error 2.99752e-7 (limit 1e-5) and exact handedness. Four controls reject
normal/tangent direction changes, reversed bitangents and a singular matrix blend.
The untextured regression also passes. Evidence: `basis-adapter` and
`basis-untextured` under `target/game-assets-fbx-scene-contract`.

The adapter is not wired into a renderer or production exporter. It refuses
absolute determinants at or below 1e-10; that is an experimental guard, not an
established production conditioning policy. The shading harness requires adapter
coverage to match every sampled position corner. Generic regenerated normals
remain a separate diagnostic and retain the differences reported below.

Debug/Release evidence in
`target/game-assets-fbx-scene-contract/normal-conventions-final` reports 1,579,555
of 1,730,256 corners above 0.1 degrees (maximum 180). The positive-determinant
subset also differs: 714,758 of 865,128 corners. At clip time zero without
reflection, 9,204 of 11,484 corners differ (maximum 22.9719 degrees). These numbers
expose a real convention difference while position and local attribute contracts
remain passing; they must not be described as deformed-shading acceptance.

Export `plate-scene-contract.gasset` after running the example preparation script.
Use `the historical validation harness -WriterVersion 8 -InstanceMode SeparateSkin
-IsolateInstances -ExpectedMaterialMaps 18` with that GLB and a new output directory.
The instance fixture coverage now checks primitive/face counts rather than assuming
one fixed animation sample count, allowing multiple clips.

The scene audit validates material factors, colours, alpha/sidedness, normal/AO
settings, packed channels, samplers and exact map payloads. Absent source maps must
remain absent; six-map materials retain the existing 38 property controls plus
roughness/metallic factors, three channel indices and a payload-length control.
The combined fixture has three materials, 18 map connections and 132 material
controls. Before the shading gate was added, Debug/Release passed 1,730,256 animated
corners with maximum error 2.61536e-8 m (`target/game-assets-fbx-scene-contract/first`).

The scene checker now also compares corner normals and UV0 and requires the source
tangent/bitangent frame for normal-mapped materials. Both builds **fail** the combined
fixture because the pinned writer omits tangent/binormal layers. This is an open
normal-mapping fidelity gate, despite the earlier data-contract checks passing.
Failure logs: `target/game-assets-fbx-scene-contract/shading-{Release,Debug}.*`;
the untextured regression remains passing in `untextured-regression`.
Version 9 above resolves this missing-layer gate for the combined fixture.

`gcad_glb_probe REFERENCE.glb EDITED.glb --face-uv NODE` checks the planar
`base.end` fixture with scale `-2,3`, rotation 30 degrees and offset `0.2,-0.1`.
It accounts for Assimp's V-coordinate flip, checks selected UVs against the affine
formula and independently derives tangent direction and handedness from triangle
geometry. Primitive/triangle order must remain unchanged. All unselected tuples,
geometry, skin weights/binds, sampled poses, hierarchy, clips, checked PBR factors
and embedded map bytes must survive. This is a fixture-specific planar-chart check,
not a general tangent reference or engine acceptance test.

Debug/Release pass for the three-primitive skinned plate (65,076 posed corners), the
six-map plate, and the authored `plate-face-uv.gasset` example. Unchanged UVs, altered
selected UVs, reversed tangent direction and flipped handedness are rejected.
Evidence and paired software snapshots: the historical validation harness.

`gcad_glb_probe REFERENCE.glb EDITED.glb --face-weights NODE` checks a selected
maximum-glTF-Y plate face assigned 25% `base` / 75% `tip`. Primitive count/order and
all geometry/shading tuples must remain unchanged by triangle corner, but selected
boundary indices may split. Reference-side expected weights are replaced only on
selected corners; candidate weights come from Assimp. Seventeen synthetic affine
bone poses compare the expected weighted positions, including positions that must
differ from the original skin. Unselected corners and other instances must retain
their weights and sampled motion. Hierarchy, clips, bind matrices, checked material
factors and embedded maps are also compared. This does not evaluate engine playback
or guarantee deformation quality between samples.

Debug/Release pass direct and authored three-primitive plate exports (65,076 posed
corners, 4,320 changed expected positions), and a smoothed two-instance plate whose
selected boundary actually splits 68 vertices. The shared fixture compares 130,152
posed corners and preserves the other instance. Unchanged output, altered selected
or unselected weights, bind matrices and winding are rejected. Existing material,
roughness and UV probe modes still pass. Evidence and reproduction scripts:
the historical validation harness. The latter consumes
the source fixture generated by the authored shared-boundary Rust test.

## Single authored root regression (2026-11-01)

Preparation wraps an authored Assimp root in an identity container before name
restoration and FBX export. The writer otherwise omits that authored root model.
The container name avoids both temporary identities and authored names. This
changes helper preparation, not the v12 Assimp patch or material schema.

Export `examples/game-assets/plate-single-root.gasset` to a fresh GLB and run
`gcad_fbx_probe --audit-prepared-scene GLB` to exercise translation, rotation,
nonuniform scale and the `gcad_export_root` name collision. The existing strict
hierarchy, ordered-corner, material and physical-unit checks apply. Local evidence:
`target/plate-single-root-audit.log` passes (maximum world error 3.165e-9 metres);
`target/lod-static-fix-skinned-regression.log` passes for the skinned LOD. Rebuild
both helper and probe after changing the shared preparation header.
## Windows Unicode and relocated bundle regression (2026-11-01)

The helper now receives UTF-16 arguments through `wmain`, converts them explicitly
to UTF-8, and opens conversion/inspection inputs through `std::filesystem::u8path`.
This fixes failures on non-ASCII executable, input and output paths. The Rust
integration test `relocated_helper_preserves_unicode_paths` checks relocated
executables, Unicode names and paths longer than 260 characters, then validates
both inspection responses and unchanged input bytes. Run it with an absolute
`GCAD_FBX_HELPER` pointing to a rebuilt helper and `GCAD_FBX_INPUT` pointing to a
canonical exported fixture:

```sh
cargo test --release --locked --features game-assets --test fbx_helper_process \
  relocated_helper_preserves_unicode_paths -- --ignored --exact
```

Evidence: `target/helper-unicode-long-regression.log` passes for the transformed
single-root fixture. `target/helper-unicode-skinned-package-regression.log` passes
the existing complete-publication, material-contract, corruption and refusal
checks with the skinned LOD fixture. The Windows static-runtime bundle in
`target/helper-bundle-unicode-verified` passes strict SHA256 manifest verification;
its PE dependency table lists only KERNEL32.dll. Same-machine relocation is
verified; clean-machine deployment, complete source distribution and Linux remain
open release gates. Review again: 2026-11-14.
## Offline source rebuild (2026-11-01)

`sh tools/game-assets.sh package-helper NEW_OUTPUT` now includes all helper native
sources/build files, the active patch and self-contained pinned Assimp/ufbx source
repositories. See [SOURCE-BUILD.md](SOURCE-BUILD.md). Run the bundle's
`sh rebuild-helper.sh NEW_DIRECTORY` with the normal native build tools; it checks
the complete checksum manifest and patch identity before compiling. It uses no
remote dependency fetch, preserves existing destinations and keeps source snapshots
free of checkout-local remotes, hooks, reflogs, FETCH_HEAD and object alternates.

Verified bundle: `target/helper-source-bundle-verified` (88 checksummed files plus
manifest, approximately 153 MiB). A fresh MSVC rebuild and the native Unicode,
long-path, material/skin inspection and skinned publication tests pass. Source
corruption and existing-output controls are refused. Dependency Git fsck passes.
Evidence lives in `target/helper-source-rebuild-verified.log`,
`target/helper-source-rebuilt-unicode.log`, `target/helper-source-rebuilt-package.log`,
`target/helper-source-tamper-control.log` and `target/helper-source-existing-output.log`.
Executable hashes differ between original and rebuilt binaries; no reproducible-
binary claim is made. Clean-machine and Linux verification remain open.