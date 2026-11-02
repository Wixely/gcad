#!/bin/sh
set -e
cd "$(dirname "$0")"
mkdir -p assets
B=https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Assets/main/Models
for m in Fox Duck CesiumMan Avocado Lantern SheenChair; do
  [ -f assets/$m.glb ] || curl -sfL -o assets/$m.glb $B/$m/glTF-Binary/$m.glb
done
if [ ! -f assets/spot.obj ]; then
  curl -sfL -o /tmp/spot.zip https://www.cs.cmu.edu/~kmcrane/Projects/ModelRepository/spot.zip
  unzip -p /tmp/spot.zip spot/spot_triangulated.obj > assets/spot.obj
fi
