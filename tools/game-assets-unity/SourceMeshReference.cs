// Restricted audit loader: canonical gcad GLB rest geometry, independent of FBX/Assimp.
// This is not a production general-purpose glTF importer.
using System;
using System.IO;
using System.Linq;
using System.Text;
using UnityEditor;
using UnityEngine;
using UnityEngine.Rendering;

public static class GcadSourceMeshReference
{
    [Serializable] class View { public int buffer,byteOffset,byteLength,byteStride; }
    [Serializable] class Accessor { public int bufferView,byteOffset,componentType,count; public string type; public bool normalized; }
    [Serializable] class Attributes { public int POSITION=-1,NORMAL=-1,TANGENT=-1,TEXCOORD_0=-1,JOINTS_0=-1,WEIGHTS_0=-1; }
    [Serializable] class Primitive { public Attributes attributes; public int indices,material,mode=4; }
    [Serializable] class SourceMesh { public Primitive[] primitives; }
    [Serializable] class Node { public string name; public int mesh=-1,skin=-1; public int[] children; public float[] translation,rotation,scale,matrix; }
    [Serializable] class Skin { public int inverseBindMatrices; public int[] joints; }
    [Serializable] class SourceScene { public int[] nodes; }
    [Serializable] class Target { public int node; public string path; }
    [Serializable] class Channel { public int sampler; public Target target; }
    [Serializable] class Sampler { public int input,output; public string interpolation="LINEAR"; }
    [Serializable] class Clip { public string name; public Channel[] channels; public Sampler[] samplers; }
    [Serializable] class Image { public int bufferView; }
    [Serializable] class Source { public View[] bufferViews; public Accessor[] accessors; public SourceMesh[] meshes; public Node[] nodes; public Skin[] skins; public SourceScene[] scenes; public Clip[] animations; public Image[] images; public int scene; }
    [Serializable] public class Counts { public int triangles,skinnedNodes,bones,images,reflectedMeshNodes; public string[] clips; }
    public sealed class Pose { public string clip; public float time; }
    sealed class Reader {
        public Source source;
        byte[] binary;
        public Reader(string path) {
            using(var reader=new BinaryReader(File.OpenRead(path))) {
                if(reader.BaseStream.Length>64*1024*1024 || reader.ReadUInt32()!=0x46546c67 || reader.ReadUInt32()!=2 || reader.ReadUInt32()!=reader.BaseStream.Length)throw new Exception("Bounded GLB v2 required");
                int length=checked((int)reader.ReadUInt32());
                if(reader.ReadUInt32()!=0x4e4f534a || length>reader.BaseStream.Length-20)throw new Exception("Invalid JSON chunk");
                source=JsonUtility.FromJson<Source>(Encoding.UTF8.GetString(reader.ReadBytes(length)));
                int count=checked((int)reader.ReadUInt32());
                if(reader.ReadUInt32()!=0x004e4942 || count>reader.BaseStream.Length-reader.BaseStream.Position)throw new Exception("Embedded binary chunk required");
                binary=reader.ReadBytes(count);
            }
        }
        public float[][] Values(int index,string type,bool skinWeights=false) {
            var accessor=source.accessors[index];var view=source.bufferViews[accessor.bufferView];
            bool unorm16=skinWeights&&accessor.componentType==5123&&accessor.normalized;
            if(accessor.type!=type || (accessor.normalized&&!unorm16) || view.buffer!=0 || accessor.count<0 || accessor.count>1000000)throw new Exception("Unsupported audit accessor");
            if(skinWeights&&(type!="VEC4"||(!unorm16&&(accessor.componentType!=5126||accessor.normalized))))throw new Exception("Float or normalized unsigned-short skin weights required");
            int components=type=="SCALAR" ? 1 : type=="VEC2" ? 2 : type=="VEC3" ? 3 : type=="VEC4" ? 4 : type=="MAT4" ? 16 : 0;
            int bytes=accessor.componentType==5123 ? 2 : accessor.componentType==5125 || accessor.componentType==5126 ? 4 : 0;
            if(components==0 || bytes==0)throw new Exception("Unsupported audit component type");
            int stride=view.byteStride==0 ? bytes*components : view.byteStride;
            if(stride<bytes*components)throw new Exception("Invalid accessor stride");
            var result=new float[accessor.count][];
            for(int i=0;i<result.Length;i++) {
                int relative=checked(accessor.byteOffset+i*stride),at=checked(view.byteOffset+relative);
                if(relative<0 || relative+components*bytes>view.byteLength || at<0 || at+components*bytes>binary.Length)throw new Exception("Accessor outside binary view");
                result[i]=new float[components];
                for(int c=0;c<components;c++) {
                    int offset=at+c*bytes;
                    result[i][c]=accessor.componentType==5126 ? BitConverter.ToSingle(binary,offset) : accessor.componentType==5125 ? BitConverter.ToUInt32(binary,offset) : BitConverter.ToUInt16(binary,offset);
                    if(unorm16)result[i][c]/=65535f;
                    if(float.IsNaN(result[i][c])||float.IsInfinity(result[i][c]))throw new Exception("Nonfinite source value");
                }
                if(skinWeights&&(result[i].Any(w=>w<0||w>1)||Math.Abs(result[i].Sum()-1)>1e-5))throw new Exception("Invalid normalized source skin weights");
            }
            return result;
        }
    }
    static Vector3 V3(float[] v) { return new Vector3(v[0],v[1],v[2]); }
    static Matrix4x4 Matrix(float[] v) {
        var result=new Matrix4x4();for(int row=0;row<4;row++)for(int col=0;col<4;col++)result[row,col]=v[col*4+row];return result;
    }
    static void World(Source source,int index,Matrix4x4 parent,Matrix4x4[] world,bool[] visited) {
        if(visited[index])throw new Exception("Repeated/cyclic source node");visited[index]=true;
        var node=source.nodes[index];
        var local=node.matrix!=null ? Matrix(node.matrix) : Matrix4x4.TRS(node.translation==null ? Vector3.zero : V3(node.translation),
            node.rotation==null ? Quaternion.identity : new Quaternion(node.rotation[0],node.rotation[1],node.rotation[2],node.rotation[3]),node.scale==null ? Vector3.one : V3(node.scale));
        world[index]=parent*local;
        if(node.children!=null)foreach(int child in node.children)World(source,child,world[index],world,visited);
    }
    public static Pose[] Poses(string path) {
        var reader=new Reader(path);
        return (reader.source.animations ?? new Clip[0]).SelectMany(clip=> {
            float duration=clip.samplers.Max(s=>reader.Values(s.input,"SCALAR").Last()[0]);
            return Enumerable.Range(0,7).Select(i=>new Pose { clip=clip.name,time=duration*i/6f });
        }).ToArray();
    }
    public static Counts Describe(string path) {
        var reader=new Reader(path);var source=reader.source;
        var world=new Matrix4x4[source.nodes.Length];var visited=new bool[source.nodes.Length];
        foreach(int root in source.scenes[source.scene].nodes)World(source,root,Matrix4x4.identity,world,visited);
        var result=new Counts { images=source.images.Length,clips=(source.animations ?? new Clip[0]).Select(c=>c.name).OrderBy(n=>n).ToArray() };
        for(int n=0;n<source.nodes.Length;n++) {
            var node=source.nodes[n];if(!visited[n] || node.mesh<0)continue;
            foreach(var primitive in source.meshes[node.mesh].primitives) {
                int count=source.accessors[primitive.indices].count;
                if(primitive.mode!=4 || count<=0 || count%3!=0)throw new Exception("Indexed triangle source required");
                result.triangles=checked(result.triangles+count/3);
            }
            if(world[n].determinant<0)result.reflectedMeshNodes++;
            if(node.skin>=0){result.skinnedNodes++;result.bones+=source.skins[node.skin].joints.Length;}
        }
        return result;
    }
    static void Animate(Reader reader,Pose pose,double? exactTime=null) {
        double sampleTime=exactTime??pose.time;
        var clip=reader.source.animations.Single(c=>c.name==pose.clip);
        foreach(var channel in clip.channels) {
            var sampler=clip.samplers[channel.sampler];var node=reader.source.nodes[channel.target.node];
            if(node.matrix!=null || (sampler.interpolation!="LINEAR"&&sampler.interpolation!="STEP"))throw new Exception("Audit requires LINEAR or STEP TRS animation");
            bool rotation=channel.target.path=="rotation";
            var times=reader.Values(sampler.input,"SCALAR").Select(v=>v[0]).ToArray();
            var values=reader.Values(sampler.output,rotation ? "VEC4" : "VEC3");
            if(times.Length==0 || times.Length!=values.Length)throw new Exception("Invalid animation keys");
            for(int i=1;i<times.Length;i++)if(times[i]<=times[i-1])throw new Exception("Unordered animation keys");
            int lo=0;while(lo+1<times.Length && times[lo+1]<=sampleTime)lo++;
            int hi=Math.Min(lo+1,times.Length-1);
            float t=hi==lo||sampler.interpolation=="STEP" ? 0 : (float)Math.Max(0,Math.Min(1,(sampleTime-times[lo])/(times[hi]-times[lo])));
            if(rotation) {
                var a=values[lo];var b=values[hi];
                var q=Quaternion.Slerp(new Quaternion(a[0],a[1],a[2],a[3]),new Quaternion(b[0],b[1],b[2],b[3]),t).normalized;
                node.rotation=new[]{q.x,q.y,q.z,q.w};
            } else {
                var v=Vector3.Lerp(V3(values[lo]),V3(values[hi]),t);var value=new[]{v.x,v.y,v.z};
                if(channel.target.path=="translation")node.translation=value;
                else if(channel.target.path=="scale")node.scale=value;
                else throw new Exception("Unsupported animated path");
            }
        }
    }
    // Test-only direct source sampler, independent of FBX curves and sidecars.
    public static void SampleTransforms(GameObject instance,string path,string clip,double seconds) {
        var reader=new Reader(path);Animate(reader,new Pose{clip=clip},seconds);
        var targets=instance.GetComponentsInChildren<Transform>(true);
        foreach(var node in reader.source.nodes) {
            var target=targets.Single(t=>t.name==node.name);
            if(node.matrix!=null)throw new Exception("TRS oracle fixture required");
            var p=node.translation??new float[]{0,0,0};var q=node.rotation??new float[]{0,0,0,1};var s=node.scale??new float[]{1,1,1};
            target.localPosition=new Vector3(-p[0],p[1],p[2]);
            target.localRotation=new Quaternion(q[0],-q[1],-q[2],q[3]);
            target.localScale=V3(s);
        }
    }
    public static double[] KeyTimes(string path,string clip) {
        var reader=new Reader(path);
        return reader.source.animations.Single(c=>c.name==clip).samplers
            .SelectMany(s=>reader.Values(s.input,"SCALAR").Select(v=>(double)v[0])).Distinct().OrderBy(t=>t).ToArray();
    }
    public static GameObject Create(string path,Pose pose=null,bool linearSkinNormals=false,bool orthogonalizeTangents=true,float diagnosticWeightCutoff=0) {
        var reader=new Reader(path);var source=reader.source;
        if(pose!=null)Animate(reader,pose);
        var world=new Matrix4x4[source.nodes.Length];var visited=new bool[source.nodes.Length];
        foreach(int root in source.scenes[source.scene].nodes)World(source,root,Matrix4x4.identity,world,visited);
        var mirror=Matrix4x4.Scale(new Vector3(-1,1,1));
        var result=new GameObject("Direct GLB rest reference");
        for(int n=0;n<source.nodes.Length;n++) {
            var node=source.nodes[n];if(!visited[n] || node.mesh<0)continue;
            foreach(var primitive in source.meshes[node.mesh].primitives) {
                if(primitive.mode!=4)throw new Exception("Triangle fixture required");
                var a=primitive.attributes;
                var positions=reader.Values(a.POSITION,"VEC3");var normals=reader.Values(a.NORMAL,"VEC3");
                var tangents=reader.Values(a.TANGENT,"VEC4");var uvs=reader.Values(a.TEXCOORD_0,"VEC2");
                var indices=reader.Values(primitive.indices,"SCALAR").Select(v=>checked((int)v[0])).ToArray();
                var transforms=Enumerable.Repeat(world[n],positions.Length).ToArray();
                if(node.skin>=0) {
                    var skin=source.skins[node.skin];var binds=reader.Values(skin.inverseBindMatrices,"MAT4").Select(Matrix).ToArray();
                    var joints=reader.Values(a.JOINTS_0,"VEC4");var weights=reader.Values(a.WEIGHTS_0,"VEC4",true);
                    for(int v=0;v<positions.Length;v++) {
                        // Diagnostic only: acceptance callers retain the complete source weights.
                        if(diagnosticWeightCutoff>0) {
                            float sum=0;
                            for(int slot=0;slot<4;slot++) {
                                if(weights[v][slot]<diagnosticWeightCutoff)weights[v][slot]=0;
                                sum+=weights[v][slot];
                            }
                            if(sum<=0)throw new Exception("Diagnostic weight cutoff removed every influence");
                            for(int slot=0;slot<4;slot++)weights[v][slot]/=sum;
                        }
                        var blended=new Matrix4x4();
                        for(int slot=0;slot<4;slot++) {
                            var joint=world[skin.joints[(int)joints[v][slot]]]*binds[(int)joints[v][slot]];
                            for(int k=0;k<16;k++)blended[k]+=joint[k]*weights[v][slot];
                        }
                        if(pose==null && (blended.MultiplyPoint3x4(V3(positions[v]))-world[n].MultiplyPoint3x4(V3(positions[v]))).magnitude>1e-6f)throw new Exception("Source rest skin is not bind-neutral");
                        transforms[v]=blended;
                    }
                }
                var transform=mirror*world[n];
                var mesh=new Mesh { indexFormat=IndexFormat.UInt32 };
                mesh.vertices=positions.Select((v,i)=>(mirror*transforms[i]).MultiplyPoint3x4(V3(v))).ToArray();
                mesh.normals=normals.Select((v,i)=> {
                    var matrix=mirror*transforms[i];
                    return (linearSkinNormals && node.skin>=0 ? matrix : matrix.inverse.transpose).MultiplyVector(V3(v)).normalized;
                }).ToArray();
                // Reflecting X and flipping texture V each reverse tangent handedness.
                var posedNormals=mesh.normals;
                mesh.tangents=tangents.Select((v,i)=> {var t=(mirror*transforms[i]).MultiplyVector(V3(v));if(orthogonalizeTangents)t-=posedNormals[i]*Vector3.Dot(t,posedNormals[i]);t=t.normalized;return new Vector4(t.x,t.y,t.z,v[3]*(transforms[i].determinant<0 ? -1 : 1));}).ToArray();
                mesh.uv=uvs.Select(v=>new Vector2(v[0],1-v[1])).ToArray();
                if(transform.determinant<0)for(int i=0;i<indices.Length;i+=3){int swap=indices[i+1];indices[i+1]=indices[i+2];indices[i+2]=swap;}
                mesh.triangles=indices;mesh.RecalculateBounds();
                var child=new GameObject(node.name+" material "+primitive.material);child.transform.SetParent(result.transform,false);
                child.AddComponent<MeshFilter>().sharedMesh=mesh;
                child.AddComponent<MeshRenderer>().sharedMaterial=AssetDatabase.LoadAssetAtPath<Material>("Assets/GcadAdapted/material_"+primitive.material+".mat");
            }
        }
        return result;
    }
}
