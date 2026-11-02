# Experimental asset fixtures

Build with `cargo build --locked --features game-assets`, then run
`sh tools/game-assets.sh prepare-fixtures` from the repository root. Existing
weights and face selections are preserved. Set `GCAD` to use another executable.
See [development commands](../../tools/game-assets.md) for GPU and native probes.

These fixtures exercise the experimental game-assets feature. They are diagnostic
inputs, not accepted production assets. CAD source files remain authoritative.

`atlas-extra-maps.gasset` exercises all six source texture maps, packed into five
preview slots by the isolated renderer/adapter proposal. Two instances share
three authored levels per slot. Occlusion strength is 0.4; emissive RGB factors
are 0.3/0.2/0.1. The active CLI validates the fixture, with a matching VS Code
configuration. Active preview support still depends on renderer adoption.

`atlas-extra-motion.gasset` adds the nonconstant occlusion/emissive slots and
nonunit factors to the rigid-motion fixture. The isolated adapter suite also
creates a two-joint blended skin from it in memory, keeping the second instance
static. Each map has a separate visible-effect control at five poses and two
image sizes. The active CLI validation has a matching VS Code configuration.

`atlas-mask-raw.gasset` is the uncorrected comparison for the identical geometry,
maps and material in `atlas-mask-coverage.gasset`. The isolated renderer proposal
compares their projected occupied pixels with a level-zero-only reference;
this reference can alias and is not a continuous area oracle.

`atlas-mask-coverage.gasset` opts into per-chart sampled alpha correction for lower
mip levels. `asset-report` exposes target, before and after pass counts and scales;
nonzero residuals are retained. The fixture has six charts, two lower levels and
two instances sharing one material. This is a sampled UV-area correction, not an
exact screen-coverage guarantee. The active preview/export still refuse explicit
chains; GPU evaluation uses the isolated renderer proposal.

`atlas-alpha.gasset` uses the same diagnostic RGBA map for opaque, masked and
blended materials, with alpha factor 0.75 and mask cutoff 0.5. All three retain
explicit mip chains. The active CLI validates the fixture; rendering these modes
is tested in the isolated alpha proposal, not the active viewer. Blend sorting
within/intersecting surfaces and mask coverage preservation across mips remain
limitations. The VS Code alpha validation entry runs the normal Debug check.

`atlas-mips.gasset` retains levels 0..2 for six map slots and shares them across
two instances. `gcad check` validates it. The active preview and GLB exporter
explicitly refuse this fixture because they cannot yet consume/transport its
chain; no output is written. Omit `mips` for the existing single-level workflow.
Explicit levels require `filter=linear`; unsafe chart footprints fail rather than
silently reducing the requested maximum. The isolated renderer proposal is still
separate from the active dependency.

`atlas-motion.gasset` combines the existing nonconstant diagnostic colour/numeric
maps with an explicit three-level atlas. One block translates, rotates 90 degrees
and scales nonuniformly; a second shared instance stays fixed. The isolated
preview test compares five poses with closed-form transformed rest geometry,
checks inverse-transpose normals and unchanged UV/mip payloads, and requires
visible texture and motion controls. This is rigid animation, not skinning or an
independent shading oracle. Texture alpha remains an explicit preview limitation.
The active CLI can check the fixture; preview/export still refuse explicit chains.

`plate-face-materials.gasset` reuses the original plate skin weights and adds red
end faces plus blue bottom faces through persisted `face_material` declarations.
Preparation captures `base.end` and `<Z` only when their records are missing.
The side walls and hole interiors retain the source material. To inspect or export:

```sh
target/debug/gcad.exe asset-preview examples/game-assets/plate-face-materials.gasset --clip bend --time 0.5 --snapshot target/plate-face-materials.png
target/debug/gcad.exe export examples/game-assets/plate-face-materials.gasset target/plate-face-materials.glb
```

The example retains the diagnostic original bend, whose deformation quality is not
accepted for production. Material assignment does not improve that geometry.

`plate-face-weights.gasset` adds a persisted 25% base / 75% tip assignment on the red
end faces, reusing the original skin resource. Material/UV preparation precedes
weight edits regardless of declaration order. `reject-shared` checks vertex indices;
existing hard-normal/UV/material seams remain separate and can open in animation.
This is a diagnostic assignment, not an accepted production bend.

```sh
target/debug/gcad.exe asset-preview examples/game-assets/plate-face-weights.gasset --clip bend --time 0.5 --snapshot target/plate-face-weights.png
target/debug/gcad.exe export examples/game-assets/plate-face-weights.gasset target/plate-face-weights.glb
```

`plate-face-uv.gasset` reuses the saved `base.end` selection and the local RGBA test
texture. Its `face_uv` declaration mirrors/scales the top chart by `-2,3`, rotates
it 30 degrees about the UV origin, then offsets it by `0.2,-0.1`. Walls, the rounded
rim and hole interiors retain their original sampling. No texture pixels change.
This static example needs the two prepared face records, but no skin weights.

```sh
target/debug/gcad.exe asset-preview examples/game-assets/plate-face-uv.gasset --snapshot target/plate-face-uv.png
target/debug/gcad.exe export examples/game-assets/plate-face-uv.gasset target/plate-face-uv.glb
```

Software snapshots demonstrate the selected chart change; their filtered appearance
does not certify the declared nearest-neighbour sampler in an engine.

The set covers static materials/maps, atlases, crease normals, assembly placement
and constrained motion, root motion, skinning, bend/twist comparisons, refinement,
slicing and LOD normal-bake inputs. Severe deformation cases intentionally expose
limitations. Passing `check` validates authoring and bindings; it does not certify
visual quality, engine import or continuous deformation bounds.

`plate-multi-clip.gasset` reuses the prepared face-selection and weight resources
from `plate-face-weights.gasset`. Its three LINEAR clips independently exercise
rotation (`bend`, 1 s), translation (`travel`, 2.3 s, intermediate key at 0.65 s)
and nonuniform scale (`stretch`, 1.75 s, intermediate key at 0.4 s). Separate clip
durations and irregular key spacing help expose conversion timing/channel errors.
Export with `gcad export examples/game-assets/plate-multi-clip.gasset OUTPUT.glb`.
The isolated FBX animation harness accepts the resulting GLB; engine acceptance
is still separate from authoring validation and independent import checks.

`plate-step-clips.gasset` uses the same geometry, skin and three tracks with STEP
interpolation. Values hold until each next key; it exercises discontinuities in
rotation, translation and scale. The FBX harness's version-6 candidate checks the
key instants and nearby samples, and rejects a changed interpolation control.

`plate-oblique-rotation.gasset` rotates the same weighted plate 120 degrees around
the `(1,1,1)` axis. Its LINEAR quaternion track exposes the difference between
spherical quaternion interpolation and straight interpolation of Euler components.
Use it with the independent FBX animation harness when testing rotation conversion.

See `docs/game-assets.md` from the repository root for command syntax and current
limitations. Software snapshots and GLB exports can replace their destinations;
commands creating derived GLBs or weight files require new output paths.

## Nested assembly fixture
