// Test-only player behaviour; do not install with the production adapter.
using System;
using System.IO;
using System.Linq;
using UnityEngine;

[DefaultExecutionOrder(20000)]
public sealed class GcadPlayerCheck : MonoBehaviour
{
    public GameObject prefab;
    [Serializable] public class Report {
        public string unityVersion,failure;
        public bool startup,animatorMoved,disabledRestoresSource,reenabledReusesMesh,reactivation,
            singularRejected,lastGoodRetained,recovered,componentCleanup,passed;
        public int frames;
        public bool shaderClock,pausedTogether,seekTogether,resumedTogether;
    }
    readonly Report report=new Report();
    GameObject model;
    GcadDeformation deformation;
    SkinnedMeshRenderer source;
    Mesh mesh;
    Animator animator;
    Transform rootBone;
    Vector3 scale;
    Vector3[] baseline,lastGood;
    int stage,frames;
    bool finished;
    bool shaderRequired;
    GcadUvScrollAnimation shader;
    double pausedTime;
    Vector3[] pausedVertices;
    int pauseFrame;
    float started;
    static void Require(bool value,string message) { if(!value)throw new Exception(message); }
    void Awake() {
        started=Time.realtimeSinceStartup;Time.captureDeltaTime=1f/60;
        try {
            model=Instantiate(prefab);deformation=model.GetComponentsInChildren<GcadDeformation>().Single();
            source=deformation.source;animator=model.GetComponentInChildren<Animator>();
            shader=model.GetComponent<GcadUvScrollAnimation>();
            shaderRequired=shader!=null;
            Require(animator && animator.runtimeAnimatorController,"Missing player Animator controller");
        } catch(Exception error) { Finish(error.Message); }
    }
    void Update() { if(!finished && Time.realtimeSinceStartup-started>45)Finish("Player lifecycle timed out"); }
    void LateUpdate() {
        if(finished)return;
        try {
            frames++;
            if(shader&&(stage<=1||stage>=8)) {
                // This fixture has one non-looping one-second state and no transitions.
                double seconds=Math.Min(1,animator.GetCurrentAnimatorStateInfo(0).normalizedTime);
                Require(shader.Sample(seconds),"Shader clock sample failed: "+shader.LastError);
                foreach(var renderer in model.GetComponentsInChildren<Renderer>())foreach(var material in renderer.sharedMaterials) {
                    var uv=material.GetVector("_GcadUvTransform");
                    Require(Math.Abs(uv.z-(seconds-2)*.125)<1e-6&&Math.Abs(uv.w+(seconds-2)*.25)<1e-6,"Displayed shader does not follow Animator time");
                }
                report.shaderClock=true;
            }
            switch(stage) {
            case 0:
                mesh=deformation.CurrentMesh;
                report.startup=mesh && deformation.LastError==null && !source.enabled && model.GetComponentsInChildren<MeshRenderer>().Single().enabled;
                Require(report.startup,"OnEnable did not initialize the posed mesh");
                baseline=mesh.vertices;stage++;break;
            case 1:
                if(frames<32)return;
                report.animatorMoved=animator.GetCurrentAnimatorStateInfo(0).normalizedTime>.25f &&
                    baseline.Zip(mesh.vertices,(a,b)=>(a-b).magnitude).Max()>.001f;
                Require(report.animatorMoved,"Animator/LateUpdate did not move the displayed vertices");
                if(shader) {
                    animator.speed=0;pausedTime=shader.SampledSeconds;pausedVertices=mesh.vertices;pauseFrame=frames;stage=8;break;
                }
                BeginLifecycle();break;
            case 8:
                Require(Math.Abs(shader.SampledSeconds-pausedTime)<1e-6&&pausedVertices.SequenceEqual(mesh.vertices),"Pause changed mesh or shader time");
                if(frames-pauseFrame<5)return;
                report.pausedTogether=true;
                animator.Play("bend",0,0);animator.Update(0);stage=9;break;
            case 9:
                Require(shader.SampledSeconds<1e-6,"Seek did not reset shader clock");
                Require(pausedVertices.Zip(mesh.vertices,(a,b)=>(a-b).magnitude).Max()>.001f,"Seek did not reset geometry");
                report.seekTogether=true;animator.speed=1;pauseFrame=frames;stage=10;break;
            case 10:
                if(frames-pauseFrame<10)return;
                Require(shader.SampledSeconds>.1,"Animator and shader did not resume");report.resumedTogether=true;
                BeginLifecycle();break;
            case 2:
                Require(lastGood.SequenceEqual(mesh.vertices),"Disabled component continued publishing poses");
                deformation.enabled=true;
                report.reenabledReusesMesh=deformation.CurrentMesh==mesh && !source.enabled && deformation.LastError==null &&
                    model.GetComponentsInChildren<MeshRenderer>().Count()==1;
                Require(report.reenabledReusesMesh,"Re-enable leaked or replaced the dynamic mesh");
                model.SetActive(false);stage++;break;
            case 3:
                model.SetActive(true);
                report.reactivation=deformation.CurrentMesh==mesh && deformation.LastError==null && !source.enabled;
                Require(report.reactivation,"GameObject reactivation failed");
                animator.enabled=false;rootBone=source.bones[0];scale=rootBone.localScale;
                lastGood=mesh.vertices;rootBone.localScale=Vector3.zero;stage++;break;
            case 4:
                report.singularRejected=deformation.LastError!=null && deformation.LastError.Contains("Singular");
                report.lastGoodRetained=lastGood.SequenceEqual(mesh.vertices) && !source.enabled;
                Require(report.singularRejected && report.lastGoodRetained,"Invalid pose replaced the last good mesh");
                rootBone.localScale=scale;stage++;break;
            case 5:
                report.recovered=deformation.LastError==null && lastGood.SequenceEqual(mesh.vertices);
                Require(report.recovered,"Automatic LateUpdate did not recover");
                Destroy(deformation);stage++;break;
            case 6:
                report.componentCleanup=!deformation && !mesh && source.enabled && model.GetComponentsInChildren<MeshRenderer>().Length==0;
                Require(report.componentCleanup,"Destroy left a mesh/renderer behind or failed to restore the source");
                Destroy(model);stage++;break;
            case 7: Finish(null);break;
            }
        } catch(Exception error) { Finish(error.Message); }
    }
    void BeginLifecycle() {
        deformation.enabled=false;
        report.disabledRestoresSource=source.enabled&&!model.GetComponentsInChildren<MeshRenderer>().Single().enabled;
        Require(report.disabledRestoresSource,"OnDisable did not restore the original renderer");
        lastGood=mesh.vertices;stage=2;
    }
    void Finish(string failure) {
        if(finished)return;finished=true;
        report.unityVersion=Application.unityVersion;report.failure=failure;report.frames=frames;
        report.passed=failure==null && report.startup && report.animatorMoved && report.disabledRestoresSource &&
            report.reenabledReusesMesh && report.reactivation && report.singularRejected && report.lastGoodRetained && report.recovered && report.componentCleanup;
        if(shaderRequired)report.passed&=report.shaderClock&&report.pausedTogether&&report.seekTogether&&report.resumedTogether;
        File.WriteAllText(Path.Combine(Application.dataPath,"..","player-report.json"),JsonUtility.ToJson(report,true));
        Application.Quit(report.passed?0:1);
    }
}
