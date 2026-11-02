# Game-asset workflow

Use for `.gasset` authoring, live game-asset preview, and GLB or FBX/PNG exports.
Reviewed 2026-11-02; FBX remains experimental. This extends the CAD workflow.

## Resolve the build

If this installed skill includes `local-runtime.md`, read it for configured local
paths. Otherwise find the intended gcad checkout, read its README and
`docs/game-assets.md`, and use its executable built with
`cargo build --locked --release --features game-assets`. Use that executable's absolute path
for every command. Verify `--help` includes `asset-capabilities` and
`asset-fbx-experimental`; a stock executable on PATH may lack these commands.
Do not replace this feature build with the stock release installation procedure.
Use an explicitly configured, existing native FBX helper. The repository's
`tools/game-assets-fbx/README.md` documents building it; do not silently download
or install a replacement.

## Model and preview

### Bespoke textures

For painted markings, clothing panels or reference-specific surface detail, prefer
`uv SOURCE projection=unwrap resolution=1024 padding=8` over tiling one generic
image across a whole part. Keep box projection for deliberately repeating materials.
Read the painting workflow in `docs/game-assets.md`. Generate a guide with
`asset-paint-guide FILE NODE NEW_DIRECTORY --resolution 1024 --padding 8`, matching
the UV settings. Use `paint.png`, labelled islands, matching model views and the
appearance reference when generating or painting an image. Keep island positions,
canvas dimensions and orientation exact; exclude guide labels and baked lighting.
Image generation is an external authoring step, not part of the gcad command.

Run `asset-paint-apply FILE GUIDE PAINTED NEW_OUTPUT.png` to validate the guide and
fill edge padding. Bind that PNG with a `map SOURCE base_colour ...` declaration,
white base-colour factor and clamp wrapping. Inspect it in the existing live viewer
and fix visible seams or misplaced details. Never claim seamless wrapping merely
because a guide was supplied to an image generator. UV changes require explicit
skin rebinding. Guides are per mesh primitive, not a shared atlas for an assembly.
Keep guides and painted sources with the asset so later edits are reproducible.

Keep authored assets, CAD sources, textures and bindings in the user's test project,
not inside the gcad checkout. Use relative resource paths and a project root that
contains those resources. Read `docs/game-assets.md` in the selected checkout before
authoring; its syntax differs from `.gcad` and permits `#` comments.
For a simple static asset, create `part.gcad` and `model.gasset` beside one another:

```text
asset 1
project .
source part part.gcad body=main
tessellate part tolerance=0.1
uv part projection=box tile=25 rotation=0
surface part base_colour=0.7,0.7,0.7,1 metallic=0 roughness=0.6
node part source=part
```

Use `.gasm` for multi-part mechanical assemblies, then reference that assembly from
the game asset. `.gasset` also supports render-node hierarchies; read the guide for
their syntax. Sources use millimetres and Z-up; export converts to metres and Y-up.

1. Run the selected executable's `check PROJECT/model.gasset` and
   `asset-capabilities PROJECT/model.gasset`. Correct failures before exporting.
2. Open `view PROJECT/model.gasset` in the existing gcad application for the user.
   Set `GCAD_FBX_HELPER` to the native helper's absolute path in the launched
   process environment so its export panel is ready. Keep the window open while
   editing: saves to the asset and its dependencies reload automatically. Reuse
   that window instead of launching a new one for every edit. Invalid edits keep
   the last valid frame visible alongside an error.
3. Iterate with `check`, source saves, and `asset-preview PROJECT/model.gasset
   --snapshot PROJECT/preview.png`; inspect the snapshot when visual verification
   is needed. Software snapshots do not support UV shader transforms: use the
   live viewer to inspect those assets, or a clearly labelled static diagnostic
   copy without the shader; do not remove the user's shader to make a check pass.
   Read the guide for animation, skin binding, materials and shaders.
   Existing CAD documents continue to use their ordinary viewer mode.

## Export on request

For sprite maps, use the existing viewer's **Export sprite sheet** panel or
`gcad asset-sprites ASSET RECIPE.json NEW_DIRECTORY`. Read
`docs/game-asset-sprites.md` for per-frame view/model/light angles, animation times,
transparent PNG sheets and JSON rectangles. Use the GPU exporter for sprites;
software diagnostic snapshots have narrower material support. The viewer's
**Lighting and turntable** controls provide independent looping rotation without
changing authored geometry or animation.

Sprite export can include `albedo.png` and view-relative `normals.png` for 2D
runtime lighting. Pair those two sheets in the game; keep the baked `sprites.png`
as a reference or pre-lit alternative. Normals are linear data (+X right, +Y up,
+Z toward the viewer), with an explicit green-flip option. Export lighting controls
include key/fill colours and intensities, ambient, exposure and off/hard/soft
traced shadows. Read the sprite guide's tracing material limits before enabling
shadows; do not silently remove unsupported maps or shaders.

Run commands with the configured absolute executable path (shown as `GCAD` here):

```text
GCAD export PROJECT/model.gasset PROJECT/exports/model.glb
GCAD asset-fbx-experimental PROJECT/exports/model.glb PROJECT/exports/model-fbx --helper ABSOLUTE_HELPER_PATH
```

Create the exports parent directory first. Choose new destinations; the ordinary
CLI GLB export can overwrite, while FBX requires a new package directory. Preserve
prior user exports unless replacement was requested. Inspect successful output:
GLB exists, FBX package contains `model.fbx`, and textured assets include separate
PNGs. Untextured assets need no fabricated PNG. Keep package metadata receipts.

The viewer's **Export game asset** panel provides the same workflow for the human.
Export uses saved source, including clips; preview pose, LOD selection, skeleton
overlays and hidden surfaces do not change exported content. Freeze the constrained
UV-scrolling PBR shader for portable static output as documented; don't promise
arbitrary engine shaders. Report FBX as experimental, and distinguish successful
export from acceptance in Unity, Unreal, Godot or another engine.

Set up the runtime, run the CLI checks, and keep the preview open during authoring.
Report artifact paths, checks, remaining limitations and the next validation step.
