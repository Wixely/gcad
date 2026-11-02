// Restore only unambiguous FBX influences into an owned mesh. Never edit the importer mesh.
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEngine;

public static class GcadFbxSkinWeights
{
    [Serializable] public class Vertex { public float[] position,weights; public int[] joints; }
    [Serializable] public class Skin { public string name; public string[] bones; public Vertex[] vertices; }
    [Serializable] public class Contract { public int protocol; public Skin[] meshes; }
    [Serializable] class Identity { public string[] capabilities; }
    const float Tolerance=1e-7f;
    static bool Finite(float value) { return !float.IsNaN(value)&&!float.IsInfinity(value); }
    // Called after the material contract has verified the executable identity and filename.
    public static Contract Read(string helper,string model) {
        var identity=JsonUtility.FromJson<Identity>(GcadFbxMaterialContract.Run(helper,"--handshake",65536));
        if(identity==null || identity.capabilities==null || !identity.capabilities.Contains("skin-inspection-v1-experimental"))
            throw new Exception("Rebuild the reviewed helper with skin inspection support");
        var result=JsonUtility.FromJson<Contract>(GcadFbxMaterialContract.Run(helper,"--inspect-skin-v1 \""+Path.GetFullPath(model)+"\"",64*1024*1024));
        if(result==null || result.protocol!=1 || result.meshes==null)throw new Exception("Invalid skin inspection response");
        int total=0;
        foreach(var skin in result.meshes) {
            if(skin==null || string.IsNullOrEmpty(skin.name) || skin.bones==null || skin.vertices==null ||
                skin.bones.Any(string.IsNullOrEmpty) || skin.bones.Distinct().Count()!=skin.bones.Length)
                throw new Exception("Invalid skin names or arrays");
            total+=skin.vertices.Length;if(total>1000000)throw new Exception("Skin vertex count exceeds bound");
            foreach(var v in skin.vertices)if(v==null || v.position==null || v.position.Length!=3 || v.position.Any(x=>!Finite(x)) ||
                v.weights==null || v.joints==null || v.weights.Length<1 || v.weights.Length>4 || v.joints.Length!=v.weights.Length ||
                v.weights.Any(x=>!Finite(x)||x<=0) || Math.Abs(v.weights.Sum()-1)>1e-4f ||
                v.joints.Any(j=>j<0||j>=skin.bones.Length) || v.joints.Distinct().Count()!=v.joints.Length)
                throw new Exception("Invalid inspected skin vertex");
        }
        return result;
    }
    struct Candidate { public Vector3 point; public float[] weights; }
    static Vector3Int Cell(Vector3 p) { return new Vector3Int((int)Math.Floor(p.x/Tolerance),(int)Math.Floor(p.y/Tolerance),(int)Math.Floor(p.z/Tolerance)); }
    public static BoneWeight[] Recover(Contract contract,SkinnedMeshRenderer renderer,float cutoff) {
        var mesh=renderer.sharedMesh;var bones=renderer.bones;
        if(bones.Any(b=>!b) || bones.Select(b=>b.name).Distinct().Count()!=bones.Length)throw new Exception("Unique imported bone names required");
        var lookup=bones.Select((b,i)=>new {b.name,index=i}).ToDictionary(b=>b.name,b=>b.index);
        var cells=new Dictionary<Vector3Int,List<Candidate>>();
        foreach(var skin in contract.meshes.Where(s=>s.name==renderer.name)) {
            if(skin.bones.Any(b=>!lookup.ContainsKey(b)))throw new Exception("FBX bone missing from imported palette");
            foreach(var v in skin.vertices) {
                var point=new Vector3(-v.position[0],v.position[1],v.position[2]);
                if(Math.Max(Math.Abs(point.x),Math.Max(Math.Abs(point.y),Math.Abs(point.z)))>100)
                    throw new Exception("Skin recovery world coordinates exceed supported spatial bound");
                var weights=new float[bones.Length];for(int i=0;i<v.joints.Length;i++)weights[lookup[skin.bones[v.joints[i]]]]=v.weights[i];
                var key=Cell(point);List<Candidate> bucket;if(!cells.TryGetValue(key,out bucket))cells.Add(key,bucket=new List<Candidate>());
                bucket.Add(new Candidate { point=point,weights=weights });
            }
        }
        var original=mesh.boneWeights;var positions=mesh.vertices;var result=new BoneWeight[positions.Length];
        for(int v=0;v<positions.Length;v++) {
            var point=renderer.transform.TransformPoint(positions[v]);var cell=Cell(point);float[] found=null;
            var before=new float[bones.Length];var bw=original[v];
            before[bw.boneIndex0]+=bw.weight0;before[bw.boneIndex1]+=bw.weight1;before[bw.boneIndex2]+=bw.weight2;before[bw.boneIndex3]+=bw.weight3;
            for(int x=-1;x<=1;x++)for(int y=-1;y<=1;y++)for(int z=-1;z<=1;z++) {
                List<Candidate> bucket;if(!cells.TryGetValue(cell+new Vector3Int(x,y,z),out bucket))continue;
                foreach(var candidate in bucket) {
                    if((candidate.point-point).sqrMagnitude>Tolerance*Tolerance)continue;
                    var pruned=candidate.weights.Select(w=>w<cutoff?0:w).ToArray();float sum=pruned.Sum();
                    if(sum<=0 || pruned.Where((w,i)=>Math.Abs(w/sum-before[i])>1e-5f).Any())continue;
                    if(found!=null && found.Where((w,i)=>Math.Abs(w-candidate.weights[i])>1e-7f).Any())
                        throw new Exception("Ambiguous coincident FBX skin weights; refusing to guess");
                    found=candidate.weights;
                }
            }
            if(found==null)throw new Exception("No unambiguous FBX weight match for "+renderer.name+" vertex "+v);
            var influences=found.Select((w,i)=>new {weight=w,index=i}).Where(w=>w.weight>0).OrderByDescending(w=>w.weight).ToArray();
            var value=new BoneWeight();
            for(int i=0;i<influences.Length;i++)switch(i) {
                case 0:value.boneIndex0=influences[i].index;value.weight0=influences[i].weight;break;
                case 1:value.boneIndex1=influences[i].index;value.weight1=influences[i].weight;break;
                case 2:value.boneIndex2=influences[i].index;value.weight2=influences[i].weight;break;
                case 3:value.boneIndex3=influences[i].index;value.weight3=influences[i].weight;break;
                default:throw new Exception("More than four recovered influences");
            }
            result[v]=value;
        }
        return result;
    }
}
