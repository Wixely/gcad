using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEngine;

public static class GcadPoseCheck
{
    [Serializable] public class Sample { public string clip; public float time; public Vector3[] points; }
    [Serializable] public class Reference { public int schema; public string coordinates; public Sample[] samples; }
    [Serializable] public class Result { public string clip; public float time, maxDistanceMetres; public int sourcePoints, unityPoints; }
    [Serializable] public class Report { public string unityVersion; public float toleranceMetres = 0.00001f; public Result[] samples; public bool passed; }
    public static float Distance(Vector3[] a, Vector3[] b) { return GcadPositionMetric.Distance(a,b); }
    public static void Run()
    {
        GcadImportCheck.Run();
        var reference = JsonUtility.FromJson<Reference>(File.ReadAllText("pose-reference.json"));
        if (reference.schema != 1 || reference.coordinates != "right-handed-y-up-metres" || reference.samples.Length != 21)
            throw new Exception("Unexpected pose reference contract");
        const string path = "Assets/Fixture/model.fbx";
        var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(path);
        var clips = AssetDatabase.LoadAllAssetsAtPath(path).OfType<AnimationClip>()
            .Where(c => !c.name.StartsWith("__preview__")).ToDictionary(c => c.name);
        var results = new List<Result>();
        foreach (var sample in reference.samples) {
            var instance = UnityEngine.Object.Instantiate(prefab);
            var points = new List<Vector3>();
            try {
                clips[sample.clip].SampleAnimation(instance, sample.time);
                foreach (var skin in instance.GetComponentsInChildren<SkinnedMeshRenderer>(true)) {
                    var baked = new Mesh();
                    try {
                        skin.BakeMesh(baked, false);
                        foreach (var local in baked.vertices) {
                            var world = skin.transform.TransformPoint(local);
                            // Convert Unity's FBX-imported left-handed world back
                            // to the source's right-handed Y-up metre coordinates.
                            points.Add(new Vector3(-world.x, world.y, world.z));
                        }
                    } finally { UnityEngine.Object.DestroyImmediate(baked); }
                }
            } finally { UnityEngine.Object.DestroyImmediate(instance); }
            var actual = points.ToArray();
            var error = Distance(sample.points, actual);
            if (results.Count == 0 && Distance(sample.points, actual.Select(p => p + Vector3.right * .1f).ToArray()) <= .00001f)
                throw new Exception("Pose metric accepted translated corruption");
            results.Add(new Result { clip=sample.clip, time=sample.time, maxDistanceMetres=error,
                sourcePoints=sample.points.Length, unityPoints=actual.Length });
        }
        var report = new Report { unityVersion=Application.unityVersion, samples=results.ToArray() };
        report.passed = results.All(r => r.maxDistanceMetres <= report.toleranceMetres);
        File.WriteAllText("pose-report.json", JsonUtility.ToJson(report, true));
        if (!report.passed) throw new Exception("Unity sampled pose differs from source; inspect pose-report.json");
    }
}
