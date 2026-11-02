#!/bin/sh
# Reproduce a refined LOD experiment, preserving its original GLB.
set -eu
[ "$#" -eq 1 ] || { echo 'Usage: GCAD=... GCAD_LOD_STATIC=1 sh game-assets-lod-fixture.sh NEW_DIRECTORY' >&2; exit 1; }
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
gcad=${GCAD:-"$root/target/debug/gcad"}
[ -f "$gcad" ] || gcad="$gcad.exe"
[ -f "$gcad" ] || { echo 'Build gcad with --features game-assets first' >&2; exit 1; }
[ ! -e "$1" ] || { echo 'Preserving existing output directory' >&2; exit 1; }
mkdir -p "$1"
output=$(CDPATH= cd -- "$1" && pwd)
fixture=examples/game-assets/plate-scene-refined.gasset
case "${GCAD_LOD_STATIC:-0}" in
    0) ;;
    1) fixture=examples/game-assets/plate-static-refined.gasset ;;
    *) echo "GCAD_LOD_STATIC must be 0 or 1" >&2; exit 1 ;;
esac
# Bindings are created separately by game-assets.sh prepare-fixtures.
"$gcad" export "$fixture" "$output/unoptimised.glb" >"$output/source.stdout" 2>"$output/source.stderr"
"$gcad" asset-lod "$fixture" plate "$output/lod.glb" \
    --primitive 0 --triangles 14000 --max-error 1 --max-uv-error 0.02 \
    --max-angle 3 --samples 17 --max-trials 5000 \
    --max-surface-error 0.1 --max-surface-p95 0.05 --max-silhouette-difference 0.01 \
    >"$output/lod-report.json" 2>"$output/lod.stderr"
echo 'Inspect target_reached and measured errors in lod-report.json; a partial reduction is not target acceptance.'
