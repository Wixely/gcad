using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using UnityEngine;

public static class GcadUvScrollAdapter
{
    [Serializable] public class Fallback {public string mode;public double seconds;}
    [Serializable] public class Description {public int version;public string template;public float[] scale,offset,speed;public Fallback fallback;}
    [Serializable] public class Entry {public string material_name;public Description description;}
    [Serializable] public class Receipt {public int version;public string format,model_sha256,source_glb_sha256;public Entry[] materials;}
    public static Receipt Read(string modelPath) {
        string path=Path.Combine(Path.GetDirectoryName(modelPath),"gcad-shaders.json");
        if(!File.Exists(path))return null;
        if(new FileInfo(path).Length>1024*1024)throw new Exception("Shader receipt exceeds 1 MiB");
        var receipt=JsonUtility.FromJson<Receipt>(File.ReadAllText(path));
        if(receipt==null||receipt.version!=1||receipt.format!="gcad-frozen-uv-scroll"||receipt.materials==null||receipt.materials.Length<1||receipt.materials.Length>1024)
            throw new Exception("Invalid shader receipt");
        string digest;using(var hash=SHA256.Create())using(var stream=File.OpenRead(modelPath))digest=BitConverter.ToString(hash.ComputeHash(stream)).Replace("-","").ToLowerInvariant();
        if(receipt.model_sha256!=digest)throw new Exception("Shader receipt does not match this FBX");
        var names=new HashSet<string>();
        foreach(var entry in receipt.materials) {
            var d=entry==null?null:entry.description;
            if(entry==null||string.IsNullOrEmpty(entry.material_name)||entry.material_name.Length>4096||!names.Add(entry.material_name)||d==null||d.version!=1||d.template!="uv-scroll-pbr"||
                d.scale==null||d.scale.Length!=2||d.offset==null||d.offset.Length!=2||d.speed==null||d.speed.Length!=2||d.fallback==null||d.fallback.mode!="freeze")
                throw new Exception("Invalid frozen UV shader binding");
            GcadUvScroll.Original(new Vector2(d.scale[0],d.scale[1]),new Vector2(d.offset[0],d.offset[1]),new Vector2(d.speed[0],d.speed[1]),d.fallback.seconds);
        }
        return receipt;
    }
    public static void Attach(GameObject instance,Receipt receipt,Dictionary<string,Material> materials) {
        if(receipt==null)return;
        var used=instance.GetComponentsInChildren<Renderer>(true).SelectMany(r=>r.sharedMaterials).ToArray();
        var bindings=receipt.materials.Select(entry=> {
            Material material;if(!materials.TryGetValue(entry.material_name,out material)||!used.Contains(material))
                throw new Exception("Shader receipt material is absent from adapted prefab");
            return new GcadUvScrollAnimation.Binding {material=material,speed=new Vector2(entry.description.speed[0],entry.description.speed[1]),frozenSeconds=entry.description.fallback.seconds};
        }).ToArray();
        var animation=instance.AddComponent<GcadUvScrollAnimation>();animation.bindings=bindings;
    }
}
