# Adopted game-asset renderer

Use `sh tools/game-assets.sh verify-gpu NEW_OUTPUT dx12 vulkan` for active gcad
checks. `reproduce-renderer` checks and reapplies the pinned crate patches;
`verify-renderer` runs tests in the fork selected by `EGUI_BENCH_MANIFEST`. See
[development commands](../game-assets.md) for complete commands and prerequisites.

On 2026-11-02 the renderer changes moved to the
[Wixely fork](https://github.com/Wixely/egui_bench/tree/gcad-renderer), based on
upstream's published 0.9.2 source. Cargo pins commit
`0c33aef02aa91dbb12ba1f556067dc806e225d16`. Make future renderer changes in that
fork and update the Cargo revision and lockfile together. The fork's
[GCAD-FORK.md](https://github.com/Wixely/egui_bench/blob/gcad-renderer/GCAD-FORK.md)
documents provenance, limitations and tests. The old vendor copy has been removed;
the five patches below preserve historical reproduction.
next review: 2026-11-14.

GPU integration tests require `--features game-assets,gpu-tests`; ordinary asset
tests remain usable without a GPU. Run the historical validation harness with the
repository as `-CandidateDirectory` and `-ExpectedTests 140` for both Windows
backends. Adoption validation is recorded under `target/renderer-adoption-20261101`.
Interactive controls and external-engine acceptance remain separate gates.

The sections below retain the experimental development history. Statements about
an unadopted dependency describe that earlier state. Renderer preparation scripts
still reproduce the pinned crate patches. The three `prepare-gcad-*` scripts apply
to the pre-adoption source snapshot (review series through 0039), not to an already
adapted checkout; do not apply the adapters twice.

`explicit-image-api.patch` adds the public `ExplicitImage`, `ImageEncoding` and
`UploadedImage` types to `viewer3d`. It does not change existing `Image` literals,
scene bindings, shaders or legacy automatic mip generation. `explicit_image.rs`
is the exact proposed module and is compiled directly by gcad's GPU probe.

`renderer-integration.patch` applies after the API patch. It adds scene image
overrides, validates their base pixels and material associations, uploads explicit
levels in the real renderer, and selects the sRGB shader path without double
decoding. Every present map in an explicit material must be explicit; colour must
be sRGB and numeric maps linear. The existing shared sampler uses the shortest
map chain's LOD bound. Legacy-only batches retain their existing path.
Explicit materials also use a separate centroid-interpolated UV attribute.
This keeps partially covered MSAA fragments inside the chart instead of sampling
extrapolated pixel-centre UVs. Legacy materials retain their original UV attribute.

The interactive preview displays an error for unsupported tracing, missing GPU or
invalid explicit scenes. `render_gpu` returns `None` for these cases and callers
can query `explicit_image_error()`. `render_soft` asserts when given explicit
images; it cannot represent their sampling policy. This is an explicit unsupported
path, not a software-rendering implementation. Explicit scenes still need fresh
scene ids when image data changes, following the existing GPU cache contract.

The constructor accepts contiguous, possibly partial mip chains with dimensions
1..8192, exact RGBA byte counts and at most 256 MiB of pixels per image. Private
storage and read-only accessors preserve validation after construction. Upload
checks the device dimension limit, creates exactly the supplied levels, chooses
`Rgba8UnormSrgb` for colour or `Rgba8Unorm` for numeric data, and returns a view
with a clamp/linear sampler capped at the last supplied level. Anisotropy is one.
It neither generates levels nor verifies chart isolation; the atlas baker owns
that separate check. Source images and GPU allocation overhead are outside the
pixel budget.

Reproduce the patch against the existing local crate archive:

Preparation verifies the archive SHA256 from Cargo.lock, extracts to a new
temporary directory, initializes a commit-free nested Git workspace, checks and
applies the patch, and compares its module with the tested source. It adds an empty
workspace table to the temporary Cargo manifest solely for isolated compilation;
this is not in the review patch. No download, installation or active dependency
configuration is performed. Original dependency license files are preserved.

- Archive SHA256: `3decd5040e37033b7a35a547b9babed56d317180a20c6dff4e29d4a456be9a79`.
- API patch SHA256: `a8105ebadc6c5fbb3222cddb0e59468f5838b655abf5bc98b98b575f9ecfad79`.
- Integration patch SHA256: `bbe68d9cd2293ef8297963d1b71f2cc60be82941bfdf1341f0098c6f060ea4b5`.

The fixture generator calls gcad's atlas packer, mip baker and shared preview
conversion. It emits two charts, three prepared map slots and four levels into a
new file, refusing replacement. The small `GCADAT01` test format contains six
expanded vertices (position, normal and UV as little-endian f32), followed by
three slot chains (u32 level count, u32 width/height, raw RGBA bytes per level).
The candidate reader requires exactly these fixture dimensions and limits input
to 1 MiB. It contains no filesystem paths or production interchange claims.

The shared `view::asset::material_images` and `mip_images` helpers apply colour
factors in linear light, preserve alpha bytes, convert negative-Y normals, and
pack metallic/roughness channels at every level. Scalar factors and normal scale
stay separate for the renderer. The existing single-level preview uses the same
conversion; the active viewer still cannot consume the mip vectors. VS Code
launch entries build both examples in Debug; change the fixture output path for
each run because existing files are preserved.
Both Debug examples build; the Debug fixture matches Release byte-for-byte and
the Debug GPU probe passes Vulkan and Direct3D 12. Evidence:
`target/renderer-atlas-debug-build.*`, `target/renderer-fixture-debug.*` and
`target/renderer-atlas-debug-gpu`. The VS Code debugger UI was not exercised.

Three API tests pass, including mixed chain lengths and encoding refusal. The
earlier Windows Release Vulkan and Direct3D 12 checks pass
using this module's upload implementation and bounded sampler: 2,000 compute RGBA
results, 100 explicit-LOD controls, 16,660 raster pixels and ten implicit-LOD
controls per backend. Evidence is under `target/renderer-image-api-gpu` and
`target/renderer-image-api-tests.*`. The poisoned textures use a separate uploader
because the candidate deliberately cannot allocate unspecified extra levels.

The complete candidate now compiles with normal Cargo resolution; the earlier
offline attempt was missing `ignore`. Cargo filled the required cache entries,
without changing gcad's active dependency or lockfile. Actual candidate renderer
tests pass on Direct3D 12: 9,216 pixels match the legacy constant-material reference
with zero byte error, including 2,213 material pixels. A deliberately green supplied
lower level changes those 2,213 pixels; a base-only normal chain restores the
reference exactly by forcing the common sampler to LOD zero. Scene tests reject
stale base pixels, mixed legacy/explicit maps and wrong encodings. Tracing refusal
is checked. Evidence: `target/renderer-integration-final-rebuilt.*` and
`target/renderer-integration-verified`.

The gcad-baked atlas test additionally compares 17,472 pixels at 128, 32 and 8
pixels square against separate constant-material references for both charts.
It initially failed with maximum byte error 18 at MSAA triangle edges. The
explicit-only centroid UV fix makes every pixel match on Direct3D 12 without
loosening the threshold. The prepared maps exercise linear-light tint,
negative-Y normal conversion and metallic/roughness packing through four levels.
All 126 gcad asset library tests pass. Evidence: `target/renderer-adapter-tests-final.*`,
`target/renderer-atlas-diagnostic.*` (failure) and `target/renderer-centroid-verified`.
The full candidate `viewer3d::` suite passes 24 tests; evidence is retained in
`target/renderer-centroid-regression.*`. These tests do not establish general
nonconstant or animated material appearance. Explicit Vulkan coverage is recorded below.

The verifier checks semantic output markers as well as test exit codes. It clears
only generated egui_bench release artifacts before testing, because switching
same-name candidate directories in a shared Cargo target reused a stale binary.
By default it creates a fresh target directory; `-TargetDirectory target` can
reuse dependencies. The source hashes must remain unchanged throughout testing.

Before adopting this in the viewer, implement and verify:

1. Adoption of the isolated gcad adapter below after the renderer decision.
   Authored immutable storage and per-level preview conversion are implemented;
   the active preview still refuses explicit chains pending adoption.
2. Broader renderer coverage: projected/animated geometry and representative
   nonconstant materials beyond these diagnostics.
3. Bounds for overlapping/cached frames and driver allocation overhead. The gcad
   adapter now preflights per-frame CPU/GPU payloads; base-pixel equality still
   scans image bytes on each validation, with local measurements below.
4. Software-preview and tracing support if those modes are required for explicit
   chains; the current candidate refuses them. Export transport remains separate.

The vendoring/maintenance decision remains open. This proposal
does not claim active gcad viewer integration, engine parity or complete game-asset support.

## Isolated authored preview adapter

`gcad-preview-adapter.patch` connects attached authored chains to explicit scene
images. Each material prepares its maps once, so instances share image slots.
It validates the completed scene, retains the other material-difference warnings,
and returns an error before a software snapshot can create a file. It also updates
the authored-chain test to expect preview success while keeping GLB refusal.
SHA256: `A535434FB923A326A64E6266BB73EB29D4DD39DDBD23D63E277A7B5EB51775B6`.

The preparation script copies the current working sources (including untracked
feature files) to a new directory. It creates a commit-free nested Git repository
and generates the patch against that baseline. Only the candidate manifest gets
the renderer path override and `egui_kittest` test dependency; those local paths
and dependency changes are excluded from the source patch. Run from the root:

Every output directory must be new. The verifier defaults to a new build target;
`-TargetDirectory target` reuses dependencies but first clears generated Release
artifacts for gcad and egui_bench to prevent stale candidate reuse. The tests
require an available GPU and never silently skip missing-adapter failures.
`gcad_preview_tests.rs` is the proposed test module copied into the candidate.
It checks exact level bytes and encodings, shared map slots, unchanged source
ownership, snapshot refusal, and authored GPU rendering against analytic material
values at 128, 32 and 8 pixels square. A separate green lower-level control must
change pixels, proving the supplied levels affect rendering. The test uses
exposure 0.2 because the fixture's uniform emission otherwise saturates the
image and masks texture errors; authored material values remain unchanged.

This fixture exercises constant averaged chart interiors and two rigid instances.
It does not establish general alpha, animation, normal variance or engine
acceptance. The proposed adapter still duplicates level-zero bytes for legacy
scene compatibility. The active gcad preflight includes these copies, all explicit
levels and legacy mip-generation scratch, with 256 MiB CPU/GPU payload limits and
4,096 image slots per frame. Source storage, driver overhead and cached/overlapping
frames are outside this accounting.

Windows Release verification: all 129 asset library tests pass. The Direct3D 12
reference matches all 17,472 pixels exactly; the supplied-mip control changes
328 pixels. Evidence: `target/gcad-preview-reproduced` and its sibling verifier
logs. Earlier failed saturation and missing-target controls are retained in
`target/gcad-preview-verified`. No active Cargo manifest or lockfile adoption was
performed.

After adding the budget preflight, all 130 active asset tests and 132 isolated
candidate tests pass. GPU reference and supplied-level controls remain unchanged.
Validation measurements: 2,000 checks of 196,608 base bytes in 3.7801 ms; 200 checks
of 16 MiB in 62.8032 ms. These single-run local timings have no acceptance threshold
and do not imply general frame-rate bounds. A corruption at the final byte is
still rejected. Evidence: `target/preview-budget-tests.*`,
`target/gcad-preview-budget-verified` and `target/gcad-preview-budget-verify.*`.
Next action: nonconstant/animated materials. Renderer adoption remains undecided.

## Explicit backend verification

Both Windows verification scripts now require Direct3D 12 and Vulkan separately.
`GCAD_RENDER_BACKEND=dx12|vulkan` restricts creation to one API; each test asserts
the returned backend and emits a marker checked by the verifier. Unset selects
Direct3D 12 for manual test invocations; any other value fails. The scripts restore
the caller's environment. No missing-adapter skip or API fallback is accepted.
Logs are named `renderer-dx12`/`renderer-vulkan` and
`asset-dx12`/`asset-vulkan`, with `.stdout` and `.stderr` suffixes.

All three API tests pass, and all three renderer integration tests pass on each
backend. Both match the 9,216-pixel material reference and 17,472-pixel atlas
reference exactly, including supplied-level and shortest-chain LOD controls.
The authored adapter passes 132 asset tests per backend and matches all 17,472
reference pixels exactly. Its supplied-mip control changes 328 pixels on Direct3D
12 and 363 on Vulkan. Each backend is compared to its own reference; the tests do
not assert arbitrary cross-backend bit identity.

Evidence: `target/renderer-dual-verified`, `target/renderer-dual-verify.*`,
`target/gcad-preview-dual-verified` and `target/gcad-preview-dual-verify.*`.
This establishes Windows headless candidate coverage only. Linux, browser,
interactive controls and engine acceptance remain
open. The active gcad dependency remains unchanged.

## Textured rigid animation

`atlas-motion.gasset` adds nonconstant maps and independent translation, rotation
and nonuniform-scale tracks. The second instance stays fixed. The candidate test
uses closed-form transformed rest positions and inverse-transpose normals at five
times, freezes the viewport bounds, checks unchanged UV/mip data and compares GPU
readback at 128 and 32 pixels square. It requires visible differences when colour
is flattened and between consecutive poses. The reference shares the renderer
and prepared textures: it verifies pose integration, not independent shading or
baking correctness. It does not cover skinning or texture alpha.

All 133 candidate asset tests pass on each backend. All 87,040 animated reference
pixels match exactly; colour and motion controls change 7,492 and 3,580 pixels
respectively on each backend. The three base maps have 290/277/141 distinct values.
Evidence: `target/gcad-animation-dense` and `target/gcad-animation-dense.*`.
The initial low-density fixture was rejected by the variation assertion, then
rebaked at density 32. Neither the variation nor image-error threshold was relaxed.
The subsequent skin and per-map checks are recorded below.

## Textured skin and map effects

The test suite creates a two-joint skin in memory from the authored atlas-motion
geometry. Both inverse binds are nonidentity; all vertices blend the joints with
Z-dependent weights. Only one joint animates, including nonuniform scale, and the
second mesh instance stays unskinned. Five poses are compared with closed-form
weighted positions and a scalar inverse transpose of the blended linear matrix.
This is the existing per-vertex shading convention, not a geometric normal
recomputed from the deformed surface. No weight file or source geometry is changed.

Both rigid and skinned tests neutralize colour, normal and packed scalar maps
separately, requiring each slot to change pixels at every pose and image size.
All 134 candidate asset tests pass per backend. Both animation cases match 87,040
reference pixels exactly on Direct3D 12 and Vulkan. For the skin, the three map
controls change 7,125/6,385/6,166 pixels on Direct3D 12 and 7,125/6,381/6,176 on
Vulkan; motion changes 2,605 on both. The reference shares the renderer and
prepared texture data, so it does not independently establish shading or baking
correctness. General deformation quality and texture alpha remain unverified.

Evidence: `target/gcad-skin-textures-verified` and
`target/gcad-skin-textures-verify.*`. Next action: candidate texture-alpha
and masked-material preview support. Renderer adoption remains undecided; no active dependency or Cargo.lock changes were made.

## Texture alpha and mask proposal

`renderer-alpha.patch` applies after the API and renderer integration patches.
It adds `AlphaMode::{Opaque, Blend, Mask { cutoff }}` and `Pbr::base_alpha`, with
opaque/factor-one defaults. Factors and cutoffs must be finite and in 0..1.
Opaque ignores both alpha inputs. Mask compares texture alpha times factor to
the cutoff, discarding strictly below it before depth writes. Blend multiplies
texture alpha, factor and look opacity and selects the transparent pipeline even
when uniform opacity is one. A compile-time check keeps the expanded uniform
within its existing 256-byte stride. Software/tracing refuse material alpha.
The existing batch transparency sort remains; this is not order-independent
transparency or general correctness for intersecting/self-overlapping surfaces.

`gcad-alpha-adapter.patch` applies after `gcad-preview-adapter.patch`. It encodes
the authored modes/factors in material keys, passes factors only once, replaces
the old unsupported-alpha warning with the sorting limitation, and refuses
software snapshots before writing output. It supports legacy and explicit images.
The active viewer has not adopted either patch.

- Renderer alpha SHA256: `B5004627007B936305F65E6D74F05CC48DC57AD1D7EDCE0532C0A74600781948`.
- gcad alpha SHA256: `149D0306209F5BDB348611D4C862825BD4FBD1BA6F8A5CEEF6E399831FD3ED18`.

Output directories must be new. Generate the atlas fixture with the earlier
documented command if absent. `-TargetDirectory target` can reuse dependencies.
The alpha renderer verifier runs the existing API/integration suite followed by
two alpha tests on each backend. Threshold/depth and uniform-blend reference
checks cover both legacy and explicit images, including equality at the cutoff,
zero alpha with cutoff zero, and independent factor/look-opacity multiplication.
The expected software refusal deliberately triggers a caught panic in one test.

All 135 gcad asset tests pass on Direct3D 12 and Vulkan. The authored alpha test
requires visible changes when modes or factors are neutralized, checks material
encoding and rejects software snapshots without files. Existing rigid/skinned
reference tests remain exact. Evidence: `target/renderer-alpha-verified`,
`target/renderer-alpha-verify.*`, `target/gcad-alpha-verified` and
`target/gcad-alpha-verify.*`. The active Debug CLI also checks `atlas-alpha.gasset`.

Mask coverage across mip levels, software/tracing support, occlusion/emissive
maps, general transparency overlap and engine acceptance remain open.
Next action: coverage-aware masked-material mip generation. Renderer
adoption remains undecided in this snapshot.

The active baker now optionally records per-chart sampled mask correction via
`coverage=mask`; see the game asset guide for its metric and limits. After these
storage/report changes, the refreshed alpha candidate passes 138 tests on both
backends in `target/gcad-mask-coverage-verified` (verifier logs are siblings named
`target/gcad-mask-coverage-verify.*`). The base adapter verifier expects 137 tests;
the current alpha adapter command uses `-ExpectedTests 139` after the additional
projected occupancy test described below.

## Projected mask occupancy diagnostic

The refreshed alpha adapter compares `atlas-mask-coverage.gasset` with the paired
`atlas-mask-raw.gasset`. It checks identical geometry, UVs, factors, base bytes,
RGB and other maps; only lower colour alpha differs. Across 24 size/yaw/zoom
combinations it compares both chains with an all-map level-zero-only reference.
A zero-alpha lower-mip control must change occupancy, and the corrected bytes
must affect at least one pixel. Separate frame IDs prevent stale GPU cache use.

The renderer readback discards alpha. Occupied means maximum RGB greater than 3
against a verified black empty-scene background. This counts partial MSAA pixels
and can be affected by shading. The base-only reference can alias; this is not a
fractional-area oracle or target-engine acceptance. Both count error and spatial
mask disagreement are recorded, and regressions are deliberately permitted.

Direct3D 12 improves/regresses/ties 2/1/21 views; summed absolute count error is
7 → 6 and spatial disagreements 9 → 10. Vulkan gives 2/3/19, count error 8 → 9,
and disagreements 10 → 11. Corrected alpha changes 3/5 occupied pixels and the
lower-mip controls change 1530/1715 respectively. UV-area correction therefore
does not guarantee a better projected mask. It remains opt-in.

All 139 candidate tests pass per backend in
`target/gcad-mask-projected-verified`; verifier logs are
`target/gcad-mask-projected-verify.*`. The active Debug CLI checks the raw fixture,
with a matching VS Code launch entry (debugger UI not tested). Reproduce with the
alpha preparation and verification commands above. The refreshed alpha patch
SHA256 is `149D0306209F5BDB348611D4C862825BD4FBD1BA6F8A5CEEF6E399831FD3ED18`.
The same test now also renders 4x/8x level-zero references and box-averages their
binary occupancy into fractions. An asymmetric synthetic image validates the
block mapping. Per-view logs report reference area, base/raw/corrected area-sum
errors, spatial L1 and reference drift; no acceptance tolerance is imposed.

Against 8x, Direct3D 12 raw/corrected area errors total 785.28125/788.28125 and
spatial L1 totals 1329.71875/1330.71875. Vulkan gives 775.515625/780.515625 and
1327.109375/1328.140625. The 4x/8x area drift is 96.21875 and 96.390625, much larger
than the correction effect. Low-resolution base errors are 792.28125/783.515625:
binary partial-MSAA occupancy is not the same convention as fractional reference
occupancy. These contrast measurements cannot establish exact area or engine
acceptance. Correction stays opt-in; no production threshold is inferred.

The latest complete run is `target/gcad-mask-super-verified`, with verifier logs
`target/gcad-mask-super-verify.*`: all 139 tests pass on both backends, and the
original binary occupancy results are unchanged. See `docs/game-assets-design.md`
for proposed camera-space acceptance gates. Next action: missing
occlusion/emissive preview slots in the isolated proposal. Renderer adoption and
explicit mip transport remain pending.

## Occlusion and emission map proposal

the historical validation harness prepares the API, integration and alpha patches,
then adds `renderer-extra-maps.patch`. The new surface slots participate in batch
keys and bind two additional textures. Occlusion uses linear red and
`1 - strength + strength * red` to attenuate the ambient term only; strength must
be finite and in 0..1. Direct light, specular highlights and emission are unchanged
by occlusion. Emission multiplies the existing linear factor by decoded sRGB RGB;
texture alpha is ignored. Missing slots preserve previous behavior.

Explicit emission is sRGB and occlusion linear. All five slots share the shortest
present mip chain. The existing three-slot validation API remains available;
the extended API validates five slots. Mixed legacy/explicit bindings, invalid
extra-slot indices and encoding mismatches are rejected. The draw uniform is now
256 bytes, still guarded against exceeding its existing stride. Software and
tracing refuse extra maps, including legacy images; debug/cap shading behavior
otherwise retains its existing conventions. This does not establish engine PBR
parity or add indirect specular occlusion to the existing lighting model.

Output directories must be new. The verifier runs existing API, atlas and alpha
checks, then extra-map contract/refusal and GPU reference tests on Direct3D 12 and
Vulkan. Per image path, nine combinations of occlusion value/strength compare
36,864 pixels with map-free references using independently computed emission and
ambient factors. White-map controls must visibly change both paths. A shortened
occlusion chain must clamp emission to level zero despite a poisoned lower mip.
Software refusal intentionally produces caught panics in test stderr.

Patch SHA256: `42C9782846362394AFDB8FE9303B3926323C183F72545236FCE187300050ADC4`.
Verified on Windows in `target/renderer-extra-v3-verified`, with verifier logs
`target/renderer-extra-v3-verify.*`: all existing API/integration/alpha checks and
both new tests pass. Reference channel error is zero on Direct3D 12; Vulkan is
zero for legacy and one for explicit images. Map controls change 7056/15876 pixels
for occlusion/emission on each path/backend. The shortest-chain control changes
140 pixels, with bounded-reference error zero/one respectively. Source hashes
remain unchanged during verification; Cargo.lock and active dependencies remain
unchanged. The initial test-only missing-Default compilation failure is retained
in `target/renderer-extra-verified`; the final candidate includes the fix.

The gcad extension is prepared separately after the alpha adapter:

`gcad-extra-maps-adapter.patch` extends preparation and budget accounting to five
slots, retaining shared instances and packed metallic/roughness. Emission and
occlusion bytes stay unchanged at every supplied level; factors remain material
parameters. Explicit emission uses sRGB and occlusion linear encoding. Legacy
snapshots with these maps refuse before writing, and the obsolete unsupported-map
warning is removed only in the candidate. The active renderer remains unchanged.

The authored fixture `atlas-extra-maps.gasset` uses nonconstant maps, nonunit
emission factors and occlusion strength. Its test checks exact extra-map payloads,
shared slots, factors, image count/CPU/GPU budget boundaries, source preservation,
software refusal and visible controls at two resolutions for each new slot.
The existing atlas reference now compares all five maps against analytic constant
textures. The extra-map candidate now runs the rigid/skinned checks on
`atlas-extra-motion.gasset`: five nonconstant slots, nonunit occlusion/emission
factors, five poses, two image sizes and a static shared instance. Each slot has
its own neutral-texture control, required to affect every pose/size. Pose checks
compare positions/normals with scalar analytic transforms, retain UVs/factors and
exact mip bytes, and compare rendered pixels. This shares the renderer and baking
with the actual path; it does not independently establish shading or engine parity.

gcad extra-map patch SHA256:
`6A2DA8FCC61B5913419BD7F1CEE1650E0A8B05F4CBE75BB75709D13B3EB3CEAE`.
The active dependency and Cargo.lock are unchanged. The fixture's VS Code entry
uses the active Debug CLI check; that command was tested, not the debugger UI.

All 140 candidate asset tests pass per backend in `target/gcad-extra-v2-verified`
(verifier logs `target/gcad-extra-v2-verify.*`). The five-map constant atlas matches
17,472 pixels exactly on both backends. Nonconstant occlusion controls change
2869/2867 legacy/explicit pixels on Direct3D 12 and 2866/2866 on Vulkan; emission
changes 2874 on each. Exact fixture accounting is CPU/GPU 415060/436900 bytes for
legacy (CPU includes scratch), and 757760/430080 for explicit. Source hashes are
unchanged during verification. The initial ambiguous test-patch context was
rejected before verification and fixed with an exact line match; its preparation
logs remain in `target/gcad-extra-prepare.*`.

The refreshed five-map animation run in `target/gcad-extra-motion-verified`
(`target/gcad-extra-motion-verify.*`) passes all 140 tests on each backend. Rigid
and skinned cases each match 87,040 reference pixels exactly. Occlusion/emission
controls change 7478/7492 rigid and 7116/7125 skinned pixels on Direct3D 12;
Vulkan gives 7482/7492 and 7113/7125. The other three slots remain visible at every
pose/size as well. Source hashes stay unchanged during verification. The active
Debug CLI validates the new fixture, with a matching VS Code configuration
(debugger UI not tested).

Next action: reconcile the milestone audit and prepare concrete remaining
adoption/engine/backend decisions. Production adoption, explicit mip transport,
software/tracing support and engine acceptance remain open.
