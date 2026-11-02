#!/bin/sh
# Reference fixture consumer test; no packages, plugins or engine installs.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
[ "$#" -eq 2 ] || { echo 'Usage: UNITY=... UNITY_VERSION=... sh check-import.sh FBX_PACKAGE NEW_PROJECT' >&2; exit 1; }
: "${UNITY:?Select an installed Unity executable}"
: "${UNITY_VERSION:?Specify its exact editor version}"
if { [ "${GCAD_CONSUMER_PLAYER:-0}" = 1 ] || [ "${GCAD_CONSUMER_RENDER:-0}" = 1 ]; } && [ "${GCAD_CONSUMER_CHECK:-0}" != 1 ]; then
    echo 'Consumer player/render checks require consumer acceptance mode' >&2; exit 1
fi
if [ "${GCAD_CONSUMER_CHECK:-0}" = 1 ]; then
    : "${GCAD_FBX_HELPER:?Select the reviewed helper for consumer adaptation}"
    if [ "${GCAD_ROOT_MOTION_CHECK:-0}" = 1 ] || [ "${GCAD_UNITY_ADAPTER_CHECK:-0}" = 1 ] || [ "${GCAD_PLAYER_CHECK:-0}" = 1 ]; then
        echo 'Run consumer-profile checks separately from other fixtures' >&2; exit 1
    fi
fi
if [ "${GCAD_CONSUMER_SKIN:-0}" = 1 ] && [ "${GCAD_CONSUMER_CHECK:-0}" != 1 ]; then
    echo 'Skinned consumer checks require consumer acceptance mode' >&2; exit 1
fi
if [ "${GCAD_CONSUMER_FAILURE:-0}" = 1 ] && [ "${GCAD_CONSUMER_CHECK:-0}" != 1 ]; then
    echo 'Cooking recovery checks require consumer acceptance mode' >&2; exit 1
fi
if [ "${GCAD_ROOT_MOTION_PLAYER:-0}" = 1 ] && [ "${GCAD_ROOT_MOTION_CHECK:-0}" != 1 ]; then
    echo 'Root-motion player requires the root-motion acceptance mode' >&2; exit 1
fi
if [ "${GCAD_ROOT_MOTION_CHECK:-0}" = 1 ]; then
    : "${GCAD_FBX_HELPER:?Select the reviewed helper for root-motion adaptation}"
    : "${GCAD_ROOT_MOTION_REFERENCE:?Select the original GLB for independent root-motion sampling}"
    if [ "${GCAD_UNITY_ADAPTER_CHECK:-0}" = 1 ] || [ "${GCAD_PLAYER_CHECK:-0}" = 1 ] || [ "${GCAD_UV_RENDER_CHECK:-0}" = 1 ] || [ -n "${GCAD_MATERIAL_REFERENCE:-}" ] || [ -n "${GCAD_LOD_COST_BASELINE:-}" ]; then
        echo 'Run root-motion acceptance separately from other fixture modes' >&2; exit 1
    fi
fi
if [ -n "${GCAD_LOD_COST_BASELINE:-}" ] && { [ "${GCAD_SOURCE_RENDER_CHECK:-0}" = 1 ] || [ "${GCAD_UNITY_ADAPTER_CHECK:-0}" = 1 ] || [ "${GCAD_PLAYER_CHECK:-0}" = 1 ] || [ "${GCAD_UV_RENDER_CHECK:-0}" = 1 ]; }; then
    echo 'Run paired LOD cost measurement separately from rendering/player checks' >&2; exit 1
fi
if [ -n "${GCAD_LOD_REFERENCE:-}" ] && [ "${GCAD_SOURCE_RENDER_CHECK:-0}" != 1 ]; then
    echo 'LOD quality requires the source-render suite' >&2; exit 1
fi
if [ "${GCAD_SOURCE_SHADER_CHECK:-0}" = 1 ] && { [ "${GCAD_SOURCE_RENDER_CHECK:-0}" != 1 ] || [ "${GCAD_DEFORMATION_ADAPTER:-0}" != 1 ]; }; then
    echo 'Shader deformation checks require source rendering and deformation adapter' >&2; exit 1
fi
if [ "${GCAD_UV_ALL_MAPS:-0}" = 1 ] && [ "${GCAD_UV_RENDER_CHECK:-0}" != 1 ]; then
    echo 'All-map UV check requires GCAD_UV_RENDER_CHECK=1' >&2; exit 1
fi
if [ "${GCAD_UV_PLAYER_CHECK:-0}" = 1 ] && [ "${GCAD_UV_RENDER_CHECK:-0}" != 1 ]; then
    echo 'UV player check requires GCAD_UV_RENDER_CHECK=1' >&2; exit 1
fi
if [ "${GCAD_PLAYER_RENDER:-0}" = 1 ] && [ "${GCAD_PLAYER_CHECK:-0}" != 1 ]; then
    echo 'Rendered player check requires player build/check' >&2; exit 1
fi
if [ "${GCAD_PLAYER_RENDER:-0}" = 1 ] && [ "${GCAD_SOURCE_RENDER_CHECK:-0}" != 1 ]; then
    echo 'Rendered player parity requires editor source rendering' >&2; exit 1
fi
if [ "${GCAD_PLAYER_CHECK:-0}" = 1 ] && [ "${GCAD_DEFORMATION_ADAPTER:-0}" != 1 ]; then
    echo 'Player check requires the deformation adapter' >&2; exit 1
fi
if [ "${GCAD_DEFORMATION_ADAPTER:-0}" = 1 ] && [ "${GCAD_UNITY_ADAPTER_CHECK:-0}" != 1 ]; then
    echo 'Deformation adapter validation requires adapter check' >&2; exit 1
fi
if [ -n "${GCAD_IMPORT_DIAGNOSTIC:-}" ]; then
    [ "${GCAD_SOURCE_RENDER_CHECK:-0}" = 1 ] || { echo 'Import diagnostic requires source rendering' >&2; exit 1; }
    case "$GCAD_IMPORT_DIAGNOSTIC" in
        compression-off|source-tangents|source-tangents-uncompressed|source-tangents-uncompressed-zero-cutoff-request|source-tangents-flip|no-resampling) ;;
        *) echo 'Unknown fixture import diagnostic' >&2; exit 1 ;;
    esac
fi
if [ "${GCAD_SOURCE_RENDER_CHECK:-0}" = 1 ] && [ "${GCAD_UNITY_ADAPTER_CHECK:-0}" != 1 ]; then
    echo 'Source rendering requires GCAD_UNITY_ADAPTER_CHECK=1' >&2
    exit 1
fi
if [ "${GCAD_SURFACE_CHECK:-0}" = 1 ] && { [ "${GCAD_SAMPLER_CHECK:-0}" = 1 ] || [ -n "${GCAD_MATERIAL_REFERENCE:-}" ] || [ -n "${GCAD_POSE_REFERENCE:-}" ] || [ "${GCAD_RENDER_CHECK:-0}" = 1 ] || [ "${GCAD_DESCRIPTION_AUDIT:-0}" = 1 ]; }; then
    echo 'Run numerical surface checks separately' >&2
    exit 1
fi
if [ "${GCAD_SAMPLER_CHECK:-0}" = 1 ] && { [ -n "${GCAD_MATERIAL_REFERENCE:-}" ] || [ -n "${GCAD_POSE_REFERENCE:-}" ] || [ "${GCAD_RENDER_CHECK:-0}" = 1 ] || [ "${GCAD_DESCRIPTION_AUDIT:-0}" = 1 ]; }; then
    echo 'Run numerical sampler checks separately' >&2
    exit 1
fi
if [ "${GCAD_UNITY_ADAPTER_RENDER:-0}" = 1 ] && [ "${GCAD_UNITY_ADAPTER_CHECK:-0}" != 1 ]; then
    echo 'Adapter rendering requires GCAD_UNITY_ADAPTER_CHECK=1' >&2
    exit 1
fi
if [ "${GCAD_UNITY_ADAPTER_CHECK:-0}" = 1 ]; then
    : "${GCAD_MATERIAL_REFERENCE:?Adapter validation needs the source GLB reference}"
    : "${GCAD_FBX_HELPER:?Select the absolute reviewed material-inspection helper}"
fi
if [ "${GCAD_DESCRIPTION_AUDIT:-0}" = 1 ] && { [ -n "${GCAD_MATERIAL_REFERENCE:-}" ] || [ -n "${GCAD_POSE_REFERENCE:-}" ] || [ "${GCAD_RENDER_CHECK:-0}" = 1 ]; }; then
    echo 'Run material-description audit separately' >&2
    exit 1
fi
if [ -n "${GCAD_MATERIAL_REFERENCE:-}" ] && { [ -n "${GCAD_POSE_REFERENCE:-}" ] || [ "${GCAD_RENDER_CHECK:-0}" = 1 ]; }; then
    echo 'Run material settings separately from pose/render checks' >&2
    exit 1
fi
package=$(CDPATH= cd -- "$1" && pwd)
[ ! -e "$2" ] || { echo 'Preserving existing project' >&2; exit 1; }
[ -f "$package/model.fbx" ]
mkdir -p "$2/Assets/Editor" "$2/Assets/Fixture" "$2/Packages" "$2/ProjectSettings"
project=$(CDPATH= cd -- "$2" && pwd)
if [ -n "${GCAD_LOD_COST_BASELINE:-}" ]; then
    : "${GCAD_FBX_HELPER:?Select the reviewed helper for LOD cost measurement}"
    mkdir -p "$project/Assets/Baseline"
    cp "$GCAD_LOD_COST_BASELINE/model.fbx" "$project/Assets/Baseline/"
    cp -R "$GCAD_LOD_COST_BASELINE/textures" "$project/Assets/Baseline/"
fi
if [ -n "${GCAD_LOD_REFERENCE:-}" ]; then cp "$GCAD_LOD_REFERENCE" "$project/lod-reference.glb"; fi
cp "$root/tools/game-assets-unity/"*.cs "$project/Assets/Editor/"
mkdir -p "$project/Assets/GcadRuntime"
cp "$root/tools/game-assets-unity/Runtime/"*.cs "$project/Assets/GcadRuntime/"
mkdir -p "$project/Assets/GcadPlayerTest"
cp "$root/tools/game-assets-unity/PlayerTest/"*.cs "$project/Assets/GcadPlayerTest/"
mkdir -p "$project/Assets/GcadShaders"
cp "$root/tools/game-assets-unity/"*.shader "$root/tools/game-assets-unity/"*.cginc "$project/Assets/GcadShaders/"
cp "$package/model.fbx" "$project/Assets/Fixture/"
if [ -f "$package/gcad-shaders.json" ]; then cp "$package/gcad-shaders.json" "$project/Assets/Fixture/"; fi
if [ -f "$package/gcad-root-motion.json" ]; then cp "$package/gcad-root-motion.json" "$project/Assets/Fixture/"; fi
if [ -f "$package/gcad-consumer-profile.json" ]; then cp "$package/gcad-consumer-profile.json" "$project/Assets/Fixture/"; fi
if [ -d "$package/textures" ]; then cp -R "$package/textures" "$project/Assets/Fixture/"; else mkdir -p "$project/Assets/Fixture/textures"; fi
# Animation, JSON and image conversion are engine-bundled modules for the player probes.
# No registry package or additional engine installation is requested.
printf '%s\n' '{"dependencies":{"com.unity.modules.physics":"1.0.0","com.unity.modules.animation":"1.0.0","com.unity.modules.jsonserialize":"1.0.0","com.unity.modules.imageconversion":"1.0.0"}}' > "$project/Packages/manifest.json"
printf 'm_EditorVersion: %s\n' "$UNITY_VERSION" > "$project/ProjectSettings/ProjectVersion.txt"
method=GcadImportCheck.Run
report=import-report.json
if [ "${GCAD_DESCRIPTION_AUDIT:-0}" = 1 ]; then
    printf '%s\n' 'fixture-only' > "$project/gcad-material-description-audit.enabled"
    method=GcadMaterialDescriptionAudit.Run
    report=description-report.json
fi
if [ -n "${GCAD_MATERIAL_REFERENCE:-}" ]; then
    cp "$GCAD_MATERIAL_REFERENCE" "$project/material-reference.glb"
    method=GcadMaterialCheck.Run
    report=material-report.json
fi
if [ "${GCAD_UNITY_ADAPTER_CHECK:-0}" = 1 ]; then
    : "${GCAD_MATERIAL_REFERENCE:?Adapter validation needs the source GLB reference}"
    : "${GCAD_FBX_HELPER:?Select the absolute reviewed material-inspection helper}"
    method=GcadAdapterCheck.Run
    report=adapter-report.json
    if [ "${GCAD_UNITY_ADAPTER_RENDER:-0}" = 1 ]; then report=adapter-render-report.json; fi
    if [ "${GCAD_SOURCE_RENDER_CHECK:-0}" = 1 ]; then report=source-animation-report.json; fi
fi
if [ -n "${GCAD_POSE_REFERENCE:-}" ]; then
    cp "$GCAD_POSE_REFERENCE" "$project/pose-reference.json"
    method=GcadPoseCheck.Run
    report=pose-report.json
fi
if [ "${GCAD_RENDER_CHECK:-0}" = 1 ]; then
    method=GcadRenderCheck.Run
    if [ "${GCAD_UNITY_NORMAL_ADAPTER:-0}" = 1 ]; then method=GcadRenderCheck.RunWithNormalImport; fi
    report=render-report.json
fi
if [ "${GCAD_SAMPLER_CHECK:-0}" = 1 ]; then
    method=GcadSamplerCheck.Run
    report=sampler-report.json
fi
if [ "${GCAD_SURFACE_CHECK:-0}" = 1 ]; then
    method=GcadSurfaceCheck.Run
    report=surface-report.json
fi
if [ "${GCAD_UV_ATTACHMENT_CHECK:-0}" = 1 ]; then
    : "${GCAD_FBX_HELPER:?Select the reviewed helper for UV attachment}"
    test -f "$package/gcad-shaders.json"
    method=GcadUvScrollAttachmentCheck.Run
    report=uv-attachment-report.json
fi
if [ "${GCAD_UV_RENDER_CHECK:-0}" = 1 ]; then
    : "${GCAD_FBX_HELPER:?Select the reviewed helper for UV rendering}"
    test -f "$package/gcad-shaders.json"
    method=GcadUvScrollRenderCheck.Run
    report=uv-render-report.json
fi
if [ -n "${GCAD_LOD_COST_BASELINE:-}" ]; then
    method=GcadLodCostCheck.Run
    report=lod-cost-report.json
fi
if [ "${GCAD_ROOT_MOTION_CHECK:-0}" = 1 ]; then
    test -f "$package/gcad-root-motion.json"
    method=GcadRootMotionCheck.Run
    report=root-motion-report.json
fi
if [ "${GCAD_CONSUMER_CHECK:-0}" = 1 ]; then
    test -f "$package/gcad-consumer-profile.json"
    method=GcadConsumerCheck.Run
    report=consumer-report.json
fi
if [ "${GCAD_CONSUMER_RENDER:-0}" = 1 ] || [ "${GCAD_UV_RENDER_CHECK:-0}" = 1 ] || [ "${GCAD_RENDER_CHECK:-0}" = 1 ] || [ "${GCAD_UNITY_ADAPTER_RENDER:-0}" = 1 ] || [ "${GCAD_SAMPLER_CHECK:-0}" = 1 ] || [ "${GCAD_SURFACE_CHECK:-0}" = 1 ] || [ "${GCAD_SOURCE_RENDER_CHECK:-0}" = 1 ]; then
    "$UNITY" -batchmode -quit -force-d3d11 -projectPath "$project" \
        -executeMethod "$method" -logFile "$project/import.log"
else
    "$UNITY" -batchmode -nographics -quit -projectPath "$project" \
        -executeMethod "$method" -logFile "$project/import.log"
fi
# A zero editor exit is necessary, but not sufficient: require the report gate.
test -f "$project/$report"
grep -q '"passed": true' "$project/$report"
if [ "${GCAD_CONSUMER_FAILURE:-0}" = 1 ]; then
    grep -q '^    "passed": true' "$project/consumer-failure-report.json"
fi
if [ "${GCAD_CONSUMER_RENDER:-0}" = 1 ]; then
    grep -q '^    "passed": true' "$project/consumer-lod-report.json"
fi
if [ "${GCAD_CONSUMER_PLAYER:-0}" = 1 ]; then
    graphics=-nographics
    if [ "${GCAD_CONSUMER_RENDER:-0}" = 1 ]; then graphics=-force-d3d11; fi
    "$project/Player/gcad-consumer-test.exe" -batchmode "$graphics" -logFile "$project/Player/player.log"
    grep -q '"passed": true' "$project/Player/consumer-player-report.json"
    if [ "${GCAD_CONSUMER_RENDER:-0}" = 1 ]; then
        grep -q '^    "passed": true' "$project/Player/consumer-lod-report.json"
        for sample in level-0 level-1 level-2 forced-wrong-level forced-visible; do
            cmp "$project/ConsumerLod/$sample.png" "$project/Player/ConsumerLod/$sample.png"
        done
        if [ "${GCAD_CONSUMER_SKIN:-0}" = 1 ]; then
            grep -q '^    "passed": true' "$project/consumer-lod-posed-report.json"
            grep -q '^    "passed": true' "$project/Player/consumer-lod-posed-report.json"
            for sample in level-0 level-1 level-2 forced-wrong-level forced-visible; do
                cmp "$project/ConsumerLodPosed/$sample.png" "$project/Player/ConsumerLodPosed/$sample.png"
            done
            for sample in level-0 level-1; do
                if cmp -s "$project/ConsumerLod/$sample.png" "$project/ConsumerLodPosed/$sample.png"; then
                    echo 'Skinned LOD pose did not change the rendered image' >&2; exit 1
                fi
            done
        fi
    fi
fi
if [ "${GCAD_ROOT_MOTION_PLAYER:-0}" = 1 ]; then
    "$project/Player/gcad-root-motion-test.exe" -batchmode -nographics -logFile "$project/Player/player.log"
    grep -q '"passed": true' "$project/Player/root-motion-player-report.json"
fi
if [ -n "${GCAD_LOD_REFERENCE:-}" ]; then grep -q '"passed": true' "$project/lod-quality-report.json"; fi
if [ "${GCAD_UV_PLAYER_CHECK:-0}" = 1 ]; then
    grep -q '"passed": true' "$project/uv-player-build-report.json"
    "$project/Player/gcad-uv-test.exe" -batchmode -force-d3d11 -logFile "$project/Player/player.log" \
        >"$project/Player/player.stdout" 2>"$project/Player/player.stderr"
    grep -q '"passed": true' "$project/Player/uv-render-report.json"
    grep -q '"passed": true' "$project/Player/uv-lifecycle-report.json"
    for sample in 0 1 2 3 4; do
        for view in 0 1 2 3; do
            cmp "$project/UvRender/sample-$sample-view-$view.png" "$project/Player/UvRender/sample-$sample-view-$view.png"
        done
    done
    printf '%s\n' '{"passed": true, "identical_editor_player_images": 20}' > "$project/uv-editor-player-report.json"
fi
if [ "${GCAD_SURFACE_CHECK:-0}" = 1 ]; then
    grep -q '"passed": true' "$project/uv-scroll-report.json"
fi
if [ "${GCAD_PLAYER_CHECK:-0}" = 1 ]; then
    grep -q '"passed": true' "$project/player-build-report.json"
    if [ "${GCAD_PLAYER_RENDER:-0}" = 1 ]; then
        "$project/Player/gcad-adapter-test.exe" -batchmode -force-d3d11 -logFile "$project/Player/player.log" \
            >"$project/Player/player.stdout" 2>"$project/Player/player.stderr"
        grep -q '"passed": true' "$project/Player/player-render-report.json"
        "$UNITY" -batchmode -nographics -quit -projectPath "$project" \
            -executeMethod GcadEditorPlayerParityCheck.Run -logFile "$project/parity.log"
        grep -q '"passed": true' "$project/source-editor-player-report.json"
    else
        graphics=-nographics
        if [ "${GCAD_SOURCE_SHADER_CHECK:-0}" = 1 ]; then graphics=-force-d3d11; fi
        "$project/Player/gcad-adapter-test.exe" -batchmode "$graphics" -logFile "$project/Player/player.log" \
            >"$project/Player/player.stdout" 2>"$project/Player/player.stderr"
        grep -q '"passed": true' "$project/Player/player-report.json"
    fi
fi
