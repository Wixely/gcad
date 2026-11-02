# FBX backend integration proposal

Status: helper approach approved; production integration incomplete. Evidence reviewed 2026-11-01;
review again by 2026-11-14 or before adopting dependencies.

The isolated Assimp v10 importer/writer plus an explicit consumer adapter can preserve the
tested asset contracts. A generic FBX import is insufficient: custom material
properties require interpretation, and regenerated surface normals do not reproduce
gcad's blended-matrix shading convention. The production CLI still exports GLB;
this proposal does not declare FBX support complete.

## Proposed decision

Current active patch: v11, SHA-256
`719d4a76baf866c62cb26eb20b1ee38191670738c94ccbe9b7ca10cdb55a6349`.
It coalesces same-joint clusters across material primitives and keeps standard
normal/emission map connections alongside custom metadata. Conflicting binds are
refused. Unity 6000.4.7f1 and 6000.5.4f1 pass the reference headless import gate;
pose/render parity and other engines remain unverified. Earlier v10 evidence below
records the development baseline, including failures subsequently found in Unity.

Use an optional native converter executable built from the pinned Assimp source
and reviewed v10 patch, with a versioned gcad request/response contract. Keep the
consumer adapter contract explicit. This is the most directly evidenced route in
the current tree and is the approved implementation route. Support must be tested
across Unity, Unreal, Godot and other consumers. Unity export requires FBX plus
separate PNG textures referenced by relative paths. Standard materials must have
a useful import without gcad-specific extensions; identify advanced features
that require an adapter explicitly.

| Route | Evidence in this workspace | Cost or unresolved issue |
| --- | --- | --- |
| Patched Assimp helper process | Windows Debug/Release independent readback, skins, clips, maps, reflected instances and basis reconstruction | Separate binary and runtime packaging, patch maintenance, custom consumer integration |
| Patched Assimp linked into Rust | Same underlying writer experiment | Rust FFI, ownership/error boundary, native build and packaging unimplemented; no demonstrated benefit over helper yet |
| Rust-native FBX writer | Candidate investigation in the design document | Scene, material, cluster, bind, animation and consumer semantics must be implemented and independently validated |
| User-supplied external converter | No accepted production converter contract | Version drift, provenance and semantic loss; cannot claim portability from file creation alone |

## Proposed boundary and behavior

The native Rust `fbx_package::PackageStage` reserves staging beside the intended
package directory, runs caller-supplied validation, and publishes via a single
no-replace directory rename. This also refuses a destination created concurrently
during validation. Unpublished stages attempt cleanup on drop; cleanup failures
are currently best-effort, and killed-process recovery remains to be integrated.
Atomic visibility is distinct from power-loss durability. Windows uses
`MoveFileExW` without replacement/copy flags; Linux uses
`renameat2(RENAME_NOREPLACE)` and fails closed when unsupported. Linux execution
has not been verified. The production helper invocation is not connected yet.

The first boundary implementation is now present: `gcad_fbx_helper --handshake`
reports protocol 1, material schema 3, the linked Assimp version and its actual
Git revision/full tracked-diff SHA-256. CMake captures that identity; Rust checks
it against the reviewed v10 pins. `gcad asset-fbx-helper --helper ABSOLUTE_PATH`
performs this check with a ten-second post-spawn deadline and 64 KiB per-stream
output limits. The library also accepts a cancellation flag and kills/reaps the
direct child on timeout, cancellation or errors. Helpers are explicitly selected,
not discovered on PATH; this is not an executable sandbox or descendant-process
containment. No global helper configuration is installed.

The helper reports `conversion_ready: false` and the experimental capability
`staged-conversion-v1-experimental`. `--convert-v1 ABSOLUTE_INPUT ABSOLUTE_STAGE`
requires an existing empty directory, performs shared preparation and independent
readback, writes FBX/PNG artifacts, checks their bytes, and returns protocol-1 JSON
with artifact paths and lengths. It never publishes the directory. The Windows
textured three-clip check produced 16 artifacts and refused a reused stage.
Rust `convert_experimental` now invokes that capability with bounded output,
timeout and cancellation, checks its exact staged artifact set and publishes with
the no-replace package publisher. Manifest validation rejects unexpected files,
directories, symlinks, duplicate/nonportable paths, wrong sizes and wrong file
signatures. Full semantic validation remains in the pinned native helper. Six
Rust tests pass; the Windows real-helper end-to-end test passes for publication,
existing-package preservation and failed-conversion cleanup. Extended Windows
texture filesystem paths are normalized separately from portable FBX references.
Evidence: `target/fbx-connected-*`. The experimental `asset-fbx-experimental` CLI
now captures a bounded private GLB snapshot and verifies its SHA-256 before
publication. Seven Rust tests pass; a Windows CLI run published 16 artifacts and
refused an existing package (`target/fbx-snapshot-tests.*`, `target/fbx-cli-*`).
Restored-scene ordered geometry, corner attributes, weights/binds and per-face
material/provenance checks now run before staged writes. Textured and reflected
fixtures pass 1,276 and 2,552 faces, rejecting five and ten deliberate corruptions
respectively; the CLI still publishes all 16 expected files with the gate enabled
(`target/fbx-restored*`). Interrupted-process recovery and packaged delivery
remain required. Do not interpret
a valid handshake as production support.
Reviewed: 2026-11-01; next review: 2026-11-14.

`helper_textures.cpp` prepares relative PNG filenames and material-slot bindings
from canonical embedded-image references. It validates PNG dimensions and pixel
decoding within per-image and aggregate budgets, retaining original encoded bytes.
`prepareTextures()` applies the bindings to an owned deep copy and removes its
embedded images, preserving the original scene. It does not write files. Native
fixture exports pass independent ufbx readback for node/material names, relative
paths, diffuse/normal material connections and absence of embedded image content.
The helper request path does not yet use this module. With
`GCAD_HELPER_PROTOCOL_TESTS=ON`, build
`gcad_fbx_textures_test` and run `ctest --test-dir BUILD_DIR -C Debug -R helper_texture`
to exercise shared image bindings, byte preservation, source-name preservation,
path isolation, invalid references and truncated PNG rejection on two fixtures.

`helper_names.cpp` restores authored node, mesh and material names from explicit
temporary-import identity maps. It updates bone and node-animation references in
the same operation, checking node-name uniqueness and reference resolution before
changing identities. This preserves gcad's existing unique-node-name contract and
prevents ambiguous Assimp name lookups. Tests cover collisions, overlong names,
missing targets and linked reference restoration; static readback also runs after
restoration. `helper_import_names.h` constructs the maps from validated GLB JSON
before tagging, after instance isolation; it handles Assimp's per-primitive mesh
suffixes. The existing shared-skinned-instance animation audit passes using this
common tagging implementation. It deliberately retains tagged names for its
identity checks. Shared `importScene`/`prepareScene` preparation now combines
validated import, instance isolation, provenance attachment, external texture
preparation and name restoration. The production request path is not wired to it.

`gcad_fbx_probe --audit-prepared-scene INPUT.glb` checks an in-memory export of
that prepared scene. Independent readback passes for the shared-skin fixture and
a textured three-clip fixture, including restored node hierarchy, physical units,
skin playback and 15 relative PNG references without embedded content. This does
not publish PNGs/FBX. External custom-material checks now verify exact connection
paths, absence of embedded image content, factors, samplers/UV sets, channel
conventions and provenance. The three-clip fixture passes 18 map connections and
138 deliberate property/path/content/provenance corruptions are rejected. The
reflected fixture and embedded-material regression also pass. Standard consumer
material acceptance is still required; these checks cover the custom contract.
The basis check still uses the explicit adapter; standard-engine normal/tangent
parity is not established. Evidence: `target/fbx-prepared-*`,
`target/fbx-external-*` and `target/fbx-material-regression.*`.

1. gcad prepares its authoritative asset scene and canonical GLB with existing
   source guards, portable map bytes and provenance. Conversion does not mutate
   CAD, bindings or texture sources.
2. gcad invokes an explicitly configured helper executable by absolute path using
   an argument vector. A handshake returns protocol version, source revision,
   patch identity and supported capabilities before accepting a request. Resolve
   only configured or packaged helpers, not an arbitrary executable on PATH.
3. A request identifies one input artifact, a new private staging directory and
   requested contract version. The helper returns machine-readable outcomes and
   diagnostics. It must retain user node/material/clip names; current audit-only
   identity renaming is not production behavior.
4. The helper checks supported interpolation, source skin identity/binds, texture
   provenance and resource limits before conversion. Repeated skinned references
   are isolated explicitly, with duplication reported. Materials use schema 3;
   writer candidate number 10 is not a material-schema version.
5. Write into staging, independently validate required structural contracts and
   publish only a complete artifact set. Cancellation/failure leaves the requested
   destination untouched. Cleanup must be confined to that request's staging
   directory. This transactional boundary is not implemented by the audit probe.
6. Report any required consumer adapter, duplication, rotation baking and lossy
   approximation. Never substitute a default material, drop animation/skin data,
   or silently accept a generic reader's visual interpretation.

CUBICSPLINE/mixed interpolation, unavailable provenance, unresolved skins, partial
tangent layers, nonfinite data and exceeded bake/allocation budgets need explicit
failure results. The current probes reject several of these cases, but production
error codes, cancellation, bounded file handling and crash recovery are still work.
The shared audit reader now rejects files above 64 MiB and JSON chunks above
16 MiB, and iteratively checks a 64-container nesting limit and 1,000,000 combined
container/key/scalar events before DOM construction. Twelve Debug/Release cases
cover malformed/over-limit refusals and exact structural boundaries, with no
destination creation or source changes. These are isolated probe guards; they do
not settle production budgets, node-graph recursion or decoded-image/importer
allocation limits. See the historical validation harness for reproduction.
The scene audit additionally checks all node components iteratively before import,
with 65,536-node/128-depth audit limits, single-parent/cycle checks, and scene-root
validation. Twenty-one Debug/Release cases cover refusals and inclusive boundaries
(the historical validation harness). Full schema/accessor validation and importer allocation
limits remain separate work; graph preflight does not establish those guarantees.
The canonical dense embedded route now also checks buffer/view/accessor occupied
byte ranges before import. Twenty-two Debug/Release cases cover layout and refusal
behavior, including matrix padding and integer-range edges. Audit ceilings of one
million elements per accessor and 128 MiB aggregate scalar storage estimates do
not establish a total importer-memory bound. Primitive expansion, accessor usage
contracts and decoded-image limits still need validation. Sparse/external data
remain explicit unsupported cases in this probe, not settled product exclusions.
Skin and animation reference/shape preflight now runs before instance isolation
and import. Sixteen Debug/Release corruption cases cover joint/bind references,
node mesh/skin references and TRS sampler/target/accessor contracts; the complete
reflected-scene regression remains passing. Binary values, mesh attribute contracts
and full skin hierarchy semantics are not established by these checks. Reproduce
with the historical validation harness and the three-clip scene-contract GLB.
Primitive reference/shape checks and repeated-use work estimates now also run
before isolation/import. Fourteen Debug/Release corruption cases cover attributes,
indices and repeated primitive/instance budgets. The 128 MiB estimate ceiling
charges attribute scalars and corners per mesh use; it is not a total-process
memory limit. The valid reflected fixture still passes. Actual BIN values and
decoded-image bounds remain unverified. See the historical validation harness.
BIN value preflight now checks finite floats, index bounds, joint palettes,
weight ranges/sums and animation time ordering; ten corruption cases pass both
builds. Image-header preflight additionally checks PNG/JPEG declarations, dimensions
and encoded/decoded-estimate budgets; sixteen cases and the full textured-scene
regression pass both builds. Pixels are not decoded, so compressed-stream validity,
decoder allocation behavior and rendered acceptance remain open. The earlier
statement that image handling was entirely unverified is superseded only for
these header/budget checks.

A valid UNORM16-weight fixture exposed incorrect normalized integer weight
extraction in the pinned Assimp importer. Both v9 builds failed later skin checks
despite passing raw-value preflight. See the plan's dated evidence.
Candidate v10 corrects that importer path and passes UNORM16, strided UNORM8 and
float fixtures in both builds. An independent raw-source/imported-weight check and
corruption control now precede export. This supersedes the v9 failure for those
fixtures only; mixed multi-set layouts and engine acceptance remain unverified.
The adapter's absolute determinant guard of 1e-10 is experimental; near-singular
conditioning and user-facing refusal policy need resolution before shipping.

## Consumer contract and evidence

| Contract | Current evidence | Limit |
| --- | --- | --- |
| Geometry, UVs and frames | Per-corner independent comparisons; mirrored/rotated UV fixture; four shading corruptions rejected | Tested fixture family, not arbitrary topology |
| Six PBR maps | 18 map connections across three materials; exact embedded bytes, samplers, factors, packed channels, 132 negative controls | Custom `gcad` properties require adapter; no engine-rendered verdict |
| Skin and instances | Source joint/bind guards, one skin per merged geometry, explicit instance isolation | Output geometry duplication; broader hierarchy/resource limits unverified |
| Animation | STEP/LINEAR, scale, irregular quaternion tracks, local Euler refinement, independent off-grid samples | Sampled approximation, not continuous error proof; finite bake limits |
| Units/reflection | Metre metadata, world transforms/determinants, removed-reflection control | No independently tested engine importer |
| Deformed basis | 1,730,256 corners, max component error 2.99752e-7, exact handedness; direction/handedness/singular controls | Explicit adapter only, not ufbx-generated normals or GPU rendering |

The generic ufbx evaluator differs from gcad's normal convention even at time zero.
Preserve that diagnostic; do not replace it with the adapter result. A selected
engine must decide and implement its normal/tangent deformation convention and
validate normal-map orientation, alpha, sidedness, colour spaces and packed maps.
Evidence directories and reproduction commands are recorded in
[the native harness README](../tools/game-assets-fbx/README.md).

## Build, packaging and license inventory

The tested source pins are Assimp
`a4643cbd30d3088bbff312d3cc2652f514fb97c0` and ufbx
`5955c5c0b042ac2dc6f32955ab206aaec0620cfc`. The full v10 patch SHA-256 is
`DDC3EBA640BAB3D0FFEAB7A81B1580E78B23478745C07CFEBE5E5100586069BB`.
The build enables the glTF importer and FBX exporter, disables other default
importers/exporters, and statically links Assimp, zlib and the ufbx probe library.

Local source notices inspected: gcad declares GPL-3.0-or-later; Assimp's LICENSE
contains BSD-style redistribution/notice terms and separate third-party notices;
ufbx offers MIT or Unlicense (proposed distribution choice: MIT); zlib has its
own notice; pugixml uses MIT; RapidJSON includes MIT plus component-specific terms.
This is an inventory, not a completed distribution license audit. Enumerate the
actual compiled dependency closure and ship its notices and required source/patch
materials. Exclude unused model/test payloads from release packaging; Assimp's
test-model exceptions must not be treated as covered by its main code license.

The Release project/compiler-log inventory subsequently identified 113 translation
units and 363 source/build inputs across the probe, ufbx, Assimp and zlib targets.
All declared translation units occur in the observed reads. Assimp's compiled
contribution directories and their local notice locations are:

| Directory | Notice source relative to Assimp | Observed notice type |
| --- | --- | --- |
| clipper | contrib/clipper/License.txt | Boost Software License 1.0 |
| earcut-hpp | contrib/earcut-hpp/LICENSE | ISC |
| open3dgc | Individual source/header notices | MIT-style text plus separate BSD-style arithmetic-codec notice |
| openddlparser | contrib/openddlparser/LICENSE | MIT |
| poly2tri | contrib/poly2tri/LICENSE | BSD-style notice |
| pugixml | contrib/pugixml/LICENSE.md | MIT |
| rapidjson | contrib/rapidjson/license.txt | MIT plus listed component exceptions |
| unzip | unzip.h, unzip.c, ioapi.c, MiniZip64_info.txt | Embedded zlib-style terms and historical Info-ZIP references |
| utf8cpp | contrib/utf8cpp/doc/LICENSE | Boost Software License 1.0 |
| zlib | contrib/zlib/LICENSE | zlib notice |

The v10 image-header preflight additionally compiles pinned stb_image 2.30, whose
MIT/Unlicense alternatives are embedded in `contrib/stb/stb_image.h`. The refreshed
v10 inventory has 113 translation units and 370 source/build inputs, including
eleven Assimp contribution directories. Reproduce with the inventory script's
`-WriterVersion 10`; evidence is `image-inventory-final` under the scene-contract
evidence root. Older v9 measurements above remain historical observations.

The pinned unzip.c defines NOUNCRYPT, and crypt.h is absent from observed compiler
reads. This is build evidence, not permission to discard notices from a source
distribution. Open3DGC must not be labelled uniformly MIT without its arithmetic
codec's separate notice. Retain the actual notices rather than this summary table.

Reproduce with `the historical validation harness -OutputDirectory
NEW_DIRECTORY`. The verified manifest is at
`target/game-assets-fbx-scene-contract/dependency-inventory-final/inputs.json`.
It hashes current source/build inputs, notice sources, project files, compiler
read logs and the executable; it verifies the expected project-reference closure.
Repeated runs produced identical JSON with neutral relative paths. There are 254
additional toolchain/Windows file reads counted without recording identifying
paths. This is not final linker-retained code, a fresh clean-build reproducibility
proof, or a complete redistribution audit. Runtime/toolchain packaging obligations
and a release notice/source bundle still require review for the selected build.

`dumpbin /DEPENDENTS` on the measured v9 Release probe shows MSVCP140, VCRUNTIME140,
VCRUNTIME140_1 and Windows CRT imports. It is **not** a self-contained executable.
Choose and test an appropriate runtime deployment or static-runtime build before
shipping; static Assimp linkage alone does not settle that choice. This proposal
does not claim that the probe is a production converter binary.

Windows x64 is the only tested native target. Linux build, deployment and runtime
verification remain release gates. Browser/WASM conversion has not been tested;
the native helper does not implement browser FBX export. An explicit browser
strategy is required if that delivery target includes FBX; GLB fallback does not
complete that requirement. This is a CLI/helper, not a service deployment.

## Costs and acceptance gates

The measured v9 Release audit executable is 1,696,768 bytes. The reflected fixture's
input is 196,976 bytes and FBX output 894,256 bytes (about 4.54 times input).
These are measured artifacts, not general size estimates or production budgets.
Instance duplication, embedded image duplication and baked curve growth must be
included in asset-specific budgets. Measure full-audit costs reproducibly with
the historical validation harness; those timings include validation
and corruption tests and must not be advertised as conversion throughput.

One Release full-audit run on 2026-11-01 took 1.119 seconds and observed an OS peak
working set of 10,108,928 bytes (50 ms observation interval). This is a single
warm local observation, not a benchmark distribution or a portable performance
promise. Evidence and executable/source hashes are in
`target/game-assets-fbx-scene-contract/backend-measurement-verified/measurements.json`.
Conversion-only timing, cold starts, pathological inputs, larger assets and
production memory enforcement remain unmeasured/unimplemented.

| Remaining action | Completion evidence |
| --- | --- |
| Select helper vs linked vs native writer; first consumer/version and representative assets | Recorded architecture/consumer decision |
| Implement handshake, conversion-only path, structured refusals, cancellation and transactional output | Integration tests, fault/cancellation tests, no destination mutation on failure |
| Audit exact dependency closure and package notices/runtime/source materials | Reproducible package manifest and clean-machine run |
| Verify Windows and Linux packages | Native build/run evidence on each target |
| Establish resource budgets and measure representative assets | Peak memory, conversion time, size and bake limits per asset family |
| Implement consumer material/basis adapter and rendered fixtures | Engine imports, snapshots and numeric checks for maps, frames, clips and instances |
| Resolve browser export strategy if included in selected delivery scope | Browser build/runtime evidence rather than native inference |

The wider plan's shader architecture, optimisation quality, platform/GPU coverage,
CAD hierarchy work and release review remain separate requirements. Selecting
this backend would not close them.
