# Lighting, turntables and sprite sheets

Reviewed: 2026-11-02. These controls appear only for game-asset documents in the
existing viewer. Normal CAD documents keep their existing interface.

Expand **Lighting and turntable**. Light and model angle each have a slider,
independent Play/Pause control and signed speed in degrees per second. Playback
loops through 360 degrees; dragging a slider pauses that rotation. Light elevation
and view azimuth/elevation are independently adjustable, including orthographic
or perspective viewing. Model rotation keeps scene lighting fixed. Navigation,
rig overlays and animation continue to work while rotating. These presentation
settings do not modify the asset or its GLB/FBX export.

Expand **Export sprite sheet**. **Use current angles and frame** copies the current
model angle, light angle and animation time into the sample lists. Enter
comma-separated values to export every combination (model outermost, then light,
then time). The current view angles, light elevation, projection, zoom and selected
clip are used. Set cell width/height, columns, transparent padding and a new output
directory; its parent must already exist. Export runs in the background and can
be cancelled between frames. Existing destinations are never overwritten.

Sprites use the loaded asset's ordinary materials, LOD 0 and hidden collision
sources. Inspector colours, overlays, alternate preview LODs and camera pan are
excluded. Framing is shared across requested animation poses, so sprites do not
change scale from frame to frame. Zoom values above one can deliberately crop.
Export produces straight-alpha `sprites.png` and `sprites.json`, which records
each pixel rectangle, angle, time, recipe and common scene bounds. Pixel
coordinates start at the sheet's top-left. Empty cells and padding are transparent.

## Lighting, shadows and runtime normals

In **Export lighting and shadows**, set key/fill colours and intensities, ambient
light and exposure. The existing angle lists and light-elevation slider determine
direction. Choose **Off**, **Hard**, or **Soft** shadows, sample count and (for soft
shadows) light angular spread. Shadows are traced between surfaces of the model;
no ground plane is added. More samples reduce noise and take longer. Lighting and
shadows affect the lit `sprites.png` only.

**Export unlit colour and normal sheets for 2D lighting** is enabled by default.
It additionally writes:

- `albedo.png`: base colour without direct light, shadows, specular, emission or AO.
- `normals.png`: visible geometry normals including the authored normal-map detail,
  rotated into the sprite's view basis. R points screen-right, G screen-up and B
  toward the camera. **Flip normal green channel** selects screen-down G instead.

All three sheets use identical cells, frame samples and raster alpha coverage.
RGB extends two texels into transparent pixels within each cell, with alpha kept
zero, to protect filtered edges. View-normal bytes are linear numeric data, not
sRGB colours; decode with `normalize(rgb * 2 - 1)`. The JSON records the convention
and files. In the game's 2D lighting material, pair `albedo.png` with `normals.png`,
using identical sprite rectangles and a linear/normal-map import for normals.
Use the lit sheet for pre-lit sprites or reference, not as the unlit colour input.
These normals drive surface lighting; they do not encode geometry for dynamic
cast shadows. Overlapping translucent layers have one blended normal per pixel.

Recipes support `normal_sheet` (default true), `normal_y_negative` (default false),
and a `lighting` object. For example:

```json
{
  "normal_sheet": true,
  "normal_y_negative": false,
  "lighting": {
    "key_colour": [255, 235, 210],
    "key_intensity": 1.0,
    "fill_colour": [205, 225, 255],
    "fill_intensity": 0.2,
    "ambient": 0.25,
    "exposure": 1.0,
    "shadows": "soft",
    "shadow_samples": 32,
    "softness": 0.08
  },
  "frames": [{ "model_angle": 45, "light_angle": 90 }]
}
```

Tracing currently rejects alpha materials, UV-transform shaders, explicit mip
images, and occlusion/emission maps. Turn shadows off for those assets; their
raster colour/albedo/normal export remains available. Errors are explicit and
preserve existing outputs. Tracing uses a different shading path from raster
preview, so enabling shadows can also change specular/ambient appearance.

## Scripted export

```
gcad asset-sprites model.gasset turntable.sprites.json new-sprite-directory
```

The [example recipe](../examples/game-assets/turntable.sprites.json) exports eight
views. Every frame can independently specify `view_azimuth`, `view_elevation`,
`model_angle`, `light_angle`, `light_elevation` (degrees) and `time` (seconds).
Light angle is an offset from the studio key/fill azimuths; key elevation is
absolute. Add a top-level `clip` name to sample animation. Times must lie within
the selected clip; for example an animation's frame 12 at 24 fps is `time: 0.5`.
Without a clip, time still samples animated materials. Omitted frame properties
use the default isometric camera, zero rotation and time, and studio key elevation.

```json
{
  "width": 256,
  "height": 256,
  "columns": 2,
  "clip": "walk",
  "frames": [
    { "view_azimuth": 20, "view_elevation": 35, "light_angle": 90, "time": 0.5 },
    { "view_azimuth": 140, "view_elevation": 60, "light_angle": 180, "time": 0.75 }
  ]
}
```

Limits: 256 frames, cells 16–2048 pixels per axis, padding 0–32 pixels, sheet axes
up to 8192 and at most 16 megapixels. Angles and times must be finite. Unknown
recipe fields are rejected. Choose additional sheets for larger collections.

The exporter uses the same GPU renderer as the viewer, including normal maps,
alpha materials and UV animation. It uses Direct3D 12 on Windows and Vulkan on other native platforms, and does not
silently substitute the software diagnostic renderer. The pinned renderer clears
background alpha to one, so two readbacks (black and white backgrounds) recover
coverage and straight RGB. This avoids colour-key holes in dark parts and retains
antialiased edge coverage. Preview/export tangent and material limitations listed
in the main game-asset guide still apply; this is not an engine renderer.

Next action (owner: user): review a sheet in the target game's sprite importer,
particularly its straight-alpha setting and cell/padding interpretation.

Verification (2026-11-02): Windows release build, Direct3D 12 sprite angle/light/pose
readback, numeric alpha recovery, recipe bounds, cancellation/output preservation,
turntable loops/navigation and live-document reload checks pass. An eight-angle
launcher sheet was visually inspected. Linux/Vulkan sprite export and target-engine
import remain untested.

Relighting verification (2026-11-02): seven sprite tests cover mapped-normal
contribution, hard/soft shadow occlusion, light-invariant runtime sheets, matching
alpha, unit-length normal encoding, green flipping, RGB bleed, limits and worker
lifecycle. The eight-view launcher export with 32-sample soft shadows completed
with exit code zero; all three sheets were inspected. Offline GPU uploads are
released between animation poses while angle-only samples share their buffers.
Target-engine 2D lighting and Linux/Vulkan remain untested.
