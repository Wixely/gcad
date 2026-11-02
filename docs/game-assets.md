# Experimental game assets

Reviewed: 2026-11-01. Review before format stabilisation
or by 2026-11-14. This guide describes the opt-in implementation in this checkout;
the [development plan](game-assets-plan.md) remains incomplete. Detailed validation
and limitations are in [the design record](game-assets-design.md).

## Build and export a static asset

Use a shell and the existing Rust toolchain. The commands below show Windows
executable names; omit `.exe` on Linux. Fixture preparation and native/GPU checks
are documented in [development commands](../tools/game-assets.md).

```sh
cargo build --locked --features game-assets
target/debug/gcad.exe check examples/game-assets/plate.gasset
target/debug/gcad.exe asset-capabilities examples/game-assets/plate.gasset
target/debug/gcad.exe export examples/game-assets/plate.gasset target/guide-plate.glb
target/debug/gcad.exe asset-report examples/game-assets/plate.gasset --shading
target/debug/gcad.exe asset-preview examples/game-assets/plate.gasset --snapshot target/guide-plate.png
```

`asset-capabilities` runs the actual GLB encoder in memory and reports destination
counts and limitations as JSON. A successful encoding is not engine certification.

For experimental FBX plus separate PNG output, first export the canonical GLB,
then select the reviewed native helper by absolute path:

```sh
gcad asset-fbx-experimental target/guide-plate.glb target/guide-plate-fbx \
  --helper /absolute/path/to/gcad_fbx_helper --timeout-seconds 120
```

On Windows supply the absolute helper `.exe` path. The output directory must not
exist, and its parent must exist. The command snapshots the GLB, checks the helper
identity, converts and validates in staging, then publishes `model.fbx` and relative
`textures/image_*.png` files together. Failure preserves an existing package.
Frozen shader exports additionally include `gcad-shaders.json`, bound to the
published FBX's SHA-256 digest. The explicit Unity adapter reads this receipt
and attaches per-instance, explicit-time UV scrolling; ordinary consumers retain
the static frozen PBR fallback. Direct native-helper invocation does not create
the receipt. See the [Unity attachment workflow](../tools/game-assets-unity/README.md).
The deadline applies separately to each helper process. Both gcad and its helper
hold shared leases while using private staging. After an interrupted run, use
`gcad asset-fbx-recover OUTPUT_PARENT` to remove abandoned version-marked stages;
it skips active leases and never selects published packages or old unmarked
stages. Recovery removes partial work; rerun the conversion to produce a package.
Rebuild older experimental helpers: the caller now requires `leased-staging-v1`.
This remains experimental: engine import/render acceptance and deployment checks
are unfinished, and some material/deformation features require the explicit adapter.
The ordinary CAD export commands and viewer workflow are unchanged.
The capability report includes `artifact.animation_payloads`: each clip's unique
encoded timestamp/channel buffer-view bytes. It excludes JSON, alignment padding
and GLB overhead and is not runtime memory. `encoded_bytes` remains the full GLB
size. Static scenes report an empty animation payload list.

Both `asset-capabilities` and `asset-report` CLI JSON include `processing_seconds`.
Evaluation covers authoring/CAD loading; the second stage covers in-memory
encoding/inspection or sampled diagnostics respectively. These are cache-dependent
wall-clock measurements excluding report serialization, output I/O and engine work.
They are measurements of this invocation, not performance guarantees or GPU costs.
`asset-bake-normals`, `asset-lod` and `asset-reduce` also include
`processing_seconds` in their diagnostic JSON: `evaluation`, `derive_and_encode`
and `write`. LOD processing includes acceptance checks and optional normal baking.
Write time measures open/write calls, not durable storage (`fsync`). These stages
exclude diagnostic JSON serialization/output and engine work.

`asset-root-motion` keeps its trajectory JSON on stdout and reports timing as
a separate JSON line on stderr. Keep those streams separate when parsing reports.
Open a `.gasset` with `gcad FILE`, the existing viewer's open picker, or drag and
drop. It activates game-asset mode in the same gcad application; ordinary CAD
documents retain their CAD interface. `asset-preview FILE` uses that same app
and also accepts an initial clip/time or a snapshot destination. The adopted GPU
renderer supports explicit mip chains, texture alpha, occlusion and emission;
unsupported software/tracing paths fail explicitly.

Leave the game-asset document open while editing: it polls consumed dependencies every
200 ms and reloads in a background worker after a quiet poll. Invalid edits retain
the last valid model with an error; correcting the source triggers another load.
The camera is retained, clips are matched by name, and time is clamped to the new
duration. A removed clip returns to rest pose. Use **Fit model** to refit the
camera to the current geometry/pose after edits. File monitoring uses modification
time and size, so edits preserving both are not detected. Interactive acceptance
is still pending. The plan also requires inspection controls for UVs,
rigs/weights, root motion and LODs alongside the existing material rendering and
animation playback controls. See the [live-viewing requirement](game-assets-plan.md#live-game-asset-viewing-required-2026-11-01).

In game-asset mode, **Scene inspector** toggles a read-only side panel. Select a
node to inspect its parent, sampled local translation/rotation/scale, mesh
primitives, material factors, texture dimensions and skin joint assignments.
Parent and joint links select the corresponding node. Selection follows the node
name across reloads and clears if that node disappears. These are scene details;
The **UVs and texture maps** section selects a primitive. Expand **UV layout**
to see its triangle edges across all authored UV tiles, with U right, V up and an
orange 0–1 reference tile. Expand **Texture image** to select a map channel and
explicit mip level. These raw-channel thumbnails preserve transparency and are
bounded to 512 pixels on their longest side; source and thumbnail dimensions are
shown. Enable **Inspect source pixels** for undownsampled inspection of the
selected channel and mip. Drag to pan, set **Source X/Y** to reach an exact region,
and adjust **Texel zoom** (1–16 UI points per texel). Hover for original RGBA bytes
and pixel coordinates; coordinates start at the top-left, before material factors.
A checkerboard shows transparency. Only the visible region is uploaded (at most
512 × 256 texels), and live reload invalidates it. This is read-only inspection;
it does not edit textures or simulate material shading.
For skinned assets, select **Weights: JOINT** in the appearance selector to inspect
that joint's influence on the animated mesh. Blue means zero influence, red means
full influence, and unskinned meshes are grey. Use shaded view; scene lighting
affects the displayed colours. **Material appearance** restores normal rendering.
Joint selection follows its name across reloads and returns to material appearance
if that joint is removed from the skins. This diagnostic does not edit weights,
change deformation, or affect exports. **Skeleton (X-ray)** adds joint markers and
parent connections from the exact displayed pose, with optional **Joint labels**.
Gold markers identify joints; grey markers identify ancestor groups needed to
connect the hierarchy. The overlay draws through surfaces and is an inspection
aid, not an occlusion test. Failed updates retain the matching previous mesh and
rig pose. Full-resolution texture navigation remains pending.
For a clip with a translation track on a top-level node, **Root motion** selects
that node's XYZ translation for inspection. The green X-ray trajectory is anchored
at its first-key position; the white ring follows the current clip time, and the
controls report displacement in millimetres. Linear keys are joined; STEP keys
are shown as jump markers without interpolated lines. This uses the same exact
translation extraction as export, displays one clip cycle, and does not extract
rotation or modify the clip. Rest pose hides the trajectory.

**LOD comparison** generates a temporary derived scene with the same reducer as
`asset-lod`. Choose a mesh node/primitive, triangle target, position/UV/angle limits,
pose sample count and trial budget, then generate in the background. Switch between
**Source** and **Generated LOD** at the same camera and animation time; the scene
inspector and weight view follow the displayed version. The report states whether
the triangle target was reached. This does not write a file or establish engine
acceptance. Parameter changes and source reloads discard the comparison; an older
worker may finish in the background, but its result is discarded. The preview uses
no additional vertex locks or optional surface/silhouette acceptance gates; use
`asset-lod` for those options and persistent GLB output.
The panel is not shown for ordinary CAD documents.

Preview conversion limits prepared images to 4,096 slots, 256 MiB of CPU RGBA
payload and 256 MiB of GPU texels per frame. Instances share material maps;
metallic/roughness use one packed slot. Explicit chains count every level and
level-zero compatibility copies; legacy estimates include the full mip chain
and scratch space. Source data, geometry, driver allocations, render targets and
overlapping frames are outside these payload limits. These checks do not prove
interactive, Linux, browser or external-engine rendering acceptance.

Ordinary `export` and snapshot output can replace an existing destination. Binding,
LOD, key-reduction, root-motion and normal-baking commands require a new output
file. Keep source assets and reference exports when comparing derived variants.
No production FBX export command or shader-authoring command is implemented.
`asset-fbx-helper --helper ABSOLUTE_PATH` checks the optional native helper's
versioned handshake and pinned backend identity; it does not export. The current
helper keeps production conversion unavailable while exposing experimental staged
conversion through a separate capability. Build it with the v12 native
tools as described in [development commands](../tools/game-assets.md).
The [isolated FBX/import probes](../tools/game-assets-fbx/README.md) are development
tools; their results do not select a production backend.

## Asset description

`.gasset` is a strict, experimental, line-oriented format. Start with `asset 1`.
Unknown commands, unknown attributes and duplicate settings fail. Paths for CAD,
maps and weight resources resolve relative to the asset document. Comments start
with `#`. Every source needs explicit tessellation and box-UV settings.

```text
asset 1
project ../..
source plate ../parts/plate.gcad body=main
tessellate plate tolerance=0.1
normals plate crease=60
uv plate projection=box tile=25 rotation=0
surface plate base_colour=0.7,0.7,0.7,1 metallic=0 roughness=0.6
node plate source=plate
```

This example is relative to `examples/game-assets/`. Omit `normals` to retain CAD
normals. Its crease angle is in degrees, from 0 to 180. It rebuilds area-weighted
normals over connected face fans before UVs and binding. It preserves triangle
positions and winding; exact-position edge matching does not repair gaps or weld
nearby surfaces. The threshold applies at each edge, not to the total fan angle.

CAD/asset coordinates use millimetres, right-handed Z-up. Node transforms are local
translation, quaternion rotation (`x,y,z,w`) and scale, composed as `T*R*S`.
`parent=NAME` establishes hierarchy. Quaternion values must be unit length.
GLB export converts once to metres, right-handed Y-up: `(x,z,-y)/1000`.
Global `--set NAME=VALUE` overrides CAD variables for all asset sources.

Sources may reference `.gcad` or `.gasm`. CAD sources default to body `main`;
assemblies default to all bodies. Assembly expansion creates `INSTANCE/BODY`
mesh nodes with evaluated placements and per-body materials. Nested assembly
ancestors become meshless groups named `INSTANCE/PART/`, with deeper paths for
nested groups. Body node names remain unchanged; transforms are relative to the
retained parent. Selecting one body retains its ancestors. Identical geometry and
materials may be shared between instances. Mechanical motion baking supports these
descendants. Explicit keys use parent-local coordinates: review keys authored for
previously flattened nested assemblies and explicitly rebind affected skins.
This is hierarchy export, not full nested assembly editing or engine acceptance.

Optional mesh operations are specified per source:

| Command | Purpose |
| --- | --- |
| `refine SOURCE levels=N triangle_budget=N` | Uniform triangle subdivision before binding |
| `slice SOURCE axis=x\|y\|z planes=MM,... triangle_budget=N` | Split triangles along ordered mesh-local planes |
| `normals SOURCE crease=DEGREES` | Explicit smoothing/hard-edge rule |
| `uv SOURCE projection=unwrap resolution=1024 padding=8` | Generate unique UV islands with native xatlas, for painting a bespoke texture |
| `atlas SOURCE width=N height=N padding=N density=N filter=nearest\|linear [mips=N coverage=mask]` | Pack charts and rebake maps; optionally retain explicit levels and correct sampled mask coverage |

Choose refinement or slicing for a source, not both. Their triangle budgets cover
all selected source bodies. Preparation order is tessellation, refinement/slicing,
normal rebuilding, box projection or automatic unwrapping/tangents, then optional atlas packing (box UVs only). Skinning
is bound afterward. These operations never alter the manufacturing CAD solids.
Generated tangents use the current derivative/orthogonalisation implementation;
they are not certified MikkTSpace tangents.

### Painting a model-specific texture

Use `uv SOURCE projection=unwrap resolution=1024 padding=8` instead of box
projection for a bespoke painted image. Each prepared mesh gets its own layout;
this does not combine an assembly into one shared atlas. Hard normal seams are
preserved. Unwrap changes vertex tuples, so regenerate existing skin bindings
explicitly after changing it. Inherited CAD textures must be removed first;
bind the newly painted maps in the `.gasset`. Do not combine unwrap with `atlas`,
which rebakes existing textures rather than creating a blank paint layout.

```sh
gcad asset-paint-guide examples/game-assets/plate-unwrapped.gasset plate target/plate-guide --resolution 1024 --padding 8
gcad asset-paint-apply examples/game-assets/plate-unwrapped.gasset target/plate-guide target/plate-guide/paint.png target/plate-painted.png
```

Use the same resolution and padding in the UV declaration and guide command.
The guide directory contains `paint.png` (blank canvas), `labelled.png` and
`regions.png` (numbered/colour-coded islands), `uv.svg` (wireframe), six matching
orthographic model views, and `guide.json` (island centres, normals and mesh/UV
fingerprint). Views show one primitive in mesh-local space, without a rig pose.
Paint on the canvas using these references and the desired appearance reference.
Keep the exact dimensions, island positions and orientation: U runs right and V
runs down in the guide images. Do not bake guide labels, wireframes, lighting or
backgrounds into the colour map. Image generators may move islands or introduce
seams; inspect the result on the model and correct it before acceptance.

`asset-paint-apply` rejects stale geometry/UVs and wrong image dimensions, masks
the image to the current islands, and extends edge pixels into the padding.
Padding reduces filtering bleed; it does not make painted features continuous
across seams. Both commands require new output paths and preserve existing files.
Then add `map SOURCE base_colour textures/painted.png license=YOUR-LICENSE
origin=YOUR-PROVENANCE` with an actual relative path, licence and provenance.
Use a white base-colour factor and clamp wrapping for an untinted atlas. Existing
live preview reloads the PNG when it is saved; GLB and FBX use the same prepared
UVs and material maps. These commands do not invoke an image generator themselves.

Native unwrap supports 1–250,000 triangles per mesh, power-of-two resolutions
64–4096 and padding 2–resolution/16 pixels. Painting rasterization also has a
work budget; simplify dense meshes or lower resolution if it is exceeded.
Unwrapping requires a native build; the optional `xatlas-rs-v2` dependency is pinned
to 0.1.4 (MIT), wrapping [xatlas](https://github.com/jpcy/xatlas) (MIT). Building the
game-assets feature now requires a C++ toolchain and libclang for bindgen. The
Windows build is verified; Linux build and engine import validation remain pending.
Manual seam selection and a whole-assembly shared paint atlas remain future work.

The atlas filter controls resampling. `linear` averages source texel coverage when
reducing an image and uses bilinear sampling when enlarging it; colour/emissive RGB
filter in linear light, while alpha and numeric maps filter numerically. `nearest`
remains point sampling. Excessive filter footprints or total tap estimates are
refused before destination allocation. Without `mips`, the result remains single-level.
Optional `mips=N` requires linear filtering and retains exactly levels 0..N,
rejecting any level whose chart sampling footprints overlap. The adopted GPU
renderer previews these chains; software rendering and tracing refuse them.
GLB export still refuses explicit chains because their transport is not implemented.
Changing their base material or prepared geometry requires rebaking. Omit `mips`
for GLB export. See `atlas-mips.gasset` for a fixture.

Opt-in `coverage=mask` requires explicit mips, `alpha=mask` and a base-colour map.
It compares 1,024 deterministic UV-area samples per chart at each integer level
with level zero, using bilinear alpha sampling and the authored factor/cutoff.
A bounded search chooses a uniform alpha multiplier per chart and level. It
modifies only lower-level base-colour alpha, including owned gutters; RGB, other
maps, factors, geometry and level zero stay unchanged. Existing sampling-footprint
overlap checks still apply. Additional limits are one Mi triangle references and
16 Mi sample evaluations (four alpha taps each), checked before baking.

`asset-report` adds `mask_coverage` records containing the material, chart, level,
sample count, target/before/after pass counts and alpha scale. Residuals are visible
as differences between `target_pass` and `after_pass`. Identical coarse texels or
lost zero-alpha support can make the target unattainable; no spatial detail is
invented. This sampled UV-area metric does not guarantee projected silhouettes,
continuous area, trilinear transitions or GPU coverage. Level zero is the rebaked
atlas, not the original source image. See `atlas-mask-coverage.gasset` and the
repository-local Debug report configuration. Omit `coverage` for unchanged baking.

The isolated alpha renderer proposal compares `atlas-mask-coverage.gasset` with
`atlas-mask-raw.gasset` across three camera angles, two zooms and four image sizes.
The reference clamps every map to level zero. Readback discards alpha, so the
diagnostic counts pixels whose maximum RGB channel exceeds 3 against a verified
black empty-scene background, including partially covered MSAA pixels. It reports
both absolute occupied-count error and spatial mask disagreements. The reference
can alias; these are projected occupancy diagnostics, not fractional area or
target-engine acceptance. Correction may improve one metric while worsening the
other. See the renderer proposal README for reproduction and measured results.

The same diagnostic also renders the level-zero reference at 4x and 8x the output
dimensions and box-averages its binary occupancy into fractional values. It reports
absolute area-sum error and per-pixel absolute error (spatial L1) for low-resolution
base, raw and corrected masks, plus the 4x/8x reference drift. These remain contrast
measurements of shaded renders, not exact alpha coverage. A low-resolution partial
MSAA pixel counts as one occupied pixel, while a supersampled block can be
fractional; the low-resolution base measurement exposes that convention mismatch.
No production acceptance threshold is inferred from these diagnostic fixtures.

## Materials and maps

### Experimental shader descriptions

Attach the selected UV-scrolling PBR description to a source with:

```text
shader plate uv-scroll.shader.json
```

This requires `project ROOT`. The path is relative to the `.gasset` file and must
resolve inside that project. See `examples/game-assets/plate-shader.gasset` and
`uv-scroll.shader.json` for the version-1 format. Description reads participate
in document dependency tracking; editing the effect does not change geometry
binding provenance. Duplicate shader commands, unknown sources/fields, invalid
parameters and shader/atlas combinations are rejected.

GPU raster preview now evaluates UV scrolling at the asset viewer's explicit
time. Without a selected clip, shader assets get a ten-second timeline; with a
clip, shader and animation share its clock. Play, pause, loop and seek use the
existing controls. Editing the JSON reloads the effect while retaining time;
invalid edits keep the last valid frame. Direct3D 12 and Vulkan comparisons pass
against independently transformed reference UVs for the base-colour fixture.
Software/tracing reject nonidentity transforms; browser/Linux runtime and all-map
shader parity are not yet verified. Ordinary CAD/PBR callers use identity UVs.

Export honors the declared fallback. `{"mode":"reject"}` refuses ordinary
export; `{"mode":"freeze","seconds":2.0}` bakes sampling UVs at that time into
an owned scene. Authored meshes remain unchanged. Tangents are regenerated after
float32 UV rounding; collapsed UV triangles fail. Skin weights, vertex order and
inverse binds are preserved while derived binding guards are updated.

GLB scene extras and `asset-capabilities` record `shader_fallbacks`, including the
original description, frozen time and lost shader animation. The frozen GLB can
be converted to ordinary FBX plus separate PNGs; the FBX helper does not preserve
the GLB fallback extras, so retain the capability report when delivering FBX.
See `plate-shader-frozen.gasset` and `uv-scroll-frozen.shader.json`. Example:

```sh
gcad export examples/game-assets/plate-shader-frozen.gasset target/plate-shader-frozen.glb
gcad asset-capabilities examples/game-assets/plate-shader-frozen.gasset
```

Frozen/live raster comparisons pass on Direct3D 12 and Vulkan for this fixture.
This is a static appearance fallback, not a dynamic engine shader. Next action: a dynamic Unity adapter with parity tests, followed by wider material
and platform coverage. Reviewed: 2026-11-01.

`surface SOURCE` overrides only named fields in the derived material. Supported
fields are `base_colour` (linear RGBA), `metallic`, `roughness`, `emissive` (linear
RGB), `alpha=opaque|mask|blend`, `cutoff`, `double_sided=true|false`, `normal_scale`,
`normal_y=positive|negative`, `occlusion_strength`, `wrap_u`/`wrap_v`
(`repeat|clamp|mirror`), `magnify`/`minify` (`nearest|linear`) and `mipmap`
(`none|nearest|linear`). `cutoff` is required exactly when `alpha=mask`.
Unsupported CAD finish features fail instead of being silently omitted.

```text
map plate base_colour maps/rgba.png license=GPL-3.0-or-later origin=gcad-test-pattern
map plate roughness maps/numeric.png license=GPL-3.0-or-later origin=gcad-test-pattern
```

Slots: `base_colour`, `roughness`, `metallic`, `normal`, `occlusion`, `emissive`.
Maps require `project ROOT`, relative paths inside that project, and nonempty
license/origin declarations. These declarations record provenance, not proof of
ownership. Each image is limited to 8192 pixels per axis; total decoded source maps
have a 256 MiB RGBA budget. Colour/emissive image bytes are sRGB; scalar maps use
numeric red-channel values. GLB packs roughness into green and metallic into blue.
`normal_y=negative` flips the normal map's green channel on export.

Original source hashes and relative paths survive as material provenance even
when later baking changes exported pixels. For a complete runnable example, use
[`plate-maps.gasset`](../examples/game-assets/plate-maps.gasset). The included map
files have their own [provenance notes](../examples/game-assets/maps/README.md).

## Lighting, turntables and sprite sheets

Game-asset preview has independent light/model angle sliders and looping playback,
plus a GPU sprite-sheet exporter with arbitrary view angles, lighting angles and
animation sample times. See [the sprite workflow](game-asset-sprites.md) for the
viewer panel, `asset-sprites` CLI, JSON recipes and transparent PNG/metadata output.

## Clips and explicit skin binding

Declare `clip NAME interpolation=linear|step`, then keys such as:

```text
node base
node tip parent=base translation=0,15,0
clip bend interpolation=linear
key bend tip time=0 rotation=0,0,0,1
key bend tip time=1 rotation=0,0,0.7071067811865476,0.7071067811865476
skin plate weights=../../target/game-asset-plate.v2.weights.json
```

Each key supplies exactly one translation, rotation or scale value. Times are in
seconds and increase strictly within each property track. LINEAR quaternion tracks
use shortest-path interpolation. Ordinary named child nodes can act as attachment
transforms; no engine-specific socket contract is implied.

Prepare the checked-in skinned fixture once, using a destination that does not exist:

```sh
target/debug/gcad.exe asset-bind examples/game-assets/plate-skinned.gasset plate target/game-asset-plate.v2.weights.json --joints base,tip --blend 'y,-15,15'
target/debug/gcad.exe check examples/game-assets/plate-skinned.gasset
target/debug/gcad.exe export examples/game-assets/plate-skinned.gasset target/guide-skinned.glb
target/debug/gcad.exe asset-report examples/game-assets/plate-skinned.gasset --samples 33 --shading
```

If the weight file already exists, validate it first; do not regenerate implicitly.
For deliberate rebinding, choose a new resource path, create it with `asset-bind`,
and update the asset's `skin` declaration to that path. Binding skips the selected
node's previous resource. Other declared skins whose files do not exist yet may
be prepared afterward, so a multi-mesh asset can be bound one mesh at a time.
Existing peer files must still be valid; malformed/stale resources and other I/O
errors are not ignored. Normal check/export requires every declared skin. This
does not automatically repair multiple stale bindings after a shared rig edit.

For that case, explicitly defer the peer skins you intend to repair next:

```sh
gcad asset-bind model.gasset high high-v2.weights.json --joints base,tip --blend y,-15,15 --defer-skin low
# Update only high's skin declaration to high-v2.weights.json, then:
gcad asset-bind model.gasset low low-v2.weights.json --joints base,tip --blend y,-15,15
# Update low's skin declaration to low-v2.weights.json, then:
gcad check model.gasset
```

`--defer-skin` accepts comma-separated peer node names or repeated options. Each
must name a declared mesh skin; unknown names, the current target and duplicates
are rejected. Deferral skips that peer's old weight resource only during this
explicit preparation operation. Undeferred existing skins must still validate,
and check/export stays blocked until all declarations reference valid bindings.
The command creates only the requested new weight file and does not edit the
document or overwrite prior weight resources.

Use one joint for rigid weights, two with `--blend 'AXIS,START,END'`, or an ordered
joint list with `--chain 'AXIS,KNOT0,KNOT1,...'` and one increasing knot per joint.
Coordinates are mesh-local millimetres; joint positions are not inferred from knots.
Persisted weights guard geometry, rig and source dependencies. CAD, tessellation,
normal-rule, variable or texture changes can require explicit rebinding. Conflicting
file observations during one evaluation fail instead of binding inconsistent input.

Mechanical joint animation is separate: `motion CLIP INSTANCE joint=NAME from=VALUE
to=VALUE duration=SECONDS rate=HZ [max_error=MM max_rate=HZ]` samples the existing
constraint solver. Inspect the linkage fixtures and design record before relying
on sampled error limits; they are not continuous-motion guarantees.

## Derived exports and diagnostics

Use each command's `--help` for current required flags. Error distances are in
millimetres unless explicitly stated otherwise; angular limits are degrees.

| Command | Inputs and result |
| --- | --- |
| `asset-report FILE` | JSON counts and sampled deformation; `--shading` also validates shading directions |
| `asset-reduce FILE OUTPUT --clip NAME --max-error MM --max-angle DEG` | Reduce LINEAR keys against sampled pose/attachment/shading limits |
| `asset-lod FILE NODE OUTPUT --triangles N --max-error MM --max-uv-error UV --max-angle DEG` | Derive one node's mesh LOD, with optional surface/silhouette acceptance limits |
| `asset-bake-normals FILE NODE SOURCE SOURCE_NODE OUTPUT --width N --height N --max-distance MM` | Project source rest normals onto the target UV mesh |
| `asset-root-motion FILE NODE OUTPUT --clip NAME --in-place NAME --axes xy` | Extract selected root translation axes and keep an in-place clip |
| `asset-root-motion FILE NODE OUTPUT --clip NAME --in-place NAME --rigid` | Extract root translation and rotation, including turning loop composition |

LOD `rejection_counts` in its JSON report classifies tested candidates rejected by
position, normal, UV, tangent, posed-normal, posed-tangent, posed-triangle or topology gates; cached skips are
reported separately. Inspect `target_reached`: successful export can contain a
partial reduction or no reduction when the constraints prevent the requested target.
Scene LOD candidates check sampled deformed normals and tangents before accepting
a collapse, then retain the independent final shading-angle validation. A rejected
candidate can therefore be skipped without losing an otherwise valid reduction.

LOD accepts locked vertex indices through `--lock`, and optional
`--max-surface-error`, `--max-surface-p95`, `--max-silhouette-difference` gates.
`--bake-normals SIZE --bake-distance MM` adds rest-normal baking after LOD acceptance.
Sampling diagnostics are not continuous surface, silhouette or time-error bounds.
Root-motion metadata requires a consumer adapter; engines do not automatically
apply gcad's GLB extras. Experimental FBX packaging now preserves either root-motion
schema in `gcad-root-motion.json`, bound to the FBX and original GLB SHA-256 hashes.
The receipt resolves source/in-place clip and root-node names while retaining the
original metre/Y-up/right-handed trajectory unchanged. It validates references,
key times, composition and redundant loop/anchor data before atomic publication.
GLB motion key times and loop duration use the same float32 rounding as its clip
accessors. Native trajectory JSON retains authored double precision. Older GLBs
with mismatched fractional metadata must be regenerated with `asset-root-motion`;
package publication and Unity adaptation reject that mismatch.
Unsupported or malformed metadata rejects the entire new package. Ordinary exports
without extracted motion have no root-motion sidecar. Direct native helper calls
do not create these gcad-owned receipts. Engine-specific application and playback
remain a separate acceptance gate. The experimental Unity adapter now consumes
motion receipts through explicit `GcadRootMotionPlayer.Sample(seconds, looping)`
calls, including negative seeks and accumulated loops. Receipt version 2 also
preserves exact in-place TRS channels for every animated node, with bounded dense
float accessors and unique names. Unity samples these keys directly to preserve
STEP boundaries and quaternion interpolation. Version 1 STEP receipts must be
re-exported; their FBX curves import discontinuities early even without resampling.
See `tools/game-assets-unity/README.md` for validation and remaining limits.
Baking currently projects geometric rest normals, not an
arbitrary source shader or source normal-map detail.

## Explicit skin-palette pruning

Explicit socket, collider and LOD authoring/consumer conventions are documented in
[the experimental consumer profile](game-assets-consumer-profile.md). They are
opt-in; node names alone do not assign engine roles.

`asset-prune-joints INPUT.gasset NEW.glb [--keep NAME,...]` removes skin-palette
entries with no positive vertex influence. It retains every animated joint,
every parent node (including attachment parents), and explicitly named keep nodes.
All scene nodes, hierarchy, clips and source weight files remain unchanged. This
reduces palette/inverse-bind payloads; it does not delete skeleton nodes, merge
bones, approximate weights or promise engine performance improvements.

The JSON report lists each skin's original/retained joint counts, removed palette
node names and `old_to_new` mapping (`null` means removed). Zero-weight slots are
remapped to a valid retained index; positive weights and inverse-bind matrices are
preserved. Source binding guards must validate before derivation. Inputs without
skins or unknown keep nodes fail; output must be a new GLB file. Timings report
evaluation, palette derivation/encoding and buffered writes.

```sh
target/debug/gcad.exe asset-prune-joints examples/game-assets/plate-skinned.gasset target/pruned-plate.glb --keep base,tip
```

This example requires its generated weights first and deliberately keeps both
joints. Savings depend on unused entries in the input palette; no threshold-based
influence removal occurs.

## Guarded face-selection records

Capture and check a selector against one declared source in a `.gasset` file:

```sh
target/debug/gcad.exe asset-face-bind examples/game-assets/plate-skinned.gasset plate base.end target/plate-end.faces.json
target/debug/gcad.exe asset-face-check examples/game-assets/plate-skinned.gasset plate target/plate-end.faces.json
```

The source argument is the name from `source NAME PATH`, not a scene-node name.
`project ROOT` is required. A wildcard assembly source requires `--body NAME`
(for example `--body crank` for the linkage fixture). The selected body must remain
within the source's declared body scope. Assembly solids and face groups are both
returned to the local coordinates used for asset export. Source dependencies include
actually consumed imported parts, including cached reads. Missing skins or texture
maps do not prevent this geometry-only operation: maps, skins and scene clips are
not evaluated. The `.gasset` description itself must still parse normally.

`asset-face-bind` requires a new `.json` file. `asset-face-check` re-evaluates the
source and reports selected face/triangle counts without changing the saved record.
Source, dependency, parameter, instance, generator or selector/geometry changes can
require explicit recapture; a successful check is not automatic rebinding. Records
contain portable relative source paths and hashes, not runtime face IDs. Selector
text is ASCII without control characters, at most 4096 bytes. The record currently
also conservatively captures the source preparation settings in its generator guard.

The bind/check commands address raw tessellation, before refinement, slicing, UV
splitting, normal rebuilding or LOD. To resolve the record into a prepared scene node:

```sh
target/debug/gcad.exe asset-face-resolve examples/game-assets/plate-skinned.gasset plate target/plate-end.faces.json
```

This command takes a **node name**, including `INSTANCE/BODY` for generated assembly
children. It loads the complete asset, validates the record against that evaluation's
CAD observations and selector result, and emits JSON with primitive 0 and selected
triangle indices. Indices address this evaluation's prepared node-local mesh and are
transient; persist the source-bound record instead. All declared textures, skins and
clips must load successfully. The command does not modify the record or asset. None
of these three commands applies edits.

Assign surface settings to the saved faces and export a new derived GLB:

```sh
target/debug/gcad.exe asset-face-material examples/game-assets/plate-skinned.gasset plate target/plate-end.faces.json target/plate-end-material.glb --surface base_colour=1,0,0,1 --surface roughness=0.2
```

Repeat `--surface KEY=VALUE` using the existing `surface` settings. Unspecified
factors and texture maps are inherited. The command validates the record, splits
selected/unselected triangles into material primitives, copies complete vertex
attributes and skin weights exactly, and changes only the requested scene instance.
Selecting every face emits one primitive. The output must be new; the source asset,
weight files and selection record remain unchanged. This command creates a derived
export. To apply the assignment during normal check, preview and export, add a
declaration to the `.gasset` document:

```text
face_material plate plate-end.faces.json base_colour=1,0,0,1 roughness=0.2
```

The first argument is a scene node, including `INSTANCE/BODY` for generated assembly
children. Record paths are relative to the `.gasset` file. Multiple declarations for
one node may select disjoint faces; overlaps are rejected even with identical settings.
Every region inherits the source material before face overrides. Stale/missing records
and missing targets fail loading. Capture records before adding the declarations;
the geometry-only capture/check commands can also operate while declarations exist.
Persisted weight files bind the mesh before material splitting. `asset-bind` validates
face assignments but returns that original geometry, so regenerated weights remain
compatible with normal loading, which then remaps them into material primitives.
Prepared face queries retain provenance after declarations split a node into material
primitives. `asset-face-resolve` uses version-2 JSON for these nodes, with a `primitives`
array containing primitive-local triangle indices and counts (including empty selections).
Single-primitive queries retain their version-1 response. Derived face-material
assignments also work on already partitioned nodes: each affected primitive inherits
its own material before overrides, and unselected regions keep their current material.
Skin weights remap from the corresponding source primitive without averaging.
At the Rust library level, `FaceMap::refined`, `FaceMap::sliced`, `FaceMap::crease_normals` and
`FaceMap::project_box` return a derived mesh and updated companion map. Captured
geometry selections retain their source-face lineage through these operations;
ordinary mesh methods do not update an existing map. `FaceMap::simplified` returns
an LOD result and updated map, locks vertices shared by source faces in addition to
existing LOD boundaries and attachment locks, and rejects removal of an entire face.
Its requested triangle target may be unreachable; inspect `lod.report.target_reached`.
`Lod::source_triangles` maps surviving triangles to their original triangle indices.
`author::load_with_faces` runs the authored preparation pipeline with companion maps;
`asset.face_map(node)` returns the guarded map for that node's prepared primitive.
For partitioned nodes, use `asset.primitive_face_maps(node)` and
`asset.resolve_face_primitives(node, record)`. Every primitive is validated before
resolving, even if the requested selection only touches one of them.
Maps are per node even when repeated assembly parts share render geometry. Refinement,
slicing, crease normals, rotated box UVs and atlas packing preserve lineage. Ordinary
`author::load` does not collect face maps. `asset.resolve_faces(node, record)` validates
persisted source inputs and selector intent against the retained CAD evaluation,
then resolves into prepared geometry. It checks the loaded snapshot, without rereading
files; reload to check later on-disk changes. `face_material::assign` returns a derived
scene, skins and explicit original primitive/vertex/triangle indices for each new primitive.
The library's `Mesh::transformed_uvs` accepts explicit triangle indices and a
`uv_edit::Transform` (scale, rotation in degrees, then offset around the UV origin).
`FaceMap::transformed_uvs` additionally retains guarded source-face lineage.
Shared selected/unselected vertices split, with `Edited::source_vertices` mapping
each output vertex to the original for explicit weight remapping. Geometry, triangle
order and unselected UV/tangent tuples remain unchanged. Existing UVs and tangents
are required; zero scales, nonfinite inputs and numerically collapsed UVs fail.
Negative scales mirror selected charts and regenerate tangent handedness using the
existing derivative-based method, not MikkTSpace. `uv_edit::edit_scene` validates a
saved source selection, edits each selected primitive, preserves materials and texture
pixels, remaps skin weights exactly, and isolates the requested shared-mesh instance.

```sh
target/debug/gcad.exe asset-face-uv examples/game-assets/plate-face-materials.gasset plate target/game-asset-plate-end.faces.json target/plate-face-uv.glb --scale=-2,3 --rotation 30 --offset=0.2,-0.1
```

The output must be new. Unselected UVs and tangent tuples stay unchanged; geometry,
joint indices and weight values are preserved through the explicit vertex remap.
UVs are transformed in their current coordinate space, including packed atlas UVs:
this changes sampling locations without repacking or rebaking textures. For ordinary
check/preview/export, persist the transform in `.gasset`:

```text
face_uv plate plate-end.faces.json scale=-2,3 rotation=30 offset=0.2,-0.1
```

The prepared `examples/game-assets/plate-face-uv.gasset` demonstrates this transform
on the top face with the existing local colour-map fixture. Its paired software
snapshots show the selected chart change. The native `--face-uv` probe verifies
imported coordinates and planar tangent frames in Debug/Release, including skinned
and six-map variants; see `tools/game-assets-fbx/README.md` for its fixture contract.
This does not establish engine sampler or shading parity.

At least one of `scale`, `rotation`, or `offset` is required; omitted values use
identity defaults. The target is a scene node and the record path is relative to the
document. Material assignments always run before UV edits, regardless of text order.
Multiple UV declarations must select disjoint triangles; overlaps fail instead of
composing transforms implicitly. Missing/stale records and invalid transforms fail
loading. `asset-bind` validates these declarations but keeps the original prepared
mesh for weight generation; normal loading subsequently remaps those weights.
Selected face-corner weights can be replaced on an existing skin:

```sh
target/debug/gcad.exe asset-face-weights examples/game-assets/plate-face-materials.gasset plate target/game-asset-plate-end.faces.json target/plate-face-weights.glb --influence base=0.25 --influence tip=0.75 --boundary reject-shared
```

Repeat `--influence JOINT=VALUE` for one to four unique existing palette joints.
Positive finite values must sum to one; the command does not normalise them.
The required boundary policy is `reject-shared` or `split`. The first rejects
selected/unselected triangles sharing a vertex index. The second duplicates selected
boundary tuples and preserves all unselected weights. Either policy can leave a
deformation seam across already separate tuples, including hard-normal/UV seams;
neither welds coincident positions or guarantees a continuous surface. Materials,
rest geometry and shading attributes remain unchanged. The output must be a new
GLB. Source selection and existing skin guards must validate before editing.
Persist the same edit with:

```text
face_weights plate plate-end.faces.json joints=base,tip weights=0.25,0.75 boundary=reject-shared
```

Both lists must have matching lengths and the boundary setting is required.
Weight declarations run after material and UV edits regardless of text order;
overlapping weight regions fail. Normal loading checks joint palette membership
and the resulting prepared boundary. `asset-bind` returns the original prepared
mesh, checking declaration syntax, joint-node existence, selectors and overlaps;
it defers palette and prepared-boundary checks until the new skin resource is
loaded normally. Existing weight resources are not rewritten by declarations.
`plate-face-weights.gasset` is a diagnostic example. The native `--face-weights`
probe independently checks its imported weights and 17 synthetic bone poses;
a smoothed shared-instance fixture additionally exercises 68 actual boundary
splits. Engine playback and production deformation quality remain unverified.
Matching Windows VS Code configurations use the capture/check/resolve commands. Capture
refuses an existing `target/plate-end.faces.json`; preserve or explicitly move it
before capturing again. Debugger interaction has not been tested.

## Validation status and next actions

The experimental Rust `asset::faces` API retains triangle-to-source-face association
for raw tessellation. `SelectionFile::capture` records source provenance, selector
intent and exact geometry fingerprints; `to_json`/`from_json` preserve that versioned
record. `resolve` checks current source inputs, re-evaluates the selector and returns
a fresh mesh, guarded face map and selected triangle indices. Reordered faces can
resolve, while changed or ambiguous bindings require explicit recapture. Runtime
kernel IDs are not persisted. Material and UV commands/declarations use these guarded
selections with explicit preparation and primitive lineage. Face-bound weight editing
is available through the guarded command and declaration described above.
Geometry fingerprints are not persistent topological
names across arbitrary boolean edits.

Windows library tests and independent native GLB import probes cover core materials,
geometry, skinning, clips and several derived exports. WASM library compilation
passes, but a complete threaded web build/runtime is unverified. Linux runtime
validation requires a Rust toolchain. CPU previews have known differences from GPU/engine rendering.
The unpatched FBX prototype loses PBR maps/properties. The isolated v10 experiment
preserves the tested custom material contracts and sampled motion, with an explicit
consumer adapter for material properties and shading bases. It is not integrated
into the production CLI and does not establish generic engine import parity; see
the [backend proposal](game-assets-fbx-backend-proposal.md).

The recorded full Windows release regression runs have 440 passing tests with default
features and 555 with `game-assets` in the isolated patch-0030 checkout (before the
subsequent face-UV additions). Both retain five known baseline failures and
13 ignored tests; neither run added a failing test name. See the design record for
commands, logs and failure categories. A passing asset check does not resolve those
repository-wide baseline failures.

### Opt-in 16-bit skin weights

`asset-quantize-weights INPUT.gasset NEW.glb --max-error MM --max-angle DEGREES`
requires explicit position and shading-angle limits. `--samples 17` controls a
uniform grid per clip; authored keys and STEP left limits are also checked, as
is rest pose. The operation writes actual core-glTF normalized unsigned-short
weights with exact integer sums of 65535, and preserves the source weight file.
It refuses an existing destination and fails before creating a file if a sampled
limit is exceeded. A JSON report includes actual sample times, maxima, limits,
weight payload sizes, total GLB bytes and processing times. Weight payload drops
from 16 to 8 bytes per vertex; total file savings depend on other content and JSON.
Tiny positive influences can round to zero; original zero slots stay zero.

This checks the effect of weight quantisation only. It does not include other
float32 export errors, prove bounds between samples, or certify engine playback.
Skinning that becomes singular during sampled shading validation fails export.
The matching Rust API is `asset::quantize::encode` with explicit `Options`.

The unpatched pinned Assimp importer does **not** correctly decode these UNORM16
weights: its float-weight path copies integer bytes without normalization. The
reviewed experimental v12 helper includes the correction first tested in v10.
Its native readback test now independently decodes both float and normalized
unsigned-short source weights and checks the resulting FBX weights, including
small positive influences. Both fixtures pass. This does not establish support
in an arbitrary unpatched Assimp build or engine importer.

The refined fixture passes Unity 6000.4.7f1/6000.5.4f1 editor and standalone
D3D11 checks through the explicit adapter, against independently decoded
quantized GLB geometry. Evidence: `target/unity-unorm16-6000.4.7f1` and its
6000.5.4f1 counterpart. This validates the tested conversion path, not arbitrary
direct GLB import. Fixture limits 0.01 mm/0.1° produce sampled weight-only errors
0.000368723 mm and 0.000874278°, reducing GLB size by 98,716 bytes. Production
application budgets remain open.

Next actions: complete remaining optimisation and engine/platform gates;
define representative production quality/performance targets.
Unity, Unreal, Godot and other OpenGL-based engines remain the requested scope;
the native helper and constrained shader template have been selected. Nothing
has been submitted upstream.
