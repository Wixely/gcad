#!/bin/sh
# Exact PNG parity for the 21-pose, four-view editor/player fixture.
set -eu
[ "$#" -eq 1 ] || { echo 'Usage: check-render-parity.sh PROJECT' >&2; exit 1; }
project=$(CDPATH= cd -- "$1" && pwd)
printf '%s\n' '{"passed": false}' > "$project/source-editor-player-exact-report.json"
grep -q '"passed": true' "$project/source-animation-report.json"
grep -q '"passed": true' "$project/Player/player-render-report.json"
pose=0
while [ "$pose" -lt 21 ]; do
    for view in 0 1 2 3; do
        cmp "$project/source-renders/pose-$pose/imported-$view.png" "$project/Player/player-renders/$pose-imported-$view.png"
    done
    pose=$((pose + 1))
done
printf '%s\n' '{"passed": true, "identical_editor_player_images": 84}' > "$project/source-editor-player-exact-report.json"
