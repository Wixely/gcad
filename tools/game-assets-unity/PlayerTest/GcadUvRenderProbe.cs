using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEngine;
using UnityEngine.Rendering;

// Compare real FBX geometry with independently shifted UVs, using production PBR.
public static class GcadUvRenderProbe
{
    [Serializable] public class Case {public double seconds,meanRgbError,controlError;public int view,changedPixels;public bool passed;}
    [Serializable] public class Report {public string unityVersion,graphicsApi;public Case[] cases;public bool seekExact,visibleChange,passed;public string[] mapSlots;public double[] mapOmissionErrors;}
    static Color32[] Capture(Camera camera,Texture2D pixels) {
        camera.Render();RenderTexture.active=camera.targetTexture;
        pixels.ReadPixels(new Rect(0,0,256,256),0,0);pixels.Apply(false,false);return pixels.GetPixels32();
    }
    static double Error(Color32[] a,Color32[] b,out int changed) {
        long sum=0;changed=0;
        for(int i=0;i<a.Length;i++){int d=Math.Abs(a[i].r-b[i].r)+Math.Abs(a[i].g-b[i].g)+Math.Abs(a[i].b-b[i].b);sum+=d;if(d>3)changed++;}
        return sum/(a.Length*3.0*255);
    }
    public static void Run(GameObject prefab,string outputDirectory,bool allMaps=false) {
        var actual=UnityEngine.Object.Instantiate(prefab);var reference=UnityEngine.Object.Instantiate(prefab);
        reference.GetComponent<GcadUvScrollAnimation>().enabled=false;
        var animation=actual.GetComponent<GcadUvScrollAnimation>();
        if(animation.bindings.Length!=1||animation.bindings[0].speed!=new Vector2(.125f,.25f)||animation.bindings[0].frozenSeconds!=2)
            throw new Exception("Frozen UV reference fixture required");
        var slots=allMaps?new[]{"_BaseMap","_RoughnessMap","_MetallicMap","_NormalMap","_OcclusionMap","_EmissionMap"}:new string[0];
        var omissionErrors=new double[slots.Length];
        foreach(string slot in slots) {
            var texture=animation.bindings[0].material.GetTexture(slot);
            if(!texture||!texture.name.StartsWith("material_0_"))throw new Exception("Missing imported map: "+slot);
        }
        var meshes=new List<Mesh>();var originals=new List<Vector2[]>();
        foreach(var filter in reference.GetComponentsInChildren<MeshFilter>(true)) {
            var mesh=UnityEngine.Object.Instantiate(filter.sharedMesh);filter.sharedMesh=mesh;meshes.Add(mesh);originals.Add(mesh.uv);
        }
        if(meshes.Count==0)throw new Exception("Imported reference geometry missing");
        var renderers=actual.GetComponentsInChildren<Renderer>(true);var bounds=renderers[0].bounds;
        foreach(var renderer in renderers)bounds.Encapsulate(renderer.bounds);
        RenderSettings.ambientMode=AmbientMode.Flat;RenderSettings.ambientLight=new Color(.3f,.3f,.3f);RenderSettings.reflectionIntensity=0;RenderSettings.skybox=null;
        var light=new GameObject("key").AddComponent<Light>();light.type=LightType.Directional;light.intensity=1;light.transform.rotation=Quaternion.Euler(35,-25,0);
        var camera=new GameObject("comparison camera").AddComponent<Camera>();camera.enabled=false;
        camera.orthographic=true;camera.orthographicSize=bounds.extents.magnitude*1.12f;
        camera.nearClipPlane=.001f;camera.farClipPlane=10;camera.allowHDR=false;camera.allowMSAA=false;
        camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=new Color(.04f,.06f,.09f);camera.renderingPath=RenderingPath.Forward;
        var target=new RenderTexture(256,256,24,RenderTextureFormat.ARGB32,RenderTextureReadWrite.Linear);target.Create();camera.targetTexture=target;
        var pixels=new Texture2D(256,256,TextureFormat.RGBA32,false,true);var previous=RenderTexture.active;
        var cases=new List<Case>();var first=new Dictionary<int,Color32[]>();bool seek=true,visible=false;
        var directions=new[]{new Vector3(1,1,-1),new Vector3(-1,1,1),Vector3.up,new Vector3(0,-1,-1)};
        Directory.CreateDirectory(Path.Combine(outputDirectory,"UvRender"));
        try {
            int sample=0;
            foreach(double seconds in new[]{0.0,.5,2,4,0}) {
                for(int m=0;m<meshes.Count;m++)meshes[m].uv=originals[m].Select(uv=>new Vector2(uv.x+(float)((seconds-2)*.125),uv.y-(float)((seconds-2)*.25))).ToArray();
                for(int view=0;view<directions.Length;view++) {
                    camera.transform.position=bounds.center+directions[view].normalized*bounds.extents.magnitude*4;
                    camera.transform.LookAt(bounds.center,view==2?Vector3.forward:Vector3.up);
                    reference.SetActive(false);actual.SetActive(true);
                    if(!animation.Sample(seconds))throw new Exception(animation.LastError);
                    var a=Capture(camera,pixels);File.WriteAllBytes(Path.Combine(outputDirectory,"UvRender/sample-"+sample+"-view-"+view+".png"),pixels.EncodeToPNG());
                    if(sample==0)first[view]=a;else if(sample==4)seek&=a.SequenceEqual(first[view]);
                    int changed;if(sample>0&&sample<4)visible|=Error(a,first[view],out changed)>.001;
                    actual.SetActive(false);reference.SetActive(true);var b=Capture(camera,pixels);
                    double error=Error(a,b,out changed);int changedPixels=changed;
                    reference.SetActive(false);actual.SetActive(true);if(!animation.Sample(seconds+.7))throw new Exception(animation.LastError);
                    var corrupted=Capture(camera,pixels);double control=Error(corrupted,b,out changed);
                    cases.Add(new Case{seconds=seconds,view=view,meanRgbError=error,changedPixels=changedPixels,controlError=control,passed=error<.0001&&control>.001});
                    if(allMaps) {
                        if(!animation.Sample(seconds))throw new Exception(animation.LastError);
                        var material=actual.GetComponentInChildren<Renderer>().sharedMaterial;
                        for(int slot=0;slot<slots.Length;slot++) {
                            var original=material.GetTexture(slots[slot]);material.SetTexture(slots[slot],Texture2D.whiteTexture);
                            try {omissionErrors[slot]=Math.Max(omissionErrors[slot],Error(Capture(camera,pixels),b,out changed));}
                            finally {material.SetTexture(slots[slot],original);}
                        }
                    }
                }
                sample++;
            }
        } finally {
            RenderTexture.active=previous;camera.targetTexture=null;target.Release();
            foreach(var item in new UnityEngine.Object[]{actual,reference,camera.gameObject,light.gameObject,target,pixels})UnityEngine.Object.DestroyImmediate(item);
            foreach(var mesh in meshes)UnityEngine.Object.DestroyImmediate(mesh);
        }
        var report=new Report{unityVersion=Application.unityVersion,graphicsApi=SystemInfo.graphicsDeviceType.ToString(),cases=cases.ToArray(),seekExact=seek,visibleChange=visible,mapSlots=slots,mapOmissionErrors=omissionErrors,passed=seek&&visible&&cases.All(c=>c.passed)&&omissionErrors.All(e=>e>1e-5)};
        File.WriteAllText(Path.Combine(outputDirectory,"uv-render-report.json"),JsonUtility.ToJson(report,true));
        if(!report.passed)throw new Exception("FBX UV scrolling rendered comparison failed");
    }
}
