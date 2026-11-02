using System;
using System.IO;
using UnityEditor;
using UnityEditor.Build;
using UnityEditor.Build.Reporting;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class GcadUvScrollRenderCheck
{
    public static void Run() {
        PlayerSettings.colorSpace=ColorSpace.Linear;
        EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Single);
        var prefab=GcadMaterialAdapter.Create(Environment.GetEnvironmentVariable("GCAD_FBX_HELPER"),"Assets/Fixture/model.fbx","Assets/GcadAdapted");
        bool allMaps=Environment.GetEnvironmentVariable("GCAD_UV_ALL_MAPS")=="1";
        GcadUvRenderProbe.Run(prefab,Directory.GetCurrentDirectory(),allMaps);
        if(Environment.GetEnvironmentVariable("GCAD_UV_PLAYER_CHECK")!="1")return;
        if(!BuildPipeline.IsBuildTargetSupported(BuildTargetGroup.Standalone,BuildTarget.StandaloneWindows64))
            throw new Exception("Installed Windows player module required");
        // Own readable copies for the test's CPU UV reference. Leave FBX import settings intact.
        var instance=UnityEngine.Object.Instantiate(prefab);
        GameObject playerPrefab;
        try {
            int index=0;
            foreach(var filter in instance.GetComponentsInChildren<MeshFilter>(true)) {
                var source=filter.sharedMesh;var mesh=new Mesh{indexFormat=source.indexFormat};
                mesh.vertices=source.vertices;mesh.normals=source.normals;mesh.tangents=source.tangents;mesh.uv=source.uv;
                mesh.subMeshCount=source.subMeshCount;
                for(int i=0;i<source.subMeshCount;i++)mesh.SetTriangles(source.GetTriangles(i),i);
                mesh.bounds=source.bounds;
                AssetDatabase.CreateAsset(mesh,"Assets/GcadAdapted/readable_"+(index++)+".asset");filter.sharedMesh=mesh;
            }
            playerPrefab=PrefabUtility.SaveAsPrefabAsset(instance,"Assets/GcadAdapted/uv-player.prefab");
        } finally {UnityEngine.Object.DestroyImmediate(instance);}
        var scene=EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Single);
        var probe=new GameObject("UV player probe").AddComponent<GcadUvPlayerCheck>();probe.prefab=playerPrefab;probe.allMaps=allMaps;
        EditorSceneManager.SaveScene(scene,"Assets/GcadAdapted/uv-player.unity");AssetDatabase.SaveAssets();
        PlayerSettings.SetScriptingBackend(NamedBuildTarget.Standalone,ScriptingImplementation.Mono2x);
        PlayerSettings.runInBackground=true;
        var build=BuildPipeline.BuildPlayer(new BuildPlayerOptions{scenes=new[]{"Assets/GcadAdapted/uv-player.unity"},
            locationPathName="Player/gcad-uv-test.exe",target=BuildTarget.StandaloneWindows64,options=BuildOptions.StrictMode});
        if(build.summary.result!=BuildResult.Succeeded)throw new Exception("UV player build failed");
        File.WriteAllText("uv-player-build-report.json","{\"passed\": true}");
    }
}
