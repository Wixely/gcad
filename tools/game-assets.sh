#!/bin/sh
# Development entry points for the optional game-asset pipeline.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
fail() { printf '%s\n' "$*" >&2; exit 1; }
usage() {
    cat <<'EOF'
Usage: sh tools/game-assets.sh COMMAND [ARGS]
  prepare-fixtures                         Preserve/create bindings, check fixtures
  build-probes [Debug|Release] [base|v10|v11|v12] Build native import/FBX probes
  package-helper NEW_OUTPUT                Build experimental Release helper bundle
  verify-gpu NEW_OUTPUT [dx12|vulkan ...]    Run active asset GPU regression tests
  verify-renderer NEW_OUTPUT ATLAS [BACKEND ...]  Test EGUI_BENCH_MANIFEST checkout
  reproduce-renderer ARCHIVE NEW_OUTPUT    Apply pinned patches to the crate

Use GCAD to select a built gcad executable, CMAKE to select CMake, and
ASSIMP_SOURCE/UFBX_SOURCE to select existing pinned dependency checkouts.
GPU backends default to dx12 and vulkan; select vulkan for Linux.
Cargo may fetch locked dependencies. Native probe executables also
expose their individual audit modes directly; see tools/game-assets-fbx/README.md.
EOF
}
check_pin() {
    [ "$(git -C "$1" rev-parse HEAD)" = "$2" ] || fail "Unexpected dependency revision: $1"
    [ -z "$(git -C "$1" status --porcelain --untracked-files=no)" ] || fail "Dependency has tracked edits: $1"
}
check_hash() {
    actual=$(sha256sum "$1")
    [ "${actual%% *}" = "$2" ] || fail "Checksum mismatch: $1"
}
new_output() {
    [ ! -e "$1" ] || fail "Preserving existing output: $1"
    mkdir -p "$1"
    output=$(CDPATH= cd -- "$1" && pwd)
}
backend_ok() { case "$1" in dx12|vulkan) ;; *) fail "Unsupported test backend: $1";; esac; }
command=${1:---help}
[ "$#" -eq 0 ] || shift
case "$command" in
--help|-h) usage ;;
prepare-fixtures)
    [ "$#" -eq 0 ] || fail 'prepare-fixtures takes no arguments; use GCAD for the executable'
    gcad=${GCAD:-"$root/target/debug/gcad"}
    [ -f "$gcad" ] || gcad="$gcad.exe"
    [ -f "$gcad" ] || fail 'Build gcad with --features game-assets, or set GCAD'
    mkdir -p target
    while IFS='|' read -r selection filename; do
        [ -e "target/$filename" ] && continue
        "$gcad" asset-face-bind examples/game-assets/plate-face-materials.gasset plate "$selection" "target/$filename"
    done <<'EOF'
base.end|game-asset-plate-end.faces.json
<Z|game-asset-plate-bottom.faces.json
EOF
    while IFS='|' read -r selection filename; do
        [ -e "target/$filename" ] && continue
        "$gcad" asset-face-bind examples/game-assets/plate-scene-refined.gasset plate "$selection" "target/$filename"
    done <<'EOF'
base.end|game-asset-refined-end.faces.json
<Z|game-asset-refined-bottom.faces.json
EOF
    while IFS='|' read -r fixture node filename joints option value; do
        [ -e "target/$filename" ] && continue
        set -- asset-bind "examples/game-assets/$fixture.gasset" "$node" "target/$filename" --joints "$joints"
        [ -z "$option" ] || set -- "$@" "$option" "$value"
        "$gcad" "$@"
    done <<'EOF'
nested-skinned|first/left.a|game-asset-nested.weights.json|bone||
linkage-part-skin|first/crank|game-asset-linkage-part.weights.json|bone||
plate-skinned|plate|game-asset-plate.v2.weights.json|base,tip|--blend|y,-15,15
plate-root-motion-skinned|plate|root-motion-skinned.weights.json|base,tip|--blend|y,-15,15
plate-scene-contract|plate|game-asset-plate-scene-contract.weights.json|base,tip|--blend|y,-15,15
plate-scene-refined|plate|game-asset-plate-scene-refined.weights.json|base,tip|--blend|y,-15,15
plate-refined|plate|game-asset-plate-refined.weights.json|base,tip|--blend|y,-15,15
plate-flex|plate|game-asset-plate-flex.weights.json|base,tip|--blend|y,-15,15
plate-flex-narrow|plate|game-asset-plate-flex-narrow.weights.json|base,tip|--blend|y,-7.5,7.5
plate-attachments|plate|game-asset-plate-attachments.weights.json|base,tip|--blend|y,-15,15
plate-twist-chain|plate|game-asset-plate-twist-chain.weights.json|bone0,bone1,bone2,bone3,bone4|--chain|y,-15,-7.5,0,7.5,15
plate-twist-sliced|plate|game-asset-plate-twist-sliced.weights.json|bone0,bone1,bone2,bone3,bone4|--chain|y,-15,-7.5,0,7.5,15
plate-twist-sliced-fine|plate|game-asset-plate-twist-sliced-fine.weights.json|bone0,bone1,bone2,bone3,bone4|--chain|y,-15,-7.5,0,7.5,15
EOF
    GCAD="$gcad" sh tools/game-assets-consumer-fixture.sh
    for fixture in examples/game-assets/*.gasset; do "$gcad" check "$fixture"; done
    ;;
build-probes)
    [ "$#" -le 2 ] || fail 'build-probes takes configuration and writer variant'
    configuration=${1:-Release}; variant=${2:-base}
    case "$configuration" in Debug|Release) ;; *) fail 'Use Debug or Release';; esac
    assimp=${ASSIMP_SOURCE:-"$root/target/game-assets-tools/assimp"}
    ufbx=${UFBX_SOURCE:-"$root/target/game-assets-tools/ufbx"}
    check_pin "$assimp" a4643cbd30d3088bbff312d3cc2652f514fb97c0
    check_pin "$ufbx" 5955c5c0b042ac2dc6f32955ab206aaec0620cfc
    case "$variant" in
    base) build="$root/target/game-assets-tools/build" ;;
    v10|v11|v12)
        case "$variant" in
        v10) expected=ddc3eba640bab3d0ffeab7a81b1580e78b23478745c07cfebe5e5100586069bb ;;
        v11) expected=719d4a76baf866c62cb26eb20b1ee38191670738c94ccbe9b7ca10cdb55a6349 ;;
        v12) expected=783bf12bee4103422f8e8d2f74b2f17ae4dd93109b25266cd67baa9838340bf9 ;;
        esac
        extension="$root/target/game-assets-tools/assimp-material-extension-$variant"
        patch="$root/tools/game-assets-fbx/experimental-material-properties-$variant.patch"
        check_hash "$patch" "$expected"
        if [ ! -e "$extension" ]; then
            git -C "$assimp" worktree add --detach "$extension" a4643cbd30d3088bbff312d3cc2652f514fb97c0
            git -C "$extension" apply --whitespace=error-all "$patch"
        fi
        [ "$(git -C "$extension" rev-parse HEAD)" = a4643cbd30d3088bbff312d3cc2652f514fb97c0 ] || fail 'Unexpected experiment base'
        actual=$(git -C "$extension" diff --binary --full-index | sha256sum)
        [ "${actual%% *}" = "$expected" ] || fail 'Experiment differs from pinned patch'
        assimp=$extension; build="$root/target/game-assets-tools/material-extension-$variant-build"
        ;;
    *) fail 'Use base, v10, v11 or v12';;
    esac
    "${CMAKE:-cmake}" -S tools/game-assets-fbx -B "$build" "-DASSIMP_SOURCE=$assimp" "-DUFBX_SOURCE=$ufbx" "-DCMAKE_BUILD_TYPE=$configuration"
    "${CMAKE:-cmake}" --build "$build" --config "$configuration" --parallel 8
    ;;
package-helper)
    [ "$#" -eq 1 ] || fail 'package-helper requires a new output directory'
    [ ! -e "$1" ] || fail 'Preserving existing package output'
    assimp="$root/target/game-assets-tools/assimp-material-extension-v12"
    ufbx=${UFBX_SOURCE:-"$root/target/game-assets-tools/ufbx"}
    [ "$(git -C "$assimp" rev-parse HEAD)" = a4643cbd30d3088bbff312d3cc2652f514fb97c0 ] || fail 'Prepare reviewed v12 sources with build-probes first'
    actual=$(git -C "$assimp" diff --no-ext-diff --binary --full-index HEAD -- | sha256sum)
    [ "${actual%% *}" = 783bf12bee4103422f8e8d2f74b2f17ae4dd93109b25266cd67baa9838340bf9 ] || fail 'Unreviewed Assimp patch'
    check_pin "$ufbx" 5955c5c0b042ac2dc6f32955ab206aaec0620cfc
    build="$root/target/game-assets-tools/helper-package-build"
    "${CMAKE:-cmake}" -S tools/game-assets-fbx -B "$build" "-DASSIMP_SOURCE=$assimp" "-DUFBX_SOURCE=$ufbx" \
        -DCMAKE_BUILD_TYPE=Release -DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded -DUSE_STATIC_CRT=ON
    "${CMAKE:-cmake}" --build "$build" --config Release --target gcad_fbx_helper --parallel 8
    helper="$build/Release/gcad_fbx_helper.exe"
    [ -f "$helper" ] || helper="$build/gcad_fbx_helper"
    "${CMAKE:-cmake}" "-DHELPER=$helper" "-DOUTPUT=$1" "-DASSIMP_SOURCE=$assimp" "-DUFBX_SOURCE=$ufbx" \
        -P tools/game-assets-fbx/package_helper.cmake
    ;;
verify-gpu|verify-renderer)
    [ "$#" -ge 1 ] || fail 'A new output directory is required'
    destination=$1; shift
    if [ "$command" = verify-renderer ]; then
        [ -n "${EGUI_BENCH_MANIFEST:-}" ] || fail 'Set EGUI_BENCH_MANIFEST to the fork checkout Cargo.toml'
        [ -f "$EGUI_BENCH_MANIFEST" ] || fail 'Renderer manifest does not exist'
        renderer_dir=$(CDPATH= cd -- "$(dirname -- "$EGUI_BENCH_MANIFEST")" && pwd)
        renderer_manifest="$renderer_dir/Cargo.toml"
        renderer_rev=$(sed -n 's/^egui_bench = { git = .*rev = "\([a-f0-9]*\)".*/\1/p' Cargo.toml)
        [ -n "$renderer_rev" ] || fail 'Missing renderer revision in Cargo.toml'
        check_pin "$renderer_dir" "$renderer_rev"
        [ "$#" -ge 1 ] || fail 'An atlas fixture is required'
        [ -f "$1" ] || fail 'Atlas fixture does not exist'
        GCAD_ATLAS_FIXTURE=$(CDPATH= cd -- "$(dirname -- "$1")" && pwd)/$(basename -- "$1")
        if command -v cygpath >/dev/null 2>&1; then GCAD_ATLAS_FIXTURE=$(cygpath -w "$GCAD_ATLAS_FIXTURE"); fi
        export GCAD_ATLAS_FIXTURE
        shift
    fi
    [ "$#" -gt 0 ] || set -- dx12 vulkan
    for backend do backend_ok "$backend"; done
    new_output "$destination"
    for backend do
        export GCAD_RENDER_BACKEND=$backend
        if [ "$command" = verify-gpu ]; then
            cargo test --locked --release --features game-assets,gpu-tests --lib asset:: -- --nocapture --test-threads=1 >"$output/$backend.stdout" 2>"$output/$backend.stderr"
            grep -qi "Verified requested backend: $backend" "$output/$backend.stdout"
            grep -Eq 'test result: ok\. [1-9][0-9]* passed; 0 failed; 0 ignored;' "$output/$backend.stdout"
            grep -q 'Textured animation: 87040 pixels' "$output/$backend.stdout"
            grep -q 'Textured skin: 87040 pixels' "$output/$backend.stdout"
        else
            for spec in 'explicit_image::tests:3' 'gcad_explicit:3' 'gcad_alpha:2' 'gcad_extra_maps:2'; do
                filter=${spec%:*}; expected=${spec##*:}; name=$(printf '%s' "$filter" | tr ':' '_')
                cargo test --locked --release --manifest-path "$renderer_manifest" --target-dir "$root/target" --features viewer3d --lib "$filter" -- --nocapture --test-threads=1 >"$output/$backend-$name.stdout" 2>"$output/$backend-$name.stderr"
                grep -q "$expected passed; 0 failed" "$output/$backend-$name.stdout"
            done
        fi
        printf 'PASS: %s %s\n' "$command" "$backend"
    done
    ;;
reproduce-renderer)
    [ "$#" -eq 2 ] || fail 'Specify the cached crate archive and a new output directory'
    archive=$1
    if command -v cygpath >/dev/null 2>&1; then archive=$(cygpath -u "$archive"); fi
    check_hash "$archive" 3decd5040e37033b7a35a547b9babed56d317180a20c6dff4e29d4a456be9a79
    new_output "$2"
    tar -xf "$archive" -C "$output"
    candidate="$output/egui_bench-0.9.2"
    git -C "$candidate" init -q
    while read -r filename pin; do
        patch="$root/tools/game-assets-renderer/$filename"
        check_hash "$patch" "$pin"
        git -C "$candidate" apply --check "$patch"
        git -C "$candidate" apply "$patch"
    done <<'EOF'
explicit-image-api.patch a8105ebadc6c5fbb3222cddb0e59468f5838b655abf5bc98b98b575f9ecfad79
renderer-integration.patch bbe68d9cd2293ef8297963d1b71f2cc60be82941bfdf1341f0098c6f060ea4b5
renderer-alpha.patch b5004627007b936305f65e6d74f05cc48dc57ad1d7edce0532c0a74600781948
renderer-extra-maps.patch 42c9782846362394afdb8fe9303b3926323c183f72545236fce187300050adc4
renderer-uv-transform.patch d47ebf9810be7ff4efff1c2fd0c872d157b8af12f6b5a7920b836c8353ea1755
EOF
    printf '\n[workspace]\n' >>"$candidate/Cargo.toml"
    printf 'Reproduced renderer: %s\n' "$candidate"
    ;;
*) usage >&2; fail "Unknown command: $command";;
esac
