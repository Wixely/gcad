using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Rendering;

public static class GcadSourceRenderCheck
{
    [Serializable] public class Sample { public int view,comparedPixels,changedPixels,visiblePixels; public double meanRgbError; }
    [Serializable] public class Control { public string name; public int changedPixels; public double meanRgbError; public bool rejected; }
    [Serializable] public class Report { public string unityVersion,graphicsApi,clip; public float time,maxPositionDistanceMetres; public Sample[] views,linearNormalDiagnostic,linearBasisDiagnostic; public Control[] controls; public bool corruptionRejected,passed,shaderSampled,topologyChanged; }
    [Serializable] public class AnimationReport { public Report[] poses; public bool passed; }
    static bool Matches(int changed,double mean,int visible) { return changed<=visible*.01 && mean<=.002; }
    static double Mean(Color32[] a,Color32[] b) {
        return a.Zip(b,(x,y)=>(double)(Math.Abs(x.r-y.r)+Math.Abs(x.g-y.g)+Math.Abs(x.b-y.b))).Sum()/(a.Length*3.0*255);
    }
    public static void Run() {
        var rest=RunPose(null,"source-renders");
        File.WriteAllText("source-render-report.json",JsonUtility.ToJson(rest,true));
        if(!rest.passed)throw new Exception("Direct GLB versus imported FBX rest rendering differs");
        var poses=GcadSourceMeshReference.Poses("material-reference.glb");
        var reports=poses.Select((pose,index)=>RunPose(pose,"source-renders/pose-"+index)).ToArray();
        var animation=new AnimationReport { poses=reports,passed=(reports.Length==0 || reports.Length==21) && reports.All(r=>r.passed) };
        File.WriteAllText("source-animation-report.json",JsonUtility.ToJson(animation,true));
        if(!animation.passed)throw new Exception("Direct GLB versus imported FBX animated rendering differs; inspect source-animation-report.json");
    }
    public static void RunLodQuality() {
        const string reference="lod-reference.glb";
        if(!GcadSourceMeshReference.Describe(reference).clips.SequenceEqual(GcadSourceMeshReference.Describe("material-reference.glb").clips))
            throw new Exception("Original and LOD clip sets differ");
        var poses=GcadSourceMeshReference.Poses(reference);
        var reports=new[]{RunPose(null,"lod-quality/rest",reference,true)}.Concat(
            poses.Select((pose,index)=>RunPose(pose,"lod-quality/pose-"+index,reference,true))).ToArray();
        var report=new AnimationReport{poses=reports,passed=(reports.Length==1 || reports.Length==22)&&reports.All(r=>r.passed)};
        File.WriteAllText("lod-quality-report.json",JsonUtility.ToJson(report,true));
        if(!report.passed)throw new Exception("Original versus LOD rendered quality gate failed");
    }
    static Report RunPose(GcadSourceMeshReference.Pose pose,string output,string referencePath="material-reference.glb",bool topologyChanged=false) {
        EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Single);
        var imported=UnityEngine.Object.Instantiate(AssetDatabase.LoadAssetAtPath<GameObject>("Assets/GcadAdapted/model.prefab"));
        bool shaderCheck=Environment.GetEnvironmentVariable("GCAD_SOURCE_SHADER_CHECK")=="1";
        var shader=imported.GetComponent<GcadUvScrollAnimation>();double shaderTime=pose==null?0:pose.time;
        if(shaderCheck&&(!shader||shader.bindings.Length!=3||shader.bindings.Any(b=>b.speed!=new Vector2(.125f,.25f)||b.frozenSeconds!=2)))
            throw new Exception("Three-material frozen shader fixture required");
        var diagnosticMeshes=new List<Mesh>();
        if(Environment.GetEnvironmentVariable("GCAD_IMPORT_DIAGNOSTIC")=="source-tangents-flip") {
            foreach(var skin in imported.GetComponentsInChildren<SkinnedMeshRenderer>()) {
                var mesh=UnityEngine.Object.Instantiate(skin.sharedMesh);diagnosticMeshes.Add(mesh);
                mesh.tangents=mesh.tangents.Select(t=>new Vector4(t.x,t.y,t.z,-t.w)).ToArray();skin.sharedMesh=mesh;
            }
        }
        if(pose!=null) {
            var clip=AssetDatabase.LoadAllAssetsAtPath("Assets/Fixture/model.fbx").OfType<AnimationClip>().Single(c=>c.name==pose.clip);
            clip.SampleAnimation(imported,pose.time);
        }
        foreach(var deformation in imported.GetComponentsInChildren<GcadDeformation>())
            if(!deformation.Refresh())throw new Exception("Deformation adapter: "+deformation.LastError);
        foreach(var skin in imported.GetComponentsInChildren<SkinnedMeshRenderer>())skin.updateWhenOffscreen=true;
        var reference=GcadSourceMeshReference.Create(referencePath,pose);
        if(pose==null&&!topologyChanged)GcadTangentDiagnostic.Run(reference,imported);
        var sourcePoints=reference.GetComponentsInChildren<MeshFilter>().SelectMany(f=>f.sharedMesh.vertices.Select(v=>f.transform.TransformPoint(v))).ToArray();
        var importedPoints=new List<Vector3>();
        foreach(var skin in imported.GetComponentsInChildren<SkinnedMeshRenderer>()) {
            var deformation=skin.GetComponent<GcadDeformation>();
            if(deformation && deformation.CurrentMesh) {
                importedPoints.AddRange(deformation.CurrentMesh.vertices.Select(v=>skin.transform.TransformPoint(v)));continue;
            }
            var baked=new Mesh();
            try { skin.BakeMesh(baked,false);importedPoints.AddRange(baked.vertices.Select(v=>skin.transform.TransformPoint(v))); }
            finally { UnityEngine.Object.DestroyImmediate(baked); }
        }
        foreach(var filter in imported.GetComponentsInChildren<MeshFilter>())
            if(!filter.GetComponentInParent<GcadDeformation>() && filter.GetComponent<MeshRenderer>() && filter.GetComponent<MeshRenderer>().enabled)
                importedPoints.AddRange(filter.sharedMesh.vertices.Select(v=>filter.transform.TransformPoint(v)));
        float positionError=GcadPoseCheck.Distance(sourcePoints,importedPoints.ToArray());
        // Diagnostic only: never substitutes for the inverse-transpose acceptance reference.
        var linearReference=pose==null ? null : GcadSourceMeshReference.Create(referencePath,pose,true);
        if(linearReference!=null)linearReference.SetActive(false);
        var linearBasis=pose==null ? null : GcadSourceMeshReference.Create(referencePath,pose,true,false);
        if(linearBasis!=null)linearBasis.SetActive(false);
        if(shaderCheck)foreach(var root in new[]{reference,linearReference,linearBasis})if(root)
            foreach(var filter in root.GetComponentsInChildren<MeshFilter>(true))
                filter.sharedMesh.uv=filter.sharedMesh.uv.Select(uv=>new Vector2(uv.x+(float)((shaderTime-2)*.125),uv.y-(float)((shaderTime-2)*.25))).ToArray();
        // Disabled SkinnedMeshRenderer bounds can differ between editor and player.
        // Frame the independently evaluated reference geometry in both consumers.
        var bounds=new Bounds(sourcePoints[0],Vector3.zero);
        foreach(var point in sourcePoints)bounds.Encapsulate(point);
        reference.SetActive(false);
        RenderSettings.ambientMode=AmbientMode.Flat;RenderSettings.ambientLight=new Color(.22f,.22f,.22f);
        var light=new GameObject("source comparison light").AddComponent<Light>();light.type=LightType.Directional;
        light.intensity=1.2f;light.transform.rotation=Quaternion.Euler(50,-35,0);
        var camera=new GameObject("source comparison camera").AddComponent<Camera>();camera.enabled=false;
        camera.orthographic=true;camera.orthographicSize=bounds.extents.magnitude*1.15f;
        camera.transform.position=bounds.center+new Vector3(1,1.5f,1).normalized*bounds.size.magnitude*3;camera.transform.LookAt(bounds.center);
        // Fit depth to this centimetre-scale fixture, just as framing is fitted in XY.
        // A fixed ten-metre range wastes precision on folded, nearly coplanar surfaces.
        float distance=bounds.size.magnitude*3,radius=bounds.extents.magnitude*1.2f;
        camera.nearClipPlane=Mathf.Max(.000001f,distance-radius);camera.farClipPlane=distance+radius;
        camera.allowHDR=false;camera.allowMSAA=false;
        camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=new Color(.02f,.025f,.035f,1);
        camera.targetTexture=new RenderTexture(512,512,24,RenderTextureFormat.ARGB32);camera.targetTexture.Create();
        Directory.CreateDirectory(output);
        try {
            var directions=new[]{new Vector3(1,1.5f,1),new Vector3(-1,1.5f,-1),new Vector3(1,-1.5f,1),new Vector3(-1,-1.5f,-1)};
            var samples=new List<Sample>();var linearSamples=new List<Sample>();var basisSamples=new List<Sample>();var controls=new List<Control>();Color32[] expected=null;
            for(int view=0;view<directions.Length;view++) {
                camera.transform.position=bounds.center+directions[view].normalized*bounds.size.magnitude*3;
                camera.transform.LookAt(bounds.center);
                imported.SetActive(false);reference.SetActive(false);
                var empty=GcadRenderCheck.Capture(camera,null);imported.SetActive(true);
                if(shaderCheck&&!shader.Sample(shaderTime))throw new Exception(shader.LastError);
                var actual=GcadRenderCheck.Capture(camera,output+"/imported-"+view+".png");
                imported.SetActive(false);reference.SetActive(true);
                expected=GcadRenderCheck.Capture(camera,output+"/source-"+view+".png");
                samples.Add(new Sample { view=view,comparedPixels=actual.Length,changedPixels=GcadRenderCheck.Changed(actual,expected),
                    visiblePixels=GcadRenderCheck.Changed(expected,empty),meanRgbError=Mean(actual,expected) });
                if(shaderCheck) {
                    reference.SetActive(false);imported.SetActive(true);
                    if(!shader.Sample(shaderTime+.7))throw new Exception(shader.LastError);
                    var wrongTime=GcadRenderCheck.Capture(camera,null);
                    int changed=GcadRenderCheck.Changed(expected,wrongTime);double mean=Mean(expected,wrongTime);
                    controls.Add(new Control{name="shader-time-view-"+view,changedPixels=changed,meanRgbError=mean,rejected=!Matches(changed,mean,samples[view].visiblePixels)});
                    imported.SetActive(false);reference.SetActive(true);
                }
                if(linearReference!=null) {
                    reference.SetActive(false);linearReference.SetActive(true);
                    var diagnostic=GcadRenderCheck.Capture(camera,output+"/linear-normal-"+view+".png");
                    linearSamples.Add(new Sample { view=view,comparedPixels=actual.Length,changedPixels=GcadRenderCheck.Changed(actual,diagnostic),
                        visiblePixels=GcadRenderCheck.Changed(diagnostic,empty),meanRgbError=Mean(actual,diagnostic) });
                    linearReference.SetActive(false);reference.SetActive(true);
                }
                if(linearBasis!=null) {
                    reference.SetActive(false);linearBasis.SetActive(true);
                    var diagnostic=GcadRenderCheck.Capture(camera,output+"/linear-basis-"+view+".png");
                    basisSamples.Add(new Sample { view=view,comparedPixels=actual.Length,changedPixels=GcadRenderCheck.Changed(actual,diagnostic),
                        visiblePixels=GcadRenderCheck.Changed(diagnostic,empty),meanRgbError=Mean(actual,diagnostic) });
                    linearBasis.SetActive(false);reference.SetActive(true);
                }
                // Exercise attribute corruption through the same acceptance gate as the real comparison.
                var meshes=reference.GetComponentsInChildren<MeshFilter>().Select(f=>f.sharedMesh).ToArray();
                foreach(string attribute in new[]{"uv","normal"}) {
                    var savedUvs=meshes.Select(m=>m.uv).ToArray();var savedNormals=meshes.Select(m=>m.normals).ToArray();
                    try {
                        foreach(var mesh in meshes) {
                            if(attribute=="uv")mesh.uv=mesh.uv.Select(uv=>uv+new Vector2(.137f,.217f)).ToArray();
                            else mesh.normals=mesh.normals.Select(normal=>-normal).ToArray();
                        }
                        var damaged=GcadRenderCheck.Capture(camera,null);
                        int changed=GcadRenderCheck.Changed(expected,damaged);double mean=Mean(expected,damaged);
                        controls.Add(new Control { name=attribute+"-view-"+view,changedPixels=changed,meanRgbError=mean,
                            rejected=!Matches(changed,mean,samples[view].visiblePixels) });
                    } finally {
                        for(int m=0;m<meshes.Length;m++){meshes[m].uv=savedUvs[m];meshes[m].normals=savedNormals[m];}
                    }
                }
            }
            reference.transform.position=Vector3.right*.1f;
            var corrupt=GcadRenderCheck.Capture(camera,output+"/corrupt.png");
            int shifted=GcadRenderCheck.Changed(expected,corrupt);double shiftedMean=Mean(expected,corrupt);
            controls.Add(new Control { name="translation",changedPixels=shifted,meanRgbError=shiftedMean,
                rejected=!Matches(shifted,shiftedMean,samples.Last().visiblePixels) });
            var report=new Report { shaderSampled=shaderCheck,unityVersion=Application.unityVersion,graphicsApi=SystemInfo.graphicsDeviceType.ToString(),clip=pose==null ? "rest" : pose.clip,time=pose==null ? 0 : pose.time,maxPositionDistanceMetres=positionError,views=samples.ToArray(),linearNormalDiagnostic=linearSamples.ToArray(),linearBasisDiagnostic=basisSamples.ToArray(),controls=controls.ToArray(),
                corruptionRejected=controls.All(c=>c.rejected) };
            // No silhouette pixels are excluded. Bound changed pixels relative to object coverage.
            report.topologyChanged=topologyChanged;
            // Nearest-vertex distance is not a surface metric for changed topology.
            // LOD quality here is rendered quality only; Rust separately gates sampled surfaces.
            report.passed=(topologyChanged||positionError<=.00001f) && samples.All(s=>s.visiblePixels>1000 && Matches(s.changedPixels,s.meanRgbError,s.visiblePixels)) && report.corruptionRejected;
            return report;
        } finally {
            foreach(var mesh in diagnosticMeshes)UnityEngine.Object.DestroyImmediate(mesh);
            camera.targetTexture.Release();UnityEngine.Object.DestroyImmediate(camera.targetTexture);
            foreach(var filter in reference.GetComponentsInChildren<MeshFilter>())UnityEngine.Object.DestroyImmediate(filter.sharedMesh);
            if(linearReference!=null) {
                foreach(var filter in linearReference.GetComponentsInChildren<MeshFilter>())UnityEngine.Object.DestroyImmediate(filter.sharedMesh);
                UnityEngine.Object.DestroyImmediate(linearReference);
            }
            if(linearBasis!=null) {
                foreach(var filter in linearBasis.GetComponentsInChildren<MeshFilter>())UnityEngine.Object.DestroyImmediate(filter.sharedMesh);
                UnityEngine.Object.DestroyImmediate(linearBasis);
            }
            foreach(var obj in new[]{reference,imported,light.gameObject,camera.gameObject})UnityEngine.Object.DestroyImmediate(obj);
        }
    }
}
