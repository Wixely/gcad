// Actual production shader blend/cull/mask states with analytic emission-only output.
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Rendering;

public static class GcadRasterCheck
{
    [Serializable] public class Case { public string mode; public bool reversed,doubleSided; public float alpha; public double maximumRgbError; public bool passed; }
    [Serializable] public class Report { public string unityVersion,graphicsApi; public Case[] cases; public bool passed; }
    static Color Capture(Camera camera,Texture2D readback) {
        camera.Render();RenderTexture.active=camera.targetTexture;
        readback.ReadPixels(new Rect(0,0,32,32),0,0);readback.Apply(false,false);
        return readback.GetPixel(16,16);
    }
    public static void Run() {
        EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Single);
        RenderSettings.ambientMode=AmbientMode.Flat;RenderSettings.ambientLight=Color.black;
        RenderSettings.ambientIntensity=0;RenderSettings.reflectionIntensity=0;RenderSettings.skybox=null;
        var mesh=new Mesh();
        mesh.vertices=new[]{new Vector3(-1,-1,0),new Vector3(-1,1,0),new Vector3(1,1,0),new Vector3(1,-1,0)};
        mesh.normals=Enumerable.Repeat(Vector3.back,4).ToArray();mesh.tangents=Enumerable.Repeat(new Vector4(1,0,0,1),4).ToArray();
        mesh.uv=new[]{new Vector2(0,0),new Vector2(0,1),new Vector2(1,1),new Vector2(1,0)};
        mesh.triangles=new[]{0,1,2,0,2,3};mesh.RecalculateBounds();
        var quad=new GameObject("raster fixture");quad.AddComponent<MeshFilter>().sharedMesh=mesh;
        var renderer=quad.AddComponent<MeshRenderer>();
        var camera=new GameObject("raster camera").AddComponent<Camera>();
        camera.enabled=false;camera.orthographic=true;camera.orthographicSize=1.25f;
        camera.transform.position=new Vector3(0,0,-2);camera.transform.LookAt(Vector3.zero);
        camera.nearClipPlane=.1f;camera.farClipPlane=10;camera.allowHDR=true;camera.allowMSAA=false;
        camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=new Color(.1f,.2f,.3f,1);
        camera.renderingPath=RenderingPath.Forward;
        camera.targetTexture=new RenderTexture(32,32,24,RenderTextureFormat.ARGBFloat,RenderTextureReadWrite.Linear);camera.targetTexture.Create();
        var readback=new Texture2D(32,32,TextureFormat.RGBAFloat,false,true);
        var previous=RenderTexture.active;var cases=new List<Case>();
        var emission=new Color(.3f,.5f,.7f,1);
        try {
            renderer.enabled=false;var background=Capture(camera,readback);renderer.enabled=true;
            foreach(string mode in new[]{"OPAQUE","MASK","BLEND"})foreach(bool reversed in new[]{false,true})
            foreach(bool doubleSided in new[]{false,true})foreach(float alpha in new[]{.2f,.3f,.7f}) {
                mesh.triangles=reversed ? new[]{0,2,1,0,3,2} : new[]{0,1,2,0,2,3};
                var shader=Shader.Find(mode=="BLEND" ? "gcad/PBR Blend" : "gcad/PBR Opaque Mask");
                if(!shader || !shader.isSupported)throw new Exception("Production PBR shader unavailable");
                var material=new Material(shader);renderer.sharedMaterial=material;
                try {
                    material.SetVector("_BaseFactor",new Vector4(0,0,0,alpha));
                    material.SetVector("_EmissionFactor",new Vector4(emission.r,emission.g,emission.b,1));
                    material.SetFloat("_Metallic",0);material.SetFloat("_AlphaMode",mode=="MASK" ? 1 : mode=="BLEND" ? 2 : 0);
                    material.SetFloat("_Cutoff",.3f);material.SetFloat("_Cull",doubleSided ? 0 : 2);
                    var actual=Capture(camera,readback);
                    bool visible=(!reversed || doubleSided) && (mode!="MASK" || alpha>=.3f);
                    var expected=!visible ? background : mode=="BLEND" ? emission*alpha+background*(1-alpha) : emission;
                    double error=new[]{Math.Abs(actual.r-expected.r),Math.Abs(actual.g-expected.g),Math.Abs(actual.b-expected.b)}.Max();
                    cases.Add(new Case { mode=mode,reversed=reversed,doubleSided=doubleSided,alpha=alpha,maximumRgbError=error,passed=!double.IsNaN(error)&&error<.002 });
                } finally { UnityEngine.Object.DestroyImmediate(material); }
            }
        } finally {
            RenderTexture.active=previous;camera.targetTexture.Release();UnityEngine.Object.DestroyImmediate(camera.targetTexture);
            foreach(var item in new UnityEngine.Object[]{readback,quad,mesh,camera.gameObject})UnityEngine.Object.DestroyImmediate(item);
        }
        var report=new Report { unityVersion=Application.unityVersion,graphicsApi=SystemInfo.graphicsDeviceType.ToString(),cases=cases.ToArray(),passed=cases.All(c=>c.passed) };
        File.WriteAllText("raster-report.json",JsonUtility.ToJson(report,true));
        if(!report.passed)throw new Exception("Production blend/cull/mask check failed; inspect raster-report.json");
    }
}
