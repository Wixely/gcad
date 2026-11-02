using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEngine;

public static class GcadSkinWeightRecoveryCheck
{
    [Serializable] class Report { public int meshes,changedVertices; public bool shiftedRejected,ambiguousRejected,sourcePreserved,passed; }
    static GcadFbxSkinWeights.Contract Copy(GcadFbxSkinWeights.Contract value) {
        return JsonUtility.FromJson<GcadFbxSkinWeights.Contract>(JsonUtility.ToJson(value));
    }
    static bool Reject(Action action,string message) {
        try { action();return false; } catch(Exception error) { return error.Message.Contains(message); }
    }
    public static void Run() {
        const string path="Assets/Fixture/model.fbx";
        var contract=GcadFbxSkinWeights.Read(Environment.GetEnvironmentVariable("GCAD_FBX_HELPER"),path);
        float cutoff=((ModelImporter)AssetImporter.GetAtPath(path)).minBoneWeight;
        var instance=UnityEngine.Object.Instantiate(AssetDatabase.LoadAssetAtPath<GameObject>(path));
        var report=new Report {sourcePreserved=true,shiftedRejected=true,ambiguousRejected=true};
        try {
            foreach(var renderer in instance.GetComponentsInChildren<SkinnedMeshRenderer>()) {
                var before=renderer.sharedMesh.boneWeights;
                var recovered=GcadFbxSkinWeights.Recover(contract,renderer,cutoff);report.meshes++;
                report.changedVertices+=before.Where((w,i)=>!w.Equals(recovered[i])).Count();
                var shifted=Copy(contract);foreach(var mesh in shifted.meshes)foreach(var v in mesh.vertices)v.position[0]+=1;
                report.shiftedRejected&=Reject(()=>GcadFbxSkinWeights.Recover(shifted,renderer,cutoff),"No unambiguous FBX weight match");
                var ambiguous=Copy(contract);var skin=ambiguous.meshes.First(s=>s.name==renderer.name);
                var rigid=skin.vertices.First(v=>v.weights.Length==1);
                int other=Enumerable.Range(0,skin.bones.Length).First(i=>i!=rigid.joints[0]);
                var extra=new GcadFbxSkinWeights.Vertex {position=(float[])rigid.position.Clone(),
                    joints=new[]{rigid.joints[0],other},weights=new[]{1-cutoff*.5f,cutoff*.5f}};
                skin.vertices=skin.vertices.Concat(new[]{extra}).ToArray();
                report.ambiguousRejected&=Reject(()=>GcadFbxSkinWeights.Recover(ambiguous,renderer,cutoff),"Ambiguous coincident FBX skin weights");
                report.sourcePreserved&=before.SequenceEqual(renderer.sharedMesh.boneWeights);
            }
            report.passed=report.meshes>0 && report.shiftedRejected && report.ambiguousRejected && report.sourcePreserved;
            File.WriteAllText("skin-weight-recovery-report.json",JsonUtility.ToJson(report,true));
            if(!report.passed)throw new Exception("Skin recovery corruption/source-preservation checks failed");
        } finally {UnityEngine.Object.DestroyImmediate(instance);}
    }
}
