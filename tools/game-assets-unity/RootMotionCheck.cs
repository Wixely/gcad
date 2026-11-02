using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.Build.Reporting;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class GcadRootMotionCheck
{
    public static void CheckTimelineRefusal() {
        bool rejected=false;
        try {GcadRootMotionAdapter.Read("Assets/Fixture/model.fbx");}
        catch(NotSupportedException error){rejected=error.Message.StartsWith("Root-motion times differ");}
        File.WriteAllText("root-motion-timeline-refusal.json","{\"passed\": "+(rejected?"true":"false")+"}");
        if(!rejected)throw new Exception("A receipt with mismatched float32 times must require regeneration");
    }
    public static void CheckStepRefusal() {
        bool rejected=false;
        try { GcadRootMotionAdapter.Read("Assets/Fixture/model.fbx"); }
        catch(NotSupportedException error) { rejected=error.Message.StartsWith("STEP root-motion playback"); }
        File.WriteAllText("root-motion-step-refusal.json","{\"passed\": "+(rejected?"true":"false")+"}");
        if(!rejected)throw new Exception("STEP motion must be refused until imported boundaries are preserved");
    }
    [Serializable] class Key {public float time,value,inTangent,outTangent;}
    [Serializable] class Curve {public string clip,path,property;public Key[] keys;public float before,end;}
    public static void Diagnose() {
        var curves=AssetDatabase.LoadAllAssetsAtPath("Assets/Fixture/model.fbx").OfType<AnimationClip>().Where(c=>!c.name.StartsWith("__preview__"))
            .SelectMany(clip=>AnimationUtility.GetCurveBindings(clip).Select(binding=> {
                var curve=AnimationUtility.GetEditorCurve(clip,binding);
                return new Curve{clip=clip.name,path=binding.path,property=binding.propertyName,keys=curve.keys.Select(k=>new Key{time=k.time,value=k.value,inTangent=k.inTangent,outTangent=k.outTangent}).ToArray(),before=curve.Evaluate(1.99999f),end=curve.Evaluate(2)};
            })).ToArray();
        File.WriteAllText("root-motion-curves.json","["+string.Join(",",curves.Select(c=>JsonUtility.ToJson(c,true)))+"]");
    }
    [Serializable] public class Report {public GcadRootMotionProbe.Report playback;public bool digestRejection,trackRejection,publicationRejection,importedCurveControl=true,passed;}
    public static void Run() {
        const string path="Assets/Fixture/model.fbx";
        var importer=(ModelImporter)AssetImporter.GetAtPath(path);
        // Readability is needed only by the test's vertex oracle, not the adapter.
        importer.isReadable=true;importer.resampleCurves=false;importer.animationCompression=ModelImporterAnimationCompression.Off;importer.SaveAndReimport();
        var receipt=GcadRootMotionAdapter.Read(path);if(receipt==null)throw new Exception("Root-motion fixture required");
        var reference=Environment.GetEnvironmentVariable("GCAD_ROOT_MOTION_REFERENCE");
        using(var hash=System.Security.Cryptography.SHA256.Create())using(var stream=File.OpenRead(reference))
            if(BitConverter.ToString(hash.ComputeHash(stream)).Replace("-","").ToLowerInvariant()!=receipt.source_glb_sha256)throw new Exception("Source oracle digest mismatch");
        var prefab=AssetDatabase.LoadAssetAtPath<GameObject>(path);
        PlayerSettings.colorSpace=ColorSpace.Linear;
        var adapted=GcadMaterialAdapter.Create(Environment.GetEnvironmentVariable("GCAD_FBX_HELPER"),path,"Assets/GcadRootMotion");
        var actual=UnityEngine.Object.Instantiate(adapted);var expected=UnityEngine.Object.Instantiate(prefab);
        GcadRootMotionProbe.Oracle oracle=null;
        try {
            oracle=GcadRootMotionProbe.CreateOracle(expected,receipt.data.schema,receipt.data.duration,receipt.node_name,receipt.motion.translation_axes,receipt.source_clip,receipt.in_place_clip,
                (instance,clip,seconds)=>GcadSourceMeshReference.SampleTransforms(instance,reference,clip,seconds),GcadSourceMeshReference.KeyTimes(reference,receipt.source_clip));
            var report=new Report{playback=GcadRootMotionProbe.Run(actual,oracle)};
            if(receipt.version==2) {
                var control=UnityEngine.Object.Instantiate(prefab);
                double time=receipt.in_place_tracks[0].times_seconds[0];
                try {
                    receipt.in_place_tracks[0].times_seconds[0]=double.NaN;
                    try {GcadRootMotionAdapter.Attach(control,path,receipt);}catch(Exception){report.trackRejection=true;}
                } finally {receipt.in_place_tracks[0].times_seconds[0]=time;UnityEngine.Object.DestroyImmediate(control);}
                if(receipt.data.schema==1&&receipt.data.translationStep) {
                    var player=actual.GetComponent<GcadRootMotionPlayer>();player.exactTrackPlayback=false;
                    try {report.importedCurveControl=!GcadRootMotionProbe.Run(actual,oracle).passed;}
                    finally {player.exactTrackPlayback=true;}
                }
            } else report.trackRejection=true;
            string receiptPath="Assets/Fixture/gcad-root-motion.json";var original=File.ReadAllText(receiptPath);
            try {File.WriteAllText(receiptPath,original.Replace(receipt.model_sha256,new string('0',64)));
                try{GcadRootMotionAdapter.Read(path);}catch(Exception){report.digestRejection=true;}
            } finally {File.WriteAllText(receiptPath,original);}
            if(receipt.version==2) {
                double time=receipt.in_place_tracks[0].times_seconds[0];
                try {
                    receipt.in_place_tracks[0].times_seconds[0]=-1;
                    var serializer=new System.Runtime.Serialization.Json.DataContractJsonSerializer(typeof(GcadRootMotionAdapter.Receipt));
                    using(var stream=new MemoryStream()){serializer.WriteObject(stream,receipt);File.WriteAllBytes(receiptPath,stream.ToArray());}
                    bool rejected=false;
                    try {GcadMaterialAdapter.Create(Environment.GetEnvironmentVariable("GCAD_FBX_HELPER"),path,"Assets/RejectedMotion");}catch(Exception){rejected=true;}
                    report.publicationRejection=rejected&&!Directory.Exists("Assets/RejectedMotion")&&!File.Exists("Assets/RejectedMotion.meta");
                } finally {receipt.in_place_tracks[0].times_seconds[0]=time;File.WriteAllText(receiptPath,original);}
            } else report.publicationRejection=true;
            report.passed=report.playback.passed&&report.digestRejection&&report.trackRejection&&report.publicationRejection&&report.importedCurveControl;
            File.WriteAllText("root-motion-report.json",JsonUtility.ToJson(report,true));
            if(!report.passed)throw new Exception("Root-motion consumer gate failed; inspect root-motion-report.json");
        } finally {UnityEngine.Object.DestroyImmediate(actual);UnityEngine.Object.DestroyImmediate(expected);}
        if(Environment.GetEnvironmentVariable("GCAD_ROOT_MOTION_PLAYER")=="1") {
            if(!BuildPipeline.IsBuildTargetSupported(BuildTargetGroup.Standalone,BuildTarget.StandaloneWindows64))throw new Exception("Installed Windows player module required");
            var scene=EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Single);
            var check=new GameObject("root-motion player check").AddComponent<GcadRootMotionPlayerCheck>();
            check.actual=(GameObject)PrefabUtility.InstantiatePrefab(adapted);check.oracle=oracle;
            const string scenePath="Assets/GcadRootMotion/check.unity";EditorSceneManager.SaveScene(scene,scenePath);
            Directory.CreateDirectory("Player");
            var result=BuildPipeline.BuildPlayer(new BuildPlayerOptions{scenes=new[]{scenePath},locationPathName="Player/gcad-root-motion-test.exe",target=BuildTarget.StandaloneWindows64,options=BuildOptions.None});
            if(result.summary.result!=BuildResult.Succeeded)throw new Exception("Root-motion player build failed");
        }
    }
}
