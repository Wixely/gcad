using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEngine;

public static class GcadConsumerFailureCheck
{
    [Serializable] class Report {public bool cookingRejected,outputRemoved,sourcePreserved,retry,existingPreserved,orphanMetaPreserved,passed;public string node,error;}
    [Serializable] class Summary {public Report[] cases;public bool passed;}
    public static void Run(string helper,string path) {
        var summary=new Summary{cases=new[]{RunCase(helper,path,"triangle_ring"),RunCase(helper,path,"convex_ring")}};
        summary.passed=summary.cases.All(c=>c.passed);
        File.WriteAllText("consumer-failure-report.json",JsonUtility.ToJson(summary,true));
        if(!summary.passed)throw new Exception("Consumer cooking recovery gate failed");
    }
    static Report RunCase(string helper,string path,string node) {
        string output="Assets/CookingRecovery_"+node;
        var report=new Report{node=node};var bytes=File.ReadAllBytes(path);var metadata=File.ReadAllBytes(path+".meta");
        var source=AssetDatabase.LoadAssetAtPath<GameObject>(path);
        var filter=source.GetComponentsInChildren<MeshFilter>(true).Single(f=>f.name==node);
        var original=filter.sharedMesh;var bad=UnityEngine.Object.Instantiate(original);
        // Nonempty, three-dimensional bounds, but every triangle is collinear:
        // passes role preflight and reaches the actual native cooker.
        bad.vertices=Enumerable.Range(0,bad.vertexCount).Select(i=>Vector3.one*(i*.001f)).ToArray();bad.RecalculateBounds();
        try {
            filter.sharedMesh=bad;
            try{GcadMaterialAdapter.Create(helper,path,output);}
            catch(Exception error){report.error=error.Message;report.cookingRejected=error.Message.Contains("Collider cooking failed for "+node);}
            report.outputRemoved=!Directory.Exists(output)&&!File.Exists(output+".meta");
        } finally {filter.sharedMesh=original;UnityEngine.Object.DestroyImmediate(bad);}
        report.sourcePreserved=File.ReadAllBytes(path).SequenceEqual(bytes)&&File.ReadAllBytes(path+".meta").SequenceEqual(metadata)&&filter.sharedMesh==original;
        if(report.cookingRejected&&report.outputRemoved) {
            var recovered=GcadMaterialAdapter.Create(helper,path,output);
            var instance=UnityEngine.Object.Instantiate(recovered);
            try {report.retry=GcadConsumerProbe.Run(instance,3).passed;}finally{UnityEngine.Object.DestroyImmediate(instance);}
            var prefabBytes=File.ReadAllBytes(output+"/model.prefab");
            try{GcadMaterialAdapter.Create(helper,path,output);}catch(Exception){report.existingPreserved=File.ReadAllBytes(output+"/model.prefab").SequenceEqual(prefabBytes);}
        }
        const string orphan="Assets/OrphanConsumer";File.WriteAllText(orphan+".meta","preserve existing metadata");
        try {try{GcadMaterialAdapter.Create(helper,path,orphan);}catch(Exception){report.orphanMetaPreserved=!Directory.Exists(orphan)&&File.ReadAllText(orphan+".meta")=="preserve existing metadata";}}
        finally {File.Delete(orphan+".meta");}
        report.passed=report.cookingRejected&&report.outputRemoved&&report.sourcePreserved&&report.retry&&report.existingPreserved&&report.orphanMetaPreserved;
        return report;
    }
}
