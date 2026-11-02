#!/bin/sh
# Prepare both bindings without requiring either resource to exist already.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
gcad=${GCAD:-"$root/target/debug/gcad"}
[ -f "$gcad" ] || gcad="$gcad.exe"
[ -f "$gcad" ] || { echo 'Build gcad with --features game-assets, or set GCAD' >&2; exit 1; }
fixture=examples/game-assets/plate-consumer-skinned.gasset
mkdir -p target
for level in high low; do
    output="target/consumer-skinned-$level.weights.json"
    if [ ! -e "$output" ]; then
        "$gcad" asset-bind "$fixture" "detail_$level" "$output" --joints base,tip --blend y,-15,15
    fi
done
"$gcad" check "$fixture"
