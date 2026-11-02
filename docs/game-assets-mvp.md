# Viewer and export MVP

Reviewed: 2026-11-02. Scope: desktop preview and GLB/FBX export
from saved `.gasset` sources in the existing gcad application.

From Git Bash at the repository root:

For agent-driven use, invoke `$gcad` and describe the asset in your project. The
skill's [game-asset workflow](../skills/gcad/references/game-assets.md) covers
opening the viewer while authoring and exporting on request. The shell command
below is an optional manual launcher; users of the skill need not run it.

```sh
sh tools/game-assets-mvp.sh
# Or open your own saved asset:
sh tools/game-assets-mvp.sh examples/game-assets/plate.gasset
```

The launcher builds the optional game-asset feature in release mode and opens the
existing viewer. Use this optimized build for character animation; debug builds
perform substantially more slowly when deforming dense meshes.
It uses the reviewed v12 helper if already built in this checkout. To use another
copy, set `GCAD_FBX_HELPER` to its absolute executable path before launching, or
enter that path in the export panel. Building the helper is documented in
[the native helper guide](../tools/game-assets-fbx/README.md). The launcher does
not download dependencies or install a helper.

Edit and save the asset or its source CAD files while the window stays open.
The preview reloads automatically and keeps the last valid model if an edit fails.
Materials, orbit/zoom, animation playback, skeleton and weight inspection are
available; consumer-profile assets also expose LOD and collision previews.

Game assets also have independent looping light/model rotation controls and
GPU sprite-sheet export. See [lighting and sprite sheets](game-asset-sprites.md)
for the viewer controls and the `asset-sprites` JSON-recipe command.

Expand **Export game asset** above the model controls. Choose **GLB** and a new
`.glb` filename, or **FBX + PNG (experimental)** and a new package directory.
The destination's parent directory must exist. Click **Export saved asset** and
wait for the success path or error message. Existing destinations are preserved.
Export runs in the background so the viewer remains usable.

Exports include the saved source's geometry, materials, rig and clips. The current
preview pose, hidden collision surfaces, weight colours and selected preview LOD
do not alter exported content. Save edits before exporting. Source changes during
loading/encoding reject that export; retry after saving. GLB embeds textures;
FBX packages contain separate PNG textures and applicable metadata receipts.

Normal `.gcad` and `.gasm` documents continue to use the CAD interface. This panel
appears only for game-asset documents. See [the authoring guide](game-assets.md)
for supported material and animation syntax and format limitations.

Verification on 2026-11-02: feature-enabled desktop build passed; GLB export and
overwrite refusal, native FBX package export, and an automated click-through of
the background export panel passed. Logs: `target/viewer-mvp-build.log`,
`target/viewer-mvp-exports.log`, `target/viewer-mvp-panel.log`. A frozen textured
shader fixture also exported to `target/viewer-mvp-textured.glb` and
`target/viewer-mvp-textured-fbx` with `model.fbx`, a PNG and shader metadata.
The preceding broader regression run had seven failures outside this new panel;
the repository does not currently have a clean full-suite result.

Skill integration was checked on 2026-11-02 using a separate temporary project:
source validation, capability reporting, GLB export, FBX with separate PNG and
shader receipt, static diagnostic snapshot inspection, and launching the viewer
with the configured helper. Both locally installed skill copies were updated;
machine-specific runtime paths live only in their local references. The bundled
skill validator could not run because its Python environment lacks PyYAML;
frontmatter shape, description length, placeholders and workflow links were
checked separately. Skill discovery in a fresh installation remains unverified.

Remaining beyond this MVP: production FBX packaging, broader engine/platform
acceptance and the full development plan. FBX is experimental; existing Unity
checks do not establish Unreal/Godot compatibility. next action:
validate representative user assets in the intended engine before release.
