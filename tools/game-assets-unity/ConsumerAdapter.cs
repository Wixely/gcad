using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Security.Cryptography;
using UnityEditor;
using UnityEngine;

// Opt-in versioned roles. Generic FBX is never renamed or mutated.
public static class GcadConsumerAdapter
{
    // Unity's cooking API returns void. Capture synchronous native diagnostics
    // during attachment (AddComponent can auto-assign MeshFilter).
    static void Cook(string node,Action action) {
        var diagnostics=new List<string>();
        Application.LogCallback capture=(message,trace,type)=>{
            if(type!=LogType.Log)diagnostics.Add(message);
        };
        Application.logMessageReceived+=capture;
        try {action();}
        finally {Application.logMessageReceived-=capture;}
        if(diagnostics.Count!=0)throw new Exception("Collider cooking failed for "+node+": "+string.Join("; ",diagnostics));
    }
    [Serializable] public class Socket {public string name,node;}
    [Serializable] public class Collision {public string node,shape;}
    [Serializable] public class Level {public double screen_height;public string[] nodes;}
    [Serializable] public class Lod {public string name,root;public Level[] levels;}
    [Serializable] public class Profile {public int version;public string engine;public Socket[] sockets;public Collision[] colliders;public Lod[] lods;}
    [Serializable] public class Receipt {public int version;public string format,model_sha256,source_glb_sha256;public Profile profile;}
    static void Name(string value) {if(string.IsNullOrWhiteSpace(value)||value.Length>4096||value.Any(char.IsControl))throw new Exception("Invalid consumer name");}
    static Transform Node(GameObject instance,string name) {
        // Preserve Hierarchy guarantees a synthetic prefab root. Its file-derived
        // name can legitimately match an authored child (for example "model").
        Name(name);var nodes=instance.GetComponentsInChildren<Transform>(true).Where(t=>t!=instance.transform&&t.name==name).ToArray();
        if(nodes.Length!=1)throw new Exception("Missing/ambiguous consumer node: "+name);return nodes[0];
    }
    public static Receipt Read(string modelPath) {
        string path=Path.Combine(Path.GetDirectoryName(modelPath),"gcad-consumer-profile.json");
        if(!File.Exists(path))return null;
        if(new FileInfo(path).Length>4*1024*1024)throw new Exception("Consumer receipt exceeds 4 MiB");
        var receipt=JsonUtility.FromJson<Receipt>(File.ReadAllText(path));
        if(receipt==null||receipt.version!=1||receipt.format!="gcad-consumer-profile"||receipt.profile==null)throw new Exception("Unsupported consumer receipt");
        using(var hash=SHA256.Create())using(var stream=File.OpenRead(modelPath))
            if(BitConverter.ToString(hash.ComputeHash(stream)).Replace("-","").ToLowerInvariant()!=receipt.model_sha256)throw new Exception("Consumer receipt does not match FBX");
        if(receipt.source_glb_sha256==null||receipt.source_glb_sha256.Length!=64||receipt.source_glb_sha256.Any(c=>!Uri.IsHexDigit(c)))throw new Exception("Invalid consumer source digest");
        var p=receipt.profile;
        if(p.version!=1||p.engine!="unity"||p.sockets==null||p.colliders==null||p.lods==null||p.sockets.Length+p.colliders.Length+p.lods.Length>4096)
            throw new Exception("Unsupported consumer profile");
        return receipt;
    }
    public static void Validate(GameObject instance,Receipt receipt) {
        if(receipt==null)return;
        if(instance.GetComponent<GcadSockets>())throw new Exception("Existing socket registry");
        var p=receipt.profile;var roles=new HashSet<Transform>();var names=new HashSet<string>();var renderers=new HashSet<Renderer>();var roots=new HashSet<Transform>();
        foreach(var socket in p.sockets) {
            Name(socket.name);var node=Node(instance,socket.node);
            if(!names.Add(socket.name)||!roles.Add(node)||node.GetComponent<Renderer>())throw new Exception("Duplicate socket or socket has geometry");
        }
        foreach(var collision in p.colliders) {
            var node=Node(instance,collision.node);var filter=node.GetComponent<MeshFilter>();
            if(!roles.Add(node)||!filter||!filter.sharedMesh||node.GetComponent<SkinnedMeshRenderer>()||node.GetComponent<Collider>()||node.childCount!=0)
                throw new Exception("Collider requires a dedicated unskinned leaf MeshFilter");
            if(collision.shape!="box"&&collision.shape!="mesh"&&collision.shape!="convex")throw new Exception("Unknown collision shape");
            var mesh=filter.sharedMesh;
            if(mesh.vertexCount==0||mesh.triangles.Length==0)throw new Exception("Empty collision mesh");
            if(collision.shape=="convex"&&mesh.triangles.Length/3>255)throw new Exception("Convex source exceeds profile limit");
            var bounds=mesh.bounds;
            if(collision.shape!="mesh"&&(bounds.size.x<=0||bounds.size.y<=0||bounds.size.z<=0))throw new Exception("Collision volume has zero extent");
        }
        names.Clear();
        foreach(var group in p.lods) {
            Name(group.name);var root=Node(instance,group.root);
            if(!names.Add(group.name)||!roles.Add(root)||!roots.Add(root)||root.GetComponent<Renderer>()||root.GetComponent<LODGroup>()||group.levels==null||group.levels.Length<2||group.levels.Length>8)
                throw new Exception("Invalid LOD group");
            float previous=1.1f;
            foreach(var level in group.levels) {
                if(!GcadRootMotionData.Finite(level.screen_height)||level.screen_height<=0||level.screen_height>1||(float)level.screen_height<=0||(float)level.screen_height>=previous||level.nodes==null||level.nodes.Length==0||level.nodes.Length>4096)
                    throw new Exception("Invalid LOD level");
                previous=(float)level.screen_height;
                foreach(var member in level.nodes) {
                    var node=Node(instance,member);var renderer=node.GetComponent<Renderer>();
                    if(!renderer||node==root||!node.IsChildOf(root)||!roles.Add(node)||!renderers.Add(renderer))throw new Exception("Ambiguous LOD renderer membership");
                }
            }
        }
        foreach(var a in roots)foreach(var b in roots)if(a!=b&&a.IsChildOf(b))throw new Exception("Nested LOD groups are unsupported");
    }
    public static void Apply(GameObject instance,Receipt receipt,string output) {
        if(receipt==null)return;Validate(instance,receipt);var p=receipt.profile;
        var sockets=instance.AddComponent<GcadSockets>();sockets.entries=p.sockets.Select(s=>new GcadSockets.Entry{name=s.name,node=Node(instance,s.node)}).ToArray();
        int index=0;
        foreach(var collision in p.colliders) {
            var node=Node(instance,collision.node);var source=node.GetComponent<MeshFilter>().sharedMesh;
            if(collision.shape=="box") {var box=node.gameObject.AddComponent<BoxCollider>();box.center=source.bounds.center;box.size=source.bounds.size;}
            else {
                var mesh=new Mesh{name="gcad collision",indexFormat=source.indexFormat,vertices=source.vertices,triangles=source.triangles};mesh.RecalculateBounds();
                AssetDatabase.CreateAsset(mesh,output+"/collision_"+(index++)+".asset");
                Cook(collision.node,()=>{
                    var collider=node.gameObject.AddComponent<MeshCollider>();collider.sharedMesh=null;collider.convex=collision.shape=="convex";collider.sharedMesh=mesh;
                });
            }
            var renderer=node.GetComponent<Renderer>();if(renderer)renderer.enabled=false;
        }
        foreach(var group in p.lods) {
            var lod=Node(instance,group.root).gameObject.AddComponent<LODGroup>();
            lod.fadeMode=LODFadeMode.None;lod.animateCrossFading=false;
            lod.SetLODs(group.levels.Select(level=>new LOD((float)level.screen_height,level.nodes.Select(n=>Node(instance,n).GetComponent<Renderer>()).ToArray())).ToArray());
            lod.RecalculateBounds();
        }
    }
}
