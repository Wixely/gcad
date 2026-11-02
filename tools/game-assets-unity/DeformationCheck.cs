using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEngine;

public static class GcadDeformationCheck
{
    [Serializable] public class Report {
        public string mesh;
        public int vertices,iterations; public double meanRefreshMilliseconds;
        public bool singularRejected,missingBoneRejected,lastGoodRetained,recovered,sourceMeshPreserved,passed;
    }
    [Serializable] public class Batch { public Report[] meshes; public bool passed; }
    public static void Run() {
        var instance=UnityEngine.Object.Instantiate(AssetDatabase.LoadAssetAtPath<GameObject>("Assets/GcadAdapted/model.prefab"));
        try {
            var reports=instance.GetComponentsInChildren<GcadDeformation>().Select(Check).ToArray();
            var batch=new Batch { meshes=reports,passed=reports.Length>0 && reports.All(r=>r.passed) };
            File.WriteAllText("deformation-report.json",JsonUtility.ToJson(batch,true));
            if(!batch.passed)throw new Exception("Deformation recovery or source preservation failed");
        } finally { UnityEngine.Object.DestroyImmediate(instance); }
    }
    static Report Check(GcadDeformation deformation) {
            if(!deformation.Refresh())throw new Exception(deformation.LastError);
            var restPositions=deformation.restMesh.vertices;var restNormals=deformation.restMesh.normals;
            var last=deformation.CurrentMesh.vertices;var normals=deformation.CurrentMesh.normals;
            var root=deformation.source.bones[0];var scale=root.localScale;
            var report=new Report { mesh=deformation.restMesh.name,vertices=last.Length,iterations=32 };
            try {
                root.localScale=Vector3.zero;
                report.singularRejected=!deformation.Refresh() && deformation.LastError.Contains("Singular");
                report.lastGoodRetained=last.SequenceEqual(deformation.CurrentMesh.vertices) && normals.SequenceEqual(deformation.CurrentMesh.normals);
            } finally { root.localScale=scale; }
            report.recovered=deformation.Refresh() && deformation.LastError==null;
            var bones=deformation.source.bones;var missing=(Transform[])bones.Clone();missing[0]=null;
            try {
                deformation.source.bones=missing;
                report.missingBoneRejected=!deformation.Refresh() && deformation.LastError.Contains("Missing");
                report.lastGoodRetained&=last.SequenceEqual(deformation.CurrentMesh.vertices);
            } finally { deformation.source.bones=bones; }
            report.recovered&=deformation.Refresh() && deformation.LastError==null;
            var clock=System.Diagnostics.Stopwatch.StartNew();
            for(int i=0;i<report.iterations;i++)if(!deformation.Refresh())throw new Exception(deformation.LastError);
            clock.Stop();report.meanRefreshMilliseconds=clock.Elapsed.TotalMilliseconds/report.iterations;
            report.sourceMeshPreserved=restPositions.SequenceEqual(deformation.restMesh.vertices) && restNormals.SequenceEqual(deformation.restMesh.normals);
            report.passed=report.singularRejected && report.missingBoneRejected && report.lastGoodRetained && report.recovered && report.sourceMeshPreserved;
            return report;
    }
}
