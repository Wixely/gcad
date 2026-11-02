#!/bin/sh
# Launch the game-asset mode of the existing application from this checkout.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
if [ "$#" -gt 1 ]; then
    printf '%s\n' 'Usage: sh tools/game-assets-mvp.sh [ASSET.gasset]' >&2
    exit 1
fi
cargo build --locked --release --features game-assets
gcad="$root/target/release/gcad"
[ -f "$gcad" ] || gcad="$gcad.exe"
helper="$root/target/game-assets-tools/material-extension-v12-build/Release/gcad_fbx_helper.exe"
if [ -z "${GCAD_FBX_HELPER:-}" ] && [ -f "$helper" ]; then
    if command -v cygpath >/dev/null 2>&1; then helper=$(cygpath -am "$helper"); fi
    export GCAD_FBX_HELPER="$helper"
fi
exec "$gcad" view "${1:-examples/game-assets/plate.gasset}"
