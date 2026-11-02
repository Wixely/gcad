# Experimental gcad FBX helper bundle

This helper accompanies a gcad build with the `game-assets` feature. Select the
helper executable by absolute path; it is never installed or discovered globally.

```sh
gcad asset-fbx-experimental source.glb new-package --helper /absolute/path/gcad_fbx_helper
```

Use the `.exe` filename on Windows. Export the input GLB from gcad first. The
package destination must be new and its parent must exist. The result contains
`model.fbx` and separate PNG textures referenced with relative paths.

`helper-identity.json` records the linked Assimp source revision and reviewed patch
hash. `SHA256SUMS` covers the bundle files. The source patch, helper/build sources,
pinned dependency source repositories and notices are included. See
`SOURCE-BUILD.md` and `rebuild-helper.sh` for an offline rebuild.
Some notices are distributed as their original source headers to
retain complete embedded copyright/license text.

Build with `sh tools/game-assets.sh package-helper NEW_OUTPUT` from the matching
gcad source checkout. This uses existing pinned dependency checkouts; it does not
download or install dependencies. Windows Release builds use the static MSVC
runtime. Linux runtime dependencies still require platform verification.

This is an experimental developer bundle, not engine certification. The helper
reports `conversion_ready: false`. The repository records Windows Unity fixture
checks with an explicit material/deformation adapter. Unreal, Godot, broader
engine assets and Linux runtime/recovery remain unverified. Keep this file and
the dependency notices and source snapshots with redistributed copies.
Clean-machine release verification remains a release gate.

Windows path handling uses UTF-16 process arguments converted explicitly to UTF-8
and native filesystem paths. The opt-in `relocated_helper_preserves_unicode_paths`
integration test covers relocated executables, source/output names containing
non-ASCII characters, paths longer than 260 characters, conversion and both
inspection commands. The 2026-11-01 static-runtime bundle passes those cases;
its PE dependency table lists only KERNEL32.dll. This is same-machine relocation
evidence, not a clean-machine or Linux test.

The Rust caller requires the `leased-staging-v1` capability. Both processes hold
shared stage leases, so `gcad asset-fbx-recover OUTPUT_PARENT` skips work still in
use by either process. Recovery removes abandoned marked stages; rerun conversion
after cleanup. Older unmarked stages and published packages are not selected.

Reviewed: 2026-11-01. Review again: 2026-11-14.
