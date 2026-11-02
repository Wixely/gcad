// Independent attribution probe. Pruned references never replace acceptance references.
using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class GcadSkinWeightDiagnostic
{
    [Serializable] public class Sample { public string clip; public float time,fullSourceDistance,prunedSourceDistance; }
    [Serializable] public class Report { public float cutoff; public Sample[] poses; }
    static Vector3[] Points(GameObject obj) {
        return obj.GetComponentsInChildren<MeshFilter>().SelectMany(f=>f.sharedMesh.vertices.Select(f.transform.TransformPoint)).ToArray();
    }
    static void DestroyReference(GameObject obj) {
        foreach(var filter in obj.GetComponentsInChildren<MeshFilter>())UnityEngine.Object.DestroyImmediate(filter.sharedMesh);
        UnityEngine.Object.DestroyImmediate(obj);
    }
    public static void Run() {
        var importer=(ModelImporter)AssetImporter.GetAtPath("Assets/Fixture/model.fbx");
        float cutoff=importer.minBoneWeight;
        var samples=GcadSourceMeshReference.Poses("material-reference.glb").Select(pose=> {
            EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Single);
            var imported=UnityEngine.Object.Instantiate(AssetDatabase.LoadAssetAtPath<GameObject>("Assets/GcadAdapted/model.prefab"));
            var full=GcadSourceMeshReference.Create("material-reference.glb",pose);
            var pruned=GcadSourceMeshReference.Create("material-reference.glb",pose,diagnosticWeightCutoff:cutoff);
            try {
                var clip=AssetDatabase.LoadAllAssetsAtPath("Assets/Fixture/model.fbx").OfType<AnimationClip>().Single(c=>c.name==pose.clip);
                clip.SampleAnimation(imported,pose.time);
                var deformations=imported.GetComponentsInChildren<GcadDeformation>();
                if(deformations.Length==0)throw new Exception("Weight diagnostic requires deformation adapter");
                foreach(var deformation in deformations)if(!deformation.Refresh())throw new Exception(deformation.LastError);
                var points=deformations.SelectMany(d=>d.CurrentMesh.vertices.Select(d.source.transform.TransformPoint)).ToArray();
                return new Sample { clip=pose.clip,time=pose.time,
                    fullSourceDistance=GcadPositionMetric.Distance(Points(full),points),
                    prunedSourceDistance=GcadPositionMetric.Distance(Points(pruned),points) };
            } finally { DestroyReference(full);DestroyReference(pruned);UnityEngine.Object.DestroyImmediate(imported); }
        }).ToArray();
        File.WriteAllText("skin-weight-diagnostic.json",JsonUtility.ToJson(new Report { cutoff=cutoff,poses=samples },true));
    }
}
