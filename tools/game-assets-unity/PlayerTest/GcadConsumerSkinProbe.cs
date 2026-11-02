using System;
using System.Linq;
using UnityEngine;

// Both LOD meshes share this fixture's base/tip skeleton. Check world-space
// movement independently of imported animation curves and mesh vertex ordering.
public static class GcadConsumerSkinProbe
{
    [Serializable] public class Report {public int renderers;public float maxTranslationError,minDeformation;public bool passed;}
    static Vector3[] Bake(SkinnedMeshRenderer renderer) {
        var mesh=new Mesh();
        try {renderer.BakeMesh(mesh);return mesh.vertices.Select(renderer.transform.TransformPoint).ToArray();}
        finally {if(Application.isPlaying)UnityEngine.Object.Destroy(mesh);else UnityEngine.Object.DestroyImmediate(mesh);}
    }
    public static Report Run(GameObject instance) {
        var skins=instance.GetComponentsInChildren<LODGroup>().Single().GetLODs().SelectMany(l=>l.renderers).Cast<SkinnedMeshRenderer>().ToArray();
        var basis=instance.GetComponentsInChildren<Transform>().Single(t=>t.name=="base");
        var tip=instance.GetComponentsInChildren<Transform>().Single(t=>t.name=="tip");
        var position=basis.position;var rotation=tip.localRotation;
        var report=new Report{renderers=skins.Length,minDeformation=float.MaxValue};
        var original=skins.Select(Bake).ToArray();var delta=new Vector3(.005f,.003f,-.002f);
        try {
            basis.position+=delta;
            for(int i=0;i<skins.Length;i++) {
                var moved=Bake(skins[i]);if(moved.Length!=original[i].Length||moved.Length==0)throw new Exception("Invalid skinned LOD vertices");
                for(int j=0;j<moved.Length;j++)report.maxTranslationError=Mathf.Max(report.maxTranslationError,(moved[j]-original[i][j]-delta).magnitude);
            }
            basis.position=position;tip.localRotation=rotation*Quaternion.AngleAxis(35,Vector3.forward);
            for(int i=0;i<skins.Length;i++) {
                var moved=Bake(skins[i]);float maximum=0;
                for(int j=0;j<moved.Length;j++)maximum=Mathf.Max(maximum,(moved[j]-original[i][j]).magnitude);
                report.minDeformation=Mathf.Min(report.minDeformation,maximum);
            }
            report.passed=skins.Length==2&&skins.All(s=>s.bones.Any(b=>b==basis)&&s.bones.Any(b=>b==tip))&&report.maxTranslationError<1e-6f&&report.minDeformation>.001f;
            return report;
        }finally{basis.position=position;tip.localRotation=rotation;}
    }
}
