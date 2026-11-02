# Game asset development commands

Use the repository's Rust, CMake and shell toolchain. Run `game-assets.sh` with
`sh` in a POSIX shell, Git Bash or WSL. It installs nothing. WSL requires its own
Linux build toolchain; a Windows Rust installation is not a Linux compiler.

```sh
cargo build --locked --features game-assets
sh tools/game-assets.sh prepare-fixtures
sh tools/game-assets.sh verify-gpu target/new-gpu-check vulkan
```

On Windows, pass `dx12 vulkan` to check both backends. GPU tests require a suitable
adapter and fail rather than skip an unavailable requested backend. Output
directories must be new; failures retain stdout/stderr logs. Existing weights and
face selections are preserved. Set `GCAD=/path/to/gcad` to select an executable.

`sh tools/game-assets-lod-fixture.sh NEW_DIRECTORY` reproduces the refined skinned
LOD experiment after `prepare-fixtures`. It retains the original GLB, writes the
derived GLB and captures its JSON report with explicit fixture error limits.
The accepted fixture recipe uses a 1 mm endpoint-correspondence allowance while
enforcing sampled surface maximum 0.1 mm, p95 0.05 mm, silhouette difference
0.01, UV difference 0.02 and shading 3 degrees. Endpoint correspondence is a
conservative internal constraint, distinct from closest-surface error.
Inspect `target_reached` as well as measured errors: a valid partial reduction
does not establish the requested triangle target. This recipe is not a production
performance benchmark or an engine acceptance claim.

For native probes, provide these existing source checkouts:

- Assimp: `a4643cbd30d3088bbff312d3cc2652f514fb97c0`
- ufbx: `5955c5c0b042ac2dc6f32955ab206aaec0620cfc`

Defaults are `target/game-assets-tools/assimp` and `ufbx`; override with
`ASSIMP_SOURCE` and `UFBX_SOURCE`. Select CMake with `CMAKE`. The script checks
revisions and tracked changes before configuring a build.

```sh
sh tools/game-assets.sh build-probes Debug base
sh tools/game-assets.sh build-probes Release v12
```

This also builds `gcad_fbx_helper`. Use the v12 build with
`gcad asset-fbx-helper --helper ABSOLUTE_PATH_TO_HELPER` to verify protocol and
backend pins. Older and base builds fail the current identity check. A valid
handshake reports `conversion_ready: false`; experimental staged conversion is
advertised through its separate capability. Production distribution remains incomplete.

To exercise the native process failure fixture, configure the existing build
with `-DGCAD_HELPER_PROTOCOL_TESTS=ON`, build the `gcad_fbx_protocol_fixture` target,
then set `GCAD_FBX_PROTOCOL_FIXTURE` to its absolute native path and run:

```sh
cargo test --locked --release --features game-assets --test fbx_helper_process -- --ignored
```

This opt-in test checks malformed JSON, oversized stdout/stderr, nonzero exit,
timeout and cancellation. It does not invoke conversion or write output assets.

The versioned modes create or validate separate patched Assimp worktrees. All modes
build the C++ probes; neither is production FBX integration. Multi-configuration
generators put executables in `Debug`/`Release`; single-configuration generators
put them at the build root. VS Code's native probe task uses CMake directly and
expects `cmake` on PATH and source checkouts in the default locations.

Individual audit modes remain available directly:

```sh
gcad export examples/game-assets/nested-assembly.gasset target/nested.glb
gcad_glb_probe target/nested.glb --nested-assembly
gcad export examples/game-assets/nested-skinned.gasset target/nested-skinned.glb
gcad_fbx_probe --audit-scene-instance-skins target/nested-skinned.glb target/new-fbx-audit
```

Resolve executable paths for your configuration. Native probes and Rust tests
retain their embedded corruption controls. Historical one-off input mutation
wrappers have been retired; these commands do not claim to rerun every historical
mutation campaign. Existing findings retain their original scope.

Verify the renderer using a clean checkout of the fork at gcad's pinned revision:

```sh
git clone --branch gcad-renderer https://github.com/Wixely/egui_bench ../egui_bench
git -C ../egui_bench checkout 0c33aef02aa91dbb12ba1f556067dc806e225d16
cargo run --locked --release --features game-assets --example game_asset_renderer_fixture -- target/new-atlas.bin
EGUI_BENCH_MANIFEST=../egui_bench/Cargo.toml sh tools/game-assets.sh verify-renderer target/new-renderer-tests target/new-atlas.bin vulkan
```

Skip cloning if the checkout already exists. The runner checks its revision and
refuses tracked edits. Cargo uses the immutable Git revision in `Cargo.toml` and
`Cargo.lock`; the former vendor directory has been removed. Make renderer changes in
the fork, validate them, then update both pins together.

For historical reproduction, `reproduce-renderer ARCHIVE NEW_OUTPUT` checks the
crate archive and five patch checksums, preserves licenses, and writes an
isolated output without modifying the active dependency. gcad's
adapters are already adopted; do not apply them again. Linux/browser runtime and
external-engine acceptance remain separate from the recorded Windows checks.

For animation CPU measurements (excluding asset loading and GPU rendering), run:

```sh
cargo run --locked --release --features game-assets --example game_asset_preview_bench -- /path/to/model.gasset
```

The benchmark validates the immutable asset once, then prepares three sampled
poses through the same deformation path as live playback.

## Experimental helper bundle

After preparing the reviewed v12 sources with `build-probes Release v12`, create a new Release
bundle with the same shell entry point:

```sh
sh tools/game-assets.sh package-helper target/helper-bundle
(cd target/helper-bundle && sha256sum -c SHA256SUMS)
```

`CMAKE` and `UFBX_SOURCE` select existing tools/checkouts. The recipe uses a separate
build directory, pins Assimp/ufbx and the reviewed patch, and includes the executable,
handshake identity, source patch, usage notes, dependency licenses/notices and
checksums. Existing output is refused. Windows builds use a static MSVC runtime.
The verified Windows executable imports only `KERNEL32.dll`; running the bundled
helper through `asset-fbx-experimental` produced the expected FBX plus 15 PNGs.
This does not establish clean-machine or Linux deployment, engine acceptance,
signed distribution or reproducible binary hashes. Evidence: `target/fbx-bundle-*`.
