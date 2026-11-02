# Game asset review preparation

Renderer adoption was authorized on 2026-11-01 after the frozen 0039 snapshot.
The active tree now pins the renderer fork and applies its three adapters;
this subsequent change is not part of the immutable 39-patch series. Provenance
is in the fork's [GCAD-FORK.md](https://github.com/Wixely/egui_bench/blob/0c33aef02aa91dbb12ba1f556067dc806e225d16/GCAD-FORK.md).
The former vendor copy has been removed. Historical adoption validation is under
`target/renderer-adoption-20261101`. The FBX helper approach is also approved;
production implementation and multi-engine acceptance remain outstanding.

Historical local patch snapshots are evidence of earlier states, not the current
submission series: they contain retired tooling and must not be applied to
reintroduce it. Prepare a fresh review series from the cleaned active tree before
submission. Current commands are in [tools/game-assets.md](../tools/game-assets.md).

Reviewed: 2026-11-01. Next review: 2026-11-14 or before submission.
This is local preparation for M6 of the
[development plan](game-assets-plan.md). No commit, push, issue, proposal or PR has
been created or sent. Maintainer acceptance and architecture agreement remain open.

## Documentation reconciliation 0039 (2026-11-01)

The four general documents are reconciled after patch 0038. The guide corrects
stale face-weight, FBX-material and UNORM16 statements without treating experimental
adapters as production support. Older patch sections, test counts and remaining-work
lists retain their historical scope. Use the newest entries and the plan's product
audit for current status. Artifact hashes and final verification are in
`target/game-assets-review/series-0039.sha256` and `0039-verify.stdout`; these external
records avoid a self-referential documentation hash.

Next action: verify the documentation artifact and assembled-tree contents,
then audit remaining requirements and available platform capabilities. No production
or external-engine completion follows from finishing review packaging.

## Patch 0038: Preserve pinned patch bytes (2026-11-01)

The first native verification of reconstructed 0037 stopped before compilation:
Windows `core.autocrlf=true` had converted embedded patches to CRLF. Their normalized
text matched, but their SHA256 pins did not. The build correctly refused the v10
patch. This exposed a gap in the earlier normalized reconstruction check.

`0038-preserve-pinned-patch-line-endings.patch` changes only `.gitattributes`, adding
`text eol=lf` to both embedded-patch patterns. SHA256:
`ac2619c23b148d676dd9f67bb074a4c87dd0e3455678823b82daa18305623947`.
`series-0038.sha256` preserves all prior artifacts. The original embedded patch
files and their pins are unchanged.

For historical reconstruction, use `git -c core.autocrlf=false` for both initial
`worktree add` and every `apply` operation, retaining the attributes-first sequence
below. The setting is command-local. Applying it only after the initial checkout
can cause index/worktree mismatches; that failed attempt is retained as evidence.
Once 0038 is present, its attributes preserve embedded patch bytes on ordinary
Windows checkouts as well.

The successful run exports fixtures using the previously verified 0036 Debug CLI;
the reconstructed Rust source has not changed. It checks eight shared body
instances over 97 analytic poses (maximum position error 1.33881e-07 m), plus a
rigid skin with nonidentity inverse bind over 33 poses (2.0825e-09 m). Each independent
GLB check rejects four corruption controls. The v10 FBX audit validates 19 authored
nodes, four reflected instances and 36 raw-weight corners, then checks 20,736
playback corners (4.30214e-09 m) and 20,736 adapter-basis corners (1.19465e-07
component error), including the associated corruption controls in both builds.
All source and generated fixture hashes remain unchanged.

Evidence: `0038-runtime-resume-v2.stdout` and `.stderr`, plus
`0038-native-resumed-v2/{build-Release,build-Debug,assembly-Release,assembly-Debug,skin-Release,skin-Debug}.stdout`
and its `fbx` directory. These prove the selected nested fixtures and explicit
consumer-adapter contract, not general FBX engine parity, runtime packaging or
Linux/browser support. Next action: reconcile and package the remaining
general documentation. Production integration remains incomplete.

## Patch 0037: FBX experiment tooling (2026-11-01)

`target/game-assets-review/0037-fbx-experiment-tooling.patch` contains 72 files,
6,810 inserted lines and 21 deleted lines against the 36 prerequisites. SHA256:
`155979511b7260297c123537923f933aea261eb740e608c9a17e1dc1b459dc17`.
`series-0037.sha256` preserves every prior checksum.

Scope: independent native probe extensions and contract checks, all ten historical
Assimp experiment alternatives, their build/verification scripts, nested assembly
and animation fixtures, preparation updates, examples documentation and the FBX
backend proposal. The launch file is reconciled to its current ordering; embedded
FBX patches receive the same scoped whitespace attribute as renderer patches.
These are isolated experiments, not a production FBX export command or adopted
dependency. Historical patch versions are alternatives against the same pinned
Assimp revision, not ten sequential patches.

Reconstruction found an application-order requirement in 0036: Git checks whitespace
before a new `.gitattributes` file takes effect. The earlier working-tree check did
not expose this because that file already existed there. Preserve the immutable
artifact and apply its attributes first. Use the same procedure for 0037, which
extends those attributes. For each patch containing a `.gitattributes` change:

```sh
git apply --index --whitespace=error-all --include=.gitattributes PATCH
git apply --index --whitespace=error-all --exclude=.gitattributes PATCH
```

Apply other patches normally with `--index --whitespace=error-all`. Check each
exit code before proceeding. No whitespace fixing or checksum changes are needed.

This change does not add native compilation, runtime or external-engine evidence.
Next action: build and exercise the packaged native probes, then package
the remaining four general documentation files. Production integration and architecture
decisions remain open. Review: 2026-11-14.

## Patch 0036: Renderer proposal tooling (2026-11-01)

`target/game-assets-review/0036-renderer-proposal-tooling.patch` contains 39 files
and 7,813 inserted lines against all 35 prerequisites. SHA256:
`9e75a64233c7730950a018f080d2ffb7b3661a02545c89b4e5265f3d511c9d17`.
`series-0036.sha256` preserves the prior manifest and appends this artifact.

Scope: all renderer proposal scripts, tests and seven pinned patches; two GPU
examples and their raster helper; five dependent atlas fixtures; the standalone
mip GPU verifier; seven launch entries and their build task. Cargo only registers
the examples. A `.gitattributes` rule disables whitespace diagnostics specifically
for embedded renderer `.patch` files, whose blank context lines contain one space.
No dependency adoption or production integration is included.

This exercises the diagnostic explicit-image upload path, not the full candidate
viewer or an external engine. Debugger UI, Linux and browser support remain
unverified. `post-0036-normalized-inventory.json` records 75 remaining differing
paths after CRLF normalization, covering FBX tools, fixtures, docs and launch
entries; Rust source matches. The separate raw inventory includes line-ending
differences. These inventories precede this documentation update.
Next action: package remaining FBX tooling and documentation.
Production decisions remain open.

## Post-0032 packaging queue (2026-11-01)

All 32 entries in `target/game-assets-review/series-0032.sha256` were rehashed and
match. The seven separate renderer/adapter patch artifacts also match their
README pins. Preserve these immutable artifacts; current source and documentation
have advanced beyond their historical reconciliation.

A preliminary comparison with the retained `face-weights-worktree` found 136
added/content-changed paths after normalizing text line endings: source 16,
examples 30, FBX tools 55, renderer proposal 26, documentation 4, and other
configuration/tests 5. This is an inventory of differences, not proof that every
path belongs in the next patch or that the retained checkout is a newly verified
reconstruction. No new review artifact is claimed from this inventory alone.

Next actions; review by 2026-11-14:

1. Reconstruct the 32 pinned prerequisites in a new isolated checkout and compare
   with the current tree. Check scope and preserve unrelated user changes.
2. Split active source work, fixtures/debug configuration, FBX experiments and
   renderer proposals into coherent prerequisite-ordered artifacts. Keep renderer
   adoption and the production FBX boundary distinct from tested experiments.
3. Validate each artifact at its actual prerequisite state, record commands,
   evidence and remaining gates, then publish a new local checksum manifest.
4. Reconcile the final assembled source without rewriting the existing series.
   No external submission, dependency adoption or engine choice is implied.

The updated product audit and outstanding decisions are in
[the development plan](game-assets-plan.md#concrete-outstanding-decisions-and-executable-follow-up).
M0-M6 remain incomplete; review packaging is independent work that can continue
while engine, renderer and backend choices remain unanswered.

## Patch 0033: Preserve evaluated instance hierarchy

Reviewed: 2026-11-01. Next review: 2026-11-14.

Proposed title: **Preserve nested part ancestry through evaluation and caching**

Evaluated parts previously retained flat body identities and placements without
their containing instance tree. This change records each instance's exact body
names, world placement and children, preserving single-body renames and selected
bodies. Whole-instance moves update its frame; partial moves update affected
descendants. A body pattern creates a body instance, not a copied parent assembly.
The cache serializes this metadata and changes its key prefix to avoid reading
old entries with the new schema. CAD body geometry remains authoritative.

Artifact: `target/game-assets-review/0033-evaluator-instance-hierarchy.patch`.
Prerequisites: all 32 entries in `series-0032.sha256`, applied in order to
`33653aa19321ac6760b7f3aefa1f779030415e0a`. SHA256:
`b48641d0ccb9ab72d63899df66d478a4f84df9e58919da9afc7bd0cb1080f210`.
The new `0033.sha256` and `series-0033.sha256` preserve the existing manifest prefix.
Five files, 211 insertions and four deletions: four model files and the evaluator
portion of `tests/assembly_hierarchy.rs`. The feature-gated asset-export test is
deliberately reserved for the following asset hierarchy patch, where its required
implementation will exist. No production renderer/FBX dependency is included.

Preparation reconstructed all pinned prerequisites in a new checkout. Apply with
strict whitespace checking and reverse checks pass. Release verification in
`hierarchy-model-v3-worktree` passes 49 existing assembly tests, one evaluator
hierarchy test and one stored-mesh test; the cache round-trip unit test also passes
with `game-assets` enabled. Candidate source/manifest hashes remain unchanged.
This is scoped Windows verification, not the full CAD suite or Linux/WASM evidence.

Reproduction and evidence in `target/game-assets-review`:

These scripts intentionally require new output paths; reproduce in a fresh review
directory rather than overwrite evidence. No commits or external submissions were
created. Next action: isolate the dependent asset hierarchy/motion changes
and their export fixture tests as patch 0034, then continue mip/renderer/FBX review
packaging. Engine and production adoption decisions remain open.

## Patch 0034: Preserve asset hierarchy and constrained motion

Reviewed: 2026-11-01. Next review: 2026-11-14.

Proposed title: **Retain evaluated assembly ancestry in asset nodes and motion**

Asset authoring now consumes the evaluator tree from patch 0033. It creates
explicit ancestor groups, computes body transforms relative to their actual parent
frame, and preserves established mesh-node names and shared geometry. Selecting
one body retains its ancestors and excludes unrelated branches. Constrained motion
finds descendant mesh nodes rather than only direct children and converts their
sampled placements back into parent-local frames. Existing hierarchy validation
and singular-frame refusal remain in the path.

Artifact: `target/game-assets-review/0034-asset-hierarchy-and-motion.patch`.
Prerequisites: the 33 ordered entries in `series-0033.sha256` at the recorded base
commit. SHA256:
`07db5b36e55b742a0e7ecd5761cd79827d4b2e0fef6bbd129721a58d8d940e03`.
`0034.sha256` and `series-0034.sha256` add it without changing prior artifacts.
Five files, 300 insertions and 17 deletions: asset module registration, the new
tree helper, only the hierarchy section of author loading, motion integration,
and the asset portion of the hierarchy test. Mip parsing/baking and unrelated
current authoring changes are intentionally absent from this prerequisite state.

Fresh Windows Release validation passes 119 asset library tests and both hierarchy
integration tests. The nested asset test covers shared geometry, selected-body
ancestry, an oblique placement, a reflected nonuniform instance, and constrained
motion against evaluator placements at 49 times. It reads emitted GLB node TRS
and parent relationships and checks their transformed positions after unit/axis
conversion. That test uses gcad's own transform machinery; it is not an independent
importer or selected-engine acceptance test. No new Linux/WASM or interactive
preview evidence is claimed.

Next action: isolate explicit atlas mip generation and subsequent material/
authoring/preview integration into prerequisite-ordered artifacts. Production
renderer/FBX adoption, transport and engine/platform acceptance remain open.

## Patch 0035: Guard explicit atlas mip chains

Reviewed: 2026-11-01. Next review: 2026-11-14.

Proposed title: **Bake and retain guarded atlas mips with sampled mask correction**

This artifact packages the complete active mip path: chart-aware level baking,
bounded sampling footprints, immutable shared chains, material/geometry guards,
authoring and optional sampled mask correction/reporting. Preview image preparation
handles each level and checks image/CPU/GPU payload budgets. The published renderer
and ordinary GLB route still refuse attached chains; no levels are silently lost
or regenerated. Renderer dependency patches remain separate proposals.

Artifact: `target/game-assets-review/0035-guarded-atlas-mip-chains.patch`.
Prerequisites: all 34 entries in `series-0034.sha256`. SHA256:
`444deb1fa2fb70e5c45fe460cc3406516974f33cb4f55c50cc5dd60ba29e68b9`.
`0035.sha256` and `series-0035.sha256` retain the existing manifest prefix.
The 18-file artifact has 1,621 text insertions and 24 deletions plus two original
synthetic PNGs. It includes their provenance/regeneration script, the two required
atlas fixtures and shared block, and two launch entries tied to the existing Debug
build task. The feature-enabled Windows/MSVC Debug CLI receives an 8 MiB stack
reserve; other profiles/platforms are unchanged by that conditional build setting.

The fresh Windows Release candidate passes all 133 asset library tests. Its Debug
CLI builds, validates both fixtures and emits the mask report's 12 chart/level
records. Launch entries parse and resolve to the existing task; the debugger UI
was not launched. Candidate source, Cargo and configuration hashes remain unchanged
during verification. Strict prerequisite-index apply and reverse checks pass.
All 16 source paths in the post-0032 inventory now match the assembled current
source after text line-ending normalization; this does not reconcile the remaining
tooling, fixtures or documentation, nor prove complete product acceptance.

The correction targets sampled UV coverage relative to rebaked level zero. It
cannot reconstruct lost support or guarantee continuous/projected/trilinear
coverage; measured GPU regressions and reference uncertainty remain documented in
the development plan. Budget counts exclude source assets, geometry, driver
allocations, render targets and overlapping frames. This artifact neither adopts
the GPU proposal nor establishes independent engine or Linux/browser acceptance.

Next action: package the renderer proposal tooling, GPU probes and their
remaining fixtures/configuration, then the isolated FBX experiment and updated
documentation. Production decisions remain open; no commit or submission occurred.

## Patch 0001: Share the procedural texture generator

Proposed title: **Share procedural texture generation between rendering and baking**

The procedural tile generator currently lives in the viewer. Move it to
`model::texture` so CPU material baking can reuse the same pixels. The viewer wraps
the returned `Map` in its existing image adapter. Pattern formulas, 256-pixel tile
dimensions, colour bytes and alpha remain unchanged; the helper is crate-private.
This patch introduces no asset format, dependency, CLI command or public API.

Local immutable artifact:
`target/game-assets-review/0001-share-procedural-textures.patch`

- Base: `33653aa19321ac6760b7f3aefa1f779030415e0a` (`0.5.2`).
- Files: `src/model/texture.rs`, `src/view/textures.rs` only.
- Size: 90 insertions, 85 deletions across two files.
- SHA-256: `7185D3340053B31CEE4AC31DE517FE882118A9D799CBC29878E0DCCEC0B3ECCE`.
- Isolated checkout: `target/game-assets-review/texture-worktree`, detached at the
  base with only those two files changed. Preserve this checkout for review.

The patch applies cleanly to the base. A text comparison confirms the generator
body is identical after substituting the return type (`Image` to `Map`) and
crate-private visibility. Existing texture tests pass in the isolated checkout:
tile-size/axis UVs, shared-part texture coordinates, and deterministic distinct
patterns (three passed). WASM library compilation also passes there. Neither
result depends on the larger feature-enabled working tree.

Commands used from the main repository root:

```sh
cargo test --manifest-path target/game-assets-review/texture-worktree/Cargo.toml --target-dir target --release --locked --lib view::textures::tests
cargo check --manifest-path target/game-assets-review/texture-worktree/Cargo.toml --target-dir target --locked --target wasm32-unknown-unknown --lib
```

Logs: `target/game-assets-review/0001-native-tests.log` and
`target/game-assets-review/0001-wasm-check.log`. The isolated diff passes whitespace
checks. GPU appearance, Linux runtime and browser runtime were not tested for this
patch. Existing renderer adapter behavior is retained; the broader accumulated
regression results belong to the full branch and are not substitutes for isolated
patch evidence.

The patch is generated with `git diff --binary --output=PATH --
src/model/texture.rs src/view/textures.rs`; do not regenerate the artifact silently
after code changes. Give a changed patch a new reviewed hash and repeat isolated
validation. Artifacts and the checkout are under ignored `target/`, not committed.

## Patch 0002: Preserve input observations across evaluations

Proposed title: **Retain conflicting file reads and record browser dependencies**

Native recording previously replaced earlier hashes and existence results when
the same path was read again or merged from another worker. Retain distinct
observations so consumers can reject inconsistent evaluation inputs, and prevent
contradictory records from being reused as unchanged cache entries. Browser reads
now capture actual consumed bytes and existence probes instead of returning empty
records. Nested and explicit worker merges retain observations. Browser cache
reuse remains conservative (`unchanged()` returns false).

Local artifact: `target/game-assets-review/0002-record-input-observations.patch`.
Base: `33653aa19321ac6760b7f3aefa1f779030415e0a`; independent of patch 0001.
SHA-256: `4C9D630E89B72C59E9F20E4C716A3B165915120F7B72AE657F7C84CC0502A01C`.
Files: `src/host.rs`, `src/host/virtual_reads.rs`, `tests/input_recording.rs`.
Size: 292 insertions, 41 deletions. Isolated checkout:
`target/game-assets-review/recording-worktree`, detached at the base. Its three
changed/new files match the main working tree byte-for-byte. Intent-to-add entries
exist only in this isolated checkout so new files are included in the patch.

The forward application check passed against baseline host/test paths in the
texture review checkout (patch 0001 touches disjoint files). In the isolated
recording checkout, two native integration tests and three virtual-recorder tests
pass in release mode. WASM library compilation passes. Native tests cover consumed
hashes surviving later writes, missing probes, nested conflicting reads, explicit
worker merges, deduplication and cache invalidation. Virtual tests also cover
cleanup after a caught nested panic. These tests require no `game-assets` feature.

```sh
cargo test --manifest-path target/game-assets-review/recording-worktree/Cargo.toml --target-dir target --release --locked --test input_recording
cargo test --manifest-path target/game-assets-review/recording-worktree/Cargo.toml --target-dir target --release --locked --lib host::virtual_reads
cargo check --manifest-path target/game-assets-review/recording-worktree/Cargo.toml --target-dir target --locked --target wasm32-unknown-unknown --lib
```

Evidence: `target/game-assets-review/0002-native-tests.log`,
`0002-virtual-tests.log` and `0002-wasm-check.log` in the same directory. Formatting
and diff checks pass. These focused results supplement the earlier full accumulated
suite runs; those earlier counts exclude the two newly added integration tests.
Browser runtime and Linux remain unverified. Metadata checks do not provide a
filesystem transaction or detect every external edit between observations.

The feature-gated `model::run_path_recorded` helper and asset provenance consumers
remain in the later asset patch; including them here would unnecessarily couple
this change to the experimental API. No dependency, Cargo feature or CLI change is
part of patch 0002. No commit or external submission has been made.

## Patch 0003: Introduce the optional derived-mesh foundation

Proposed title: **Add an opt-in derived mesh with validation and binding stamps**

Add `game-assets = []` and expose `asset` only with that feature. The isolated
module contains `Mesh`, `BindingStamp`, CAD tessellation, complete-attribute
validation and conservative geometry/normal binding checks. Render coordinates
remain millimetres, right-handed Z-up; destination conversion belongs to later
export adapters. Tessellation borrows the CAD solid and retains separate hard-edge
normal tuples. This patch introduces no dependencies or CLI/format dispatch changes.

Local artifact: `target/game-assets-review/0003-derived-mesh-foundation.patch`.
Base: `33653aa19321ac6760b7f3aefa1f779030415e0a`; independent of patches 0001/0002.
SHA-256: `851AB6D6EB96B8D8B9AE41BFF1A711A08C6057D7FFBE057B5F3D0F5941CAA073`.
Files: `Cargo.toml`, `src/lib.rs`, `src/asset.rs`; 308 insertions across three files.
Isolated checkout: `target/game-assets-review/mesh-worktree`, detached at the base.
The asset module is extracted from the working tree with only its 25 child-module
declarations removed; mesh implementation and all four mesh tests are unchanged.
The Cargo change adds only the empty opt-in feature, not examples or dependencies.

Forward application passed against baseline paths in the recorder checkout (that
checkout's edits touch disjoint files). All 37 isolated release library tests pass,
including the four mesh tests. Those cover invalid/incomplete attributes,
geometry/order/normal binding changes, UV/tangent edits and negative zero, and
CAD tessellation units/hard edges/unchanged solid volume. Native library compilation
with features disabled and WASM library compilation with the feature enabled pass.

```sh
cargo test --manifest-path target/game-assets-review/mesh-worktree/Cargo.toml --target-dir target --release --locked --features game-assets --lib
cargo check --manifest-path target/game-assets-review/mesh-worktree/Cargo.toml --target-dir target --locked --no-default-features --lib
cargo check --manifest-path target/game-assets-review/mesh-worktree/Cargo.toml --target-dir target --locked --target wasm32-unknown-unknown --features game-assets --lib
```

Logs: `target/game-assets-review/0003-native-tests.log`, `0003-default-check.log`
and `0003-wasm-check.log` in the same directory. Diff whitespace checks pass.
The lockfile is unchanged. Scene/material types, UV/normal preparation, animation,
authoring, model dependency exposure and export remain later review units. This is
an experimental API proposal, not a stable-schema decision or completed M1. No
commit or external submission has been made; runtime/engine validation remains open.

## Patch 0004: Portable material values and declared image inputs

Proposed title: **Add portable PBR material values and explicit texture provenance**

Prerequisite: patch 0003 applied to the baseline. This patch adds the existing
`material.rs` and `texture_input.rs` implementations unchanged, their two module
declarations, and the original two 2-by-2 PNG fixtures with provenance README.
It covers linear material factors, sRGB versus numeric map semantics, scalar
channel packing, normal-Y conversion, alpha/sampler values, unsupported finish
diagnostics, project-relative image inputs, original hashes and declared licenses.
It adds no dependency, CLI or exporter. Scene types need these material contracts.

Artifact: `target/game-assets-review/0004-portable-material-inputs.patch`.
SHA-256: `44331545A2291179B2A0B4A9C46CB8C1633B6059D9E46F003EFB1A33B93C1339`.
Six files: 621 text insertions plus two 133-byte PNGs. Isolated checkout:
`target/game-assets-review/material-worktree`. Patch 0003 is staged as its
prerequisite; the unstaged diff is patch 0004. No commit was created. The original
implementation files match the main working tree byte-for-byte.

All 41 release library tests pass in this isolated configuration. The four added
tests cover normal conversion/channel packing and source preservation; invalid
material/map values; CAD finish conversion/unsupported fields; and image budgets,
project boundaries and required provenance. WASM library compilation passes.
Logs: `target/game-assets-review/0004-native-tests.log` and `0004-wasm-check.log`.
The patch applied successfully with its prerequisite when preparing patch 0005.
Binary fixtures are original diagnostic patterns, not downloaded artwork; keep
their license/provenance README with the patch. No engine appearance claim follows
from these library results.

## Patch 0005: Scene transforms and sampled clip representation

Proposed title: **Add validated asset scenes and sampled transform clips**

Prerequisites: patches 0003 and 0004 applied in order. `scene.rs` and `animation.rs`
form one review unit because scenes contain clips and clip sampling uses scene
transforms. Their files and all existing tests are copied unchanged. The patch
introduces local TRS transforms, hierarchy/shared-mesh instances, material slots,
placement decomposition, STEP/LINEAR property tracks, shortest-path quaternion
sampling and loop/clamp behavior. It does not add authoring syntax, constraint
baking, skinning or format-specific export.

Artifact: `target/game-assets-review/0005-scene-transforms-and-clips.patch`.
SHA-256: `C0805DF933C874C3637C3B58225B77DF240A98F4348D94985C4430E971CC4FD1`.
Three files, 670 insertions. Isolated checkout:
`target/game-assets-review/scene-worktree`, with prerequisites staged and patch
0005 unstaged. Forward application also passes against the material checkout.

All 46 isolated release library tests pass. The five new scene/animation tests
cover placement decomposition including reflections/half-turns, hierarchy and
transform validity, shared geometry, interpolation and loop bounds, and malformed
or singular animation input. WASM compilation with the feature enabled and native
library compilation with it disabled pass. Logs:
`target/game-assets-review/0005-native-tests.log`, `0005-wasm-check.log` and
`0005-default-check.log`. Both new patch diffs pass whitespace checks. These remain
experimental API proposals; engine pose/appearance acceptance and runtime browser
validation are not established. Nothing was committed or submitted.

For each checkout above, reproduce library tests with:

```sh
cargo test --manifest-path target/game-assets-review/material-worktree/Cargo.toml --target-dir target --release --locked --features game-assets --lib
cargo test --manifest-path target/game-assets-review/scene-worktree/Cargo.toml --target-dir target --release --locked --features game-assets --lib
cargo check --manifest-path target/game-assets-review/scene-worktree/Cargo.toml --target-dir target --locked --target wasm32-unknown-unknown --features game-assets --lib
cargo check --manifest-path target/game-assets-review/scene-worktree/Cargo.toml --target-dir target --locked --no-default-features --lib
```

## Patch 0006: Prepare UVs, tangent frames and normal/triangle topology

Proposed title: **Add explicit render-mesh preparation before asset binding**

Apply after patches 0003, 0004 and 0005 in order. The runtime mesh operations only
need the mesh foundation, but the canonical patch context follows the scene chain
so that module declarations apply cleanly in the review sequence. `uv.rs`,
`normals.rs`, `refine.rs` and `slice.rs` are copied byte-for-byte from the main
working tree, including all ten existing tests. The five-file patch adds box
projection, derivative tangent generation, crease-angle normals, uniform
subdivision and explicit plane slicing. Operations return derived meshes and
preserve CAD solids; preparation precedes binding. No authoring or CLI is added.

Artifact: `target/game-assets-review/0006-mesh-preparation.patch`.
SHA-256: `FBE3576FBA48757DBCF16D84F8C4CBA6DAB5A9F18D972CCAE6BFC80A140CB700`.
Five files, 756 insertions. Isolated checkout:
`target/game-assets-review/preparation-worktree`, with patches 0003–0005 staged
as prerequisites and patch 0006 unstaged. All 56 release library tests pass in
that complete configuration. Feature-enabled WASM library compilation passes.
Forward application succeeds against the scene checkout; diff checks pass.

Tests cover UV chart splits and tangent frames, invalid UV inputs, hard/smooth
normal fans and topology contacts, subdivision budgets and geometry preservation,
slice bounds and plane intersection behavior. These are the existing tests, not
substitute smoke tests. Tangents remain the current derivative implementation,
not certified MikkTSpace; the patch does not make engine appearance claims.

```sh
cargo test --manifest-path target/game-assets-review/preparation-worktree/Cargo.toml --target-dir target --release --locked --features game-assets --lib
cargo check --manifest-path target/game-assets-review/preparation-worktree/Cargo.toml --target-dir target --locked --target wasm32-unknown-unknown --features game-assets --lib
```

Evidence: `target/game-assets-review/0006-native-tests.log` and
`0006-wasm-check.log`. The initial mesh-only draft passed 47 library tests but
failed sequential application after the scene patch because both edited module
declarations. That draft is retained as `0006-initial-mesh-only-draft.patch`, with
its separate test log, for traceability; it is not the canonical review artifact.
The corrected patch above was retested after changing the prerequisite context.

The GLB module currently references skinning and both root-motion representations;
its existing tests also cover those paths. It cannot yet be advertised as an
isolated static-export patch without deliberately separating those dependencies.
Preserve their coverage in the later review units. Nothing was committed
or submitted, and no new runtime behavior was introduced in the main working tree.

## Patch 0007: Source-bound mesh provenance

Proposed title: **Guard mesh bindings with consumed CAD input provenance**

Prerequisites: patches 0002–0006, applied in order. Add project-relative input
identifiers, consumed content hashes and existence observations, versioned binding
records, and the feature-gated `model::run_path_recorded` helper. The helper gathers
dependencies reached through cached parts and worker evaluations. The provenance
implementation is unchanged except that two native test fixtures now create their
temporary parent directories, fixing failures in a fresh isolated checkout.

Artifact: `target/game-assets-review/0007-source-binding-provenance.patch`.
SHA-256: `1275DAEA12B72C9318E618501D53AB42DE46602641D296F94F5D65781C9DACE5`.
Three files, 529 insertions. Isolated checkout:
`target/game-assets-review/provenance-worktree`, with prerequisites staged. All 65
release library tests plus both input-recording integration tests pass. The six
provenance tests include conflicting reads/existence, consumed-byte capture, repeated
part dependencies, binding changes and portable path/version validation. The initial
fixture failures and draft are retained as `0007-initial-tests.log` and
`0007-initial-draft.patch`; the canonical patch includes the fixture correction.

Evidence: `target/game-assets-review/0007-native-tests.log` and the earlier
compile check `0007-wasm-check.log`. The complete patch chain through 0008 also
passes a fresh WASM compile after package-cache invalidation. No CLI or authoring
syntax is part of this patch. Native cache metadata is not a filesystem transaction.

## Patch 0008: Explicit skin binding and CPU deformation

Proposed title: **Add explicit skin weights, bind poses and CPU deformation**

Apply after patch 0007. The existing `skin.rs` file and seven tests are copied
unchanged, with one module declaration. This introduces rigid/two-joint/chain
assignment, four-slot weights, explicit inverse binds, persisted source/rig/geometry
guards, CPU position deformation, and inverse-transpose shading frames. It has no
GLB dependency and does not introduce automatic weighting or engine integration.

Artifact: `target/game-assets-review/0008-explicit-skin-binding.patch`.
SHA-256: `EE41C9AB6FEE10E4877874C47273F137D152EACF7195D1FF9EA5A607AF345100`.
Two files, 871 insertions. Isolated checkout:
`target/game-assets-review/skin-worktree`, with corrected prerequisites staged.
Forward application passes against the provenance checkout. A fresh release build
passes 72 library tests and two input-recording tests. The log explicitly contains
all seven skin test names, including analytic bend/rest recovery, invalid weights,
rig/geometry persistence, chain knots and reflected/singular shading transforms.
Fresh feature-enabled WASM compilation also passes; feature-disabled native library
compilation passed. Logs: `target/game-assets-review/0008-native-tests.log`,
`0008-wasm-check.log`, `0008-default-check.log`.

### Shared build-cache caveat

Switching these same-name/version worktrees through one Cargo target directory
reused a provenance test binary during an attempted skin run: 65 tests ran instead
of the expected 72. That result is invalid skin evidence and is retained only as
`0008-stale-cache-tests.log`. Touching the crate root did not force recompilation.
The verified skin run followed package-scoped release cache cleaning; the WASM run
also followed package-scoped cache cleaning for its target. Only generated files
under the verified repository `target/` directory were removed. Source, patches,
logs and dependency downloads were preserved.

Use fresh target directories for independent review, or invalidate the package
cache before switching stages when sharing one. Verify the expected test names,
not merely command exit status. Reproduction for the current skin checkout:

```sh
cargo clean --manifest-path target/game-assets-review/skin-worktree/Cargo.toml --target-dir target --release -p gcad
cargo test --manifest-path target/game-assets-review/skin-worktree/Cargo.toml --target-dir target --release --locked --features game-assets --lib --test input_recording
cargo clean --manifest-path target/game-assets-review/skin-worktree/Cargo.toml --target-dir target --target wasm32-unknown-unknown -p gcad
cargo check --manifest-path target/game-assets-review/skin-worktree/Cargo.toml --target-dir target --locked --target wasm32-unknown-unknown --features game-assets --lib
```

The earlier stage logs contain their stage-specific tests/counts; they do not replace
the fresh skin result. Neither patch establishes target-engine poses or browser
runtime behavior. Nothing was committed or submitted.

## Patch 0009: Static, animated and skinned GLB encoding

Proposed title: **Encode validated asset scenes and explicit skins as GLB**

Prerequisites: patches 0002–0008 in order. The patch adds the existing GLB encoding
core and its entire six-test module, plus one module declaration. It includes
millimetre/Z-up to metre/Y-up conversion, aligned buffers/accessors, material and
PNG bindings, shared scene instances, animation channels, explicit skin attributes
and converted inverse binds. It rejects unsupported/invalid material data and
float32 collapse rather than producing an apparently valid but damaged export.

Artifact: `target/game-assets-review/0009-glb-scene-export.patch`.
SHA-256: `6924E70B9A60CC1F55572A5DC021DC7F1A1AB450DD52FF0E50AB4A52EF38DCFA`.
Two files, 729 insertions. Isolated checkout:
`target/game-assets-review/glb-worktree`, with prerequisites staged. Forward
application passes against the skin checkout. After package-scoped cache
invalidation, all 78 release library tests and both input-recording tests pass.
The log explicitly includes all six GLB test names. Fresh feature-enabled WASM
compilation passes, and the isolated diff has no whitespace errors. Logs:
`target/game-assets-review/0009-native-tests.log`, `0009-wasm-check.log`.

Extraction removes only the contiguous two root-motion export wrappers from the
isolated copy, from the translation-root wrapper documentation to immediately
before `encode_with_extras`. The full working tree retains those functions.
Everything else in `glb.rs`, including its complete test module, is unchanged.
The first source-comparison command selected a test-only import at the top of the
file; comparing the actual `mod tests` section confirms byte-identical coverage.

Root-motion types and their tests belong to a later review unit. The rigid-motion
test fixture currently uses authored `.gasset` loading, so that unit follows
authoring rather than silently omitting its tests. This ordering is a review split,
not removal of root-motion support from the implemented branch. No CLI, runnable
example, engine adapter or production FBX backend is introduced in patch 0009.

```sh
cargo clean --manifest-path target/game-assets-review/glb-worktree/Cargo.toml --target-dir target --release -p gcad
cargo test --manifest-path target/game-assets-review/glb-worktree/Cargo.toml --target-dir target --release --locked --features game-assets --lib --test input_recording
cargo clean --manifest-path target/game-assets-review/glb-worktree/Cargo.toml --target-dir target --target wasm32-unknown-unknown -p gcad
cargo check --manifest-path target/game-assets-review/glb-worktree/Cargo.toml --target-dir target --locked --target wasm32-unknown-unknown --features game-assets --lib
```

Existing independent import/preview evidence remains recorded for the full branch;
the isolated patch evidence above is library validation and compilation, not new
engine/runtime acceptance. Nothing was committed or submitted.

## Patch 0010: Bake procedural materials through the shared CPU generator

Proposed title: **Bake CAD finish patterns into portable material maps**

Apply patches 0001–0009 first. Unlike the earlier representation patches, this
patch requires the shared CPU generator from 0001. It adds `bake.rs` unchanged and
one module declaration: finish/material conversion, explicit box UV settings,
procedural colour tiles and generated normal maps. Original meshes and source maps
are retained; unsupported finish features still fail. No authoring/CLI is added.

Artifact: `target/game-assets-review/0010-procedural-material-baking.patch`.
SHA-256: `73F5DBFCE42721D936EA3FB48AABA62BD514BABB3D5C5EFFE2E07B91DD801EDA`.
Two files, 179 insertions. Isolated checkout:
`target/game-assets-review/bake-worktree`, with prerequisites staged. Fresh
package-scoped release rebuilding passes 82 library tests and both input-recording
tests. All four baking tests appear in the log: procedural patterns, explicit-image
precedence and unsupported finishes, flat/slope normal conventions, and untextured
finish UV settings. Fresh feature-enabled WASM compilation passes. The implementation
file matches the main working tree byte-for-byte.

Evidence: `target/game-assets-review/0010-native-tests.log` and
`0010-wasm-check.log`. The patch context applies after GLB; the complete prerequisite
chain, including 0001, was applied in the isolated checkout. CPU baking correctness
does not certify engine appearance or tangent compatibility.

## Patch 0011: Atlas packing and portable-map rebaking

Proposed title: **Pack UV charts and rebake portable material maps**

Apply after patch 0010. `atlas.rs` and its four tests are unchanged from the full
working tree, with one module declaration. The patch provides deterministic chart
packing with padding/density constraints, overlap diagnostics, and nearest/bilinear
map rebaking with colour-versus-numeric sampling rules. It preserves source geometry
and material factors. This is not general mesh retopology or chart-safe mip generation.

Artifact: `target/game-assets-review/0011-atlas-packing-and-rebaking.patch`.
SHA-256: `B25A9DA709F12A07D43A23F67EBE8A2C9AE85ADD32634DEA8518E7C7889286C0`.
Two files, 635 insertions. Isolated checkout:
`target/game-assets-review/atlas-worktree`, with prerequisites staged. Forward
application passes against the baking checkout. Fresh release rebuilding passes
86 library tests plus both input-recording tests. The log includes all four atlas
tests: chart packing, overlap rejection/shared edges, wrap/filter/colour-space
sampling, and all-map rebaking/padding with source preservation. Fresh feature-enabled
WASM compilation passes. Both patch diffs pass whitespace checks.

Evidence: `target/game-assets-review/0011-native-tests.log` and
`0011-wasm-check.log`. Use the commands below, substituting `bake-worktree` to
reproduce patch 0010. Cache invalidation is necessary when sharing the target
directory across these same-name/version package stages, as explained above.

```sh
cargo clean --manifest-path target/game-assets-review/atlas-worktree/Cargo.toml --target-dir target --release -p gcad
cargo test --manifest-path target/game-assets-review/atlas-worktree/Cargo.toml --target-dir target --release --locked --features game-assets --lib --test input_recording
cargo clean --manifest-path target/game-assets-review/atlas-worktree/Cargo.toml --target-dir target --target wasm32-unknown-unknown -p gcad
cargo check --manifest-path target/game-assets-review/atlas-worktree/Cargo.toml --target-dir target --locked --target wasm32-unknown-unknown --features game-assets --lib
```

Neither patch establishes Linux/browser runtime or target-engine acceptance.
Source files and tests were not changed to make these isolated runs pass.
No commit or external submission has been made.

## Patch 0012: Asset authoring, constrained motion and surface settings

Proposed title: **Load versioned asset documents with CAD sources and motion tracks**

Apply patches 0001-0011 in order first. This unit groups `author.rs`, `motion.rs`
and `surface_settings.rs`: authoring calls motion sampling, and the motion tests
exercise authored CAD assemblies. Nine required asset fixtures accompany the
modules; their CAD sources are already in the baseline and map images arrive in
0004. All 15 tests from these three modules are retained. No CLI dispatch is added.

Artifact: `target/game-assets-review/0012-asset-authoring-and-motion.patch`.
SHA-256: `B281207A4F8C1FBE8F011E9419281500AAF1935C2473602651E4F7E18F5E8CDE`.
Fourteen files, 2540 insertions and one deletion. Isolated checkout:
`target/game-assets-review/author-worktree`, with prerequisite patches staged.
Forward application against the atlas checkout and whitespace checks pass.

A fresh package rebuild initially passed 98 tests but failed three authoring tests:
their temporary-directory setup assumed a checkout-local `target` parent existed.
Those tests now use `create_dir_all` in both the main tree and isolated checkout.
No production authoring logic changed. Preserve `0012-authoring-initial-draft.patch`
and `0012-initial-tests.log` as the original evidence. The unit also carries the
existing test-only viewer progress-listener correction: scope the listener to its
simulation and assert that unrelated thread progress is absent. This prevents
concurrent CAD evaluations from contaminating that test's expected progress rows.

The corrected run passes **101 library tests and two input-recording integration
tests**, including all 11 authoring, one constrained-motion and three surface tests.
A fresh feature-enabled WASM library check passes. The three implementation files
match the main working tree byte-for-byte. Cargo.lock remains unchanged.

Evidence: `target/game-assets-review/0012-native-tests.log` and
`0012-wasm-check.log`. Reproduce using the 0011 commands above with
`author-worktree` substituted for `atlas-worktree`. The initial package clean
invalidated the shared cache; the corrected run rebuilt the changed author module.
The WASM package was separately cleaned before its check.

These checks cover the isolated library chain, not the full integration suite,
Linux/browser runtime or target-engine acceptance. The previously recorded full
suite still has five baseline failures. No commit or submission was made.

## Patch 0013: Translation and rigid root-motion extraction

Proposed title: **Extract root motion while preserving original animation clips**

Apply patches 0001-0012 first. This unit adds `root_motion.rs`, `rigid_motion.rs`
and their eight existing tests, the authored plate fixture, and the two GLB
wrappers deferred from 0009. The complete GLB file now matches the main working
tree. Translation extraction selects axes; rigid extraction preserves the root
pivot and composes translation/rotation trajectories, including reverse loops.
Original clips remain available alongside the derived in-place clip.

Artifact: `target/game-assets-review/0013-root-motion-extraction.patch`.
SHA-256: `324A57DC524178E5F3438A556A42FC96E8AD9FD70FE9E508D694DA9D26F37BE5`.
Five files, 821 insertions. Isolated checkout:
`target/game-assets-review/root-motion-worktree`. Prerequisites are staged;
forward application after 0012 and whitespace checks pass. The two modules and
complete GLB encoder match the main working tree byte-for-byte.

Fresh release validation passes **109 library tests and two input-recording tests**.
All three translation and five rigid tests ran, covering recomposition, source
preservation, reverse loops, invalid records, pure-rotation pivots, skin export
and GLB trajectory metadata. A fresh feature-enabled WASM library check passes.
Evidence: `target/game-assets-review/0013-native-tests.log` and
`0013-wasm-check.log`. Reproduce with the earlier package-clean/build commands,
substituting `root-motion-worktree` as the manifest directory.

No implementation or test changes were needed for this extraction. The metadata
still needs a consumer adapter; library validation does not establish engine root
motion playback. No commit or external submission was made.

## Patch 0014: Sampled diagnostics and GLB capability reporting

Proposed title: **Report asset costs, sampled deformation and actual GLB encoding results**

Apply patches 0001-0013 first. This unit adds unchanged `report.rs` and
`capabilities.rs` modules and their four tests. Reports count shared/instanced
geometry, source RGBA bytes, animation keys and skin influences, and sample
geometry/shading behavior. Capability inspection invokes the actual encoder in
memory and reports destination counts or its first failure. It explicitly retains
experimental status and an unverified engine flag. CLI dispatch comes later.

Artifact: `target/game-assets-review/0014-asset-diagnostics.patch`.
SHA-256: `81D7A46EA6F7773804FCDC39F9869ADE3C582C550E567C8A84DFE97911AF15EF`.
Three files, 438 insertions. Isolated checkout:
`target/game-assets-review/report-worktree`, with prerequisites staged. Forward
application after 0013, whitespace checks and byte comparisons of both modules
against the main tree pass.

Fresh release validation passes **113 library tests and two input-recording tests**.
The four module tests cover instance counting with off-grid STEP sampling,
material image packing and missing UV rejection, skin mesh variants, and encoder
failures without partial success. A fresh feature-enabled WASM library check passes.
Evidence: `target/game-assets-review/0014-native-tests.log` and
`0014-wasm-check.log`. Reproduce using the earlier package-clean/build commands
with `report-worktree` as the manifest directory.

Sampled diagnostics are not continuous error bounds or quality acceptance; encoder
success does not certify engine rendering. No implementation changes, commits or
external submissions were made for this review unit.

## Patch 0015: Sampled clip reduction

Proposed title: **Reduce animation keys within sampled deformation limits**

Apply patches 0001-0014 first. This unit adds unchanged `reduce.rs` and its two
existing tests, plus one module declaration. It removes eligible interior LINEAR
keys only when sampled mesh positions, attachment frames and shading directions
remain within explicit tolerances. Endpoints and STEP tracks are retained; trial
and reference-size limits bound work. The source clip remains unchanged.

Artifact: `target/game-assets-review/0015-sampled-clip-reduction.patch`.
SHA-256: `299D6FF429F0F3A846C49F657194A97BB84109B782BBC192624BD831F8531DC4`.
Two files, 390 insertions. Isolated checkout:
`target/game-assets-review/reduce-worktree`, with prerequisites staged. Forward
application after 0014 and whitespace checks pass. The implementation matches the
main working tree byte-for-byte.

Fresh release validation passes **115 library tests and two input-recording tests**.
Both reduction tests ran: endpoint/STEP/reference preservation with trial limits,
and rejection based on skinned vertices or meshless-node orientation. Fresh
feature-enabled WASM compilation passes. Evidence:
`target/game-assets-review/0015-native-tests.log` and `0015-wasm-check.log`.
Reproduce with the earlier package-clean/build commands and `reduce-worktree`.

This review step does not prove continuous-time error bounds or engine playback.
No implementation changes, commits or external submissions were made.

## Patch 0016: Surface-distance and silhouette metrics

Proposed title: **Measure sampled surface distance and orthographic coverage changes**

Apply patches 0001-0015 first. This unit adds unchanged `surface.rs` and
`silhouette.rs` with all three existing tests and two module declarations.
Surface comparisons use bidirectional closest-triangle distances at vertices,
edge midpoints and triangle centroids, reporting maximum, p95 and RMS statistics.
Silhouette comparisons rasterise opaque double-sided geometry along three
principal axes at an explicit resolution and report symmetric-difference coverage.
These are prerequisites for later LOD acceptance gates; no CLI is added here.

Artifact: `target/game-assets-review/0016-surface-and-silhouette-metrics.patch`.
SHA-256: `53943037804D1691BE5FC534DB9A45ECF8C766C6D34DBA1A0123DC88719DD3E7`.
Three files, 405 insertions. Isolated checkout:
`target/game-assets-review/metrics-worktree`, with prerequisites staged. Forward
application after 0015 and whitespace checks pass. Both modules match the main
working tree byte-for-byte.

Fresh release validation passes **118 library tests and two input-recording tests**.
All three metric tests ran: analytic planes/missing surface/retriangulation,
spatial-tree versus exhaustive triangle-distance search, and identical/missing/
displaced silhouette coverage. Fresh feature-enabled WASM compilation passes.
Evidence: `target/game-assets-review/0016-native-tests.log` and
`0016-wasm-check.log`. Reproduce using the earlier package-clean/build commands
with `metrics-worktree` as the manifest directory.

The statistics are not area-weighted surface percentiles or continuous Hausdorff
bounds. Pixel-centre coverage is not a continuous contour bound or a target-engine
camera profile. No implementation changes, commits or external submissions were
made for this review unit.

## Patch 0017: High-to-low normal-map projection

Proposed title: **Bake tangent-space normals from source geometry onto derived meshes**

Apply patches 0001-0016 first. This unit adds unchanged `projection.rs`, all nine
existing tests, two CAD plate asset fixtures and one module declaration. CPU
projection searches along both signs of the target normal in a shared millimetre
frame, applies distance/normal/miss limits, and distinguishes chart coverage,
misses, hits and padding. Scene application preserves shared instances and skins.

Artifact: `target/game-assets-review/0017-normal-map-projection.patch`.
SHA-256: `F2DC4BBC93BE7C468D0FD4D3D3C0120203722991C2AD5DFBE9D2A85BB3255E14`.
Four files, 980 insertions. Isolated checkout:
`target/game-assets-review/projection-worktree`, with prerequisites staged.
Forward application after 0016 and whitespace checks pass. The module and both
fixtures match the main working tree byte-for-byte.

Fresh release validation passes **127 library tests and two input-recording tests**.
All nine projection tests ran: CAD identity projection; slope/mirrored tangent
frames and source preservation; explicit misses/overlap rejection; coverage and
invalid options; bounded padding; shared-instance/skin placement; nearest-surface
normal rejection; coincident-surface selection; and spatial-tree versus exhaustive
intersection search. Fresh feature-enabled WASM compilation passes. Evidence:
`target/game-assets-review/0017-native-tests.log` and `0017-wasm-check.log`.
Reproduce with the earlier package-clean/build commands and `projection-worktree`.

This is pixel-centre geometry-normal projection, without supersampling, a cage,
source normal-map detail transfer or general material-map transfer. Target-engine
shading and tangent compatibility remain unverified. No implementation changes,
commits or external submissions were made for this review unit.

## Patch 0018: Pose-aware endpoint-collapse LOD

Proposed title: **Derive LOD meshes with explicit remapping and sampled acceptance gates**

Apply patches 0001-0017 first. This unit adds unchanged `lod.rs`, all four existing
tests and one module declaration. Endpoint collapse retains complete vertex tuples,
locks boundaries/attachments and checks cumulative correspondence error across
supplied poses. Scene derivation remaps explicit skin weights and reports sampled
surface, silhouette and shading changes, with optional acceptance limits. Rejection
witness caching avoids reevaluating candidates with unchanged rejection evidence.

Artifact: `target/game-assets-review/0018-pose-aware-lod.patch`.
SHA-256: `E6E36C9DBD99069EF121F5A3C4DC3AC090D7163134AAEF6175810AD0CC4BEA24`.
Two files, 1062 insertions. Isolated checkout:
`target/game-assets-review/lod-worktree`, with prerequisites staged. Forward
application after 0017 and whitespace checks pass. The LOD implementation matches
the main working tree byte-for-byte. All asset submodules match in content after
normalising Git checkout CRLF/LF differences.

Fresh release validation passes **131 library tests and two input-recording tests**.
All four LOD tests ran: acceptance-limit rejection, boundary/attachment/attribute
and cumulative pose preservation, cached versus uncached search, and remapped skin
deformation with stale-binding rejection, projected normals and GLB encoding.
Fresh feature-enabled WASM compilation passes. Evidence:
`target/game-assets-review/0018-native-tests.log` and `0018-wasm-check.log`.
Reproduce with the earlier package-clean/build commands and `lod-worktree`.

The library module series is now packaged; CLI/viewer integration remains separate.
These checks do not establish representative engine-camera budgets, continuous
motion/surface acceptance or production asset quality. No implementation changes,
commits or external submissions were made for this review unit.

## Patch 0019: Native asset preview adapter

Proposed title: **Preview derived assets through the existing native renderer**

Apply patches 0001-0018 first. This unit adds unchanged `view/asset.rs` and its two
existing tests, plus native-only feature-gated module exposure. It builds renderer
frames from authored scenes and sampled clips, converts portable material maps,
provides playback controls and software snapshots, and reports known preview/export
differences. CLI dispatch follows separately.

Artifact: `target/game-assets-review/0019-native-asset-preview.patch`.
SHA-256: `FDCECD0781F0AEDA298607707BE9B6EB013612117B5AFFB6A13EECC2887C11BB`.
Two files, 522 insertions. Isolated checkout:
`target/game-assets-review/preview-worktree`, with prerequisites staged. Forward
application after 0018 and whitespace checks pass. Both files match the main tree
byte-for-byte.

Fresh release validation passes **133 library tests and two input-recording tests**.
Both preview tests ran: material tint/normal convention/numeric-map packing with
source preservation and snapshot generation; and instanced geometry with sampled
clip positions and invalid-clip rejection. The generated 960x720 software snapshot
at `preview-worktree/target/game-asset-textured-preview.png` was visually inspected:
a blue textured four-hole plate is visible with intact geometry. This is software
rendering evidence, not GPU material parity.

Fresh feature-enabled WASM library compilation passes; the native-only preview
module is excluded on WASM. Evidence: `target/game-assets-review/0019-native-tests.log`
and `0019-wasm-check.log`. Reproduce with the earlier package-clean/build commands
and `preview-worktree` as the manifest directory.

GPU controls and appearance remain unverified. Known differences include generated
tangent frames, double-sided preview, incomplete alpha/sampler support and omitted
occlusion/emissive maps. No implementation changes, commits or external submissions
were made for this review unit.

## Patch 0020: Feature-gated asset CLI dispatch

Proposed title: **Expose experimental asset workflows through the CLI**

Apply patches 0001-0019 first. This unit adds the existing 581 lines in `main.rs`:
eight asset subcommands and `.gasset` check/export dispatch. Native preview remains
platform-gated; the whole asset command set is feature-gated. The file matches the
main working tree byte-for-byte. No library logic or dependencies change.

Artifact: `target/game-assets-review/0020-asset-cli-dispatch.patch`.
SHA-256: `DC8EB43FE4DA17DFBF5EEC19FA1C4AFAC6D36D489D5CA55F60492D6641183C31`.
One file, 581 insertions. Isolated checkout:
`target/game-assets-review/cli-worktree`, with prerequisites staged. Forward
application after 0019 and whitespace checks pass.

A package-clean release build of the feature-enabled executable passes, as does a
feature-disabled release CLI compile check. Evidence:
`target/game-assets-review/0020-build.log` and `0020-default-check.log`.

```sh
cargo clean --manifest-path target/game-assets-review/cli-worktree/Cargo.toml --target-dir target --release -p gcad
cargo build --manifest-path target/game-assets-review/cli-worktree/Cargo.toml --target-dir target --release --locked --features game-assets --bin gcad
cargo check --manifest-path target/game-assets-review/cli-worktree/Cargo.toml --target-dir target --release --locked --bin gcad
```

The fresh executable successfully ran the following smoke checks from the isolated
checkout. Outputs and separate stdout/stderr logs are preserved under its
`target/cli-smoke` directory, using the check names in the first column.

| Check | Input and options | Evidence/limits |
| --- | --- | --- |
| check/export | `plate.gasset` | Valid asset; static GLB created |
| capabilities/report | `plate.gasset`, report `--shading` | Both JSON outputs parse; exportable true and engine_verified false |
| preview | `plate.gasset`, software snapshot | 960x720 four-hole plate visually inspected; no GPU claim |
| reduce | `plate-motion.gasset`, clip slide, max-error/max-angle 0.01 | New GLB; original two keys retained |
| root | `plate-root-motion.gasset`, node root, clip travel, in-place name, rigid | New GLB contains rigid root-motion metadata |
| bake | `plate-atlas.gasset` from `plate.gasset`, node plate, 64x64, distance 0.1 | 55 covered/hit pixels, zero misses, 115 padded pixels |
| lod | `plate.gasset`, node plate, triangles 1200, error 0.1, UV error 0.01, angle 5, trials 20 | Valid output; 1276 triangles retained, target_reached false |
| bind | Generated project-relative plate/bone document, joints bone | New versioned JSON weights with source bindings |
| reject | Repeat reduce to its existing output | Nonzero exit and unchanged destination SHA-256 |

The binding smoke document is preserved as `target/cli-smoke/bind.gasset`; all other
fixtures are under `examples/game-assets` in preceding patches. All five emitted
GLBs have valid magic/version/declared length and parseable glTF 2.0 JSON. These
container checks are not independent engine imports. The LOD and reduction smoke
cases exercise dispatch, not successful simplification; earlier module tests cover
actual reduction. Derived-output rejection was exercised for key reduction only.
The full integration regression suite was not rerun for this unchanged CLI extraction.
No implementation changes, commits or external submissions were made.

## Patch 0021: Rust fixture generators and debug entries

Proposed title: **Provide reproducible GLB fixtures and geometry-only normal references**

Apply patches 0001-0020 first. This unit adds the unchanged `game_asset_glb` and
`game_asset_normals_reference` Rust examples and their Cargo required-feature gates.
It also introduces the three Cargo build tasks and 12 matching Rust launch entries
from the main VS Code configuration. Native import-probe tasks/launches are deferred
to the tools unit; configuration serialization differs, but selected entries retain
their original fields. No configuration in the main working tree was changed.

Artifact: `target/game-assets-review/0021-rust-fixture-generators.patch`.
SHA-256: `E4D624A02D3FE3EB582D45B67F4341E082617F92511408D8CAF5F38C3E11945A`.
Five files, 656 insertions. Isolated checkout:
`target/game-assets-review/examples-worktree`, with prerequisites staged. Forward
application after 0020 and whitespace checks pass. Cargo.toml and both Rust examples
match the main working tree byte-for-byte. All selected launch tasks and referenced
asset fixture paths resolve in this checkout.

Fresh package-clean Release and Debug builds pass. All seven GLB fixture modes
(static, rich, scene, baked, atlas, normal-bake and skin) ran in Release, producing
valid GLB headers/lengths and parseable glTF 2.0 JSON. The geometry reference contains
1276 triangles in millimetres without expected normals; repeat output creation fails
and leaves its hash unchanged. All three Debug executables ran (CLI plate check,
static GLB example and crease reference). Debug and Release reference JSON hashes
match. These checks do not repeat independent engine/import-probe validation.

Evidence: `target/game-assets-review/0021-build.log`, `0021-debug-build.log` and
`examples-worktree/target/fixture-smoke/` (outputs and separate stdout/stderr).
Commands, after package-scoped cache invalidation:

```sh
cargo build --manifest-path target/game-assets-review/examples-worktree/Cargo.toml --target-dir target --release --locked --features game-assets --example game_asset_glb --example game_asset_normals_reference
cargo build --manifest-path target/game-assets-review/examples-worktree/Cargo.toml --target-dir target --locked --features game-assets --bin gcad --example game_asset_glb --example game_asset_normals_reference
```

Launch entries use ordinary repository-local Debug build paths. Native debugger
attachment and interactive preview were not exercised. No implementation changes,
commits or external submissions were made for this review unit.

## Patch 0022: Remaining authored fixtures and repeatable preparation

Proposed title: **Provide authored deformation fixtures with explicit weight preparation**

Artifact: `target/game-assets-review/0022-authored-fixtures.patch`.
SHA-256: `7FC89C391C20AC1C5DF60D99D9A4065BBE0A09F266316C6337619AEB60AE3A2B`.
Fifteen files, 299 insertions. Isolated checkout:
`target/game-assets-review/fixtures-worktree`, with prerequisites staged. Forward
application after 0021 and whitespace checks pass. Fixtures, README and preparation
script match the main working tree byte-for-byte. Generated weights stay in ignored
`target` storage and are excluded from the patch.

Using the Debug CLI verified in 0021, a clean run generated all nine weight resources
and successfully checked all **25 asset fixtures**. A second run preserved all nine
resources and checked all 25 again; SHA-256 comparisons confirm no weight changed.
No Rust rebuild was required for this fixture/script-only unit.

Evidence: `target/game-assets-review/0022-prepare.log`, `0022-repeat.log` and
`0022-weight-hashes-before.json`. The new preparation script and README also exist
in the main tree. Existing stale resources are never automatically rebound; the
subsequent check fails and requires deliberate correction. The stale-resource
behavior relies on the already-tested authoring guards, not a new corruption test.
Passing `check` establishes authored input/binding validity, not production-quality
deformation, engine import or rendering. No commits or external submissions were made.

## Patch 0023: Native import and FBX feasibility tools

Proposed title: **Provide independent native import probes for asset fixtures**

Artifact: `target/game-assets-review/0023-native-import-validation-tools.patch`.
SHA-256: `1811635C40E65970048461B770647F636DD7B4DD718077161CC29ADF05874AF2`.
Seventeen files, 2823 insertions and 231 deletions. Isolated checkout:
`target/game-assets-review/tools-worktree`, with prerequisites staged. Forward
application after 0022 and whitespace checks pass. Every packaged tool source
matches the main working tree byte-for-byte.

Both native configurations independently imported all seven GLB modes generated
in 0021: static, rich, scene, baked, atlas, normal-bake and skin. Atlas uses
`rich.glb atlas.glb --atlas`; normal-bake uses `normal-bake.glb --normal-detail`.
An initial atlas invocation using only one file was rejected with usage text; its
log is retained, and corrected runs use `verified-` log prefixes. All 14 corrected
imports pass. Both configurations also ran the synthetic FBX static/skinned probe:
static geometry/material/texture-link checks pass, and sampled skin maximum error
is `2.00972e-14 cm` in Release. Outputs and logs are in
`target/game-assets-review/0023-probe-results`.

These are fixture-specific semantic probes, not general format validators or
engine renderers. This change did not repeat every authored-asset audit mode; their
earlier evidence remains in the design record. Known authored FBX PBR material
losses, backend selection and engine acceptance remain unresolved. No production
FBX writer is introduced, and nothing was committed or submitted.

## Patch 0024: Platform guards for stored-part test helpers

Proposed title: **Gate Unix inode test helpers on Unix targets**

This independent baseline patch needs none of the asset series. The two stored-part
invalidation tests already have `cfg(unix)`; their inode-dependent helper and
helper-only Path import/write function need matching guards so the integration
test target compiles on Windows. The cross-platform mesh-store test is unchanged.
No tests or production behavior are removed or changed.

Artifact: `target/game-assets-review/0024-stored-test-platform-guards.patch`.
SHA-256: `A6993C299B2B4944ACA34AC490450F078DF927334DB02985AAB44EC39F8E623F`.
One file, five insertions and one deletion. Isolated checkout:
`target/game-assets-review/stored-worktree`, directly on the original baseline.
The patch also applies after 0023. Whitespace checks pass.

After package-scoped cache invalidation, this command passes the one Windows
storage test, `stored_meshes_come_back_the_same`:

```sh
cargo test --manifest-path target/game-assets-review/stored-worktree/Cargo.toml --target-dir target --release --locked --test stored
```

Evidence: `target/game-assets-review/0024-stored-tests.log`. The two existing Unix
invalidation tests are not compiled or run on Windows; Linux execution remains
unverified. This change was already present in the main tree and is separated from
asset functionality for review. No commits or external submissions were made.

## Working-tree reconciliation after 0024

Comparing tracked modifications with the 0023 checkout leaves only README.md and
this test guard patch. All other modified tracked production files match after
CRLF/LF normalisation. The asset submodule files were reconciled in 0018; the root
module declaration ordering differs between the incremental series and main tree.
Documentation and the existing README plan paragraph still need packaging.
The plan/design/review records must retain the distinction between implementation,
local test evidence, engine acceptance and maintainer approval. Packaging does not
complete the broader asset/FBX/shader requirements.

## Patch 0025: Documentation snapshot and final source reconciliation

Proposed title: **Document the experimental asset workflows and validation limits**

Apply patches 0001-0024 first. This unit adds the guide, development plan, design
record and this review snapshot, preserves the existing README plan
paragraph, and adds the guide link. It reconciles `src/asset.rs` declaration order
with the main tree; module visibility and non-declaration source content are
unchanged. The records explicitly retain the incomplete engine, FBX, shader,
platform and quality-acceptance work.

Artifact: `target/game-assets-review/0025-documentation-and-reconciliation.patch`.
Its SHA-256 is recorded externally in `target/game-assets-review/series.sha256`,
alongside all 25 canonical patch hashes. It cannot be embedded in this document
because this document is part of the patch. The manifest excludes initial drafts.
Isolated checkout: `target/game-assets-review/documentation-worktree`.

The final assembly is checked for ordered patch application, whitespace errors,
relative Markdown link targets and normalized content parity with modified/new
working-tree files. Reconciliation evidence is retained in
`target/game-assets-review/0025-reconciliation.log`. Runtime evidence remains the
scoped tests and commands recorded above; documentation and declaration reordering
do not constitute another full regression run or engine acceptance.

This is a local snapshot, not a submitted PR or an approval to publish. Future
implementation or evidence changes require a new reviewed artifact. Broad scope
completion remains unproven: production FBX, shader authoring/adapters, engine
import/rendering/playback, accepted representative budgets and platform validation
are still outstanding. No commits or external submissions were made.

## Patch 0026: Opt-in sampled skin-weight quantisation

Proposed title: **Export UNORM16 skin weights behind explicit sampled error limits**

Reviewed 2026-10-31; next review 2026-11-14. This patch adds the quantisation library,
GLB storage path, shared deformation comparison helper, explicit CLI, native probes,
guide and Windows debug configurations. It preserves ordinary float-weight export.
Diagnostics/timing additions to other commands and palette pruning are intentionally
outside this patch; they remain in the main worktree for separate review.

- Artifact: `target/game-assets-review/0026-sampled-weight-quantisation.patch`.
- Prerequisites: canonical patches 0001 through 0025, unchanged `series.sha256`.
- Size: ten files, 553 insertions and four deletions.
- SHA-256: `4512ED2BAC049534F2977A6044C6A9575E48651FA592A0ECC432E1FE44DBFF79`.
- Separate checksum: `target/game-assets-review/0026.sha256`; the original manifest
  and all original artifacts remain unchanged.
- Isolated checkout: `target/game-assets-review/quantize-worktree`, with the original
  series staged and the quantisation delta unstaged. No commits were created.

All 135 isolated feature-enabled library tests and both input-recording integration
tests pass. The isolated WASM library compiles. The isolated CLI produces a flex-plate
GLB byte-identical to the main output checked by the native probes. Eight whole-file
copies match main byte-for-byte; the CLI and user-guide additions were extracted
without the unrelated post-snapshot work. Patch application and whitespace checks
pass against the original `assembled-worktree`. Evidence is in
`0026-isolated-tests.log`, `0026-isolated-wasm.log`, `0026-cli-help.log`,
`0026-cli-smoke.json` and `0026-reconciliation.log` under the review directory.

Commands, from the main repository root:

```sh
cargo test --manifest-path target/game-assets-review/quantize-worktree/Cargo.toml --target-dir target --release --locked --features game-assets --lib --test input_recording
cargo check --manifest-path target/game-assets-review/quantize-worktree/Cargo.toml --target-dir target --locked --target wasm32-unknown-unknown --features game-assets --lib
```

Package artifacts were cleared before switching the shared Cargo target between
same-name/version worktrees; confirm stage-specific test counts rather than accepting
a cached binary from another checkout. The main tool build supplies the native
Debug/Release evidence described in the guide; no fresh isolated native dependency
build or full repository regression run is inferred here.

The pinned Assimp importer fails UNORM16 skin-weight decoding. Raw decoding verifies
65 sampled poses independently but does not repair or certify the importer. Engine
acceptance and quality budgets remain open. This review record is newer than the
immutable patch and is not embedded in it.

## Patch 0027: Explicit unused skin-palette pruning

Reviewed 2026-10-31; next review 2026-11-14. Proposed title: **Prune unused skin
palette entries with explicit remapping and protected nodes**.

- Artifact: `target/game-assets-review/0027-explicit-palette-pruning.patch`.
- Prerequisites: canonical 0001–0026; three files, 165 insertions.
- SHA-256: `155F9271F1F3A566A991B5BF603467BAE2BB6975325D2A5CEBAAC9C852C5AD7A`;
  separate checksum in `0027.sha256`.
- Isolated checkout: `target/game-assets-review/palette-worktree`.

The patch contains `Skin::prune_palette`, its protection/remapping/deformation test,
the CLI command and guide. It removes palette entries only; no scene nodes or
animation tracks are removed and no positive weights are approximated. All 136
isolated library tests and two input-recording tests pass; WASM library compilation
passes. Package artifacts were cleared before switching the shared Cargo target.
The CLI's pruned and keep-control GLBs match the main outputs byte-for-byte.
Overwrite and unknown-keep failures preserve existing output/source files.

## Patch 0028: Encoded clip costs and structured processing times

Reviewed 2026-10-31; next review 2026-11-14. Proposed title: **Report encoded clip
payload sizes and separate CLI processing stages**.

- Artifact: `target/game-assets-review/0028-encoded-costs-and-processing-times.patch`.
- Prerequisites: canonical 0001–0027; three files, 147 insertions and ten deletions.
- SHA-256: `2B6EC73161769F11D66610E1089C9AE9E79C8A1CE3331D27944E26114EC93C59`;
  separate checksum in `0028.sha256`.
- Isolated checkout: `target/game-assets-review/diagnostics-worktree`.

The capability report measures actual unique timestamp/channel buffer views per
encoded clip, separately from total GLB size and without claiming runtime memory.
CLI JSON reports wall-clock evaluation, processing and writing stages as applicable.
Root-motion stdout remains the strict reusable trajectory document; its timing JSON
is emitted separately to stderr. The guide distinguishes those streams and excludes
engine work, report output and storage durability from the relevant measurements.

## Patch 0029: Updated evidence and documentation reconciliation

Reviewed 2026-10-31; next review 2026-11-14. This documentation-only extension
captures the requirement audit, cost/timing and palette/quantisation evidence,
Assimp compatibility finding, shader feasibility proposal and review records for
0026–0028. It does not introduce shader implementation or resolve engine decisions.

- Artifact: `target/game-assets-review/0029-updated-evidence-and-reconciliation.patch`.
- Prerequisites: canonical 0001–0028.
- Files: `docs/game-assets-design.md`, `docs/game-assets-plan.md`,
  `docs/game-assets-review.md`.
- SHA-256 is stored outside this self-describing document in `0029.sha256` and
  `series-0029.sha256`; the original `series.sha256` is unchanged.
- Assembly checkout: `target/game-assets-review/reconciled-worktree`.

Ordered application of the original 28 patches leaves only these three documentation
files different from main. The final reconciliation checks all 95 changed/new files,
using byte equality or CRLF/LF-normalized equality for known text formats, and verifies
local Markdown file targets and whitespace. Evidence is recorded in
`target/game-assets-review/0029-reconciliation.log`. Runtime claims remain those of
the scoped implementation patches; this documentation pass is not a new test run.

All artifacts remain local and immutable. There are no commits, pushes or external
submissions. The implementation remains incomplete: selected-engine material/motion validation,
production FBX, shader implementation/adapter, accepted quality/performance budgets,
remaining mesh/face authoring gaps, and desktop/browser platform acceptance still
require work. Review packaging does not establish product completion.

## Patch 0030: Guarded face provenance and material authoring

Reviewed 2026-11-01; next review 2026-11-14. This extension retains source-face
lineage through tessellation, refinement, slicing, normals, UVs, LOD and material
partitioning. Versioned selections validate source observations, semantic selectors
and geometry before use. Prepared queries address material primitives explicitly.
Derived and persisted material assignments preserve complete tuples, skin weights
and unaffected shared instances, and reject overlapping declarations.

- Artifact: `target/game-assets-review/0030-face-provenance-and-material-authoring.patch`.
- Prerequisites: canonical 0001–0029, verified against `series-0029.sha256`.
- Isolated checkout: `target/game-assets-review/face-worktree`.
- Scope: face APIs and source loader, material assignment, source/preparation/LOD
  hooks, CLI commands, six debug configurations, one skinned fixture and preparation,
  paired native importer checks, guide/plan/review evidence.
- SHA-256 is recorded in `0030.sha256` and `series-0030.sha256` outside this document.

The first prerequisite apply attempted to override line-ending handling and failed
against the clean Windows checkout. Normal repository settings applied all 29
prerequisites successfully; prior artifacts were not changed. gcad package artifacts
were cleaned before changing checkout paths to prevent cross-checkout Cargo reuse.
The lockfile remains unchanged. All work is local: no commits, pushes or submissions.
Engine acceptance, production FBX, shaders, face-bound UV/weight editing, platform
runtime checks and representative quality budgets remain outside this patch's proof.

## Patch 0031: Guarded face UV authoring

Reviewed 2026-11-01; next review 2026-11-14. Selected source faces now support
UV-origin scale, rotation and offset, including mirrored charts. Shared boundary
vertices split explicitly; unchanged tuples and skin weights retain exact source
remaps. Tangents regenerate for selected vertices. Persisted `face_uv` declarations
run after material assignments, reject overlapping regions and retain face lineage.
Binding validates declarations while preserving the original prepared-mesh contract.

- Artifact: `target/game-assets-review/0031-guarded-face-uv-authoring.patch`.
- Prerequisites: canonical 0001-0030, verified against `series-0030.sha256`.
- Isolated checkout: `target/game-assets-review/uv-worktree`.
- Scope: mesh/scene UV API, declaration loader, face-map retention, CLI, native
  importer checks, textured fixture, two debug configurations and documentation.
- SHA-256 is recorded in `0031.sha256` and `series-0031.sha256` outside this document.

The isolated checkout passes 152 library tests, two input-recording tests, WASM
compilation, default-feature CLI compilation and the feature-enabled release build.
Fresh preparation validates all 27 fixtures. Five exported GLBs (skinned and six-map
reference/edited pairs, plus the persisted textured example) match the tested main
exports byte-for-byte. Debug/Release native probes independently verify imported UV
coordinates and tangent frames, plus unchanged attributes and 65,076 synthetic posed
corners for the skinned fixture. Native probe sources are byte-identical to main;
those existing binaries were reused, with no separate isolated CMake build claimed.

This is scoped verification, not a fresh full-regression run. Patch 0030 retains the
latest full-suite evidence and its five known failures. Derivative tangents remain
distinct from MikkTSpace; UV changes do not rebake atlas textures. Engine shading,
GPU/browser/Linux runtime, production FBX, shader implementation and accepted quality
budgets remain unverified or incomplete. Next action: implement guarded
face-bound weight editing with explicit shared-boundary semantics and deformation
checks. Review packaging does not establish product completion.

## Patch 0032: Guarded face weight authoring

Reviewed 2026-11-01; next review 2026-11-14. Selected source-face corners can now
receive one to four named existing skin influences through the API, CLI and
persisted `face_weights` declarations. Positive finite weights must sum to one.
An explicit boundary policy rejects shared vertices or duplicates selected tuples;
unselected weights remain exact. Splits isolate shared mesh instances and retain
source-face lineage. Material and UV preparation precede weight edits; overlapping
weight regions fail. Existing weight resources remain unchanged.

- Artifact: `target/game-assets-review/0032-guarded-face-weight-authoring.patch`.
- Prerequisites: canonical 0001-0031, verified against `series-0031.sha256`.
- Isolated checkout: `target/game-assets-review/face-weights-worktree`.
- Scope: weight API, declarations, CLI, two debug configurations, diagnostic fixture,
  authored boundary tests, independent importer mode, documentation and evidence.
- SHA-256 is recorded in `0032.sha256` and `series-0032.sha256` outside this document.

The isolated checkout passes 155 library tests and two input-recording tests, WASM
compilation, default-feature CLI compilation and the feature-enabled release build.
Fresh preparation validates all 28 fixtures. Reference, direct-edit and authored
GLBs match main's decoded/tested exports byte-for-byte. Existing Debug/Release native
probes pass 65,076 posed corners, including 4,320 expected changed positions.
Native probe sources match main exactly; no separate isolated CMake build is claimed.
The isolated library suite includes the authored two-instance smoothed-plate test
whose shared boundary requires 68 vertex splits, plus declaration/API parity and
preservation of the other instance. Main's native shared export additionally passes
130,152 posed corners. Negative import controls and the other probe modes pass as
documented in `target/game-assets-face-weights-import`.

Binding validates declaration syntax, selectors, overlap and joint-node existence,
but defers palette membership and final prepared-boundary validation until normal
loading of the newly bound skin. Neither boundary policy promises a watertight
deformation across already separate shading tuples. Synthetic pose samples do not
establish engine playback or continuous error bounds. Production FBX, shaders,
engine/platform acceptance and representative quality budgets remain incomplete.
Next action: audit remaining product requirements against the assembled
implementation and continue the highest-priority unblocked implementation work.

## Remaining review series

| Area | Prerequisites and outstanding work |
| --- | --- |
| Input observation/provenance infrastructure | Recorder packaged as patch 0002; provenance and feature-gated model helper packaged as patch 0007 |
| Derived mesh/scene representation | Mesh/material/scene/clip representation packaged as patches 0003–0005; model helper in 0007 and authoring in 0012. Settle API boundaries before stabilisation |
| Static GLB/material portability | GLB, baking, atlas and authoring packaged through patch 0012; CLI packaged in 0020. Selected-engine appearance evidence remains missing |
| Animation and skins | Clip representation, CPU binding/deformation, GLB encoding and constrained motion packaged through patch 0012; root motion packaged in 0013; engine pose checks and accepted deformation budgets remain |
| Production FBX | Backend/tooling decision; resolve PBR material loss and packaging before submission |
| Optimisation | Diagnostics, reduction, metrics, projection and LOD packaged in 0014-0018; CLI packaged in 0020; accepted budgets and engine-camera tests remain |
| Shader authoring/adapters | Selected engine and M4 acceptance; implementation and runtime validation remain |

Next: audit remaining product requirements against the assembled implementation,
preserving focused tests and explicit feature boundaries. Resolve the pending engine, tooling and converter choices. Any external submission still requires explicit authorization and the
full outgoing-content/privacy review; this document does not grant it.
