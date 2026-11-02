// Test-only rendered consumer comparison. Reference meshes are built directly from GLB.
using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEngine;
using UnityEngine.Rendering;

public sealed class GcadPlayerRenderCheck : MonoBehaviour
{
    [Serializable] public class Pose { public GameObject reference; public AnimationClip clip; public float time; }
    [Serializable] public class Sample { public string clip; public float time; public int view,visiblePixels,changedPixels; public double meanRgbError; }
    [Serializable] public class Geometry { public string clip; public float time,maxPositionDistanceMetres; }
    [Serializable] public class Report { public string unityVersion,graphicsApi,failure; public Sample[] views; public Geometry[] geometry; public int controls,rejectedControls; public bool passed,shaderSampled; }
    public GameObject prefab;
    public bool shaderCheck;
    public Pose[] poses;
    readonly List<Sample> samples=new List<Sample>();
    readonly List<Geometry> geometry=new List<Geometry>();
    readonly Report report=new Report();
    Camera camera;
    string output;
    bool finished;
    float started;
    static readonly Vector3[] Directions={new Vector3(1,1.5f,1),new Vector3(-1,1.5f,-1),new Vector3(1,-1.5f,1),new Vector3(-1,-1.5f,-1)};
    IEnumerator Start() {
        started=Time.realtimeSinceStartup;output=Path.Combine(Application.dataPath,"..","player-renders");Directory.CreateDirectory(output);
        if(!Initialize())yield break;
        for(int i=0;i<poses.Length;i++) { if(!RunPose(i))yield break;yield return null; }
        Finish(null);
    }
    bool Initialize() {
        try {
            if(SystemInfo.graphicsDeviceType!=GraphicsDeviceType.Direct3D11)throw new Exception("D3D11 player required");
            RenderSettings.ambientMode=AmbientMode.Flat;RenderSettings.ambientLight=new Color(.22f,.22f,.22f);
            var light=new GameObject("comparison light").AddComponent<Light>();light.type=LightType.Directional;
            light.intensity=1.2f;light.transform.rotation=Quaternion.Euler(50,-35,0);
            camera=new GameObject("comparison camera").AddComponent<Camera>();camera.enabled=false;camera.orthographic=true;
            camera.allowHDR=false;camera.allowMSAA=false;camera.clearFlags=CameraClearFlags.SolidColor;
            camera.backgroundColor=new Color(.02f,.025f,.035f,1);
            camera.targetTexture=new RenderTexture(512,512,24,RenderTextureFormat.ARGB32);camera.targetTexture.Create();
            return true;
        } catch(Exception error) { Finish(error.Message);return false; }
    }
    static int Changed(Color32[] a,Color32[] b) {
        int count=0;for(int i=0;i<a.Length;i++)if(Math.Abs(a[i].r-b[i].r)+Math.Abs(a[i].g-b[i].g)+Math.Abs(a[i].b-b[i].b)>6)count++;return count;
    }
    static double Mean(Color32[] a,Color32[] b) {
        double sum=0;for(int i=0;i<a.Length;i++)sum+=Math.Abs(a[i].r-b[i].r)+Math.Abs(a[i].g-b[i].g)+Math.Abs(a[i].b-b[i].b);return sum/(a.Length*3.0*255);
    }
    static bool Matches(int changed,double mean,int visible) { return changed<=visible*.01 && mean<=.002; }
    static Vector3[] Points(GameObject model) {
        return model.GetComponentsInChildren<MeshFilter>().SelectMany(f=>f.sharedMesh.vertices.Select(v=>f.transform.TransformPoint(v))).ToArray();
    }
    Color32[] Capture(string path=null) {
        var previous=RenderTexture.active;var texture=new Texture2D(512,512,TextureFormat.RGBA32,false);
        try {
            camera.Render();RenderTexture.active=camera.targetTexture;texture.ReadPixels(new Rect(0,0,512,512),0,0);texture.Apply();
            if(path!=null)File.WriteAllBytes(path,texture.EncodeToPNG());return texture.GetPixels32();
        } finally { RenderTexture.active=previous;Destroy(texture); }
    }
    void Control(Color32[] expected,Color32[] damaged,int visible) {
        report.controls++;if(!Matches(Changed(expected,damaged),Mean(expected,damaged),visible))report.rejectedControls++;
    }
    bool RunPose(int index) {
        GameObject actual=null,reference=null;var owned=new List<Mesh>();
        try {
            var pose=poses[index];actual=Instantiate(prefab);
            foreach(var animator in actual.GetComponentsInChildren<Animator>())animator.enabled=false;
            pose.clip.SampleAnimation(actual,pose.time);
            foreach(var deformation in actual.GetComponentsInChildren<GcadDeformation>())
                if(!deformation.Refresh())throw new Exception(deformation.LastError);
            var shader=actual.GetComponent<GcadUvScrollAnimation>();
            if(shaderCheck) {
                if(!shader||shader.bindings.Length!=3||shader.bindings.Any(b=>b.speed!=new Vector2(.125f,.25f)||b.frozenSeconds!=2))throw new Exception("Frozen shader fixture required");
                if(!shader.Sample(pose.time))throw new Exception(shader.LastError);
            }
            reference=Instantiate(pose.reference);
            foreach(var filter in reference.GetComponentsInChildren<MeshFilter>()) {
                var mesh=Instantiate(filter.sharedMesh);owned.Add(mesh);filter.sharedMesh=mesh;
            }
            var actualPoints=Points(actual);var referencePoints=Points(reference);
            geometry.Add(new Geometry { clip=pose.clip.name,time=pose.time,
                maxPositionDistanceMetres=GcadPositionMetric.Distance(actualPoints,referencePoints) });
            var renderers=actual.GetComponentsInChildren<Renderer>();
            var bounds=new Bounds(referencePoints[0],Vector3.zero);
            foreach(var point in referencePoints)bounds.Encapsulate(point);
            var displayed=renderers.Where(r=>r.enabled).ToArray();
            if(displayed.Length==0)throw new Exception("No active adapted renderer");
            foreach(var renderer in renderers.Concat(reference.GetComponentsInChildren<Renderer>())) {
                foreach(var material in renderer.sharedMaterials)if(!material.shader.isSupported)throw new Exception("Unsupported player shader");
            }
            float distance=bounds.size.magnitude*3,radius=bounds.extents.magnitude*1.2f;
            camera.orthographicSize=bounds.extents.magnitude*1.15f;
            camera.nearClipPlane=Mathf.Max(.000001f,distance-radius);camera.farClipPlane=distance+radius;
            for(int view=0;view<Directions.Length;view++) {
                camera.transform.position=bounds.center+Directions[view].normalized*distance;camera.transform.LookAt(bounds.center);
                // Hide renderers without restarting Animator/component lifecycle between captures.
                foreach(var renderer in displayed)renderer.enabled=false;
                reference.SetActive(false);var empty=Capture();
                foreach(var renderer in displayed)renderer.enabled=true;
                var imported=Capture(Path.Combine(output,index+"-imported-"+view+".png"));
                foreach(var renderer in displayed)renderer.enabled=false;
                reference.SetActive(true);var expected=Capture(Path.Combine(output,index+"-source-"+view+".png"));
                int visible=Changed(expected,empty);
                samples.Add(new Sample { clip=pose.clip.name,time=pose.time,view=view,visiblePixels=visible,
                    changedPixels=Changed(expected,imported),meanRgbError=Mean(expected,imported) });
                if(shaderCheck) {
                    reference.SetActive(false);foreach(var renderer in displayed)renderer.enabled=true;
                    if(!shader.Sample(pose.time+.7))throw new Exception(shader.LastError);
                    Control(expected,Capture(),visible);
                    if(!shader.Sample(pose.time))throw new Exception(shader.LastError);
                    foreach(var renderer in displayed)renderer.enabled=false;reference.SetActive(true);
                }
                var normals=owned.Select(m=>m.normals).ToArray();
                try {
                    for(int m=0;m<owned.Count;m++)owned[m].normals=normals[m].Select(n=>-n).ToArray();
                    Control(expected,Capture(),visible);
                } finally { for(int m=0;m<owned.Count;m++)owned[m].normals=normals[m]; }
                if(view==Directions.Length-1) {
                    reference.transform.position+=Vector3.right*.1f;Control(expected,Capture(),visible);
                }
            }
            return true;
        } catch(Exception error) { Finish(error.Message);return false; }
        finally {
            if(actual){actual.SetActive(false);Destroy(actual);}if(reference){reference.SetActive(false);Destroy(reference);}
            foreach(var mesh in owned)Destroy(mesh);
        }
    }
    void Update() { if(!finished && started>0 && Time.realtimeSinceStartup-started>120)Finish("Rendered player timed out"); }
    void Finish(string failure) {
        if(finished)return;finished=true;
        report.unityVersion=Application.unityVersion;report.graphicsApi=SystemInfo.graphicsDeviceType.ToString();report.failure=failure;
        report.views=samples.ToArray();report.geometry=geometry.ToArray();
        report.shaderSampled=shaderCheck;
        report.passed=failure==null && samples.Count==84 && geometry.Count==21 && geometry.All(g=>g.maxPositionDistanceMetres<=.00001f) && report.controls==(shaderCheck?189:105) && report.controls==report.rejectedControls &&
            samples.All(s=>s.visiblePixels>1000 && Matches(s.changedPixels,s.meanRgbError,s.visiblePixels));
        File.WriteAllText(Path.Combine(Application.dataPath,"..","player-render-report.json"),JsonUtility.ToJson(report,true));
        if(camera && camera.targetTexture){camera.targetTexture.Release();Destroy(camera.targetTexture);}
        Application.Quit(report.passed?0:1);
    }
}
