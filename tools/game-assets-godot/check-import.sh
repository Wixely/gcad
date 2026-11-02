#!/bin/sh
# Isolated GLB import acceptance. Uses an existing Godot installation only.
set -eu
[ "$#" -eq 2 ] || { echo 'Usage: GODOT=/path/to/godot GODOT_VERSION=4.x... sh check-import.sh SOURCE.glb NEW_PROJECT' >&2; exit 1; }
: "${GODOT:?Select an installed Godot executable}"
: "${GODOT_VERSION:?Specify the exact output of Godot --version}"
root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
[ -f "$1" ] || { echo 'Source GLB is missing' >&2; exit 1; }
[ ! -e "$2" ] || { echo 'Preserving existing project' >&2; exit 1; }
actual=$("$GODOT" --version)
[ "$actual" = "$GODOT_VERSION" ] || { echo 'Godot version mismatch' >&2; exit 1; }
case "$actual" in 4.*) ;; *) echo 'This probe requires Godot 4' >&2; exit 1;; esac
mkdir -p "$2"
project=$(CDPATH= cd -- "$2" && pwd)
cp "$1" "$project/model.glb"
cp "$root/check_import.gd" "$project/"
cat > "$project/project.godot" <<'EOF'
config_version=5
[application]
config/name="gcad import acceptance"
[rendering]
renderer/rendering_method="gl_compatibility"
EOF
"$GODOT" --headless --path "$project" --script res://check_import.gd > "$project/import.log" 2>&1
test -f "$project/import-report.json"
grep -q '"passed": true' "$project/import-report.json"
