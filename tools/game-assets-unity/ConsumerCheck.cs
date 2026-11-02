using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.Build.Reporting;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class GcadConsumerCheck
{
    public static void CheckPrecisionRefusal() {
        const string path="Assets/Fixture/model.fbx";
        var receipt=GcadConsumerAdapter.Read(path);var prefab=AssetDatabase.LoadAssetAtPath<GameObject>(path);
        receipt.profile.lods[0].levels[0].screen_height=.6000000001;receipt.profile.lods[0].levels[1].screen_height=.6;
        bool rejected=false;try{GcadConsumerAdapter.Validate(prefab,receipt);}catch(Exception){rejected=true;}
        File.WriteAllText("consumer-precision-refusal.json","{\"passed\": "+(rejected?"true":"false")+"}");
        if(!rejected)throw new Exception("Collapsed float32 LOD thresholds were accepted");
    }
    [Serializable] class Report {public GcadConsumerProbe.Report components;public bool digestRejection,referenceRejection,rendered=true,passed;}
    public static void Run() {
        const string path="Assets/Fixture/model.fbx",receiptPath="Assets/Fixture/gcad-consumer-profile.json";
        PlayerSettings.colorSpace=ColorSpace.Linear;
        var importer=(ModelImporter)AssetImporter.GetAtPath(path);importer.preserveHierarchy=true;importer.SaveAndReimport();
        var receipt=GcadConsumerAdapter.Read(path);if(receipt==null)throw new Exception("Consumer fixture required");
        string helper=Environment.GetEnvironmentVariable("GCAD_FBX_HELPER");
        if(Environment.GetEnvironmentVariable("GCAD_CONSUMER_FAILURE")=="1")GcadConsumerFailureCheck.Run(helper,path);
        var prefab=GcadMaterialAdapter.Create(helper,path,"Assets/GcadConsumer");
        var instance=UnityEngine.Object.Instantiate(prefab);
        var report=new Report();
        int expectedMeshColliders=receipt.profile.colliders.Count(c=>c.shape!="box");
        bool render=Environment.GetEnvironmentVariable("GCAD_CONSUMER_RENDER")=="1";
        bool skinned=Environment.GetEnvironmentVariable("GCAD_CONSUMER_SKIN")=="1";
        var shader=render?AssetDatabase.LoadAssetAtPath<Shader>("Assets/GcadShaders/ConsumerLodDiagnostic.shader"):null;
        try {
            report.components=GcadConsumerProbe.Run(instance,expectedMeshColliders,skinned);
            if(render){var rendered=GcadConsumerLodProbe.Run(instance,shader,"ConsumerLod");File.WriteAllText("consumer-lod-report.json",JsonUtility.ToJson(rendered,true));report.rendered=rendered.passed;}
            if(render&&skinned){var posed=GcadConsumerLodProbe.RunPosed(instance,shader,"ConsumerLodPosed");File.WriteAllText("consumer-lod-posed-report.json",JsonUtility.ToJson(posed,true));report.rendered&=posed.passed;}
        }finally{UnityEngine.Object.DestroyImmediate(instance);}
        var original=File.ReadAllText(receiptPath);
        try {
            File.WriteAllText(receiptPath,original.Replace(receipt.model_sha256,new string('0',64)));
            try{GcadConsumerAdapter.Read(path);}catch(Exception){report.digestRejection=true;}
            receipt.profile.lods[0].levels[1].nodes=new[]{"collision"};File.WriteAllText(receiptPath,JsonUtility.ToJson(receipt));
            bool refused=false;try{GcadMaterialAdapter.Create(helper,path,"Assets/RejectedConsumer");}catch(Exception){refused=true;}
            report.referenceRejection=refused&&!Directory.Exists("Assets/RejectedConsumer")&&!File.Exists("Assets/RejectedConsumer.meta");
        }finally{File.WriteAllText(receiptPath,original);}
        report.passed=report.components.passed&&report.digestRejection&&report.referenceRejection&&report.rendered;
        File.WriteAllText("consumer-report.json",JsonUtility.ToJson(report,true));
        if(!report.passed)throw new Exception("Consumer profile gate failed");
        if(Environment.GetEnvironmentVariable("GCAD_CONSUMER_PLAYER")=="1") {
            var scene=EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Single);
            var probe=new GameObject("consumer profile check").AddComponent<GcadConsumerPlayerCheck>();
            probe.instance=(GameObject)PrefabUtility.InstantiatePrefab(prefab);
            probe.expectedMeshColliders=expectedMeshColliders;
            probe.skinned=skinned;
            probe.render=render;probe.lodShader=shader;
            const string scenePath="Assets/GcadConsumer/check.unity";EditorSceneManager.SaveScene(scene,scenePath);Directory.CreateDirectory("Player");
            var build=BuildPipeline.BuildPlayer(new BuildPlayerOptions{scenes=new[]{scenePath},locationPathName="Player/gcad-consumer-test.exe",target=BuildTarget.StandaloneWindows64,options=BuildOptions.None});
            if(build.summary.result!=BuildResult.Succeeded)throw new Exception("Consumer player build failed");
        }
    }
}
