using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.Build;
using UnityEditor.Build.Reporting;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class GcadPlayerBuildCheck
{
    [Serializable] class Report { public string unityVersion,result; public bool passed; }
    public static void Run() {
        if(!BuildPipeline.IsBuildTargetSupported(BuildTargetGroup.Standalone,BuildTarget.StandaloneWindows64))
            throw new Exception("Installed Windows player module required");
        var source=AssetDatabase.LoadAssetAtPath<GameObject>("Assets/GcadAdapted/model.prefab");
        var instance=(GameObject)PrefabUtility.InstantiatePrefab(source);
        GameObject prefab;
        try {
            var controller=AnimatorController.CreateAnimatorControllerAtPath("Assets/GcadAdapted/player.controller");
            var state=controller.layers[0].stateMachine.AddState("bend");
            state.motion=AssetDatabase.LoadAllAssetsAtPath("Assets/Fixture/model.fbx").OfType<AnimationClip>().Single(c=>c.name=="bend");
            controller.layers[0].stateMachine.defaultState=state;
            var animator=instance.GetComponentInChildren<Animator>();
            if(!animator)animator=instance.AddComponent<Animator>();
            animator.runtimeAnimatorController=controller;animator.cullingMode=AnimatorCullingMode.AlwaysAnimate;
            prefab=PrefabUtility.SaveAsPrefabAsset(instance,"Assets/GcadAdapted/player.prefab");
        } finally { UnityEngine.Object.DestroyImmediate(instance); }
        var scene=EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Single);
        if(Environment.GetEnvironmentVariable("GCAD_PLAYER_RENDER")=="1") {
            var check=new GameObject("gcad rendered player check").AddComponent<GcadPlayerRenderCheck>();check.prefab=prefab;
            check.shaderCheck=Environment.GetEnvironmentVariable("GCAD_SOURCE_SHADER_CHECK")=="1";
            var poses=GcadSourceMeshReference.Poses("material-reference.glb");
            var clips=AssetDatabase.LoadAllAssetsAtPath("Assets/Fixture/model.fbx").OfType<AnimationClip>().Where(c=>!c.name.StartsWith("__preview__")).ToDictionary(c=>c.name);
            check.poses=poses.Select((pose,index)=> {
                var reference=GcadSourceMeshReference.Create("material-reference.glb",pose);
                try {
                    int mesh=0;
                    if(check.shaderCheck)foreach(var filter in reference.GetComponentsInChildren<MeshFilter>())
                        filter.sharedMesh.uv=filter.sharedMesh.uv.Select(uv=>new Vector2(uv.x+(pose.time-2)*.125f,uv.y-(pose.time-2)*.25f)).ToArray();
                    foreach(var filter in reference.GetComponentsInChildren<MeshFilter>())
                        AssetDatabase.CreateAsset(filter.sharedMesh,"Assets/GcadAdapted/reference_"+index+"_"+(mesh++)+".asset");
                    var snapshot=PrefabUtility.SaveAsPrefabAsset(reference,"Assets/GcadAdapted/reference_"+index+".prefab");
                    return new GcadPlayerRenderCheck.Pose { reference=snapshot,clip=clips[pose.clip],time=pose.time };
                } finally { UnityEngine.Object.DestroyImmediate(reference); }
            }).ToArray();
        } else new GameObject("gcad player lifecycle check").AddComponent<GcadPlayerCheck>().prefab=prefab;
        EditorSceneManager.SaveScene(scene,"Assets/GcadAdapted/player.unity");AssetDatabase.SaveAssets();
        PlayerSettings.SetScriptingBackend(NamedBuildTarget.Standalone,ScriptingImplementation.Mono2x);
        PlayerSettings.runInBackground=true;
        var result=BuildPipeline.BuildPlayer(new BuildPlayerOptions {
            scenes=new[]{"Assets/GcadAdapted/player.unity"},locationPathName="Player/gcad-adapter-test.exe",
            target=BuildTarget.StandaloneWindows64,options=BuildOptions.StrictMode
        });
        var report=new Report { unityVersion=Application.unityVersion,result=result.summary.result.ToString(),passed=result.summary.result==BuildResult.Succeeded };
        File.WriteAllText("player-build-report.json",JsonUtility.ToJson(report,true));
        if(!report.passed)throw new Exception("Player build failed");
    }
}
