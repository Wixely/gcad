using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using UnityEngine;

// Diagnostic colours distinguish actual draw selection from component metadata.
public static class GcadConsumerLodProbe
{
    [Serializable] public class Sample {public float height;public int red,green,other;public bool passed;}
    [Serializable] public class Report {public string unityVersion,graphics;public Sample[] samples;public bool forcedLevelControl,cullControl,passed;}
    static void Destroy(UnityEngine.Object value){if(Application.isPlaying)UnityEngine.Object.Destroy(value);else UnityEngine.Object.DestroyImmediate(value);}
    public static Report RunPosed(GameObject instance,Shader shader,string directory) {
        var tip=instance.GetComponentsInChildren<Transform>().Single(t=>t.name=="tip");var rotation=tip.localRotation;
        try {tip.localRotation=rotation*Quaternion.AngleAxis(35,Vector3.forward);return Run(instance,shader,directory);}
        finally {tip.localRotation=rotation;}
    }
    static Sample Capture(Camera camera,Texture2D pixels,float height,string path) {
        camera.Render();RenderTexture.active=camera.targetTexture;pixels.ReadPixels(new Rect(0,0,256,256),0,0);pixels.Apply(false,false);
        var sample=new Sample{height=height};
        foreach(var p in pixels.GetPixels32()) {
            if(p.r>200&&p.g<20&&p.b<20)sample.red++;
            else if(p.g>200&&p.r<20&&p.b<20)sample.green++;
            else if(p.r>20||p.g>20||p.b>20)sample.other++;
        }
        File.WriteAllBytes(path,pixels.EncodeToPNG());return sample;
    }
    public static Report Run(GameObject instance,Shader shader,string directory) {
        if(!shader||!shader.isSupported||SystemInfo.graphicsDeviceType==UnityEngine.Rendering.GraphicsDeviceType.Null)throw new Exception("LOD rendering requires the diagnostic shader and a graphics device");
        Directory.CreateDirectory(directory);
        var group=instance.GetComponentsInChildren<LODGroup>().Single();var levels=group.GetLODs();
        var renderers=levels.SelectMany(l=>l.renderers).Distinct().ToArray();var originals=renderers.Select(r=>r.sharedMaterials).ToArray();
        // Multiple camera renders and poses occur in one frame in this probe.
        var skins=renderers.OfType<SkinnedMeshRenderer>().ToArray();var recalculate=skins.Select(s=>s.forceMatrixRecalculationPerRender).ToArray();
        var materials=new[]{new Material(shader),new Material(shader)};materials[0].SetColor("_Color",Color.red);materials[1].SetColor("_Color",Color.green);
        var cameraObject=new GameObject("consumer LOD camera");var camera=cameraObject.AddComponent<Camera>();
        var target=new RenderTexture(256,256,24,RenderTextureFormat.ARGB32,RenderTextureReadWrite.Linear);target.Create();
        var pixels=new Texture2D(256,256,TextureFormat.RGBA32,false,true);var active=RenderTexture.active;
        float oldBias=QualitySettings.lodBias;int oldMaximum=QualitySettings.maximumLODLevel;
        try {
            foreach(var skin in skins)skin.forceMatrixRecalculationPerRender=true;
            for(int i=0;i<levels.Length;i++)foreach(var renderer in levels[i].renderers)renderer.sharedMaterials=renderer.sharedMaterials.Select(_=>materials[i]).ToArray();
            camera.enabled=false;camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=Color.black;camera.fieldOfView=60;camera.aspect=1;camera.nearClipPlane=.0001f;camera.farClipPlane=10;camera.allowHDR=false;camera.allowMSAA=false;camera.targetTexture=target;
            QualitySettings.lodBias=1;QualitySettings.maximumLODLevel=0;group.ForceLOD(-1);
            var center=group.transform.TransformPoint(group.localReferencePoint);
            Action<float> position=height=>{float distance=group.size/(2*Mathf.Tan(camera.fieldOfView*Mathf.Deg2Rad/2)*height);camera.transform.position=center+Vector3.up*distance;camera.transform.rotation=Quaternion.LookRotation(-Vector3.up,Vector3.forward);};
            var samples=new List<Sample>();var heights=new[]{.9f,.3f,.05f};
            for(int i=0;i<heights.Length;i++) {
                position(heights[i]);var sample=Capture(camera,pixels,heights[i],Path.Combine(directory,"level-"+i+".png"));
                sample.passed=sample.other==0&&(i==0?sample.red>100&&sample.green==0:i==1?sample.green>100&&sample.red==0:sample.red==0&&sample.green==0);samples.Add(sample);
            }
            position(.9f);group.ForceLOD(1);var wrong=Capture(camera,pixels,.9f,Path.Combine(directory,"forced-wrong-level.png"));
            position(.05f);group.ForceLOD(0);var visible=Capture(camera,pixels,.05f,Path.Combine(directory,"forced-visible.png"));
            var report=new Report{unityVersion=Application.unityVersion,graphics=SystemInfo.graphicsDeviceType.ToString(),samples=samples.ToArray(),forcedLevelControl=wrong.green>100&&wrong.red==0,cullControl=visible.red>10&&visible.green==0};
            report.passed=report.samples.All(s=>s.passed)&&report.forcedLevelControl&&report.cullControl;return report;
        } finally {
            group.ForceLOD(-1);for(int i=0;i<renderers.Length;i++)renderers[i].sharedMaterials=originals[i];
            for(int i=0;i<skins.Length;i++)skins[i].forceMatrixRecalculationPerRender=recalculate[i];
            QualitySettings.lodBias=oldBias;QualitySettings.maximumLODLevel=oldMaximum;RenderTexture.active=active;camera.targetTexture=null;target.Release();
            Destroy(pixels);Destroy(target);Destroy(cameraObject);foreach(var material in materials)Destroy(material);
        }
    }
}
