using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Rendering;

public static class GcadRenderCheck
{
    public static void RunWithNormalImport() {
        GcadNormalTextureImport.Configure("Assets/Fixture/model.fbx");
        Run();
    }
    [Serializable] public class Report {
        public string unityVersion, graphicsApi;
        public int visiblePixels, baseMapChangedPixels, normalMapChangedPixels;
        public string[] normalTextureTypes;
        public bool passed;
    }
    public static Color32[] Capture(Camera camera, string filename) {
        var previous = RenderTexture.active;
        var image = new Texture2D(512,512,TextureFormat.RGBA32,false);
        try {
            camera.Render(); RenderTexture.active = camera.targetTexture;
            image.ReadPixels(new Rect(0,0,512,512),0,0); image.Apply();
            if (filename != null) File.WriteAllBytes(filename,image.EncodeToPNG());
            return image.GetPixels32();
        } finally { RenderTexture.active=previous; UnityEngine.Object.DestroyImmediate(image); }
    }
    public static int Changed(Color32[] a, Color32[] b) {
        return a.Zip(b,(x,y) => Math.Abs(x.r-y.r)+Math.Abs(x.g-y.g)+Math.Abs(x.b-y.b)>6).Count(v=>v);
    }
    public static void Run() {
        GcadImportCheck.Run();
        EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Single);
        var instance = UnityEngine.Object.Instantiate(AssetDatabase.LoadAssetAtPath<GameObject>("Assets/Fixture/model.fbx"));
        var renderers = instance.GetComponentsInChildren<Renderer>(true);
        var originals = renderers.Select(r=>r.sharedMaterials).ToArray();
        var copies = new List<Material>();
        for(int r=0;r<renderers.Length;r++) {
            var materials=originals[r].Select(m=>new Material(m)).ToArray();
            copies.AddRange(materials); renderers[r].sharedMaterials=materials;
        }
        var bounds=renderers[0].bounds;
        foreach(var renderer in renderers) bounds.Encapsulate(renderer.bounds);
        RenderSettings.ambientMode=AmbientMode.Flat;
        RenderSettings.ambientLight=new Color(.22f,.22f,.22f);
        var light=new GameObject("test light").AddComponent<Light>();
        light.type=LightType.Directional; light.intensity=1.2f;
        light.transform.rotation=Quaternion.Euler(50,-35,0);
        var camera=new GameObject("test camera").AddComponent<Camera>();
        camera.enabled=false; camera.orthographic=true;
        camera.orthographicSize=bounds.extents.magnitude*1.15f;
        camera.transform.position=bounds.center+new Vector3(1,1.5f,1).normalized*bounds.size.magnitude*3;
        camera.transform.LookAt(bounds.center);
        camera.nearClipPlane=.0001f; camera.farClipPlane=10;
        camera.clearFlags=CameraClearFlags.SolidColor; camera.backgroundColor=new Color(.02f,.025f,.035f,1);
        camera.allowHDR=false; camera.allowMSAA=false;
        camera.targetTexture=new RenderTexture(512,512,24,RenderTextureFormat.ARGB32);
        camera.targetTexture.Create();
        Directory.CreateDirectory("renders");
        var report=new Report { unityVersion=Application.unityVersion, graphicsApi=SystemInfo.graphicsDeviceType.ToString() };
        try {
            foreach(var r in renderers)r.enabled=false;
            var empty=Capture(camera,null);
            foreach(var r in renderers)r.enabled=true;
            var baseline=Capture(camera,"renders/baseline.png");
            report.visiblePixels=Changed(baseline,empty);
            var baseMaps=copies.Select(m=>m.GetTexture("_MainTex")).ToArray();
            foreach(var m in copies)m.SetTexture("_MainTex",null);
            report.baseMapChangedPixels=Changed(baseline,Capture(camera,"renders/no-base-map.png"));
            for(int i=0;i<copies.Count;i++)copies[i].SetTexture("_MainTex",baseMaps[i]);
            report.normalTextureTypes=copies.Select(m=> {
                var path=AssetDatabase.GetAssetPath(m.GetTexture("_BumpMap"));
                var importer=AssetImporter.GetAtPath(path) as TextureImporter;
                return importer ? importer.textureType.ToString() : "missing";
            }).ToArray();
            foreach(var m in copies){m.SetTexture("_BumpMap",null);m.DisableKeyword("_NORMALMAP");}
            report.normalMapChangedPixels=Changed(baseline,Capture(camera,"renders/no-normal-map.png"));
            report.passed=report.visiblePixels>1000 && report.baseMapChangedPixels>100 && report.normalMapChangedPixels>100 &&
                report.normalTextureTypes.All(t=>t=="NormalMap");
            File.WriteAllText("render-report.json",JsonUtility.ToJson(report,true));
        } finally {
            camera.targetTexture.Release(); UnityEngine.Object.DestroyImmediate(camera.targetTexture);
            foreach(var m in copies)UnityEngine.Object.DestroyImmediate(m);
            UnityEngine.Object.DestroyImmediate(instance);
        }
        if(!report.passed)throw new Exception("Unity material render check failed; inspect render-report.json and renders");
    }
}
