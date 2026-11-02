# Godot GLB import acceptance (pending runtime verification)

Reviewed: 2026-11-02. Next review: 2026-11-16.

This isolated Godot 4 probe uses an existing engine executable. It downloads and
installs nothing, preserves existing output directories, and copies an embedded
gcad GLB into a fresh project. It uses Godot's built-in
[GLTFDocument](https://docs.godotengine.org/en/stable/classes/class_gltfdocument.html)
and [ArrayMesh](https://docs.godotengine.org/en/stable/classes/class_arraymesh.html)
APIs. No plugins or package dependencies are required.

```sh
GODOT=/absolute/path/to/godot GODOT_VERSION=EXACT_VERSION_OUTPUT \
  sh tools/game-assets-godot/check-import.sh source.glb new-project
```

Set `GODOT_VERSION` to the exact output from that executable's `--version`.
The runner requires both a zero engine exit and `import-report.json` with a
passing gate. A script parse/runtime error must not count as acceptance.

The script compares instantiated mesh, surface, triangle and skinned-instance
counts against the original GLB JSON's active scene. It checks finite imported
positions, skin bind presence, skeleton attachment and named clip presence.
These are structural checks, not animation/deformation accuracy or rendered PBR
acceptance. Generic GLB geometry is tested; Unity consumer roles are not applied.
External texture/buffer dependencies are not copied by this runner.

Suggested first fixtures: `plate-consumer-profile.gasset` (static instances),
`plate-consumer-skinned.gasset` (two skins and a clip), then a textured fixture.
Prepare/export them with the existing gcad tools. Keep each engine/version run
in a separate new directory.

Status: shell syntax validated; Godot execution and GDScript parsing remain
unverified because no Godot executable was found on PATH. This does not establish
Godot compatibility. Next action: run this gate with an available Godot executable, add corruption controls, then extend it to sampled deformation, material and rendered checks.
