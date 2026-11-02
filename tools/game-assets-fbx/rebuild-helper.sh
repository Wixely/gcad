#!/bin/sh
# Offline rebuild using only the sources distributed in this helper bundle.
set -eu
[ "$#" -eq 1 ] || { echo 'Usage: CMAKE=... sh rebuild-helper.sh NEW_DIRECTORY' >&2; exit 1; }
bundle=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
[ ! -e "$1" ] || { echo 'Preserving existing rebuild directory' >&2; exit 1; }
(cd "$bundle" && sha256sum --strict -c SHA256SUMS)
mkdir -p "$1"
work=$(CDPATH= cd -- "$1" && pwd)
for dependency in assimp ufbx; do
    git clone --no-hardlinks "$bundle/source/$dependency.git" "$work/$dependency"
done
[ "$(git -C "$work/assimp" rev-parse HEAD)" = a4643cbd30d3088bbff312d3cc2652f514fb97c0 ]
[ "$(git -C "$work/ufbx" rev-parse HEAD)" = 5955c5c0b042ac2dc6f32955ab206aaec0620cfc ]
patch="$bundle/experimental-material-properties-v12.patch"
git -C "$work/assimp" apply --check "$patch"
git -C "$work/assimp" apply "$patch"
actual=$(git -C "$work/assimp" diff --no-ext-diff --binary --full-index HEAD -- | sha256sum)
[ "${actual%% *}" = 783bf12bee4103422f8e8d2f74b2f17ae4dd93109b25266cd67baa9838340bf9 ]
"${CMAKE:-cmake}" -S "$bundle/source/helper" -B "$work/build" \
    "-DASSIMP_SOURCE=$work/assimp" "-DUFBX_SOURCE=$work/ufbx" \
    -DCMAKE_BUILD_TYPE=Release -DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded -DUSE_STATIC_CRT=ON
"${CMAKE:-cmake}" --build "$work/build" --config Release --target gcad_fbx_helper --parallel 8
echo "Rebuilt helper: $work/build (Release subdirectory with a multi-config generator)"
