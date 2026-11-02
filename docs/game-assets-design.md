# Game asset feasibility and scene proposal

- Status: M0 in progress; schema and backend are proposals
- Reviewed: 2026-11-01
- Next review: 2026-11-14, or before adopting a backend
- Baseline: `33653aa19321ac6760b7f3aefa1f779030415e0a`

## Current implementation

This is a cumulative design and evidence record. Dated experiments below describe
their state at that point; later results can supersede their limitations. The
[user guide](game-assets.md) describes active commands. The renderer was adopted on 2026-11-01; it is now vendored and connected to the active
GPU preview for explicit mip chains, alpha and five material-map slots.
The CLI exports GLB only. The v10 FBX experiment requires its custom consumer
adapter and is not production integration. Shader authoring and selected-engine
acceptance remain incomplete. Current review artifacts and reproducible checks
are recorded in [review preparation](game-assets-review.md).

The optional `game-assets` Cargo feature exposes an experimental derived-asset
pipeline with no added Cargo dependencies. Opt-in `.gasset` check/export routes use
it; existing CAD format dispatch is unchanged. The mesh layer tessellates borrowed
CAD solids at an explicit tolerance, retaining hard edges through complete
position/normal tuples. It validates finite attributes, matching counts, normal
and tangent bases, valid indices and nondegenerate triangles. Portable UVs are
generated explicitly rather than taken from CAD surface parameters.
`BindingStamp` hashes ordered positions, normals and triangle indices with SHA-256.
Changing those values invalidates bindings; changing UVs or tangents does not.
Signed zero is canonicalised. This conservative guard is not a persistent face ID.
Separate skin validation checks influences, inverse binds and rig guards; version-2
authoring weights also require recorded source-input provenance.

Run the focused tests with:

```sh
cargo test --release --locked --features game-assets --lib asset::
```

The implemented experimental paths include scene/assembly GLB export, core PBR
maps and procedural baking, UV projection/atlas preparation, normal projection,
explicit skin binding, rigid/constrained/skinned clips, root-motion extraction,
sampled LOD/key reduction, diagnostics and native preview. Plane-aligned subdivision
and piecewise weight authoring provide additional deformation experiments. Export
capability reports use a real in-memory GLB encode. These paths have focused tests
and independent native fixtures described below; they do not establish production
support or complete milestone acceptance. Actual engine import, production FBX,
shader authoring, broader topology/quality acceptance and cross-platform verification
remain unfinished. The FBX writer/reader experiment stays isolated in native tools.

### Live authoring viewer requirement

The native implementation hosts assets as a document mode in the existing gcad
`App`, sharing its window, file picker and open/drop handling. Game-asset controls
are shown only for `.gasset` files; CAD documents continue through the original
CAD panels, evaluation and watching paths. There is no separate asset `eframe::App`
or window runner. The compatibility `asset-preview` command opens the shared app.

The game-asset workflow must provide the same ongoing visual feedback as ordinary
CAD authoring: leave the viewer open while the agent edits the source and observe
dependency changes automatically. The native preview now polls recorded inputs in
a background worker and reloads after a quiet poll. Cached CAD dependencies merge
into the outer recording. Failed loads retain the last valid scene and keep
watching missing/new inputs; successful loads retire unused dependencies. Results
whose recorded inputs have changed are rejected. File checks use modification time
and size, following the host cache convention; preserved-metadata edits are not
detected. Interactive acceptance and complete inspection controls remain pending.

Reload must cover the asset document and consumed dependencies, coalesce saves,
reject obsolete results, retain the last valid scene with a visible stale/error
status, and recover after corrections. Preserve the camera and compatible clip
selection/time across successful reloads. Geometry, material/UV/texture, rig/weight,
animation/root-motion, LOD and future shader work need corresponding inspection
controls. The acceptance scenarios and implementation priority are recorded in
the [live-viewing requirement](game-assets-plan.md#live-game-asset-viewing-required-2026-11-01).
Reviewed: 2026-11-01; next review: 2026-11-14.

### CLI entry points

The current opt-in CLI entry points are listed below. Run `gcad COMMAND --help`
from a build with `--features game-assets` for required arguments and limits.

| Command | Purpose |
| --- | --- |
| `check FILE.gasset` | Evaluate sources and validate the authored scene and bindings |
| `export FILE.gasset OUTPUT.glb` | Encode the scene, materials, clips and skins as GLB |
| `asset-capabilities FILE.gasset` | Dry-run GLB encoding; report actual destination features and limitations |
| `asset-bind` | Explicitly create guarded rigid, two-bone or piecewise-chain weights |
| `asset-report` | Count resources and measure sampled deformation; `--shading` validates shading bases |
| `asset-preview` | Native preview or CPU diagnostic PNG snapshot |
| `asset-root-motion` | Create an in-place clip and translation or rigid root-motion metadata |
| `asset-reduce` | Reduce clip keys against declared sampled error limits |
| `asset-lod` | Create a sampled-pose-checked mesh variant, optionally baking normals |
| `asset-bake-normals` | Project source geometry normals into a target rest-pose normal map |

Fixtures with `skin` declarations require their weight-generation command before
ordinary loading. Bindings are explicit derived artifacts, not checked-in assumptions
that survive CAD changes. See each fixture's comments and verification section below.

### Static GLB prototype

`Mesh::project_box(tile_mm)` creates repeating UV charts based on dominant
triangle normals and splits shared vertices at chart boundaries. Tangents come
from UV derivatives, averaged within each chart and orthogonalised against the
normals. Mirrored UV islands require split vertices; conflicting handedness and
degenerate UVs are rejected. This is not MikkTSpace and does not establish
compatibility with arbitrary externally baked tangent-space normal maps.

`asset::glb::encode` writes one indexed mesh and core metallic/roughness material
to a self-contained GLB. It converts `(x,y,z)` millimetres to `(x,z,-y)/1000`
metres, including normal and tangent basis rotation. Float32-rounded geometry is
validated to catch overflow or collapsed triangles. Base-colour RGBA pixels are
sRGB (alpha linear); roughness and metallic inputs use linear red-channel values,
packed into green and blue without gamma conversion. Missing scalar maps use
255 so the corresponding scalar factor remains authoritative. Mismatched image
dimensions require explicit resampling. Images are embedded PNGs, use a top-left
origin. Sampler defaults are repeat wrapping, linear magnification and trilinear
minification; clamp/mirror wrapping, nearest/linear filters and optional mipmap
filtering are explicit settings.

The shared `asset::material::Material` also supports normal, occlusion and emissive
maps, normal scale, occlusion strength, emissive factors, double-sided rendering
and explicit Opaque/Mask/Blend modes. Opaque deliberately ignores alpha in rendering,
as specified by glTF; Mask carries its cutoff. Normal maps use +Y by default;
explicit -Y input is converted by reversing green on a copy. Emissive maps are
sRGB; normal and occlusion values are linear. Core emissive factors are limited
to zero through one; stronger emission requires a future extension.

`Material::from_finish` reuses existing finish colour, roughness, metallic,
emission and image maps. sRGB finish colours become linear factors and opacity
below one chooses Blend. Missing roughness/metallic use asset defaults 1/0; viewer
default equivalence is not claimed. A combined error reports unsupported specular,
clearcoat, transmission, IOR, procedural textures and unapplied UV scale/rotation.
No physical material values are modified or silently repurposed.

`asset::scene` represents named nodes with local translation, xyzw quaternion
rotation and scale, shared meshes, and material-indexed primitives. It validates
references, unique node names, cycles, finite values and nonsingular composed
float64 transforms. `asset::glb::encode_scene` preserves hierarchy and mesh
sharing, converting local transforms once; it also rejects local values outside
float32 range and scales that round to zero. Near-unit quaternions are normalised.
Reflections remain in node transforms, requiring determinant-aware winding in
the renderer. General float32 world-transform precision remains an engine concern.
Full CAD assembly/provenance integration, general texture baking and
full CAD-finish portability remain unfinished.

`asset::bake::finish` now bakes the existing procedural patterns on the CPU into
256-pixel repeating colour/normal maps, reusing the viewer's unchanged pattern
generator. Explicit image maps override generated channels. It applies texture
tile size and rotation to asset box UVs and regenerates tangents, returning a
derived mesh/material while preserving source values. Generated normals use
periodic central height differences with strength 1.5 per texel and +Y following
increasing V. Colour retains the linear finish factor; the viewer's 1.3 tint gain
is deliberately absent. The viewer's long-axis projection differs, so identical
viewer appearance is not claimed. Resolution selection, atlas integration, high-to-low
baking and engine visual validation remain. Unsupported finish fields still fail.
The conventions follow the [Khronos glTF specification source](https://github.com/KhronosGroup/glTF/blob/main/specification/2.0/Specification.adoc).

### High-to-low normal projection foundation (reviewed 2026-10-31)

`asset::projection::normals` borrows low/high meshes in the same millimetre
coordinate frame. Low geometry needs non-overlapping UVs in [0,1], normals and
tangents; high geometry supplies triangles and interpolated vertex normals.
Each covered texel centre interpolates a low position and orthonormal tangent
basis. A deterministic triangle BVH searches both directions along its normal,
within an explicit distance. The nearest hit wins; coincident hits within a fixed
roundoff band prefer the normal aligned with the target. This is a second search
around the true nearest distance, not an accumulating tolerance. The band is
64 machine epsilons times the maximum absolute geometry/sample coordinate (at
least 1 mm). A nearest normal below the
configured alignment threshold is rejected rather than projecting through it.
Successful normals are encoded in numeric RGBA8 tangent space with positive Y.
The map does not include any existing high-surface normal texture.

Options require dimensions, maximum distance, minimum normal alignment and an
allowed missed-pixel fraction. The result contains a separate coverage mask
(outside chart, missed, hit) and counts plus maximum hit distance. Misses and
outside pixels retain neutral normal texels. No hits, no covered pixels or an
exceeded miss budget fail explicitly. Shared UV edges use stable first-triangle
ownership. Interior UV overlap is rejected even between disconnected charts.
Dimensions are capped at 4096, UV validation at two million candidate pairs and
raster work at one hundred million candidate samples.

`Bake::padded` performs deterministic four-neighbour dilation up to 32 pixels into
empty UV space. Covered projection misses are never filled. The coverage mask marks
padded texels separately (3), and the report counts them. Distance is Manhattan
pixel distance; equally near sources resolve by stable scan/queue order. This does
not generate chart-safe mipmaps or guarantee separation where chart gutters are
smaller than the requested radius.

`apply_to_scene` transforms the source primitive into the target's rest-local
frame with inverse-transpose normals, bakes and pads, then isolates the target's
shared mesh/material before applying its normal map. Geometry, node transforms,
other instances and skin bindings remain unchanged. It sets positive-Y normals,
normal scale 1, clamp sampling and no mipmaps. Existing colour/scalar/emissive maps
must already use clamp addressing, otherwise the operation fails with an atlas
instruction. Source normal textures are not sampled. Animated poses are not baked.

The native command creates a new GLB only and prints a JSON coverage report:

```sh
target/release/gcad.exe asset-bake-normals examples/game-assets/plate-atlas.gasset plate examples/game-assets/plate.gasset plate target/game-asset-plate-normal-bake-aligned.glb --width 256 --height 256 --max-distance 0.1 --padding 2
```

Both inputs use `.gasset`; `--primitive` and `--source-primitive` default to zero.
Distance is in target mesh-local millimetres, not world millimetres after node
scaling. `--min-normal-dot` and `--max-missed-fraction` default to zero: all covered
pixels must hit an aligned surface. Failure occurs before opening the output;
existing outputs are never overwritten. Global variable overrides apply to both
source documents.

Supersampling, cages, material-map transfer, LOD workflow integration and independent
import/render validation remain. The library neither changes CAD solids nor mutates either
input mesh. The BVH, analytic slope, mirrored tangent basis, negative-direction
projection, nearest-backface rejection, partial hits and uncovered chart space
have focused tests, supplemented by padding and shared-instance/skin export tests.
An identity-projection regression uses the curved CAD plate: edge barycentrics are
clamped back inside their triangle and BVH traversal boxes receive a floating-point
roundoff margin. Triangle intersections and the zero-miss budget remain unchanged.
The focused asset/preview suite passes 68/68 after the edge-alignment correction.
The Windows release CLI identity fixture hits all 1349 covered pixels with zero
misses, maximum distance `4.33e-15 mm`, and fills 2031 padding pixels. Its GLB is
79448 bytes, SHA-256
`C5E207CCF19C7DD421731C657A3CA83DD240DFED4164E1A6EBA198DC64BA8294`.
The failed pre-correction bake created no output. Repeating the successful command
against its existing output fails and preserves that hash. This is pipeline and
identity-projection evidence, complemented by the analytic detail-transfer check below.

The example's `--normal-bake` fixture projects the plane
`z = 1 + 0.2*x + 0.3*y` onto two flat 100-by-100 mm patches. One patch has mirrored
U coordinates. This deliberately simple surface provides an independent analytic
normal; it is not a production high-poly benchmark. All 1152 covered texels hit;
two-pixel Manhattan dilation adds 392 pixels. The exported low geometry stays flat.
Assimp plus the pinned stb PNG decoder reconstructs world-space normals from the
imported tangent/bitangent/normal basis and decoded pixels. Debug and Release agree:
all 1544 covered/padded normals lie within `0.226419 degrees` of the analytic slope;
504 outside pixels remain neutral. Inverting the green channel produces at least
`32.6438 degrees` error, a normal-convention negative control. Import checks also
verify UV mirroring, geometry, handedness, clamp addressing and no mipmaps. The
existing independent textured-atlas check still passes after adding this probe.

```sh
cargo build --release --locked --features game-assets --example game_asset_glb
target/release/examples/game_asset_glb.exe target/game-asset-normal-detail.glb --normal-bake
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-normal-detail.glb --normal-detail
```

The fixture GLB is 2552 bytes, SHA-256
`9625F8E80165979EA5D6579308F7C972443D82A004A8DA1CA33DC50D0CA00EA4`.
The VS Code `Normal detail bake fixture (Windows)` configuration generates the
same artifact using a Debug build; its JSON is checked, interactive debugging
is unverified. Curved/high-poly detail, LOD export integration, mipmaps and engine
shading still need validation; this test does not establish those capabilities.

Normal transfer is also available directly in `asset-lod` with
`--bake-normals SIZE --bake-distance MM [--bake-padding PIXELS]`.
The size/distance arguments require each other; padding defaults to two pixels.
After geometric acceptance, the retained original scene supplies high geometry
and normals, while the derived scene supplies the low primitive. Baking uses the
rest pose, requires zero missed covered pixels, and accepts only non-opposed source
normals. Projection distance is target mesh-local millimetres. Input UVs must be
packed; use an explicit `atlas` operation before LOD generation. A failed bake
prevents output creation. Existing normal maps are replaced; source normal textures
are not combined into the bake. Reports include `normal_bake` only when requested.
Geometric/pose acceptance and the reported shading-angle limit still measure mesh
attributes, not rendered shading with the baked texture. They do not certify the
normal map throughout an animation.

The skinned-grid regression bakes its reduced mesh from the original geometry,
checks 1024 hits with zero misses, validates its remapped skin, preserves the other
instance's original material, and encodes the combined skinned GLB. The refined
CAD fixture is `examples/game-assets/plate-lod-bake.gasset`:

```sh
target/release/gcad.exe asset-lod examples/game-assets/plate-lod-bake.gasset plate target/game-asset-plate-lod-normal-aligned.glb --triangles 5000 --max-error 1 --max-uv-error 0.02 --max-angle 30 --max-trials 10000 --bake-normals 256 --bake-distance 1
```

The Windows release command reduces this fixture from 5104 to 5000 triangles and
3666 to 3614 vertices in 52 trials. Maximum correspondence error is `0.315093 mm`
and sampled mesh shading-angle deviation is `24.20214 degrees`. Baking hits all
1349 covered texels with zero misses and 2031 padding pixels; maximum projection
distance is `4.04e-15 mm` for these coplanar collapses. This verifies workflow
integration, not visual acceptance of the 24-degree mesh shading change. The GLB
has SHA-256
`15FD019BF420F8C06288B09B56A3E34294029B47B9A23DF9DAC9EA5F86995F2F`.
An initial stricter run made no collapses and correctly reported an unmet target;
its successful bake was not treated as reduction evidence. Omitting required
`--bake-distance` fails CLI parsing without creating an output. The focused
asset/preview tests pass 68/68 with the combined skinned and edge-normal regressions.

Independent import exposed a projection defect despite zero missed rays: shared
hard-edge hits could select another coincident face's shading normal by triangle
order, producing up to 45.319 degrees of error. The fixed nearest-hit alignment
rule above corrects this. Earlier pre-alignment identity/LOD normal outputs are
superseded; the independent probe rejects the retained old LOD as a negative control.

The stronger target of 3000 triangles reaches 2614 vertices in 1241 evaluated trials,
with 47745 cached rejections. Maximum correspondence error is 1 mm, sampled mesh
shading angle 26.77283 degrees and projection distance 0.036239 mm. All 1349 covered
texels hit; padding adds 2031 pixels. Assimp/stb checks in Debug and Release use
independent UV rasterization and double-precision plane/barycentric ray projection
against the original mesh. A fixed 0.02-micrometre coincidence band accommodates
float32 imported geometry. Maximum projected normal error drops from 1.57433 to
0.318367 degrees after applying the baked texture. One sampled texel changes by
over one degree before correction, so this remains limited coverage, not engine
or continuous shading acceptance. UV-matched surface positions differ by up to
0.036883 mm; geometric projection, rather than UV matching alone, is required.

```sh
target/release/gcad.exe export examples/game-assets/plate-lod-bake.gasset target/game-asset-plate-lod-normal-source.glb
target/release/gcad.exe asset-lod examples/game-assets/plate-lod-bake.gasset plate target/game-asset-plate-lod-normal-3000.glb --triangles 3000 --max-error 1 --max-uv-error 0.02 --max-angle 30 --max-trials 10000 --bake-normals 256 --bake-distance 1
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-plate-lod-normal-source.glb target/game-asset-plate-lod-normal-3000.glb --lod-normals
```

The 3000-triangle artifact is 166104 bytes, SHA-256
`816986FC71A23F19587A9D0457F83049086C8B6086C191B8315FA87EA1F44E02`.

### Experimental atlas library (reviewed 2026-10-31)

`asset::atlas::pack` groups existing UV charts by shared vertices, rejects
overlapping UV triangles within a chart, and packs chart rectangles with explicit
pixel density and padding. Deterministic shelf packing does not rotate charts or
silently lower density. It preserves positions, normals, triangle indices and
vertex order, remaps UVs and regenerates derivative tangents. It does not unwrap
folded charts, weld geometry or merge materials. Point-connected charts remain
one chart. Validation stops after two million candidate overlap comparisons.

`Atlas::rebake` resamples all six supported material maps through the inverse UV
transform, filling chart rectangles and padding from the original wrapped image.
Repeat, mirror and clamp addressing are supported. The caller explicitly chooses
nearest or linear sampling. Linear sampling averages the source texel area covered
by each destination pixel on minified axes and uses bilinear weights on other
axes. This separable filter follows the atlas's axis-aligned UV transform, including
wrapped padding and fractional edge texels. Colour and emissive RGB filter in linear light
before sRGB encoding; scalar/normal channels and alpha interpolate numerically.
Normal convention, material factors and alpha mode are retained. Output samplers
clamp and disable mipmaps to avoid cross-chart mip bleeding. This single-level
resampling does not provide chart-safe mip generation, high-to-low projection,
alpha-coverage preservation or engine appearance acceptance. Normal channels stay
numeric; filtering does not bake unresolved normal variance into roughness.
Dimensions are limited to 16..8192 and total output maps to 256 MiB per bake.
Linear footprints are bounded to 65,536 estimated taps per axis and the complete
bake to 256 Mi estimated source-pixel taps across maps/charts. These conservative
work estimates are checked before destination allocation; excessive work is refused
rather than silently reducing filter quality. Nearest sampling remains point-based.

`Atlas::rebake_mip(material, level)` provides a separate experimental CPU operation
for an explicit lower level. It keeps the original UV transforms, uses
`max(1, dimension >> level)` dimensions, and resamples the original material with
the same linear-light area filter. A bounded ownership map reserves every texel
in each chart's bilinear sampling rectangle, including clamped image edges.
Overlapping footprints refuse the requested level before output-map allocation.
The test includes a bypass control where a coarse chart overwrites a neighbour
and makes a red-chart sample blue. Conservative rectangles can refuse levels that
a more exact chart-shape analysis could permit. Anisotropic footprints are not
covered. The existing map-byte and filtering-work budgets apply to each level.

This API returns one standalone material with mip sampling disabled. It does not
attach levels to `Material`, export them, enable viewer sampling or provide a
complete chain. Multiple distinct charts cannot retain independent colours at a
shared 1-by-1 level; runtime maximum-LOD handling or a different texture layout is
still needed. CLI atlas export remains single-level. Alpha-coverage preservation,
normal-variance treatment and engine acceptance remain unfinished.

`Atlas::rebake_chain(material, max_level)` stores exactly the requested contiguous
levels, including level zero, in a `MipChain`. Read-only `levels()` and `max_lod()`
accessors expose the payload and required sampling bound. Every lower-level layout
and the aggregate 256 MiB RGBA-map budget are checked before baking output maps.
An unsafe level fails the complete request; it is never silently omitted. Each
level is baked from the original material, with no iterative quantization from
the previous level. Per-level tap budgets still apply. The aggregate budget covers
output map pixels, not source inputs, layout metadata or temporary copies.

Inspection of the locked `egui_bench 0.9.2` source found a renderer integration
barrier: `viewer3d::Image` stores only width, height and RGBA bytes; private
`gpu::mip_levels` creates byte-averaged levels automatically; a shared sampler uses
repeat addressing, linear mip filtering and anisotropy up to eight. The current
gcad adapter cannot upload the explicit chain or set a per-material LOD bound
through that interface. A renderer API change is needed before preview can use
these levels. No dependency fork, Cargo patch or viewer behavior changed here.

The standalone `game_asset_mip_gpu` example verifies the GPU-side contract using
existing wgpu APIs. It uploads exactly three NPOT levels for colour and scalar
textures and uses clamp addressing, linear spatial/mip filtering, anisotropy one
and maximum LOD two. A separate texture with green extra levels tests the bound:
the bounded sampler preserves the expected values, while an unbounded sampler
exposes the sentinel. Explicit `textureSampleLevel` readbacks pass on Windows
Vulkan and Direct3D 12 with 2,000 RGBA results per backend. This establishes
neither general asset appearance nor integration with the viewer. A subsequent
rasterized fixture packs two red/blue charts and checks ordinary `textureSample`
at 64, 8, 2 and 1 pixels square. Both backends pass 16,660 pixels per run with
exact bounded chart colours; ten unbounded pixels expose green extra levels.
The next chart level is refused for footprint overlap. This covers implicit LOD
for flat diagnostic charts, with anisotropy disabled; oblique surfaces, general
alpha coverage and normal-variance behavior remain unverified.

The optional authoring command is
now extended with `mips=MAX_LEVEL` (1..13, requiring `filter=linear`). When present,
the author retains exactly levels zero through the requested maximum as an
immutable `Arc<MipChain>` on the derived material. Material clones share the chain;
material equality includes its contents, so geometry/material deduplication does
not discard different mip data. Each chain retains its baked geometry and base
material. Validation rejects changed base pixels, factors, conventions, sampler
or provenance, and scene validation rejects changed primitive geometry, including
UVs. This is a conservative exact-geometry guard, not a promise that arbitrary
face partitioning, UV edits or LOD operations can reuse a chain. Rebuild after
those operations. Chain storage includes a geometry copy and a retained base
material in addition to the caller's base material; these copies are outside the
RGBA bake budget. Existing texture reports still exclude explicit mip payloads.

The active GLB encoder and preview explicitly reject attached chains rather than
dropping them or generating replacement levels. `check` and library preparation
can consume `atlas-mips.gasset`; the proposed renderer is not yet the active
dependency. The existing command without `mips` is
`atlas NAME width=256 height=256 padding=2 density=16 filter=linear`.
All five settings are required; filtering can also be `nearest`. Settings may
precede the source declaration. Packing follows refinement and UV projection and
precedes skin loading. For assemblies, each body's primitive gets its own atlas;
this does not combine different body materials into one image. Atlas settings do
not change the geometry binding stamp or CAD input guard, so existing guarded
weights remain reusable. Geometry-changing settings still require rebinding.

Library tests cover packing separation, invalid overlaps, legal shared edges,
inverse UV mapping, source preservation, wrap modes, gamma-correct interpolation,
all six maps and padding. GLB checks decode the embedded colour PNG byte-for-byte
and check clamp/no-mipmap sampling. A shared-edge regression from the curved CAD
plate uses edge-relative projections to avoid cancellation in overlap testing.
The plate authoring test reloads existing guarded skin weights after atlas packing.
`examples/game-assets/plate-atlas.gasset` supplies an untextured CAD UV fixture.
The textured CAD-box fixture also passes independent import as described below.

The Windows release CLI successfully checked and exported the atlas fixture
(76332 bytes, SHA-256
`7D89BF26F23F06E7D2F4756F632757D6580D6D40D4853894F63CB4631FAD0ECF`):

```sh
cargo build --release --locked --features game-assets --bin gcad
target/release/gcad.exe check examples/game-assets/plate-atlas.gasset
target/release/gcad.exe export examples/game-assets/plate-atlas.gasset target/game-asset-plate-atlas.glb
```

The repository-local VS Code configuration `Game asset atlas export (Windows)`
uses the corresponding Debug build. Its JSON was validated; interactive debugging
was not exercised. The focused asset/preview suite remains 59/59 passing after
authoring integration and the CAD shared-edge regression fix.

The existing CAD-box example's `--atlas` variant packs the `--rich` material into
a 256-by-128 atlas at 16 pixels per source UV unit with two-pixel padding and
nearest filtering. The independent Assimp importer and its pinned stb PNG decoder
check six charts, unchanged geometry/indices/normal/tangent data, five packed
textures, clamp addressing and nearest/no-mipmap sampling. It reconstructs source
UVs from imported chart bounds and compares every unambiguous chart/padding pixel
to independently sampled source images. Debug and Release checks agree on
35760 exact RGBA comparisons, including 8320 padding comparisons. 12030 samples
on source nearest-filter boundaries are excluded because imported float32 UV
rounding can choose either texel. A known source RGBA pixel calibrates PNG decode.
The existing rich-material import check also still passes.

```sh
cargo build --release --locked --features game-assets --example game_asset_glb
target/release/examples/game_asset_glb.exe target/game-asset-atlas-source.glb --rich
target/release/examples/game_asset_glb.exe target/game-asset-atlas-textured.glb --atlas
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-atlas-source.glb target/game-asset-atlas-textured.glb --atlas
```

The textured atlas GLB is 8884 bytes, SHA-256
`ED11CC4018D9E1DEAA3F703905DE50BFC55EBC48E2FE9069E654C0FC84E6E5EF`.
This verifies nearest-filtered box charts and texture packaging, not general
bilinear resampling quality, engine rendering, minification or mipmaps.

Reproduce the CAD fixture from the repository root:

The probe needs the pinned dependency checkouts documented in
[its README](../tools/game-assets-fbx/README.md). No dependency installation or
download happens automatically. The sample writes its specified output file;
its parent directory must exist.

On 2026-10-31 the Release fixture produced 24 vertices, 12 triangles and a 3188-byte
GLB. SHA-256: `C2699433C5C1B3728025E17A5DF134E459440A38A73526298E64A9031A04D64A`.
Independent Assimp import passed in Release and Debug: correct one-metre X extent,
Y-up bounds, outward winding, normals/tangents, PBR factors and two connected
embedded PNGs. Rust tests additionally decode the embedded image pixels and
check exact channel packing. A relocated GLB also passed independent import.
The rich variant is 3920 bytes, SHA-256
`849263224EDDBFEA2F562000450400811B3881D93CB1A03026F43DC4BCA943B4`.
It passes independent checks of five connected PNGs, normal scale, occlusion
strength, emission, alpha Mask/cutoff/base alpha, double-sidedness and nearest
sampling with clamp/mirror wrapping. Rust tests cover all three alpha modes,
normal green-channel conversion, invalid factors/maps and finish conversion.
The scene variant is 6104 bytes, SHA-256
`C7E10363A940D3B973AFB8B710E73A27951CFA4B55DA003596349B54E1D8D22F`.
It has a rotated parent and two instances sharing one two-primitive mesh with
two materials; one instance uses mirrored, nonuniform scale. The primitives
store 48 vertices total (compaction is deferred), with 12 triangles per instance.
Independent import checks hierarchy, sharing, material slots, transformed normals
and tangents, determinant-aware winding and analytic world bounds within `1e-5 m`.
Expected bounds in metres are `[-0.75,0,-4]..[0.75,1,-2]` for the mirrored instance
and `[-0.25,0,-1.5]..[0.25,0.25,-0.5]` for the other instance.
The baked wood variant is 266368 bytes, SHA-256
`6E897990D5C6EB6A7235FE45C2E00612B2A53EAC9125BDDFB4D98669573C7328`.
Independent Windows Release and Debug imports pass: two connected 256x256 PNG
colour/normal tiles, linear finish tint and analytic UV values for all vertices
at 250 mm tile size and 90-degree rotation, plus the base geometry/tangent checks.
The ordinary 2x2 fixture fails the baked check as expected. The importer checks
PNG headers rather than decoding pixels; rendered appearance remains unverified.
The repository-local Debug build and executable commands passed for the Rust
fixture and C++ importer; interactive debugger attachment is unverified.
Engine appearance and Khronos validator checks are
still unverified. M1 is not accepted on this evidence alone.

## Experimental static authoring

Use a separate line-oriented `.gasset` file with an obligatory `asset 1` header.
Resolve source and texture paths relative to that file, using the existing host
read abstraction so browser sources remain possible. Unknown versions, commands,
attributes and references must fail rather than silently fall back.

With `--features game-assets`, `gcad check` and `gcad export` accept this syntax:

```text
asset 1
source prop parts/prop.gcad
tessellate prop tolerance=0.1
uv prop projection=box tile=100
```

Each named source requires explicit tessellation and UV settings. Multiple `.gcad`
sources become separate root nodes by default; `body=NAME` selects a body (default `main`).
`.gasm` sources instead default to `body=*`, importing all evaluated bodies. Each
source instance becomes a transform group with children named `INSTANCE/BODY`.
Nested CAD paths remain in the mesh-node name for existing body references.
The asset adapter emits retained assembly ancestors as meshless groups named
`INSTANCE/PART/` (and deeper paths with a trailing slash). Terminal single-body
parts use their existing mesh node. `body=NAME` selects one evaluated body while
retaining its assembly ancestors and placement; unrelated branches are omitted.

The evaluator now retains `AssemblyInstance` metadata: authored instance names,
exact evaluated body membership, nested children and frames in model coordinates.
This records ancestry before single-body renaming can erase it. Importing a body
subset prunes empty branches while preserving its ancestors; repeated imports
clone their own trees. A move of every member moves the corresponding frame,
while partial moves update only fully moved descendants. Existing CAD geometry,
body names and placement maps remain the source of geometry and body identity.
Pattern copies represent the copied body as a separate instance. Stored part
caches persist this tree under a new cache-key version; old cache entries are not
interpreted as the new format. Asset groups use parent-relative transforms, while
mesh geometry stays in the same local coordinates. Mechanical motion baking
finds descendant bodies and converts CAD poses to their static parent frames;
its error check composes those frames back into world coordinates.
Explicit authored keys remain local to the target's parent, so keys previously
written against a flattened nested assembly need review after this hierarchy
change. Existing rig stamps detect the changed node hierarchy and require explicit
rebinding. This does not establish independent engine or FBX acceptance.

Assembly solids are converted from world geometry back to local geometry using
the recorded placement inverse, then exported with that placement exactly once.
Affine TRS decomposition preserves reflection and nonuniform scale; singular or
sheared placements fail instead of being approximated. Each part uses its evaluated
colour/opacity/finish. An explicit CAD colour override takes precedence over the
retained material-library colour, matching the CAD viewer. Mesh sharing requires
exact geometry/attribute and material equality; source labels alone are insufficient.
Repeated asset instances of the same assembly share the imported meshes.

Clips can animate the authored assembly group or generated parts using
`key CLIP INSTANCE/BODY ...`. Imported names resolve after CAD expansion; a missing
body fails with its target name rather than binding to a different node index.
Key ordering, duplicate times and property validation still apply before loading
CAD. Translation/rotation/scale values replace the corresponding local channel;
they are not offsets from the part's rest pose. Unanimated channels retain rest
values, and other instances sharing the mesh are unaffected. Generated children
also accept `skin INSTANCE/BODY weights=PATH` and `asset-bind` targets;
assembly-group skin declarations are rejected because the groups have no mesh.
Source provenance is stored per node even when geometry is shared across sources.
This preserves each body's binding guards without sacrificing mesh sharing.
The `linkage-part-skin.gasset` fixture binds `first/crank` to a separate bone and
animates a 10 mm lift. Windows CLI bind/check/export and independent Debug/Release
Assimp `--linkage-skin` checks passed on 2026-10-31. The importer verifies complete
rigid weights, inverse bind placement, nine analytic poses, and a separate unskinned
second crank. Maximum error is `2.50766e-09 m` (limit `1e-6 m`). GLB SHA-256:
`94DC113BD972012D43EE064BEBC0704E56A3CDC27DB3E0FF77982881B8CB2642`.
Unit coverage also checks distinct source identities for shared geometry, stale
parameter rejection, explicit rebinding with corrupt old weights, missing targets,
and rejection of meshless assembly groups. This rigid skin fixture does not establish
deformation quality for bending mechanical parts or engine playback.
Mechanical joints are not automatically converted into skeletal joints. An explicit
`motion CLIP INSTANCE joint=NAME from=VALUE to=VALUE duration=SECONDS rate=HZ [max_error=MM max_rate=HZ]`
command bakes a linearly driven independent joint through the existing constraint
solver. Turn values are degrees; slide values are millimetres. The bake starts from
the evaluated source rig, solves consecutive poses, and writes local LINEAR TRS
tracks for every imported body of the selected instance. Other instances retain
their rest poses. Manual clips and motion clips must have distinct names.
The number of intervals is `ceil(duration * rate)` (1 through 10000), evenly spaced
over the duration with both endpoints included. Thus the effective rate can be
slightly higher than requested. Independent/coupled turn steps and sampled part
rotations must stay below 90 degrees; increase the rate if rejected. Joint travel
and the initial move to `from` are limited to 10000 joint units to bound solver work.
Missing/fixed/driven joints, invalid limits and failed constraints are errors.
Quaternion signs are made continuous. Constant channels currently remain in the
clip; key reduction is separate future work. No CAD geometry is changed or
retessellated for motion. Existing explode directives do not apply to export.
Optional `max_error=MM` requires a finite positive budget. The loader solves a second
trajectory at every key and at the quarter, half and three-quarter points of each
interval. It compares every render vertex with clip interpolation in world-space
millimetres, including the instance and its ancestors' scales. Exceeding the budget
fails with the clip, part, time and observed error, suggesting an increased rate.
Motion baking runs after all source meshes have expanded, so checks see the complete
scene. The check does not include skin deformation, export quantisation, or other
clips; each motion clip drives its selected rigid assembly instance independently.
Optional `max_rate=HZ` enables bounded adaptive uniform sampling and requires
`max_error`. It must be finite, at least the starting rate, and imply no more than
10000 intervals. Only sampled-error and rotation-spacing failures trigger a retry:
double the interval count, capped at `ceil(duration * max_rate)`, and restart from
the evaluated source rig. Other errors propagate immediately. The first passing
bake is retained; this does not find the smallest possible key set or refine only
individual intervals. Omitting `max_rate` preserves strict fixed-rate behavior.
This sampled budget does not guarantee constraints at all times or a continuous
error bound. Broader mechanisms and engine playback remain to be validated.
The adaptive linkage unit fixture starts at 2 intervals and accepts 32 within its
96-interval ceiling for a `0.001 mm` budget. Its sampled transforms equal an explicit
32-interval bake. Tests cover exhausted ceilings, missing budgets, and synthetic
sliding joints with reversed travel, inherited motion and a 2:1 coupling; fixed
and coupled-driven joints cannot be selected as independent drivers. The synthetic
slider checks do not establish CAD authoring or engine import for that mechanism.
Windows CLI check/export of `linkage-adaptive.gasset` and Debug/Release independent
`--linkage-adaptive` import passed on 2026-10-31. The analytic four-bar probe checks
33 keys and 96 intermediate samples: maximum key error `1.58119e-08 m`, maximum
between-key error `5.30855e-07 m`, both below `1e-6 m`. GLB SHA-256:
`702833A11FCC6AE6358B9F992B7FB0210DB3B1983937813BE01D992381EC106F`.
Windows CLI verification on 2026-10-31 accepts `max_error=0.01` and rejects
`max_error=0.0001`: the first reported coupler error is `0.000320438 mm` at
`0.010416667 s`. Accepted output is byte-identical to the previously checked GLB;
the independent analytic import probe still passes. Unit checks cover tight and
invalid budgets, world-space scaling and solver rejection at a joint limit.
The linkage fixture uses a `0.01 mm` budget and 24 intervals for a 30-degree, one-second
drive: key poses match CAD `pose`, with maximum sampled midpoint vertex error
`0.000941463 mm` against separately evaluated CAD poses.
Windows CLI check/export and independent Debug/Release Assimp `--linkage-motion`
checks passed on 2026-10-31. The independent probe calculates the four-bar's upper
circle-intersection branch and compares all vertices at 25 keys and 24 midpoints.
Maximum key error is `1.59144e-08 m` (limit `1e-6 m`); midpoint error is
`9.42545e-07 m` (fixture limit `1e-5 m`). Output GLB SHA-256:
`A01C2C2CF43616AD4C24A00BF26515AA7F026DBDD225140C41F7B675F0B53AA0`.
Refinement budgets cover all evaluated bodies of a source before repeating it via
asset nodes. Face-selector identity and original nested hierarchy reconstruction remain.

`examples/game-assets/linkage.gasset` imports the existing mechanical linkage
twice and animates one source group. Windows Release check/export passed with
4 shared meshes, 10 nodes and a 118552-byte GLB (SHA-256
`9E72930BAE06BC472A8C1F44149C32EE48F56662C26D8C968C77850812AB377A`). The
software snapshot was inspected: two separated linkages retain the red crank
and orange links. Unit checks compare recovered world bounds to evaluated CAD,
verify sharing and colour overrides, test reflected/half-turn TRS decomposition,
reject shear, and reject a source-wide refinement budget exceeded across parts.
The Release Assimp `--linkage` probe verifies eight placed child bodies sharing
four meshes, the red colour override, and the group's one-second 10 mm lift
channel. Maximum instance-offset error is `7.45058e-09 m` (limit `1e-6 m`).

`linkage-part-motion.gasset` targets `first/crank` after body expansion and leaves
the second instance at rest. Windows check/export and Debug/Release independent
Assimp `--linkage-part` probes passed on 2026-10-31. Nine sampled poses compare
every crank vertex against its rest placement plus the authored 10 mm lift;
maximum error is `7.45058e-09 m`, below `1e-6 m`. The GLB SHA-256 is
`0C2FF112B699BBD832309B001855AD779BAA510FD70A55DB7C48401C66748B8A`.
Unit coverage rejects nonexistent evaluated parts and duplicate key times, and
checks that untouched rotation channels and the other instance retain their rest
values. This is explicit part animation; mechanical constraints are not baked.

```sh
target/release/gcad.exe export examples/game-assets/linkage.gasset target/game-asset-linkage.glb
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-linkage.glb --linkage
```
Quoted paths, full-line `#` comments and optional UV `rotation=DEGREES` are accepted.
Settings may precede their source; unknown references, duplicate commands/attributes
and unsupported versions/features fail. Source paths resolve beside the asset file.
Existing CAD finish maps and patterns pass through the baker; unsupported finish
features fail. Missing body colour uses sRGB `[0.7,0.7,0.7]`, opacity defaults to 1.
Global CLI variable overrides apply to each source. Explicit UV settings override
CAD texture scale/rotation. Exports currently accept only GLB, with no `--section`.

Optional `refine NAME levels=N triangle_budget=N` uniformly splits every derived
triangle into four per level (1–8 levels). It runs before UV generation and skin
assignment, preserving winding, shared edges and existing hard-edge vertex splits.
Midpoint normals are normalised endpoint averages; opposing normals fail. The
operation returns a new mesh and leaves CAD and the original mesh untouched.
The exact final triangle count must fit the explicit budget before allocation.
It refines the existing piecewise-planar surface; it does not reproject to CAD,
smooth geometry, optimise topology or remap existing UV/skin data. Refined skins
require explicit fresh binding; refinement settings enter the source guard.

`gcad asset-report FILE.gasset [--samples N]` emits a version-1 JSON diagnostic
report on stdout and evaluation/report timings on stderr. It validates the full
asset, including stale weight guards, before reporting. Counts distinguish stored
vertices/triangles from instanced triangles and primitive instances. Primitive
instances are a draw-work estimate, not measured renderer draw calls. Source RGBA
texture bytes count each material map slot separately, excluding GPU compression,
mipmaps, deduplication and export packing. Animation key counts are not encoded
clip byte sizes.

Each skin reports its joint count, a 0–4 influence histogram, and per-clip minimum/
maximum triangle area and edge-length ratios relative to its world-space rest
geometry, plus maximum world-space displacement in mm. Samples include N regular
times (2–10001), all authored keys and the immediately preceding representable
time at each positive STEP key. Actual times are included in JSON. This is a
finite diagnostic sample, not a continuous bound, self-intersection check or
automatic quality acceptance. Rigid-only animations currently have counts but no
deformation rows. Source-reference surface errors require a declared reference
and remain specific to the independent plate probe. Nothing is simplified or
written back to the asset by reporting.

```sh
target/release/gcad.exe asset-report examples/game-assets/plate-flex.gasset --samples 17
```

Windows Release CLI verification passed for the centred plate and the shared-mesh
instance fixture; invalid sample counts are rejected. For the plate, Rust reports
area ratios `0.2895627085–2.1960111629`, maximum edge ratio `1.5726779800`, and
maximum displacement `21.3190056053 mm`. Area/edge measurements agree with the
independent Assimp probe within `0.001` absolute ratio. The influence histogram is
94 rigid and 12252 two-influence vertices. One local run took 0.174 s evaluation
and 0.036 s reporting; these timings are observations, not performance guarantees.

Explicit `node NAME [source=NAME] [parent=NAME] [translation=X,Y,Z]
[rotation=X,Y,Z,W] [scale=X,Y,Z]` commands replace the implicit root nodes.
A node without a source is a transform group. Source meshes are shared by all
instances. Parents may appear later; missing references, cycles, duplicate names,
singular scales and invalid quaternions fail before loading CAD. Every source
must have an instance when explicit nodes are used. Translation is in millimetres;
rotation is a unit xyzw quaternion; transforms compose parent * translation *
rotation * scale. Defaults are identity. `plate-instances.gasset` demonstrates a
rotated group with mirrored/nonuniform and ordinary instances of one CAD plate.

On Windows, Release check/export and independent Assimp import of this authored
scene passed: three named nodes sharing one mesh, correct reflection and analytic
world bounds within `1e-6 m`. The mirrored instance bounds are
`[-0.045,0,-0.19]..[0.045,0.012,-0.11]`; the ordinary instance bounds are
`[-0.015,0,-0.07]..[0.015,0.003,-0.03]`. Output is 76588 bytes, SHA-256
`C42F21F4E84B4BA42C627053F77F81E4CFF18FDB3ABF6461D1F8F2352675FE33`.
Use `gcad_glb_probe OUTPUT.glb --plate-scene` to repeat this check.

```sh
cargo build --release --locked --features game-assets --bin gcad
target/release/gcad.exe check examples/game-assets/plate.gasset
target/release/gcad.exe export examples/game-assets/plate.gasset target/game-asset-plate.glb
```

The VS Code `Game asset check (Windows)` configuration builds the feature-enabled
Debug CLI. Its build and check commands passed on Windows; debugger attachment
is unverified. Release check/export passed for the existing plate source, producing
76332 bytes, SHA-256
`B74F44563FCBFBEE6FBE7CB762B54B11209406963D8BDCB28CDDD0362CDDF92E`.
Unsupported FBX output is rejected. Existing plate CAD check and OBJ export also
passed. This plate GLB has not yet undergone independent import or visual checks.
Evaluated `.gasm` body conversion, generated-part animation, explicit rebinding,
optional refinement, generated-child skin authoring and preview routing are connected.
Nested assembly hierarchy,
continuous motion bounds and complete deformation acceptance remain.
Game operations are not added to `.gcad`/`.gasm`; mechanical joints are unchanged.

## Proposed scene contract

### LOD library foundation

`asset::lod::simplify` is an experimental per-primitive endpoint-collapse API.
It accepts a target triangle count, position/UV/angular limits, a trial budget,
explicit locked vertex indices, and deformed positions in original vertex order
for representative poses. Rest positions are checked automatically. The caller
must supply millimetre coordinates and relevant pose extremes when using the
primitive API directly. `asset::lod::derive_node` and the CLI collect these samples
for a selected scene node and primitive.

Only existing complete vertex tuples are retained. The result includes output-to-
original indices and an original-to-output representative map. `Lod::remap` applies
the same selection to weights or other per-vertex payloads without averaging or
implicit renormalisation. Skin bindings must be explicitly rebuilt after topology
changes; the old binding correctly fails its geometry guard.

Boundary/chart vertices, explicit attachment vertices, and rest-pose crease edges
above the angular threshold are locked. Each primitive must have consistently
oriented manifold edges and connected manifold vertex fans; duplicate faces,
unreferenced vertices and degenerate source pose triangles are rejected. Collapses
must satisfy the edge-link condition, preserve manifold topology, avoid triangle
and UV flips, and keep surviving face directions within the angular limit in each
sample. Material boundaries require callers to process primitives separately.

Every original vertex is compared with its final retained representative in rest
and all supplied poses; successive collapses cannot reset this error budget.
The reported maximum is a conservative barycentric-correspondence displacement
bound, not a closest-surface percentile. A target may remain unmet when topology,
attribute, pose or work limits prevent further collapses. The source mesh is never
modified. Synthetic grid tests cover locked boundaries/attachments, tuple remaps,
UV budgets, deformation-driven rejection, invalid topology and exact retained-
vertex skin deformation after rebuilding the binding.

`gcad asset-lod FILE.gasset NODE NEW.glb --triangles N --max-error MM
--max-uv-error UV --max-angle DEGREES` creates a separate export. Optional settings
are `--primitive` (default 0), `--samples` (17), `--max-trials` (1000) and
`--lock INDEX[,INDEX]` for attachment vertices in the original selected primitive.
The output path must be new. The JSON report includes sampled pose count, vertex
and triangle counts, target status, trial count, correspondence error and shading
angle. A target blocked by constraints/work limits remains explicitly unmet.

The adapter samples rest and every clip independently, including all keys, regular
times and STEP left limits. It checks world-space positions and deformed shading
directions, remaps weights, and rebuilds the selected skin binding. Normals/tangents
that exceed the final angular budget cause export failure; the current search does
not retry alternative collapses based on that final check. Meshes are duplicated
only when other nodes use them; unselected instances retain original geometry.
Other primitives, bones, node transforms and clips are preserved. Source weight
files are not rewritten; they remain bound to the original asset.

The LOD JSON report now includes bidirectional closest-triangle measurements for
every sampled world-space pose, labelled with clip/time (or rest). Samples comprise
each referenced indexed vertex, each indexed edge midpoint and each triangle
centroid. A bounding-volume hierarchy accelerates point-to-triangle queries.
Each direction reports sample count, maximum, nearest-rank p95 and RMS distance in
millimetres. The percentile is weighted by these discrete samples, including split
tuples at seams, not by triangle area. These statistics are separate from the
conservative correspondence bound used during collapse selection. They are neither
continuous Hausdorff bounds nor visibility/silhouette measurements and do not by
themselves establish production acceptance. Tests cover parallel offset planes,
missing surface, coplanar retriangulation and agreement with exhaustive triangle
search across a multi-leaf hierarchy.

The 20000-triangle flex-plate LOD was re-evaluated on 2026-10-31 at 18 poses,
with 65434 source-side and 64186 LOD-side spatial samples per pose. At the final
bend pose, sampled maximum distances are `0.023928070 mm` source-to-LOD and
`0.022537518 mm` LOD-to-source; RMS values are `0.000482862 mm` and
`0.000361657 mm`. Sample p95 is near floating-point zero because more than 95%
of these sample locations are unchanged; it does not imply that the maximum
deviation is negligible. At rest, maxima are `0.015521508 mm` and `0.015244494 mm`.
All per-pose sampled maxima stay below the `0.165414691 mm` correspondence bound.
The GLB remains byte-identical to the independently verified cached-search export.

Each pose also reports three principal-axis orthographic silhouette comparisons
at 256x256 resolution. Both meshes use the same square-pixel projection, fitted to
their combined referenced-vertex bounds with a two-pixel margin. Coverage is opaque,
double-sided and sampled at pixel centres; holes remain visible in the triangle
coverage. Reports give axis (0=X, 1=Y, 2=Z), resolution, millimetres per pixel,
occupied/union/differing pixels, and symmetric-difference area divided by union
area. Two empty masks produce a null fraction, not a zero-error claim. This does
not model material alpha, perspective, arbitrary engine cameras or subpixel
contours. A small raster difference is not a bound on contour displacement.

Optional `asset-lod` flags `--max-surface-error MM`, `--max-surface-p95 MM` and
`--max-silhouette-difference FRACTION` enforce final sampled limits before the
output file is opened. Surface limits apply to both directions at every pose;
silhouette limits apply to every nonempty reported view and require at least one
resolved view per pose. Limits are validated and recorded in the JSON report.
Failures identify the pose, direction or axis, measured error and requested limit.
These gates reject the derived export; they do not change search budgets or retry
alternative LODs. Tests cover independent rejection by each limit, invalid limits,
retriangulated silhouettes, missing coverage, disjoint shapes and empty edge views.

Windows CLI checks on 2026-10-31 pass the flex-plate LOD with
`--max-surface-error 0.03 --max-surface-p95 0.000001
--max-silhouette-difference 0.01`. All 54 views (18 poses, three axes) have nonempty
masks and zero differing pixels at 256x256; this does not establish subpixel or
arbitrary-view equality. The accepted GLB hash remains
`8DD82A77CBAA42A94AB0B3D67241707F51279A3921275F5A8B07B9526ABAF355`.
Repeating with `--max-surface-error 0.01` fails on the rest pose's
`0.015521508 mm` forward maximum and creates no output file. The source asset,
weights and earlier exports remain intact.

An independent native scanline probe now imports both the original and reduced
GLBs through Assimp, evaluates their exported skin weights and transforms, and
compares coverage at rest plus 17 clip poses. It fills horizontal scanline spans,
rather than using the Rust pixel/triangle-edge tests. Windows Debug and Release
passed on 2026-10-31: all 54 shared 256x256 views have zero differing pixels and
at least 4526 occupied pixels. Analytic square/retriangulation and missing-half
calibrations pass; shifting the imported LOD by 1 mm produces differing coverage.
This confirms the fixture's reported raster agreement after export, with the same
resolution and view limitations described above.

Remaining LOD work includes arbitrary-camera profiles,
larger CAD fixtures, performance improvements and engine
acceptance. The current checks do not establish the plan's complete LOD requirement.

Windows CLI and independent Debug/Release verification ran on 2026-10-31 using
`plate-flex.gasset`, target 20000 triangles, `--max-error 2 --max-uv-error 0.1
--max-angle 20 --samples 17`. At 100 trials it produces 12293 vertices and 20310
triangles; at 1000 trials, 12179 vertices and 20082 triangles. Both reports correctly
retain `target_reached: false`; the trial ceilings stop the search. The 1000-trial
run checks 18 poses including rest, with `0.164820774 mm` maximum correspondence
displacement and `13.027754674 degrees` maximum sampled shading-direction change.
The independent `--plate-flex` probe verifies retained weights/binds and analytic
vertex deformation at 17 poses, maximum error `6.2023e-09 m`. Centroid deformation
error remains `0.317286 mm` maximum, p95 `0.0296094 mm`; those are existing analytic
deformation diagnostics, not an independent closest-surface comparison to the
original mesh. Output SHA-256:
`FA1EB1CFB068980816CD32AC22049D0D6D612F12166C45596BA9C9BB200A41AB`.
Those runs preceded rejection caching. The search now stores the original vertex
or triangle that proves an attribute/pose rejection. A repeat is skipped only while
all of that witness's representative indices remain unchanged. Changed mappings
force a fresh evaluation. Topology failures are not cached, because they depend on
a wider neighborhood. Reports separate actual `trials` from `cached_rejections`;
cache skips do not consume the trial budget. Tests compare complete meshes, remaps
and measured errors with uncached search under four position budgets and a deformed
grid pose, confirming identical results when neither search exhausts its budget.
The same CAD settings now reach exactly 20000 triangles and 12138 vertices in
215 evaluated trials, skipping 1110 unchanged rejection witnesses. Sampled
correspondence error is `0.165414691 mm`; shading angle remains
`13.027754674 degrees`. Independent Debug/Release import still passes all retained
vertices at 17 poses, maximum analytic vertex error `6.2023e-09 m`. Centroid
deformation error is `0.317286 mm` maximum and `0.0299216 mm` p95. These are
sampled deformation diagnostics, not closest-surface LOD metrics. Verified on
2026-10-31; output SHA-256:
`8DD82A77CBAA42A94AB0B3D67241707F51279A3921275F5A8B07B9526ABAF355`.

### Opt-in animation key reduction

`gcad asset-reduce FILE.gasset NEW.glb --clip NAME --max-error MM --max-angle DEGREES`
creates a new export while retaining the source asset as the unoptimised reference.
Existing output files are refused. `--subdivisions` defaults to 4 (1..16);
`--max-trials` defaults to 1000 (1..10000). The command prints a JSON report with
original/reduced property-key counts, trial count, sampled times and maximum errors.

The reducer greedily tries interior LINEAR keys in track order. It retains each
track's first/last keys, clip duration, all tracks and nodes, and every STEP key.
Each candidate is compared against the original clip at all original key times,
subdivisions of their combined intervals, and STEP left limits. Checks include
world-space mesh vertices, skin-deformed vertices, all node origins and transformed
axes (including meshless attachments), rigid normals/tangents and CPU-deformed
skin normals/tangents. Reflection and tangent handedness changes are rejected.
Singular or invalid candidate poses are rejected. Errors accumulate against the
original, never against the last accepted candidate. Limits are sampled; they do
not prove continuous error bounds, engine rendering, collision behavior or shader
equivalence. Export rounding and prior bake errors are separate contributions.

The original sampled poses are cached up to ten million vectors and 10001 sample
times. Hitting the trial limit returns the best verified candidate reached so far,
with `trial_limit_reached` in the report. No claim of minimal key count is made.
Unit tests cover endpoint/STEP preservation, trial limits, a meshless node's
orientation, and a skinned rotation whose vertex displacement exceeds the budget
despite permissive angular limits.

Windows CLI verification on 2026-10-31 reduced `linkage-adaptive.gasset` from
396 to 40 property keys in 372 trials, without exhausting the 1000-trial limit.
With `--max-error 0.01 --max-angle 0.01 --subdivisions 4`, reported maximum errors
are `0.008683752 mm` and `0.009849460 degrees`. The GLB shrank from 129304 to
123096 bytes; geometry and textures dominate this fixture's file size. Debug and
Release independent `--linkage-reduced` probes pass 129 analytic four-bar poses:
maximum error `8.68383e-06 m`, within the `1.1e-5 m` combined fixture allowance.
The reduced GLB SHA-256 is
`C155ACA0C63364C51F07188FD5FDE6335FE63DD0F30C325F102A2D2A7681D9B0`.
Attempting the same output path fails and leaves the existing file hash unchanged.
Independent skinned-reduction evidence follows; engine validation remains.

`plate-flex-dense.gasset` preserves the existing flex plate's rest rig and guarded
weights, adding nine uniformly spaced rotation keys for the same constant-speed
bend. With `--max-error 0.001 --max-angle 0.001`, reduction removes seven keys;
maximum CPU position/direction errors are `8.8818e-15 mm` and `4.2309e-14 degrees`.
Windows Debug/Release independent `--plate-flex` checks pass all 12346 vertices at
17 poses, maximum vertex error `6.2023e-09 m`. The reduced output is byte-identical
to the original two-key fixture, SHA-256
`7023406C0C40FC38216EA4AC0E898BA41B164079943D890415384A0A0B9B1AA5`.

`plate-flex-eased.gasset` uses 33 keys sampling `angle = 90 * time^2` degrees on
the same rig. With `--max-error 0.05 --max-angle 0.1`, it reduces to 17 keys in
31 trials. CPU errors against the authored reference are `0.023124472 mm` and
`0.087890625 degrees`. Debug/Release `--plate-flex-eased` checks pass 129 poses
against the continuous quadratic analytic bend: maximum vertex error
`2.31259e-05 m`, within a `6e-5 m` allowance covering the `0.05 mm` reduction budget,
source-key interpolation and export rounding. The file shrinks from 1975892 to
1975572 bytes; SHA-256 is
`40176790C900F7DEF1E78521F1F9E2B9F89126265E546B964258A433B00DACE7`.
The linear-motion probe correctly rejects the eased output. Both validations ran
on 2026-10-31; unoptimised source assets and local reference GLBs are retained.
These checks establish sampled vertex motion and bindings, not independently
verified shading directions, continuous bounds, engine playback, or acceptance of
the plate's surface deformation quality (maximum centroid error remains about
`0.317 mm`).

Keep source evaluation separate from derived data. A scene owns nodes, mesh
primitives, materials, images and named rigid clips; future extensions add skeletons.
One vertex index addresses the whole attribute tuple. Never weld positions alone:
duplicated vertices at UV seams and hard edges are intentional. Use material
primitives to preserve boundaries before any simplifier is introduced.

Internal coordinates remain right-handed, Z-up and millimetres. Use column vectors
with world = parent * local; keep local transforms instead of flattening hierarchy.
For glTF, propose `(x,y,z) -> (x,z,-y) / 1000`, which preserves handedness and maps
Z-up to Y-up. Convert normals without the length scale, and conjugate transforms,
inverse binds and animation by the same basis conversion. Each adapter converts
exactly once. FBX must declare its own matching axis and unit metadata.

Non-uniform scaling needs inverse-transpose normal transformation. Reflections
need consistent winding and tangent handedness handling. Reject singular transforms.
Tests must cover a known one-metre dimension, nested transforms, reflection and
non-uniform scale before export is accepted. Quaternion rotations use xyzw; time
is seconds. UV origin and normal-map conventions must be explicit in the adapter.

The glTF contract must follow the
[Khronos specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html),
including attribute types, aligned buffers, skinning and animation constraints.

### Rigid animation implementation

`asset::animation` supplies named clips with translation/rotation/scale tracks,
STEP or LINEAR interpolation, seconds and local CAD coordinates. Track times
must be finite, nonnegative and strictly increasing; node/property targets cannot
repeat in a clip. Quaternion keys must be near unit length. Linear scale tracks
cannot cross zero, while step tracks may change reflection instantaneously.
`Clip::sample` returns transforms without mutating the rest scene, preserving
untracked properties. Translation/scale use linear interpolation; rotation uses
normalised shortest-path slerp. Playback clamps per track or loops over
`[0,duration)`; looping is a caller policy, not portable GLB metadata.

GLB now writes multiple named animation channel/sampler sets, converts translation
units and all TRS axes once, and normalises/sign-aligns quaternion keys. Animation
buffer views omit vertex-buffer targets. Export rejects float32 time collapse,
overflow and scale collapse. Tests inspect output key bytes as well as numeric
sampling and rejection cases. Viewer playback, root motion and procedural/constraint
baking remain unverified or absent.

Author clips with `clip NAME interpolation=linear|step`, then one property per
`key CLIP NODE time=SECONDS translation=X,Y,Z|rotation=X,Y,Z,W|scale=X,Y,Z` line.
Each node/property track must appear in strictly increasing time order; keys are
not silently sorted. References resolve after parsing, allowing forward node/clip
declarations. Empty clips, duplicate declarations and unknown references fail.
Interpolation currently applies to every track in that clip. The
`examples/game-assets/plate-motion.gasset` fixture has two two-second linear clips:
a parent rotation (`swing`) and a child translation (`slide`).

Windows Release CLI check/export and independent Debug/Release Assimp imports
passed. Eleven samples per clip had maximum Euclidean position error
`1.51589e-08 m` against analytic poses (limit `1e-6 m`). The 77312-byte GLB has
SHA-256 `E3B89B55400C531FD43DECAAB57DD9ED8A164EFBCE1412EEA35F890A834178B0`.
This verifies authored LINEAR translation/rotation and hierarchy, not skins,
engine import or viewer animation.

The additional `plate-scale.gasset` fixture verifies LINEAR nonuniform scaling
and STEP scaling with a reflection jump at one second and restoration at two.
Independent Debug/Release imports retain interpolation modes and pass eleven
samples per clip, including immediately before/after the discontinuity. Maximum
position error is `5.12272e-09 m` (limit `1e-6 m`), and determinant signs agree.
Changing the STEP clip to LINEAR correctly fails authoring validation because
the interpolation would pass through zero scale. The 77308-byte GLB has SHA-256
`B18C571686D575D5F92889EA699CD07D0F2F5B511D1E930EBCF40E05E27DCE9C`.
Repeat with `gcad_glb_probe OUTPUT.glb --plate-scale` after exporting that fixture.

```sh
target/release/gcad.exe export examples/game-assets/plate-motion.gasset target/game-asset-plate-motion.glb
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-plate-motion.glb --plate-motion
```

## CPU skin-binding foundation

`asset::skin::Skin::bind` binds a mesh node to an ordered list of existing named
scene nodes, computing inverse binds as inverse(joint world) * mesh world.
Each primitive supplies one four-influence tuple per vertex, indexed into the
joint list. Values must be finite, nonnegative and sum to one; duplicate nonzero
influences, invalid joint references and zero-weight vertices fail. There is no
silent normalisation, truncation or automatic weighting. The joint list uses up
to 65536 entries for future u16 export attributes.

Bindings retain mesh stamps, node identities/hierarchy and mesh/joint rest-world
transforms. Changed geometry, names, hierarchy or bind transforms require explicit
rebinding. `deform_positions` evaluates linear blend skinning in world millimetres
from a separate pose, preserving the original scene. Analytic tests cover a
nonidentity mesh/joint rest placement, rigid endpoints, a 50/50 blend under a
90-degree bend and restoration to the rest pose.

`asset::glb::encode_skinned_scene(scene, bindings)` writes explicit skin bindings,
u16 `JOINTS_0`, float32 `WEIGHTS_0` and MAT4 inverse bind accessors. Bind matrices
are conjugated by the same mm/Z-up to metre/Y-up basis as geometry, and matrices
that collapse at float32 precision are rejected. A bound node receives a separate
mesh variant, preserving other unskinned instances of its source geometry. This
currently duplicates geometry buffers; compaction is deferred. Multiple skins
on one node are rejected. Byte-level tests check joints, weights, inverse-bind
translation and unskinned-instance isolation. `encode_scene` remains the unskinned
convenience API; skins are passed explicitly rather than silently stored elsewhere.
Skin declarations now load persisted weights through the authoring path. A native
asset preview and software PNG snapshots are connected; interactive GPU validation
remains. CPU normal/tangent deformation is available as a separate
verification path through `Skin::deform_basis`.
For each vertex, this path blends the joint linear matrices, applies the inverse
transpose to the source normal, and transforms and Gram–Schmidt orthogonalises
the tangent. Normal/tangent directions are normalised and tangent handedness
flips when the blended determinant is negative. Singular blended matrices fail
explicitly, including a 50/50 blend of identity and a 180-degree rotation, even
though the individual joints are invertible. Unit tests cover that case and a
tilted normal under reflected nonuniform scale.

This is a declared CPU shading model, not a claim that every engine uses this
algorithm. Spatial weight derivatives are ignored; these normals need not match
the deformed triangle surface. Normals recomputed from geometry, rendered normal
maps, culling under reflection and engine-specific behavior still need validation.
`asset-report --shading` enables this strict check at all report sample times and
includes `max_normal_tangent_dot` when tangents exist. Without that flag, position
diagnostics remain available for singular blends. The dot measures orthogonality,
not shading quality, and is omitted if no tangent pairs were checked.
Windows Release `asset-report plate-flex.gasset --shading --samples 17` passed
with maximum absolute normal/tangent dot `2.22044604925031e-16`. The asset check
also passed. This verifies the CPU reference only; no rendered evidence is implied.

### Native asset preview

`gcad asset-preview FILE.gasset [--clip NAME] [--time SECONDS]` opens game-asset
document mode in the existing gcad app using the egui_bench renderer. It has rest-pose/clip selection,
play/pause, looping, timeline scrubbing, camera navigation/reset and diagnostic
view modes. It evaluates rigid world transforms and CPU skinned positions/normals;
a singular shading pose pauses playback and explicitly retains the previous pose.
Framing is fixed to the initial pose during playback. This does not edit CAD or
persist preview settings. The repository includes a Windows VS Code launch entry.

`--snapshot OUTPUT.png` instead renders a 960×720 software PNG without opening a
window. Each snapshot frames its sampled pose independently. Example:

```sh
target/release/gcad.exe asset-preview examples/game-assets/plate-flex.gasset --clip bend --time 1 --snapshot target/game-asset-flex-bent.png
```

The adapter supplies base-colour, converted normal and packed metallic/roughness
images, material factors and emission colour. It bakes linear base-colour tint
into a copied sRGB colour image because the renderer replaces vertex colour when
a texture exists; that copy introduces 8-bit quantisation. Uniform Blend opacity
is supported. The preview lists its differences: no occlusion/emissive image maps,
texture alpha/Mask or custom sampler control; two-sided drawing; renderer-derived
tangent frames rather than exported tangents. It is not engine-parity evidence.

Windows Release software snapshots of the centred plate at 0 and 1 seconds were
visually inspected: the flat and bent surfaces and four holes are visible. The
shared-instance and analytic translation adapter test passes. GPU controls,
GPU textured appearance and actual debugger attachment are not yet verified.
The software textured adapter now has a separate passing test: a 2×2 checker
uses linear tint `[0.25,0.5,1]`, negative-Y normal conversion and scalar
roughness/metallic maps. It checks the copied white pixel becomes sRGB
`[137,188,255]`, normal green becomes 200, packed G/B become 128/64, image bindings
and UV counts remain correct, and the source material is unchanged. Its software
PNG was visually inspected for the tinted checker and mapped shading. This does
not establish GPU texture behavior or parity with exported tangents.
Reproduce with `cargo test --release --locked --features game-assets --lib
view::asset::tests`; both preview tests pass. The generated
`target/game-asset-textured-preview.png` has SHA-256
`D79DF9E6A696A7614FFD3F23FC627DFCC2117E80ACCCEAC9D98996DC148652A2`.
PNG SHA-256 values: rest
`ACB97E7B3027DF00FC76A7244148A7E05385EBE3B1F214EFCCEBD44C1A04F5D6`, bent
`8DF8A3CB17C62863F238FF8128CE9E277926AF2D7D9F8BF6E6ECE491F68DFAC7`.
The `game_asset_glb --skin` fixture contains a six-vertex textured ribbon, two
bones, nonidentity mesh/joint bind placements, rigid endpoints, 50/50 middle
weights and one-second bend/twist clips. An unskinned copy shares the source mesh.
Independent Windows Debug/Release Assimp imports passed: weights, hierarchy,
texture connection, unskinned isolation and 108 sampled vertex positions (nine
times per clip). Maximum Euclidean error against analytic world positions was
`1.58127e-07 m`, below `1e-6 m`. The 5124-byte GLB has SHA-256
`FCACE386A2D2EF440B6F5BB5858903B770B2DE0B2B5574B751E62542E8896B40`.
Relocated import and the Debug fixture build/run also passed; a static file
correctly failed the skin probe. Interactive debugger attachment is unverified.
This synthetic ribbon does not establish CAD tessellation quality under
deformation, rendered skin normal/tangent correctness or engine support.

```sh
cargo run --release --locked --features game-assets --example game_asset_glb -- target/game-asset-skin.glb --skin
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-skin.glb --skin
```

## Provenance and rebinding

`skin NODE weights=PATH` attaches a version-2 JSON weight resource to an existing
mesh node. `Asset::weight_file(&skin).to_json()` captures the explicit assignment as
ordered joint names, per-primitive vertex weights, mesh stamps and a rig stamp.
The rig stamp includes node identities/order/hierarchy and mesh/joint rest-world
transforms; it excludes clips, permitting animation edits without rebinding.
Weight values may be edited while retaining those stamps; invalid weights still
fail validation. Changed geometry (even with the same vertex count), changed
rest transforms, unknown joints and unsupported JSON versions/fields fail before
export. Rebinding requires explicitly creating a fresh resource; there is no
automatic rebinding or weight-painting UI.

`gcad asset-bind FILE NODE OUTPUT --joints JOINT` assigns every vertex rigidly.
With two ordered joints and `--blend AXIS,START,END`, it generates clamped linear
weights along the mesh-local x/y/z axis in millimetres. It is an explicit spatial
assignment, not automatic rigging. The interval must be finite and increasing.
The command skips only the selected node's previous weight resource to allow
initial binding or explicit regeneration; all other skins still validate. It
creates a new file and refuses to overwrite any existing file. For rebinding,
write a different output and update the skin declaration after reviewing it.

```sh
target/release/gcad.exe asset-bind examples/game-assets/plate-skinned.gasset plate target/game-asset-plate.v2.weights.json --joints base,tip --blend y,-15,15
target/release/gcad.exe check examples/game-assets/plate-skinned.gasset
target/release/gcad.exe export examples/game-assets/plate-skinned.gasset target/game-asset-plate-skinned.glb
```

The example references the generated resource under ignored `target/`; run the
binding command first. Weight-generation repeatability and interval clamping have
unit tests. Shape quality under bending still requires evaluation; successful
binding alone does not establish useful deformation.

Windows Release CLI generation/check/export passed for this example. The 183148-byte
GLB has SHA-256 `EE9A6BFEE48565F807E69B3756956B62AD1CD27AE556A26D89A92AE10AFD1E86`.
An overwrite attempt failed and left the weight file hash unchanged. Changing
`w` to 41 rejected the old weights; explicit `asset-bind --set w=41` to a new path
succeeded. Independent Windows Debug/Release Assimp sampling now checks the
authored plate with `gcad_glb_probe OUTPUT.glb --plate-skin`: 1240 vertices,
1276 triangles and 17 evenly spaced poses from 0 through 1 second. The maximum
vertex error against analytic linear-blend skinning is `7.67988e-09 m`, below
the `1e-6 m` import tolerance. Bone hierarchy, weights and clip timing also pass.

This fixture is **not accepted as a useful deformable asset**. Posed/rest triangle
area ratios span `0.000630532` to `16.0863`, and maximum edge stretch is `2.45428`.
Comparing each deformed triangle centroid with continuous analytic deformation
of its rest centroid gives maximum error `5.07022 mm` and p95 `0.479665 mm`.
Percentiles weight each triangle/pose equally, not by area; they are sampled
interpolation diagnostics, not Hausdorff, silhouette or volumetric errors.
The explicit y-axis weight field around the offset tip joint can nearly collapse
triangles. Denser topology can reduce interpolation error but cannot by itself
repair that weight field. Next compare topology refinement and revised weighting,
preserving this unmodified reference. Normals, rendering and engine behavior
remain unverified.

The optional `plate-refined.gasset` comparison uses two uniform refinement levels
and an explicit 20416-triangle budget. Windows Release generation/export and
independent import passed: 12346 vertices, 20416 triangles, maximum vertex error
`7.95722e-09 m`. Centroid error fell to maximum `0.316888 mm`, p95 `0.0299784 mm`
(about 16 times lower at 16 times the triangle count). Area ratios still span
`0.00027982` to `5.40694`, with maximum edge stretch `2.52575`. It is also not
accepted as a deformable asset: refinement addresses interpolation, not the
near-singular deformation caused by the weight/joint configuration.
GLB SHA-256: `B5C63314488A7125CF1024AA9120DA4ABE17A579D747C15D7BC9CAED771D6754`.

```sh
target/release/gcad.exe asset-bind examples/game-assets/plate-refined.gasset plate target/game-asset-plate-refined.weights.json --joints base,tip --blend y,-15,15
target/release/gcad.exe export examples/game-assets/plate-refined.gasset target/game-asset-plate-refined.glb
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-plate-refined.glb --plate-skin
```

Two further fixtures isolate the effect of a different joint configuration and
then a narrower weight transition. Both retain the refined surface. `plate-flex`
places the tip at CAD `(0,0,1.5)` mm and rotates 90 degrees around CAD X, bending
through the thickness instead of rotating within the plate plane. It retains
the original y blend from -15 to 15 mm. `plate-flex-narrow` changes only the blend
interval to -7.5 through 7.5 mm. Windows Debug and Release independent imports
passed the same 17-pose analytic checks; the wrong weight-profile mode was also
verified to fail. These comparisons change the intended motion and do not claim
to reproduce or repair the original in-plane motion.

| Refined fixture | Max vertex error (m) | Triangle area ratio range | Max edge stretch | Max centroid error (mm) | p95 (mm) |
| --- | ---: | --- | ---: | ---: | ---: |
| Original in-plane joint | 7.95722e-09 | 0.00027982–5.40694 | 2.52575 | 0.316888 | 0.0299784 |
| Centred through-thickness, wide blend | 6.2023e-09 | 0.289562–2.19601 | 1.57268 | 0.317286 | 0.0289137 |
| Centred through-thickness, narrow blend | 4.52075e-09 | 0.361226–7.52538 | 1.51879 | 0.644916 | 0.0535102 |

The wide through-thickness candidate avoids near-collapse in these samples.
Narrowing its blend worsens maximum area expansion and sampled interpolation
error. Keep the wider blend as the candidate for subsequent normal/tangent and
rendered checks, without declaring production acceptance. No self-intersection,
silhouette, volume or engine appearance tests have been performed. Quality budgets
still require representative-asset criteria.

```sh
target/release/gcad.exe asset-bind examples/game-assets/plate-flex.gasset plate target/game-asset-plate-flex.weights.json --joints base,tip --blend y,-15,15
target/release/gcad.exe export examples/game-assets/plate-flex.gasset target/game-asset-plate-flex.glb
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-plate-flex.glb --plate-flex
target/release/gcad.exe asset-bind examples/game-assets/plate-flex-narrow.gasset plate target/game-asset-plate-flex-narrow.weights.json --joints base,tip --blend y,-7.5,7.5
target/release/gcad.exe export examples/game-assets/plate-flex-narrow.gasset target/game-asset-plate-flex-narrow.glb
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-plate-flex-narrow.glb --plate-flex-narrow
```

GLB SHA-256 values: wide
`7023406C0C40FC38216EA4AC0E898BA41B164079943D890415384A0A0B9B1AA5`, narrow
`BD36AA376A1E9B51F99FD0080CCC769809038855B38F4F70BA172A2AEED4F06F`.
The schema-v2 Release workflow also passed generation/check/export, producing the
same GLB hash. Changing only `unused_override=1` correctly failed with a stale
CAD-input diagnostic. Unit tests additionally reject a newline-only source edit.

`asset::author::load` now returns an `Asset` containing the scene and its validated
skins; `Asset::encode_glb` preserves both. The CLI uses this combined path for
check/export, preventing skin loss through the unskinned convenience encoder.
Weight paths resolve relative to the `.gasset` file through `host::read`.
Authoring now requires `project ROOT` for source-guarded skin binding; ROOT resolves
beside the asset file. Version-2 weights include the source records described below.
CAD byte changes (even a newline), overrides (including unused ones), source/body
identity, tessellation and UV settings invalidate the saved binding. Clip-only edits
remain valid. Version-1 resources require explicit rebinding; no migration silently
accepts their missing source guards. Standalone `Skin::weight_file` remains available
for synthetic library meshes, but its records lack the inputs required by authoring.

`model::Source` currently records file, body and variables; assembly placements
are stored separately. `select::Groups` stores labelled surfaces. Neither is a
durable per-face identifier across topology regeneration. `export::assembly`
already reconstructs local products and placements, but its product equivalence
must not be assumed to include all per-instance render material overrides.

Proposed binding identity combines a project-relative source ID, body/instance
path, sorted parameter values, dependency content hashes, tessellation settings,
generator version and the exact derived mesh stamp. Do not export absolute local
source paths. Resolve selectors anew after regeneration and reject zero or
ambiguous matches where one face is required. Persist explicit rebinding rather
than attaching old vertex weights to an equal-sized replacement mesh. Topology
stamps should be checked before operations that modify UVs or weights.

`asset::provenance` now implements a version-1 JSON binding record containing
relative source/body/instance identity, sorted parameters and dependency SHA-256
hashes, tessellation tolerance, generator version and the mesh binding stamp.
`Binding::capture` explicitly creates a binding; `require_current` rejects
changed inputs or geometry. UV/tangent edits alone do not invalidate it. Capture
after operations that split or reorder vertices. JSON loading rejects unknown
fields/versions, invalid numeric settings, missing source hashes and nonportable
paths, including absolute paths and parent traversal. Signed-zero parameters are
canonicalised. Records are local authoring artifacts, not embedded export metadata.
On native targets, `Inputs::with_recorded_reads` consumes `host::recording` output
from CAD evaluation. It stores content hashes and file-check outcomes (including
missing optional inputs), replacing the previous dependency set. It uses the
consumed-byte hashes rather than rereading files, strips the lexical project root
and rejects outside-root paths. Root containment is a portability check, not a
symlink sandbox. Source-guarded authoring uses `model::run_path_recorded`, merging
dependencies reached through the CAD part cache, including worker evaluations.
Disk-cache validation records its dependency reads as well. Direct filesystem
access outside the host abstraction is not recorded. External dependencies and
configuration probes outside ROOT are rejected, rather than omitted from guards.
The existing recorder retains the latest observation per
path, so concurrent file edits during evaluation are not fully detected. Browser
integration and selector/face provenance remain.
The record detects accidental staleness; it is not a signature or authenticity check.

## FBX candidate investigation

These are candidates, not approved dependencies. Reviewed upstream source on
2026-10-31; moving branches must be pinned before any prototype is accepted.

| Candidate | Evidence | Remaining work / cost |
| --- | --- | --- |
| Rust `fbxcel` | Its [README](https://github.com/lo48576/fbxcel) documents binary FBX 7.4/7.5 writing; [manifest](https://raw.githubusercontent.com/lo48576/fbxcel/master/Cargo.toml) declares MIT OR Apache-2.0 and passive maintenance | Low-level writer, not an asset exporter. gcad would own scene objects, skin clusters, bind poses, animation stacks, materials and connections. Compile and WASM feasibility untested. |
| Assimp native library / optional command-line converter | The [FBX exporter](https://github.com/assimp/assimp/blob/master/code/AssetLib/FBX/FBXExporter.cpp) contains geometry, skin and animation export code; [license](https://github.com/assimp/assimp/blob/master/LICENSE) permits source/binary redistribution with notice conditions | Requires native C++ tooling or a separately supplied executable. Verify two clips, weights, bind matrices and texture paths with an independent reader. No executable was found on PATH. WASM integration untested. |
| Autodesk FBX SDK | Autodesk [documents reading/writing/conversion](https://aps.autodesk.com/developer/overview/fbx-sdk) | Separate SDK and license terms require review before selection; no SDK downloaded or license accepted. Not the default proposal. |

Assimp's exporter explicitly assumes bind-pose input and makes assumptions about
skinned mesh instancing. The prototype must test these restrictions and reject or
expand unsupported instances rather than silently losing placement.

Assimp has passed the isolated native feasibility fixture described below;
`fbxcel` remains a possible Rust-only route if implementing FBX semantics is
justified. This is not a production backend selection. Do not describe import-only
readers as writers. External converter acceptance remains undecided.

The prototype must contain a textured indexed mesh, a two-bone hierarchy,
nonidentity bind transforms and two named clips (bend and twist). Check names,
weights, sampled deformation and units after independent import. Texture paths
must be relative and resolvable after relocating the output directory. Archive
tool versions, hashes and measured tolerances with the result. Only then select
a backend and document exact dependency/license/redistribution obligations.

### Native FBX prototype result

The project-owned [probe](../tools/game-assets-fbx/README.md) uses Assimp revision
`a4643cbd30d3088bbff312d3cc2652f514fb97c0` and independent ufbx reader revision
`5955c5c0b042ac2dc6f32955ab206aaec0620cfc`. These dependencies are isolated under
ignored `target/`; gcad's Cargo dependencies are unchanged.

On 2026-10-31, Windows x64 MSVC with CMake 4.2.3-msvc3 passed Release and Debug
builds and executions. Both a static and a skinned textured binary FBX import
correctly through ufbx. Checks cover numeric geometry, UV and normal values,
material texture connections, relative texture paths, centimetre units, two bones
with nonidentity bind transforms, weights and two named one-second clips. Across
nine samples per clip, maximum Euclidean vertex error was `2.00972e-14 cm`
against analytic expected deformation (limit `0.001 cm`). A relocated copy of
the output directory also passed verification without regeneration.

Release output SHA-256 values for this run (FBX generation timestamps can vary):

| File | SHA-256 |
| --- | --- |
| `static.fbx` | `A0368B7C89D925DC0E5ACF6015BBB9598EF937B10C16F1B59052F1269A561839` |
| `skinned.fbx` | `731E037C650BEDDE1BAB4D6FD6C84F1E997DCE8D819F75A76BCECAE3507AAC1C` |
| `checker.bmp` | `82D6D7E62F69EA5976EBE1304D13C30F9661ECDD6FC14A191842E4B77E3ECBBA` |

This proves the native API candidate on a small fixture, not engine import or a
converter executable's preservation of glTF materials. Production PNG/JPEG and
PBR packing, general rotation sequences, scale/reflection, instancing and target
engine validation remain. The VS Code configuration's Debug build and executable
were exercised; interactive debugger attachment was not tested.

## Native baseline evidence

Windows x86_64 MSVC; rustc 1.99.1 and Cargo 1.99.1. Cargo.lock SHA-256:
`234DAC7BAE155451C9A8E2C97EB8818FF0426A4B437AEF0AAF0D2E6560C4456E`.

The initial `cargo test --release --locked` failed to compile `tests/stored.rs`:
its `stored` helper unconditionally imported Unix `MetadataExt` and called `ino`.
Its two callers already have `cfg(unix)`. Matching guards on that helper and its
Unix-only write helper unblock compilation without disabling additional tests.
The existing Unix-only cache tests still do not establish Windows cache coverage.

After the test-only platform guards, `cargo test --release --locked --no-fail-fast`
completed: **435 passed, 5 failed, 13 ignored**. The five existing failures are:

- `job_format`: all three tests reject the stored 0.4.0 jobs because the current
  program is 0.5.2; fixture regeneration was not performed.
- `ops_doc::every_operation_has_a_test_that_matches_its_status`: the existing
  `output::drawing_labels_sit_clear_of_the_view` test is absent from `docs/ops.md`.
- `simulation::probe_on_a_curved_face_snaps_to_the_mesh`: its von Mises probe
  assertion fails. This was observed once; determinism has not been established.

The opt-in asset/preview tests passed **76/76**, including rigid and translation root-motion,
high-to-low normal
projection and atlas packing/rebaking,
CAD dimensions/source volume,
hard edges, UV chart splitting and mirrored handedness, physical tile scale,
GLB alignment/coordinate conversion/PNG packing, core material fields and
finish conversion, hierarchy composition, instances and material slots, plus
invalid-input rejection. After sharing the CPU pattern generator, the complete
library suite (`cargo test --release --locked --features game-assets --lib`)
passed **52/52**, including existing viewer texture tests. Baking tests exercise
all 17 patterns, deterministic maps, analytic UV rotation/physical scale/tangents,
flat and sloped height normals, explicit image precedence and source preservation.
Provenance tests cover JSON round trips, changed parameters/dependencies/body/
instance/tolerance/generator, same-count geometry changes, portable-path rejection
and unknown schema versions/fields.
The native recording fixture evaluates a CAD box from disk and verifies consumed
content hashes, missing optional files, changed-input rejection, outside-root
rejection and omission of the host directory from persisted JSON.
Focused rustfmt and `git diff --check`
passed. Raw baseline output is local at `target/game-assets-baseline.log` and is
not intended for publication. The normal baseline ran with the asset feature off.

## Translation root-motion foundation (reviewed 2026-10-31)

`asset::root_motion::extract` takes a scene, clip/node indices, an explicit X/Y/Z
axis mask and a new in-place clip name. It returns a cloned in-place clip and a
version-1 motion record, preserving the scene and original clip. The node must be
top-level and have a translation track. Selected translations are held at the
first key's value; extracted displacement starts at zero. Unselected translations,
rotation, scale and other tracks are preserved. STEP/LINEAR interpolation and
original key times are retained, so recomposition is algebraically exact between
keys, subject to ordinary floating-point rounding; no resampling is introduced.

The motion record carries node/clip names, axis mask, duration, interpolation,
times and displacement vectors in CAD millimetres/Z-up coordinates. JSON parsing
rejects unknown fields/versions, invalid times, nonfinite values, displacement on
unselected axes and a nonzero initial displacement. This is experimental metadata,
not an engine-specific convention or unit conversion.

`Motion::sample(time, looping)` returns absolute displacement. Non-looping sampling
clamps to key endpoints. Looping adds whole-cycle endpoint displacement to the
wrapped sample; negative time traverses cycles backwards. The full clip duration
is used even if translation keys occupy only part of the clip. Add this displacement
once to the named root's in-place translation at the same sample time. Applying it
to the entire asset instead would incorrectly move unrelated roots. Applications
that consume frame deltas should subtract consecutive absolute samples.

Tests cover STEP/LINEAR recomposition, a rotating root with a skinned child,
source preservation, delayed/early-ending translation tracks, forward/reverse
cycles, JSON round trips and invalid inputs. Rotational/yaw extraction and engine root-motion
conventions remain. Existing rotation tracks are deliberately retained in-place;
this library does not claim rotational root-motion support.
The full focused asset/preview suite passes 71/71 on Windows after this addition.

`asset-root-motion FILE NODE NEW.glb --clip NAME --in-place NEW_NAME --axes xy`
exports the original clips plus the appended in-place clip and embedded motion
metadata in a single new file. Axes must be a nonempty, duplicate-free subset of
lowercase `xyz` in CAD coordinates; there is no implicit ground-plane choice.
All validation/encoding happens before creating the destination. Existing outputs
are never overwritten. The stdout JSON is the native millimetre/Z-up Motion record.

`glb::encode_root_motion` uses `extras.gcad_root_motion` for the experimental export
record. It includes schema 1, source/in-place animation indices, the node index,
translation-axis selection, duration, interpolation, times, displacement keys and
loop displacement. Its explicit units are metres, right-handed, Y-up, converted
with the same `(x,z,-y)/1000` basis as geometry; `rotation_extracted` is false.
Consumers must opt in to reading these extras; ordinary glTF import does not imply
root-motion support. The original source animation remains available. Motion keys
retain JSON precision while glTF animation accessors use float32; reconstructed
exported poses therefore have normal float32 quantization error.

```sh
cargo build --release --locked --features game-assets --bin gcad
target/release/gcad.exe asset-root-motion examples/game-assets/plate-root-motion.gasset root target/game-asset-root-motion.glb --clip travel --in-place travel-in-place --axes xy
```

This CAD fixture translates and rotates a root, leaves vertical translation in the
in-place clip, and retains an unrelated mesh instance. The repository-local VS Code
`Translation root-motion export (Windows)` configuration uses the corresponding
Debug command. Interactive debugging and engine import are not verified.
The Windows release command exports both `travel` and `travel-in-place`; inspected
GLB metadata contains the converted loop displacement `[0.1,0,-0.05]` metres,
times `[0,1,2]`, and the expected animation/node references. The artifact is
78640 bytes, SHA-256
`5A1D3B831789BD603CF57B63F1BC43625D0CDE9E368957D93FC8B8EC641B9403`.
Repeating the command refuses the existing file and preserves its hash. Duplicate
axes are rejected without creating an output. The asset/preview suite remains
71/71 passing with the GLB metadata/skinned export assertions.

Independent Assimp import plus RapidJSON parsing of the GLB extras now verifies
this linear CAD fixture in Debug and Release. The probe samples all 1240 vertices
at 17 clip times and eight forward/reverse loop times, verifies preserved vertical
motion and rotation, checks the unrelated shared instance, and compares the
original clip against an analytic translated/rotated plate. Maximum recomposition
error is `5.96046e-8 m`; maximum analytic error is `1.49012e-8 m`. Omitting extracted
motion produces `0.111803 m` error, a negative control. These checks validate this
linear exported fixture; native STEP/skinned tests do not establish independent
STEP or skinned-engine import support.

```sh
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-root-motion.glb --root-motion
target/game-assets-tools/build/Debug/gcad_glb_probe.exe target/game-asset-root-motion.glb --root-motion
```

The probe uses RapidJSON headers already present in the pinned Assimp source
checkout, independently of Rust serde. No Cargo/runtime dependency or integration
was added. Rotational root-motion extraction is the next implementation step.

## Rigid root-motion foundation (reviewed 2026-10-31)

`asset::rigid_motion::extract` adds explicit full translation/quaternion rotation
extraction for a top-level node. It preserves the source and returns an in-place
clip plus a separate version-1 rigid motion record. A rotation track is required;
translation can be absent, in which case the rest translation is retained. Scale
and unrelated channels remain animated in-place. This is full rigid extraction,
not yaw-only or partial-translation extraction with an inferred residual policy.

Let `A(t) = T(t) R(t)` be the source root's rigid pose, sampled with the existing
STEP/LINEAR translation and shortest-path quaternion interpolation. The external
motion is `D(t) = A(t) inverse(A(0))`. The in-place root holds the sampled initial
translation and rotation while keeping `S(t)`. Applying `D(t)` on the **left** of
the in-place root matrix reconstructs `T(t) R(t) S(t)`. This handles nonzero starting
positions/rotations and animated nonuniform scale without resampling. Translation
and rotation retain independent source times/interpolation; consumers must evaluate
them before composing matrices, rather than linearly interpolating displacement
matrices or independently adding translation offsets.

Looping evaluates `D(end)^cycles D(phase)`. Each turning cycle therefore rotates
subsequent travel. Negative cycles use the inverse endpoint transform. Binary
exponentiation limits work; sampling rejects more than one million accumulated
cycles and nonfinite results. This is not a proven long-run numerical error bound.
Native JSON stores CAD millimetre/Z-up rest values and the original root channels,
whose node indices are local to the record and must equal zero. Unsupported schema,
unknown fields, scale channels, missing rotation and out-of-duration keys fail.

Tests exercise recomposition with a nonzero anchor rotation, animated nonuniform
scale, LINEAR/STEP rotation, preserved unrelated nodes, forward/reverse turning
cycles, a rotation-only root pivot and JSON validation. Independent import remains
next. Existing translation
root-motion APIs and exported metadata retain their previous behavior.
The full focused asset/preview suite passes 75/75 on Windows after this addition.

`asset-root-motion ... --rigid` now selects full rigid extraction explicitly;
it conflicts with `--axes`, and one of these modes is required. Both preserve the
original clips, append the requested in-place clip and create a new GLB only.
`glb::encode_rigid_root_motion` exports `extras.gcad_root_motion` schema 2 with
`mode: rigid`, the original absolute translation/quaternion channels, sampled
initial rigid anchor and endpoint loop transform. All exported positions are
metres/Y-up and quaternion axes use the same basis conversion. Each channel keeps
its own times and interpolation. Scale remains in the in-place animation.

The metadata declares `left-multiply-in-place-root` and
`endpoint-power-times-phase` composition, and the million-cycle limit. Consumers
must sample the absolute channels, compute the relative rigid transform and then
compose it; interpolating the endpoint transform or treating translations as
independent displacement keys is incorrect. Schema-1 translation exports are
unchanged. The CLI stdout record is the native rigid-motion JSON in CAD units.

```sh
target/release/gcad.exe asset-root-motion examples/game-assets/plate-root-motion.gasset root target/game-asset-rigid-root-motion.glb --clip travel --in-place travel-rigid-in-place --rigid
```

The repository-local VS Code `Rigid root-motion export (Windows)` configuration
uses the Debug CLI. Its JSON is checked; interactive debugging is unverified.
The Windows release export is 79120 bytes, SHA-256
`2022007D1AD576FCE351CF32A7B0C2638D5700BEFA8E820001C26C6FF1BA4E09`.
Its inspected metadata has anchor `[0.01,0.03,-0.02]` metres, endpoint translation
`[0.13,0.01,-0.06]` metres and a 90-degree Y-axis loop rotation. The original and
in-place clips are both retained. The focused suite passes 76/76, including a
skinned export check of channel conversions, anchor and loop transform. A repeated
export refuses the existing output and preserves its hash; combining `--rigid`
with `--axes` fails without creating a file. Translation-only export retains its
previous SHA-256 byte-for-byte.

Independent Assimp/RapidJSON checks now pass in Debug and Release for this rigid
fixture. They compare all 1240 plate vertices across 17 poses and ten forward/reverse
turning-loop times, verify the frozen in-place root and unchanged unrelated instance,
and compare accumulated motion with an analytic 90-degree-per-cycle path. Maximum
recomposition error is `1.666e-8 m`; analytic loop error is `6.67441e-8 m`. Reversing
the multiplication order produces `0.0433692 m` error, a composition negative control.
The schema-1 translation importer regression also still passes. This exported
fixture is linear and unskinned; independent STEP, animated-scale, skinned and
engine-runtime validation remain separate requirements.

```sh
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-rigid-root-motion.glb --rigid-motion
target/game-assets-tools/build/Debug/gcad_glb_probe.exe target/game-asset-rigid-root-motion.glb --rigid-motion
```

## Export capability reporting (2026-10-31)

`asset-capabilities FILE.gasset` runs the ordinary GLB encoder in memory and
prints version-1 JSON. It writes no export file. Success reports the exact
encoded byte count, destination mesh variants, nodes, skins, animations,
embedded PNG count, per-material texture slots/alpha/double-sided state and
samplers. These describe the encoded artifact, including metallic/roughness
channel packing and any extra mesh variant needed to separate skinned from
unskinned instances. The report also records coordinate conversion and known
limitations. This costs a full encode, including PNG compression.

Encoding failures produce `exportable: false`, the first encoder error, no
artifact summary and a nonzero CLI exit status. Authoring failures happen before
this stage and use the normal CLI error path. No second validator can diverge
from export precision, geometry, texture or skin requirements. A successful
report is not a visual-quality verdict: `engine_verified` remains false, FBX
is explicitly an isolated native prototype without production CLI support, and
runtime shaders are not certified. Root motion requires the separate extraction
command; ordinary capability reports do not imply root-motion metadata exists.

The repository-local VS Code configuration runs this report on the atlas fixture.
Interactive debugging remains unverified.

Windows verification on 2026-10-31: all 79 focused asset/preview tests pass;
the CLI builds. New tests cover destination mesh variants for shared skinned
instances, all six source texture slots packed into five PNGs, BLEND/double-sided
materials, missing UV rejection and float32 geometry collapse. The atlas fixture
CLI report succeeds with 76332 encoded bytes, one node/mesh/material and no image
maps (this fixture has UVs but no textures). A temporary asset with an overflowing
node translation emits structured failure JSON, no artifact and a nonzero exit
status. `rustfmt --check`, tracked diff whitespace checks and launch-configuration
JSON parsing pass. Existing compiler warnings remain. No engine, Linux or browser
verification was performed for this addition.

## CAD twist and attachment fixture (2026-10-31)

`examples/game-assets/plate-attachments.gasset` extends the refined CAD plate
with independent one-second bend and twist clips. Twist rotates the tip 90 degrees
about CAD Y; bend rotates about CAD X. Explicit linear weights span CAD Y from
-15 to 15 mm. The tip pivot is at CAD Z=1.5 mm. Two nested meshless named nodes,
`attachment` and `payload`, carry offsets of 10 mm along X and 5 mm along Y.
These use ordinary scene hierarchy, with no new authoring command or engine socket
semantics. Skin deformation applies to the plate; attachment frames follow the
bone rigidly, rather than inheriting blended per-vertex weights.

Generate fresh output paths (binding refuses to overwrite):

```sh
target/debug/gcad.exe asset-bind examples/game-assets/plate-attachments.gasset plate target/game-asset-plate-attachments.weights.json --joints base,tip --blend y,-15,15
target/debug/gcad.exe export examples/game-assets/plate-attachments.gasset target/game-asset-plate-attachments.glb
target/debug/gcad.exe asset-report examples/game-assets/plate-attachments.gasset --samples 33 --shading
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-plate-attachments.glb --attachments
target/game-assets-tools/build/Debug/gcad_glb_probe.exe target/game-asset-plate-attachments.glb --attachments
```

Both independent native configurations pass 66 poses over 12346 vertices and
20416 triangles. The probe checks imported clip bounds, hierarchy, weights and
inverse binds against analytic deformation. It compares attachment origins and
three basis endpoints, covering orientation as well as position. Maximum skin
error is `8.01154e-9 m`; maximum attachment frame error is `2.32831e-9 m`.
An omitted-rotation negative control differs by `0.0141421 m`.
GLB SHA-256: `C66BAB9F52D6C5F3055CE45B1F651B13892940C0F28F72AD30FBF2CCAE70B2FE`.

Strict CPU shading diagnostics pass. Twist triangle-area ratios range from
`0.577309` to `1.238140`, and edge-length ratios from `0.601358` to `1.510012`.
Maximum absolute normal/tangent dot is `2.22045e-16`. The area shrinkage and
stretch mean this is export-fidelity evidence, not acceptance of CAD tessellation
or linear blend skinning for production deformation. Engine sockets, actual engine
poses, retopology decisions and asset-specific quality budgets remain open.
No Rust implementation changed in this step; the previous 79-test result remains
the latest library evidence. Native probe builds and executions were rerun; Linux,
browser and interactive debugging were not tested.

## Explicit piecewise weight authoring (2026-10-31)

`asset-bind --chain AXIS,KNOT0,KNOT1,...` pairs the supplied ordered `--joints`
with strictly increasing finite positions in mesh-local millimetres. Each vertex
blends linearly between its two neighbouring knots; positions outside the interval
use the endpoint joint. Knot positions do not infer joint transforms or hierarchy.
At most two influences are used, retaining the existing four-slot representation,
skin binding guards and version-2 weight-file format. The mode conflicts with
`--blend`; the original two-joint blend and rigid modes are unchanged. Library
callers use `Skin::assign_chain`. This is explicit authoring, not automatic weighting.

The new `plate-twist-chain.gasset` fixture uses five bones with rotations of
0, 22.5, 45, 67.5 and 90 degrees about the common CAD Y axis. They are independently
posed nodes, not a parent chain that would accumulate those angles. Nested named
attachments follow the final bone. Generate fresh output paths with:

```sh
target/debug/gcad.exe asset-bind examples/game-assets/plate-twist-chain.gasset plate target/game-asset-plate-twist-chain.weights.json --joints bone0,bone1,bone2,bone3,bone4 --chain y,-15,-7.5,0,7.5,15
target/debug/gcad.exe export examples/game-assets/plate-twist-chain.gasset target/game-asset-plate-twist-chain.glb
target/debug/gcad.exe asset-report examples/game-assets/plate-twist-chain.gasset --samples 33 --shading
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-plate-twist-chain.glb --twist-chain
target/game-assets-tools/build/Debug/gcad_glb_probe.exe target/game-asset-plate-twist-chain.glb --twist-chain
```

Windows verification: 81 focused asset/preview tests pass and the CLI builds.
Tests cover nonuniform knots, endpoint clamping, repeatability, persisted bindings,
rest-pose recovery and invalid knots/joints. Invalid knots and conflicting CLI modes
fail without creating a weight file. Both native configurations independently check
five imported bones, piecewise weights, clip timing, inverse binds and attachment
frames over 33 poses. Maximum vertex error is `7.67988e-9 m`; attachment frame error
is `2.32831e-9 m`. GLB SHA-256:
`3CB17653910449BD4F3C29FB9C1A1B9E8F3EA44016456AD83C85C3E05E2261D6`.

Minimum sampled vertex radial ratio is `0.980785`, consistent with the
`cos(11.25 degrees)` bound for blending adjacent 22.5-degree rotations. However,
triangle-area ratios range from `0.355175` to `7.689384`; edge ratios range from
`0.608603` to `1.616703`. Strict shading validation passes with maximum absolute
normal/tangent dot `2.22045e-16`. This does **not** establish better overall mesh
quality than the two-bone fixture: distributing the twist changes the deformation
field and exposes severe triangle distortion despite less radial shrinkage.
The evidence supports evaluating deformation-aligned retopology rather than
accepting uniform CAD refinement plus extra bones as a sufficient solution.
Engine, Linux, browser and interactive-debug verification remain outstanding.

## Plane-aligned mesh subdivision (2026-10-31)

Experimental `slice NAME axis=x|y|z planes=MM,... triangle_budget=N` subdivides
derived source triangles at explicit mesh-local planes before UVs and binding.
It is an alternative to `refine` for a source; combining the two is rejected.
Library callers use `Mesh::sliced`. New vertices interpolate positions and normals;
complete-tuple edges share intersections while existing hard-edge splits remain
separate. Original CAD solids are untouched. This is plane-aligned subdivision,
not quad retopology or reprojection onto exact CAD surfaces.

Planes must be finite, strictly increasing, and number at most 1024. Triangle
budgets apply across all bodies of an assembly source, and a 20-million triangle
visit limit bounds work. Settings are included in source provenance, so changes
require explicit rebinding. UV/skin-bearing meshes cannot be sliced directly.
Offsets within `64 * f64::EPSILON * max(1, abs(plane), max(abs(axis coordinate)))`
are classified as on-plane without moving the original vertices. This prevents
roundoff slivers found in the CAD fixture; numerically invalid results still fail
mesh or export validation. Near-plane triangles may cross by that tiny tolerance.

`plate-twist-sliced.gasset` keeps the five-bone twist field but replaces uniform
refinement with 17 Y planes, spaced 1.875 mm from -15 to 15 mm. Generate fresh
weights and export (prepare the unsliced chain reference as described above):

```sh
target/debug/gcad.exe asset-bind examples/game-assets/plate-twist-sliced.gasset plate target/game-asset-plate-twist-sliced.weights.json --joints bone0,bone1,bone2,bone3,bone4 --chain y,-15,-7.5,0,7.5,15
target/debug/gcad.exe export examples/game-assets/plate-twist-sliced.gasset target/game-asset-plate-twist-sliced.glb
target/debug/gcad.exe asset-report examples/game-assets/plate-twist-sliced.gasset --samples 33 --shading
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-plate-twist-chain.glb target/game-asset-plate-twist-sliced.glb --twist-sliced
target/game-assets-tools/build/Debug/gcad_glb_probe.exe target/game-asset-plate-twist-chain.glb target/game-asset-plate-twist-sliced.glb --twist-sliced
```

Windows results: 85 focused asset/preview tests pass and the CLI builds. Tests
cover winding/area, shared cuts and hard-edge splits, determinism, budgets,
roundoff vertices, strict authoring and the CAD fixture through float32 export.
The existing assembly budget test now also checks source-wide slice accounting.
Both independent native configurations pass 33 poses and check every triangle
against all slice planes (1e-8 m tolerance for imported float32 data). The result
has 3836 vertices and 6104 triangles versus 12346/20416 for uniform refinement.
Maximum analytic skin error is `6.35504e-9 m`; attachment frame error is
`2.32831e-9 m`. Relative rest-area and signed-volume differences from the uniform
reference are `3.48615e-9` and `3.40248e-7`. These scalar comparisons are not a
general surface-distance proof.

| Sampled metric | Uniform refinement | Plane-aligned subdivision |
| --- | ---: | ---: |
| Minimum triangle-area ratio | 0.355175 | 0.653895 |
| Maximum triangle-area ratio | 7.689384 | 1.470310 |
| Minimum edge-length ratio | 0.608603 | 0.603007 |
| Maximum edge-length ratio | 1.616703 | 1.640363 |
| Minimum vertex radial ratio | 0.980785 | 0.980785 |

Strict shading checks pass. Improved triangle areas with fewer triangles support
this option, but edge distortion remains substantial and the technique does not
establish general deformation quality. Continuous surface fidelity, better topology,
quality budgets and actual engine poses remain to be evaluated.
GLB SHA-256: `18642054552695F0A20065A40E50FD85FE8E04D686425B44F7CCE1C277500723`.
No Linux/browser/interactive-debug checks were performed. Debug configuration JSON,
Rust formatting and tracked whitespace checks pass; existing warnings remain.

## Interior twist-surface fidelity (2026-10-31)

The independent native twist probe now tests the rendered surface between vertices.
At each of 33 poses it samples 12 non-vertex points from a denominator-four
barycentric grid plus the centroid of every triangle. It compares the barycentric
interpolation of imported skinned vertices with the analytic continuous piecewise
weight field evaluated at the corresponding rest point. Analytic evaluation uses
double precision. This separates nanometre-scale vertex export agreement from
millimetre-scale error caused by representing a nonlinear deformation with flat
triangles. Rest-pose calibration stays below `2.086e-9 m`.

Results agree in Windows Debug and Release:

| Mesh | Triangles | Point/pose samples | Maximum (mm) | p95 (mm) | RMS (mm) |
| --- | ---: | ---: | ---: | ---: | ---: |
| Uniform refinement | 20416 | 8758464 | 0.557077 | 0.00346769 | 0.0143805 |
| 17 planes, 1.875 mm spacing | 6104 | 2618616 | 0.755170 | 0.0116931 | 0.0173543 |
| 33 planes, 0.9375 mm spacing | 18228 | 7819812 | 0.383883 | 0.00379651 | 0.00718717 |

The coarse sliced mesh improves triangle-area ratios but worsens correspondence
error. The finer sliced mesh lowers sampled maximum and RMS compared with uniform
refinement within the same 20416-triangle budget, while p95 is slightly worse.
Samples are equally weighted per face/pose; shared-edge samples are repeated and
this is not an area-weighted distribution. Tessellations therefore sample different
point sets. These are correspondence errors, not closest-surface distances,
continuous bounds or unsampled-time guarantees. No production tolerance is inferred.

`plate-twist-sliced-fine.gasset` preserves the five-bone rig and uses 33 explicit
planes. It has 10082 vertices, 18228 triangles, sampled area ratios
`0.653895..1.474437`, and edge ratios `0.598427..1.641889`. Shading checks pass.
The native reader checks all plane boundaries, analytic skin and nested attachment
frames. Relative rest-area/signed-volume differences from uniform refinement are
`4.65687e-9` / `2.76749e-7`. A 65-plane attempt exceeded the same triangle budget
and failed before creating weights; it was not accepted by increasing the budget.

Prepare fresh weights/output and use the existing uniform reference:

```sh
target/debug/gcad.exe asset-bind examples/game-assets/plate-twist-sliced-fine.gasset plate target/game-asset-plate-twist-sliced-fine.weights.json --joints bone0,bone1,bone2,bone3,bone4 --chain y,-15,-7.5,0,7.5,15
target/debug/gcad.exe export examples/game-assets/plate-twist-sliced-fine.gasset target/game-asset-plate-twist-sliced-fine.glb
target/debug/gcad.exe asset-report examples/game-assets/plate-twist-sliced-fine.gasset --samples 33 --shading
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-plate-twist-chain.glb target/game-asset-plate-twist-sliced-fine.glb --twist-sliced-fine
target/game-assets-tools/build/Debug/gcad_glb_probe.exe target/game-asset-plate-twist-chain.glb target/game-asset-plate-twist-sliced-fine.glb --twist-sliced-fine
```

The existing `--twist-chain` and `--twist-sliced` modes also emit these surface
metrics. GLB SHA-256 for the fine fixture:
`7A4AC1CF42D1F90AA68757C2ABDCB9EB604435CB8DD01EC02F438633A541D5A9`.
Only fixtures/native probes/docs changed in this step; the prior 85-test result
remains the latest Rust evidence. The new native checks were built and run in both
configurations. Engine, Linux, browser and interactive-debug verification remain.

## Accumulated Windows release regression check (2026-10-31)

After the asset, binding, motion, optimisation and subdivision additions, the full
default-feature release suite was rerun using `cargo test --release --no-fail-fast`.
Result: **435 passed, 5 failed, 13 ignored**, matching the recorded baseline counts
and exact failing test names. The failures remain the three incompatible 0.4 job
fixtures in `job_format`, the missing drawing-label entry in `ops_doc`, and the
curved-face probe in `simulation`. No baseline fixture was regenerated or unrelated
failure repaired. Log: `target/game-assets-regression-default.log`.

The full `--features game-assets` run initially exposed a test interaction absent
from the focused asset suite: the viewer simulation test listened to process-wide
progress while CAD asset tests emitted events concurrently. Its assertion failed,
poisoning the callback mutex and causing three secondary asset-test failures. The
integration targets otherwise retained exactly the five baseline failures.

The viewer **test only** now uses the existing scoped `listen_here` API and explicitly
emits an unrelated-thread event that must not appear in its results. Production
listener behaviour is unchanged. A complete release library rerun with the feature
enabled passes **118 tests**, including all 85 focused asset/preview tests and the
viewer regression. This is a repaired-library rerun plus the completed integration
run, not a claim that one post-fix full-suite invocation was green. Logs:
`target/game-assets-regression-enabled.log` and
`target/game-assets-regression-library-fixed.log`.

The extracted CPU pattern generator was compared with the original source: its
implementation matches after substituting the return type and function visibility.
`Cargo.lock` remains unchanged at SHA-256
`234DAC7BAE155451C9A8E2C97EB8818FF0426A4B437AEF0AAF0D2E6560C4456E`.
Tracked whitespace checks pass. These are Windows CPU/release results; ignored GPU
tests and Linux/browser verification remain outstanding.

## Explicit derived surface settings (2026-10-31)

`surface SOURCE key=value ...` supplies core PBR overrides for a source, including
all generated bodies of an assembly source. One directive per source is allowed;
forward references work, unknown/duplicate keys and unknown sources fail. Only
specified settings change. The CAD material and solid remain unchanged. Overrides
apply after normal CAD-finish conversion and before atlas rebaking; unsupported
CAD finish features still fail rather than being silently discarded.

| Setting | Values and meaning |
| --- | --- |
| `base_colour` | Four linear RGBA factors, each 0..1; unlike inherited CAD sRGB colour |
| `metallic`, `roughness` | Numeric factor 0..1 |
| `emissive` | Three linear RGB factors, each 0..1 |
| `alpha` | `opaque`, `blend`, or `mask` |
| `cutoff` | Finite nonnegative threshold, required exactly with `alpha=mask` |
| `double_sided` | `true` or `false` |
| `normal_scale` | Finite nonnegative normal-map scale |
| `normal_y` | `positive` or `negative`; declares the source normal-map convention |
| `occlusion_strength` | Factor 0..1 |
| `wrap_u`, `wrap_v` | `repeat`, `clamp`, or `mirror` |
| `magnify`, `minify` | `nearest` or `linear` |
| `mipmap` | `none`, `nearest`, or `linear` |

Inherited image maps remain intact. Negative-Y conversion happens on an export
copy; the source pixels are unchanged. Map-specific controls have no visible effect
without the corresponding map. Atlas rebaking uses the source wrap settings, then
outputs its existing clamp/no-mipmap sampler contract. These settings do not add
external image loading, arbitrary shader support or per-body material assignment.
They do not change geometry or invalidate weights merely for a factor/sampler edit.

Example and verification:

```sh
target/debug/gcad.exe export examples/game-assets/plate-surfaces.gasset target/game-asset-plate-surfaces.glb
target/debug/gcad.exe asset-capabilities examples/game-assets/plate-surfaces.gasset
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-plate-surfaces.glb --surfaces
target/game-assets-tools/build/Debug/gcad_glb_probe.exe target/game-asset-plate-surfaces.glb --surfaces
```

The fixture has independently authored MASK and BLEND plate instances. Native
Debug/Release imports verify exact linear RGBA, metallic/roughness/emissive factors,
cutoff and double-sided state. Capability reporting agrees with the exported modes.
All 89 focused asset/preview tests pass and the CLI builds; new coverage includes
strict parsing, preserving inherited fields/maps, source immutability, normal-Y
conversion, sampler GLB values and authored-source isolation. Formatting, tracked
whitespace and launch-configuration JSON checks pass. This establishes encoded
material semantics, not engine blending/rendering behaviour or GPU preview parity.
No Linux/browser/interactive-debug checks were performed.
GLB SHA-256: `66043EB3550ADC3818CD744D316BAE2AECB8D43DA68496C8C9DE910A4242EEB6`.

## Explicit image inputs and provenance (2026-10-31)

`map SOURCE SLOT PATH license=DECLARATION origin=DECLARATION` supplies an explicit
image for `base_colour`, `roughness`, `metallic`, `normal`, `occlusion` or `emissive`.
Paths are relative to the `.gasset` document; `project ROOT` is required, and resolved
files must stay within that project (including symlink resolution). Unknown sources,
slots, duplicate slots, missing declarations and missing/undecodable images fail.
Licence/origin strings are declarations, not inferred ownership or verified rights.

Maps override corresponding inherited/generated CAD maps after baking and before
surface overrides and atlas rebaking. Existing image decoders are reused; output
remains embedded PNG. Base-colour/emissive RGB is sRGB, alpha is linear, scalar maps
use numeric red, and `normal_y` declares normal orientation. Metallic/roughness
dimensions must agree; no implicit resampling occurs. Each axis is limited to 8192
pixels, and explicit decoded maps per source share a 256 MiB RGBA budget, checked
from dimensions before each decode. Assembly bodies own derived material copies;
this is not a cap on total process/export memory.

Materials retain `texture_sources` records with slot, project-relative path,
SHA-256 of the original bytes, license and origin. GLB material extras preserve them
as `gcad_texture_source_inputs`; capability reports expose them too. Derived atlas,
LOD and root-motion paths retain this original input lineage through material copies.
It does not claim exported pixels equal original files after baking, packing or
normal conversion. No absolute host path is placed in these records. Read capture
also includes these maps: changing one conservatively invalidates source-bound
weights and requires explicit rebinding even when geometry is unchanged.

`plate-maps.gasset` and two tiny original PNGs in `examples/game-assets/maps` exercise
all slots. Their GPL-3.0-or-later provenance and exact pixels are documented beside
the files; no third-party image was downloaded.

```sh
target/debug/gcad.exe export examples/game-assets/plate-maps.gasset target/game-asset-plate-maps.glb
target/debug/gcad.exe asset-capabilities examples/game-assets/plate-maps.gasset
target/game-assets-tools/build/Release/gcad_glb_probe.exe target/game-asset-plate-maps.glb --maps
target/game-assets-tools/build/Debug/gcad_glb_probe.exe target/game-asset-plate-maps.glb --maps
```

Independent Windows Debug/Release probes verify all 80 decoded RGBA bytes across
five embedded images, including packing and negative-Y conversion, all six provenance
records against independently calculated source hashes, and sampler/normal/occlusion
controls. GLB SHA-256:
`968E2359E7A83BAF71B578ECBFDD44EB7E3E5945612E42AD5C65CBCE6684AB81`.
All 91 focused asset/preview tests pass and the CLI builds. New tests cover
preserved pixels, metadata export, changed-map binding rejection,
missing images, parser errors, project boundaries and decoded-size limits.
Engine appearance, Linux/browser operation and interactive debugging remain unverified.
Capability reporting includes all six source records. Formatting, tracked whitespace
and debug-configuration JSON checks pass; existing compiler warnings remain.

## FBX conversion audit with authored maps (2026-10-31)

The isolated Assimp/ufbx tool now audits the actual `plate-maps.gasset` GLB through
Assimp's unmodified glTF importer and FBX writer, then reads the result independently
with ufbx. This extends the earlier direct-scene static/skinned feasibility fixture;
it does not select a production backend or link Assimp into gcad.

Run either route with a **new** output directory (existing directories are refused):

```sh
target/game-assets-tools/build/Release/gcad_fbx_probe.exe --audit-maps target/game-asset-plate-maps.glb target/fbx-maps-raw-new
target/game-assets-tools/build/Release/gcad_fbx_probe.exe --audit-maps-metres target/game-asset-plate-maps.glb target/fbx-maps-metres-new
```

Each produces `converted.fbx` and `report.json`. Exit success means the audit
completed, not that conversion passed production acceptance; reports explicitly
set `production_ready: false`. The source is checked for all six connected PBR slots
before writing, so downstream omissions cannot be confused with missing inputs.

Windows Debug/Release reports agree:

| Property | GLB source | Raw FBX route | Metre-metadata route |
| --- | --- | --- | --- |
| Embedded images | 5 | 3 | 3 |
| Base-colour/normal/emissive maps | Connected | Connected, byte-identical PNG payloads | Same |
| Roughness/metallic/occlusion maps | Connected | Not connected | Not connected |
| Roughness factor / ufbx resolved value | 0.75 | 0.209431 | 0.209431 |
| Metallic factor / ufbx resolved value | 0.5 | 0; no value present | Same |
| World extents in metres, relative to source | 1 | 0.01 on all axes | 1 on all axes |

The raw exporter defaults to centimetres without converting imported metre-valued
positions. The second route supplies `UnitScaleFactor=100` and
`OriginalUnitScaleFactor=100` on the Assimp scene (FBX units are centimetres per
source unit), leaving geometry unchanged. ufbx then reports unit metres=1 and all
three world extents agree within 1e-6 relative tolerance. This corrects scale for
the tested static fixture only; animated/skinned conversion needs separate checks.

ufbx's PBR interpretation of legacy FBX material properties is not a guarantee
that a target engine will resolve the same factors. The missing maps and changed
resolved roughness still rule out this unmodified route as a complete core-PBR
exporter. A production candidate needs an explicit material adapter and tested
engine conventions or another backend. Alpha behaviour, provenance transfer,
normal strength, general sampler support and engine rendering remain unaccepted.
No dependency was added or third-party source modified.

Evidence: `target/fbx-maps-audit-release-final/report.json` and matching Debug report;
`target/fbx-maps-metres-release/report.json` and matching Debug report. Raw FBX
SHA-256 for the recorded run:
`765316EAFB29EE2D23BC69DD8A79765DBCA8ACF6B67735F53A82F9630CA9FF05`.
FBX bytes may include writer timestamps; the semantic reports are the comparison.
No Rust code changed, so the 91-test result remains the latest focused Rust evidence.
Linux/browser/engine and interactive-debug checks remain outstanding.

## FBX public material API boundary (2026-10-31)

Inspection of the pinned Assimp exporter confirms that its FBX material path reads
an explicit legacy-property list and emits a fixed texture-connection map. It does
not enumerate arbitrary `aiMaterial` properties into FBX material properties.
In the pinned `code/AssetLib/FBX/FBXExporter.cpp`, the material block begins near
line 1484 and texture connection mapping near line 1730; the latter has no PBR
roughness/metalness/occlusion connection entries. This explains the observed loss
without requiring an engine assumption.

The isolated `--audit-maps-api` experiment sets `gcad_pbr_roughness=0.75` through
`aiMaterial::AddProperty`, verifies it through `aiMaterial::Get` before export,
and queries the FBX material properties with independent ufbx afterward. Debug
and Release agree: the property is present in Assimp but absent in FBX. The
metre-unit correction still passes; the same three map connections remain missing.
Reports are `target/fbx-maps-api-release/report.json` and
`target/fbx-maps-api-debug/report.json`.

```sh
target/game-assets-tools/build/Release/gcad_fbx_probe.exe --audit-maps-api target/game-asset-plate-maps.glb target/fbx-api-new
```

This rejects the assumption that adding PBR/custom properties through the current
public material API alone will fix export. It does not prove that all extensions
or other FBX writers fail. Production options remain:

| Route | Current evidence | Required next work |
| --- | --- | --- |
| Unmodified Assimp converter | Loses PBR maps/factors and needs explicit metre metadata | Not acceptable for the complete material contract |
| Extend the Assimp writer | Existing geometry/skin/clip prototype works; material serializer is the gap | Select a tested engine material convention; implement properties, texture connections/channel semantics, alpha/sidedness and provenance; repeat independent and engine tests |
| Rust semantic writer over a low-level library | Earlier candidate review only | Approve dependency choice, implement the FBX scene/material/skin/animation contract, validate independently |
| External converter | No conversion service in exposed tools/MCPHub recipes, and no converter found on PATH | Select a converter and test its actual outputs |

The tool discovery check did not install, connect or configure anything and does
not establish that no converter exists elsewhere on the machine. The choice between
native and external conversion remains open in this snapshot. Recommended next action:
select the engine/version and conversion route, then implement the corresponding
integration. Shader adapters depend on the same
engine decision. Rust code was unchanged in this investigation.

## WASM compile compatibility and local platform inventory (2026-10-31)

The installed `wasm32-unknown-unknown` target exposed a regression in explicit
texture inputs: authoring called a texture loader gated out on WASM. The loader
now resolves paths through existing `host::canonicalize` rather than direct
`std::fs::canonicalize`, and is available on both targets. Native resolution still
uses filesystem canonicalisation; WASM uses the host virtual-file abstraction.
Image decoding, hashing, project-relative metadata and size checks remain shared.

Verified with installed Rust 1.99.1, without installing tools:

```sh
cargo check --locked --target wasm32-unknown-unknown --features game-assets --lib
cargo check --locked --target wasm32-unknown-unknown --lib
cargo test --locked --features game-assets --lib texture_input
```

Both WASM library checks pass. Both targeted native tests pass, covering project
boundaries/size limits and full authored-map provenance with changed-file binding
rejection. The earlier 91-test run remains the broader focused result. `Cargo.lock`
is unchanged. Logs: `target/game-assets-wasm-check-fixed.log`,
`target/game-assets-wasm-default-check.log`, `target/texture-input-host-tests.log`.
Existing cross-target viewer warnings and the host Windows-resource build warning
remain; the checks completed successfully.

This is compile coverage, not a linked/threaded web deployment or browser-runtime
test. In particular, `capture_inputs` still explicitly rejects source-guarded
authoring on WASM because the browser host does not yet record input dependencies.
The full `web/build.sh` requires nightly Rust and may install wasm-bindgen-cli;
it was inspected but not run. Stable Rust and rust-src are installed; nightly is
not. The existing WSL distribution responds as Linux and has a C compiler, but
`cargo` and `rustc` are not on its PATH. This is not proof that no Rust toolchain
exists elsewhere. No Linux toolchain or integration was installed, and no Linux
build is claimed. Further environment setup requires an explicit tooling decision.

## Remaining decisions and next steps

### Accumulated release regression after normal authoring (2026-10-31)

`cargo test --release --locked --no-fail-fast` completed on Windows with 438 passed,
five failed and 13 ignored. Exact failing test names match the recorded baseline:
three `job_format` compatibility cases, the `ops_doc` coverage assertion, and
`simulation::probe_on_a_curved_face_snaps_to_the_mesh`. The three additional passes
relative to the 435-pass baseline are the virtual-file recorder tests. This is a
full suite result, not just a library run; it is not a green suite.
Evidence: `target/game-assets-final-default-regression.log`. This run includes the
native conflicting-input recording correction and subsequent normal-authoring work.
`cargo test --release --locked --features game-assets --no-fail-fast` also completed:
535 passed, the same five failures, and 13 ignored. Its complete library target
passes all 133 tests. Exact failing test names were compared with the prior default
baseline, not merely failure counts. Evidence:
`target/game-assets-final-enabled-regression.log`. Thus neither accumulated run
introduces a failing test relative to that baseline; the five existing failures
remain unresolved. `Cargo.lock` retains its prior hash. Linux, threaded browser
runtime, GPU preview and selected-engine acceptance remain unverified. No additional
tooling was installed for these runs.

### User-facing workflow guide (2026-10-31)

`docs/game-assets.md` now documents the implemented experimental workflow separately
from this chronological evidence record: feature build, source syntax, geometry
preparation, material/maps, explicit binding, clips, derived exports, overwrite
behavior, coordinates and known runtime/export limits. README and the plan link to
it. Static check, capability dry run, GLB export, shading report and software snapshot
commands were exercised against `plate.gasset`; outputs are `target/guide-*`.
Report diagnostics must remain separate from JSON stdout. Local guide links and
diff whitespace checks pass. Binding and optimisation commands were checked against
current source/help and existing evidence, not rerun to overwrite existing resources.
The guide does not claim milestone completion or production FBX/shader support.
Next: resolve the pending engine/toolchain/converter choices, continue integration,
and prepare reviewable changes after runtime validation.

### Independent crease-normal import and preview (2026-10-31)

The new native `--crease-normals` probe checks the authored hard/smooth GLB through
Assimp. An initial attempt to rebuild adjacency from float32 exported positions
reported smooth normal-vector differences up to `0.318412`: distinct full-precision
CAD positions had become coincident, inventing connections absent in the authored
mesh. The check was not relaxed to accept these differences. Instead,
`game_asset_normals_reference` writes a geometry-only JSON reference with the
original f64 positions and triangle corners. No normals, smoothing groups or
expected results are supplied. The probe checks every reference corner against
the exported float32 corner, then independently traverses face fans to compute
area-weighted normals using full-precision topology.

Debug and Release agree: each plate has 1276 triangles, 3421 smooth corners differ
from their face normals, maximum hard normal-vector error is `3.74782e-08`, and
maximum smooth error is `4.09511e-08` (required below `1e-6`). Unit normals,
orthogonal unit tangents, UV presence and identical hard/smooth triangle corners
also pass. Logs: `target/crease-probe-{release,debug}.log`; build logs:
`target/crease-probe-build-{release,debug}.log`. The reference writer build/run is
recorded in `target/crease-reference-build.log`. The reference example and Debug
configuration refuse an existing output file. Reproduction is in the probe README.

`gcad asset-preview examples/game-assets/plate-normals.gasset --snapshot
target/game-asset-plate-normals.png` produced a software preview which was visually
inspected: the left hard plate has visibly faceted perimeter/hole shading, while
the right smooth plate has softer shading on the same geometry. No missing parts
or obvious holes were visible. This renderer generates tangent frames and draws
both sides; its preview does not establish GPU or engine rendering parity.
Next: validate the chosen runtime/engine when the pending tooling and engine
decisions are resolved. Existing FBX material and shader requirements remain open.

### Explicit crease-angle normal authoring (2026-10-31)

`normals SOURCE crease=DEGREES` is an opt-in `.gasset` operation, applied after
tessellation/refinement/slicing and before UV projection, tangents, atlas packing
and skin binding. Angles must be finite in `[0,180]`; duplicate settings, unknown
sources and attributes are rejected. Omitting it preserves existing CAD normals.
The setting and algorithm version enter the source-binding generator identifier,
so changing the rule requires explicit rebinding even if geometry happens to match.

`Mesh::crease_normals` uses exact-position edge adjacency (signed zero canonicalised)
and joins face-corner fans only across oppositely directed, two-face edges within
the angle threshold. Area-weighted geometric normals replace analytic CAD normals.
Hard creases split complete tuples; point contacts and nonmanifold edges do not
create smoothing connections. Triangle positions, order and winding are preserved.
The threshold is local to each edge; transitive fan normals are not bounded to that
angle. This is shading authoring, not vertex movement, topology repair, approximate
welding, retopology or artist-selected edge tagging. Near-coincident edges remain
separate. Undefined/cancelled fan normals fail; existing UV/tangent input is rejected
by this pre-binding library operation. CAD solids are borrowed and remain unchanged.

`examples/game-assets/plate-normals.gasset` shows the same CAD plate with crease
angles 0 and 60 degrees. The authoring test checks identical triangle corners,
flat face normals at zero degrees, nonflat smoothed corners at 60 degrees, settings
in provenance, strict parsing, valid tangents and successful GLB encoding.
Mesh tests additionally cover a split-index 90-degree wedge, deterministic output,
binding invalidation, nonmanifold edges, point contacts and invalid inputs.
The initial UV-rejection test incorrectly attempted box tangents on synthetic
inconsistent wedge normals; it now supplies valid UV attributes directly to test
the intended precondition. No production tangent behavior was changed for that test.

CLI reproduction: `cargo run --locked --features game-assets -- export
examples/game-assets/plate-normals.gasset target/game-asset-plate-normals.glb`.
The repository-local **Crease normals GLB export (Windows)** Debug configuration
uses the same fixture and ordinary CLI build task. GPU/engine appearance and
interactive debugging remain unverified; this operation does not select an engine.

Verified on Windows: all 133 feature-enabled debug library tests pass
(`cargo test --locked --features game-assets --lib`), the fixture exports via the
CLI, and `cargo check --locked --target wasm32-unknown-unknown --features game-assets
--lib` passes. Logs: `target/crease-normals-library.log`,
`target/crease-normals-cli.log`, `target/crease-normals-wasm.log`. Formatting, launch
JSON parsing and diff checks pass; no Cargo dependency was added. Next: independently
verify exported crease normals and rendering. Engine/version selection and
representative appearance/deformation budgets remain open.

### Authored FBX skin and unit conversion audit (2026-10-31)

`tools/game-assets-fbx/skin_conversion_probe.h` extends the isolated native probe
to convert the authored plate attachment GLB, rather than only a synthetic native
scene. Both raw and metre-metadata routes use the already pinned Assimp exporter
and independent ufbx reader. No production dependency or converter was installed.
The source GLB SHA-256 is
`C66BAB9F52D6C5F3055CE45B1F651B13892940C0F28F72AD30FBF2CCAE70B2FE`.

The audit establishes control-point correspondence and checks skin/clip counts,
clip durations and attachment hierarchy. Across 66 poses and 4042368 face-corner
comparisons, the raw route has maximum skin error `0.02390996999 m` and attachment
origin error `0.0118075918399 m`. Supplying metre unit metadata reduces these to
`5.24774693874e-10 m` and `2.61627059775e-10 m`; maximum rest error is
`7.16558394298e-18 m`. The raw route is a failing unit-conversion control.
Independent analytic expectations use the authored plate's linear blend weights,
bend/twist motion and child offsets. Debug and Release report identical results.
This verifies sampled positions for this fixture, not arbitrary animation,
attachment orientation, shading, target-engine import or continuous error bounds.

Reproduction commands and the Debug launch configuration are documented in the
probe README. Evidence: `target/fbx-authored-skin-{raw,metres}-{release,debug}-final/`
contains FBX files and reports; build logs are `target/fbx-skin-build-{release,debug}.log`.
Audit exit zero means completed observation; `sampled_geometry_pass` records the
position result and `production_ready` remains false. Existing roughness, metallic,
AO and material-property loss still prevents selecting the raw Assimp route as a
production backend.

Platform inventory follow-up: WSL Cargo/rustc were also absent from the standard
per-user `.cargo/bin` location. Linux Rust and nightly/web tooling remained
unavailable; no installation or Linux runtime validation was performed.
Current direction: the native helper and multi-engine scope are approved, and
repository tooling conventions apply. Validate available runtimes and the
production material route; installing missing toolchains remains a separate action.

### Native conflicting-input recording correction (2026-10-31)

Review of native `host::Reads` found that repeated and merged reads replaced earlier
hashes/existence results. An evaluation could consume two versions of a dependency
and leave only the final version in the asset binding record. Native observations
now retain distinct stamps, hashes and file outcomes, deduplicating identical
entries. Existing provenance validation therefore rejects conflicting consumed
versions instead of issuing a binding for an inconsistent evaluation. Native cache
reuse also rejects contradictory records, including hash conflicts even when file
metadata is identical. This does not turn metadata-based cache checks into a full
filesystem transaction or detect all external writes between observations.

The regression uses actual native reads, a nested recording, an independently
recorded worker, repeated merges, and a missing file created during evaluation.
It verifies conflicting-content/existence rejection and successful capture of
consistent repeated observations. The targeted test passes:
`cargo test --locked --features game-assets --lib native_recording_rejects`.
Log: `target/native-read-conflicts-test.log`. The complete Windows debug library
suites also pass: 129 tests with `game-assets`, 36 with default features. Commands:
`cargo test --locked --features game-assets --lib` and `cargo test --locked --lib`.
Logs: `target/native-read-conflicts-library.log` and
`target/native-read-conflicts-default-library.log`. Formatting and `git diff --check`
pass; `Cargo.lock` is unchanged. Integration tests were not rerun for this correction,
and the prior five baseline integration failures remain unresolved.
This correction follows the browser recorder work;
the earlier statement that native recording semantics were unchanged applies only
to that earlier change.

### Browser input recording follow-up (2026-10-31)

The earlier native-only source-guard restriction is now removed. Browser virtual
file reads record hashes of the actual consumed bytes, file probes record their
boolean result, and nested evaluations merge their observations. Existing model
worker-recording merges also apply. Repeated identical observations are deduplicated;
conflicting contents or existence outcomes remain available for provenance
validation to reject. A read holds an immutable byte snapshot while hashing, so
a concurrent virtual-file replacement cannot alter the consumed record.

The recorder is isolated from browser APIs in `src/host/virtual_reads.rs`; native
tests cover consumed hashes, missing probes, nested recordings, explicit worker
merges, conflicting reads and cleanup after a caught evaluation panic. Shared
provenance validation now resolves paths against either the native working
directory or browser virtual root. Its observation test rejects conflicting hashes,
changed existence and dependencies outside the project. Native host recording
semantics are unchanged. Browser `unchanged()` still returns false, preserving the
previous conservative cache policy.

Both default and feature-enabled WASM library checks pass, and the full native
feature-enabled library suite passes all 128 tests, including the three isolated
recorder tests and the conflicting-observation provenance test.
Command: `cargo test --locked --features game-assets --lib`.
Logs: `target/virtual-reads-library-tests.log`, `target/virtual-reads-wasm-check.log`,
`target/virtual-reads-wasm-default-check.log`, `target/virtual-reads-tests.log`.
No toolchain/dependency was installed; `Cargo.lock` is unchanged. This establishes
compile compatibility and native tests of the recording logic, not threaded browser
execution or browser asset-authoring UI support. Next: validate browser
virtual-file authoring in a supported runtime when the web toolchain is available.

1. Native baseline evidence and focused prototype tests recorded; preserve
   the five baseline failures as separate follow-up work.
2. Select first engine/version and representative asset budgets;
   decide whether an optional external FBX converter is acceptable.
3. Native Assimp/ufbx prototype passed; next compare the optional converter
   route and test the chosen engine before proposing production integration.
4. Review experimental `.gasset` syntax and scene contract
   before stabilising the public format.
5. Static GLB, core materials and scene hierarchy pass independent import;
   procedural tile baking and persisted provenance guards pass library checks.
   Native evaluation-time dependency capture now preserves content hashes,
   missing-file observations and existence probes. The runnable baked fixture
   now passes independent import. Static authoring/check/GLB export are connected;
   named instances/transforms and persisted skin weights are supported. Motion
   key reduction, atlas baking, high-to-low normal projection, LOD baking and
   translation/rigid root-motion exports now have focused evidence. Explicit
   export capability reporting now uses the real GLB encoder. Next target the
   selected engine. Adaptive sampling, sampled vertex error
   budgets, explicit motion baking and generated-child
   skin targets are connected. The evaluated
   `.gasm` linkage conversion now passes independent import.
   Version-2 authoring weights now enforce source-input binding checks.

M0 remains incomplete. M1-M5 and upstream submission remain; nothing has been
published or sent upstream.

## Shader integration feasibility and proposed first slice (2026-10-31)

Update 2026-11-01: the constrained UV-scrolling PBR template was selected.
The renderer has already been vendored for the approved PBR work. The historical
inspection below describes the starting point, not the current dependency state.
`asset::shader` now provides the initial version-1 JSON library contract and
`examples/game-assets/uv-scroll.shader.json` demonstrates it. Integration into
asset documents, preview, reload and export remains unfinished.

The initial contract requires `version`, `template`, `scale`, `offset`, `speed`
and `fallback`. The template name is `uv-scroll-pbr`; fallback is either
`{"mode":"reject"}` or `{"mode":"freeze","seconds":number}`. No texture or
source-code resource paths are accepted by this description; bindings will use
the existing material slots. Descriptions are limited to 16 KiB. Both scale
components must be in [0.0001, 10000]; absolute offset and evaluated UV bounds
are 1,000,000; absolute speed is at most 10,000 UV units/second. Explicit time
must be in [0, 1,000,000] seconds. All values must be finite. These are initial
supported bounds, not user-specified production budgets. Invalid evaluated
coordinates fail instead of wrapping or silently dropping the effect.

Positive scale preserves tangent orientation; mirroring and rotation are outside
this first template. Preview uses top-left UVs and explicit seconds. Destination
adapters must convert UV and velocity conventions together. Static freeze is an
explicit requested transform, not yet a baked/exportable shader implementation.

Status: template architecture selected; initial description library implemented,
with preview/export/engine integration outstanding. The following inspection/proposal is retained as design context.
Review: 2026-11-14, or when the renderer/engine version changes.

The installed, Cargo-locked `egui_bench` 0.9.2 source was inspected rather than
assuming that its public material API accepts shaders:

| Evidence in the installed crate | Consequence |
| --- | --- |
| `src/viewer3d/mod.rs`: private `gpu`, `soft`, `trace` modules; public `Material::pbr`, `Pbr`, `Surface`, `Scene`, `Look` | Existing public entry points accept PBR values and image bindings, not a user shader program or template |
| `src/viewer3d/gpu.rs:333`: `create_shader_module` uses `include_str!("shader.wgsl")` | Raster WGSL is fixed at crate build time; a gcad-only material wrapper cannot replace it |
| `src/viewer3d/shader.wgsl:142`: fragment stage samples colour, normal and metallic/roughness at the same input UV | A shared UV transform is a bounded first runtime effect; changing baked image pixels is not equivalent to time-dependent shader evaluation |
| `src/viewer3d/trace.rs`: separately assembled trace WGSL; `soft.rs`: separate CPU implementation | Raster support alone would leave other preview modes inconsistent; capability gating or matching implementations are required |
| Installed package manifest declares `MIT AND OFL-1.1` | Any proposed vendored renderer needs its original notices and font licenses preserved; no package files or dependency pins have been changed |
| `src/view/asset.rs`: current gcad adapter provides `Finish::pbr` and reports map/sampler/alpha/tangent differences | Existing asset preview is not evidence of M5 shader authoring or full material parity |

### Selected template architecture

Start with a constrained, versioned template description, not arbitrary WGSL or a
general graph compiler. Keep it alongside the optional asset description, with
typed finite parameters, explicit existing texture-slot references, and a declared
fallback. A parser must reject unknown versions, templates, parameters and resource
requests rather than silently treating them as ordinary PBR.

The proposed first template is **UV-scrolling PBR**: shared two-dimensional UV scale,
offset and speed, evaluated as `uv * scale + offset + seconds * speed` before the
existing colour, normal and metallic/roughness samples. Time is supplied explicitly
by the asset preview, so pause, seek, loop and deterministic capture have one clock.
Its first supported material subset would be opaque PBR with those three bindings;
unsupported alpha, occlusion/emission maps and sampler behavior must be rejected
or explicitly reported, not declared equivalent. Normal maps require the existing
UV/tangent validation. Coordinate conventions and limits must be specified before
syntax is committed. This effect is a proposal, not an inferred user preference.

Prepare an isolated renderer extension that introduces a typed per-draw template
configuration while retaining the default shader path for all existing callers.
Do not edit the Cargo registry copy. Review a project-local patch/vendor approach
with dependency provenance and licensing before switching the build to it. No
dependency change, upstream contact, or publication is authorized by this proposal.

Generic GLB/FBX must not promise transfer of the animated shader. Require an explicit
fallback choice: fail export, or freeze the effect at a declared time and bake the
UV transform into a derived mesh with corresponding tangent checks. Preserve the
unmodified mesh and record template, parameters, time, lost animation and consumer
requirements in the capability report. A selected engine adapter must implement
the dynamic effect separately and prove parity; custom extras alone do not do so.

Hot reload should validate a candidate description, resource bindings and compiled
pipeline before swapping it into the preview. On failure retain the last valid
pipeline, show an actionable diagnostic, and keep camera/clip state. Avoid compiling
on every frame. Set explicit limits on description size, parameter count, bound
textures and reload work before implementation; avoid an unrestricted source-code
escape hatch in version 1.

### Alternatives and acceptance work

| Alternative | Tradeoff |
| --- | --- |
| Constrained templates plus isolated renderer extension (recommended) | Smallest demonstrable dynamic shader path; each added effect and preview backend needs an explicit contract |
| Constrained graph first | More authoring flexibility, but adds node typing, cycle checks, lowering, diagnostics and resource accounting before proving a first renderer/engine adapter |
| Defer shader implementation until engine selection | Respects milestone ordering and avoids selecting a shader contract before the consumer is known; retains all M5 work as outstanding |

Before claiming M5, provide parser/limit/invalid-program tests; deterministic
preview evidence at multiple times; successful and failed reload tests preserving
the prior frame; explicit fallback export checks; and a selected engine adapter
with matching parameters, texture colour spaces, sampling and timing. Windows GPU
and selected browser runtime tests remain required, not replaced by CPU snapshots
or WASM compilation. Linux validation remains separately outstanding. The current
material/animation/optimisation engine acceptance gates are still open, so this
proposal does not mark their dependent shader milestone ready or complete.

Next action: connect the selected template's description to asset
authoring, live preview, explicit static fallback and the dynamic Unity adapter.
The initial library is not production shader support or M5 acceptance.

### Proposed camera-space mask acceptance gate (2026-11-01)

The mask correction currently optimizes sampled UV area. The projected contrast
diagnostic and its 4x/8x references are characterization tools, not a substitute
for the M0 asset, camera and engine decisions. Acceptance should require:

- select representative assets, engine/version, camera poses,
  output resolutions and allowed area/spatial error budgets. Include small
  silhouettes, oblique surfaces, cutout interiors and animated/deformed poses.
- use a coverage-preserving measurement with matching sample conventions
  for candidate and reference. Validate it against known fractional coverage;
  counting every partial MSAA pixel as one is insufficient for area acceptance.
- demonstrate reference convergence below the selected error budget before
  judging a candidate. Record uncertainty and per-view results; an aggregate
  improvement must not hide a view exceeding its budget.
- check both total coverage and spatial error, preserving raw and corrected
  chains and reporting regressions. Match the engine's actual alpha cutoff,
  filtering and sample settings; do not infer parity from the candidate preview.

No numeric threshold or mandatory correction policy has been selected. Correction
stays opt-in. Review after M0 decisions or by 2026-11-14. These gates leave engine
acceptance open while independent work on missing material preview slots proceeds.
