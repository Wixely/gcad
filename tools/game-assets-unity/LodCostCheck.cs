using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using UnityEditor;
using UnityEngine;

// Fixture measurement, not a production frame-time acceptance budget.
public static class GcadLodCostCheck
{
    [Serializable] public class Timing {public string clip;public float seconds;public double[] originalMilliseconds,lodMilliseconds;public double originalMedian,lodMedian;}
    [Serializable] public class Report {
        public string unityVersion,scope;public int originalVertices,lodVertices,originalTriangles,lodTriangles,warmups,batches,iterations;
        public Timing[] poses;public bool passed;
    }
    static GameObject Prepare(string model,string output) {
        var importer=(ModelImporter)AssetImporter.GetAtPath(model);
        importer.importNormals=ModelImporterNormals.Import;importer.importTangents=ModelImporterTangents.Import;
        importer.animationCompression=ModelImporterAnimationCompression.Off;importer.SaveAndReimport();
        var prefab=GcadMaterialAdapter.Create(Environment.GetEnvironmentVariable("GCAD_FBX_HELPER"),model,output,true);
        var instance=UnityEngine.Object.Instantiate(prefab);
        foreach(var animator in instance.GetComponentsInChildren<Animator>())animator.enabled=false;
        return instance;
    }
    static void Refresh(GcadDeformation[] components) {foreach(var component in components)if(!component.Refresh())throw new Exception(component.LastError);}
    static double Measure(GcadDeformation[] components,int iterations) {
        var clock=System.Diagnostics.Stopwatch.StartNew();for(int i=0;i<iterations;i++)Refresh(components);clock.Stop();
        return clock.Elapsed.TotalMilliseconds/iterations;
    }
    static double Median(double[] values){var sorted=values.OrderBy(v=>v).ToArray();return sorted[sorted.Length/2];}
    public static void Run() {
        PlayerSettings.colorSpace=ColorSpace.Linear;
        GameObject original=null,lod=null;
        var report=new Report{unityVersion=Application.unityVersion,warmups=4,batches=7,iterations=8,
            scope="Editor CPU GcadDeformation.Refresh wall time including mesh publication; excludes clip sampling, rendering, GPU, imports and allocations outside Refresh. Alternating paired batches. Fixture measurements, no production budget."};
        try {
            original=Prepare("Assets/Baseline/model.fbx","Assets/GcadOriginal");
            lod=Prepare("Assets/Fixture/model.fbx","Assets/GcadLod");
            var a=original.GetComponentsInChildren<GcadDeformation>();var b=lod.GetComponentsInChildren<GcadDeformation>();
            if(a.Length==0||a.Length!=b.Length)throw new Exception("Matching skinned mesh counts required");
            report.originalVertices=a.Sum(d=>d.restMesh.vertexCount);report.lodVertices=b.Sum(d=>d.restMesh.vertexCount);
            report.originalTriangles=a.Sum(d=>d.restMesh.triangles.Length/3);report.lodTriangles=b.Sum(d=>d.restMesh.triangles.Length/3);
            var source=AssetDatabase.LoadAllAssetsAtPath("Assets/Baseline/model.fbx").OfType<AnimationClip>().Where(c=>!c.name.StartsWith("__preview__")).OrderBy(c=>c.name).ToArray();
            var derived=AssetDatabase.LoadAllAssetsAtPath("Assets/Fixture/model.fbx").OfType<AnimationClip>().Where(c=>!c.name.StartsWith("__preview__")).ToDictionary(c=>c.name);
            if(source.Length==0||source.Length!=derived.Count)throw new Exception("Matching animation clips required");
            var poses=new List<Timing>();
            foreach(var clip in source) {
                if(!derived.ContainsKey(clip.name)||Math.Abs(clip.length-derived[clip.name].length)>1e-6)throw new Exception("Clip duration changed");
                foreach(float fraction in new[]{0f,.5f,1f}) {
                    var timing=new Timing{clip=clip.name,seconds=fraction*clip.length,originalMilliseconds=new double[report.batches],lodMilliseconds=new double[report.batches]};
                    clip.SampleAnimation(original,timing.seconds);derived[clip.name].SampleAnimation(lod,timing.seconds);
                    for(int warm=0;warm<report.warmups;warm++){Refresh(a);Refresh(b);}
                    for(int batch=0;batch<report.batches;batch++) {
                        if(batch%2==0){timing.originalMilliseconds[batch]=Measure(a,report.iterations);timing.lodMilliseconds[batch]=Measure(b,report.iterations);}
                        else {timing.lodMilliseconds[batch]=Measure(b,report.iterations);timing.originalMilliseconds[batch]=Measure(a,report.iterations);}
                    }
                    timing.originalMedian=Median(timing.originalMilliseconds);timing.lodMedian=Median(timing.lodMilliseconds);poses.Add(timing);
                }
            }
            report.poses=poses.ToArray();
            report.passed=report.lodVertices<report.originalVertices&&report.lodTriangles<report.originalTriangles&&poses.All(p=>p.originalMedian>0&&p.lodMedian>0);
        } finally {
            if(original)UnityEngine.Object.DestroyImmediate(original);if(lod)UnityEngine.Object.DestroyImmediate(lod);
            File.WriteAllText("lod-cost-report.json",JsonUtility.ToJson(report,true));
        }
        if(!report.passed)throw new Exception("LOD cost fixture did not complete or reduce geometry");
    }
}
