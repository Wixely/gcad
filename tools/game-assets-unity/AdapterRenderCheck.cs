using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Rendering;

public static class GcadAdapterRenderCheck
{
    [Serializable] public class Control { public string property; public int changedPixels; }
    [Serializable] public class Report { public string unityVersion,graphicsApi; public int visiblePixels; public Control[] controls; public bool passed; }
    public static void Run() {
        EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Single);
        var instance=UnityEngine.Object.Instantiate(AssetDatabase.LoadAssetAtPath<GameObject>("Assets/GcadAdapted/model.prefab"));
        var renderers=instance.GetComponentsInChildren<Renderer>(true);
        var copies=new List<Material>();
        foreach(var renderer in renderers) {
            var materials=renderer.sharedMaterials.Select(m=>new Material(m)).ToArray();
            copies.AddRange(materials);renderer.sharedMaterials=materials;
        }
        var bounds=renderers[0].bounds;foreach(var renderer in renderers)bounds.Encapsulate(renderer.bounds);
        RenderSettings.ambientMode=AmbientMode.Flat;RenderSettings.ambientLight=new Color(.22f,.22f,.22f);
        var light=new GameObject("adapter light").AddComponent<Light>();
        light.type=LightType.Directional;light.intensity=1.2f;light.transform.rotation=Quaternion.Euler(50,-35,0);
        var camera=new GameObject("adapter camera").AddComponent<Camera>();
        camera.enabled=false;camera.orthographic=true;camera.orthographicSize=bounds.extents.magnitude*1.15f;
        camera.transform.position=bounds.center+new Vector3(1,1.5f,1).normalized*bounds.size.magnitude*3;
        camera.transform.LookAt(bounds.center);camera.nearClipPlane=.0001f;camera.farClipPlane=10;
        camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=new Color(.02f,.025f,.035f,1);
        camera.allowHDR=false;camera.allowMSAA=false;
        camera.targetTexture=new RenderTexture(512,512,24,RenderTextureFormat.ARGB32);camera.targetTexture.Create();
        Directory.CreateDirectory("adapter-renders");
        var report=new Report { unityVersion=Application.unityVersion,graphicsApi=SystemInfo.graphicsDeviceType.ToString() };
        try {
            foreach(var renderer in renderers)renderer.enabled=false;
            var empty=GcadRenderCheck.Capture(camera,null);
            foreach(var renderer in renderers)renderer.enabled=true;
            var baseline=GcadRenderCheck.Capture(camera,"adapter-renders/baseline.png");
            report.visiblePixels=GcadRenderCheck.Changed(baseline,empty);
            var controls=new List<Control>();
            foreach(var property in new[]{"_HasNormal","_Metallic","_Roughness","_OcclusionStrength","_AlphaMode"}) {
                var saved=copies.Select(m=>m.GetFloat(property)).ToArray();
                foreach(var material in copies)material.SetFloat(property,0);
                controls.Add(new Control { property=property,changedPixels=GcadRenderCheck.Changed(baseline,
                    GcadRenderCheck.Capture(camera,"adapter-renders/without"+property+".png")) });
                for(int i=0;i<copies.Count;i++)copies[i].SetFloat(property,saved[i]);
            }
            var emission=copies.Select(m=>m.GetVector("_EmissionFactor")).ToArray();
            foreach(var material in copies)material.SetVector("_EmissionFactor",Vector4.zero);
            controls.Add(new Control { property="_EmissionFactor",changedPixels=GcadRenderCheck.Changed(baseline,
                GcadRenderCheck.Capture(camera,"adapter-renders/without-emission.png")) });
            for(int i=0;i<copies.Count;i++)copies[i].SetVector("_EmissionFactor",emission[i]);
            report.controls=controls.ToArray();
            report.passed=report.visiblePixels>1000 && controls.All(c=>c.changedPixels>10) &&
                copies.All(m=>!ShaderUtil.ShaderHasError(m.shader));
            File.WriteAllText("adapter-render-report.json",JsonUtility.ToJson(report,true));
        } finally {
            camera.targetTexture.Release();UnityEngine.Object.DestroyImmediate(camera.targetTexture);
            foreach(var material in copies)UnityEngine.Object.DestroyImmediate(material);
            UnityEngine.Object.DestroyImmediate(instance);UnityEngine.Object.DestroyImmediate(camera.gameObject);
            UnityEngine.Object.DestroyImmediate(light.gameObject);
        }
        if(!report.passed)throw new Exception("Adapter rendered controls failed; inspect adapter-render-report.json");
    }
}
