using System;
using System.Linq;
using UnityEngine;

public static class GcadConsumerProbe
{
    [Serializable] public class MeshCase {public string name;public int triangles;public bool convex,ownedReadable,hidden,hole,rim,outside,corruption,passed;}
    [Serializable] public class Report {public string unityVersion;public MeshCase[] meshCases;public GcadConsumerSkinProbe.Report skin;public bool socket,collision,hiddenCollision,lods,corruption,passed;}
    // The ring fixture has inner/outer radii 6/10 mm and axial length 10 mm.
    // A triangle collider retains its hole; convex cooking must fill that hole.
    static MeshCase Ring(MeshCollider collider) {
        bool expectedConvex=collider.name=="convex_ring";
        if(!new[]{"triangle_ring","convex_ring","reflected_ring"}.Contains(collider.name))throw new Exception("Unknown collision fixture");
        var mesh=collider.sharedMesh;var bounds=mesh.bounds;
        int axis=0;for(int k=1;k<3;k++)if(bounds.size[k]<bounds.size[axis])axis=k;
        var axial=Vector3.zero;axial[axis]=1;var radial=Vector3.zero;radial[(axis+1)%3]=1;
        var sorted=new[]{bounds.size.x,bounds.size.y,bounds.size.z}.OrderBy(v=>v).ToArray();
        if(Math.Abs(sorted[0]-.01f)>.001f||Math.Abs(sorted[1]-.02f)>.001f||Math.Abs(sorted[2]-.02f)>.001f)throw new Exception("Unexpected ring dimensions");
        Func<float,Ray> ray=offset=>new Ray(collider.transform.TransformPoint(bounds.center+axial*.1f+radial*offset),collider.transform.TransformVector(-axial).normalized);
        RaycastHit hit;
        var result=new MeshCase{name=collider.name,convex=collider.convex,triangles=mesh.triangles.Length/3,
            ownedReadable=mesh.isReadable&&mesh!=collider.GetComponent<MeshFilter>().sharedMesh,
            hidden=!collider.GetComponent<Renderer>().enabled,
            hole=collider.Raycast(ray(0),out hit,1)==expectedConvex,
            rim=collider.Raycast(ray(.008f),out hit,1),outside=!collider.Raycast(ray(.015f),out hit,1)};
        var originalRay=ray(.008f);var position=collider.transform.localPosition;
        try {collider.transform.localPosition+=Vector3.one*3;Physics.SyncTransforms();result.corruption=!collider.Raycast(originalRay,out hit,1);}
        finally {collider.transform.localPosition=position;Physics.SyncTransforms();}
        result.passed=result.convex==expectedConvex&&result.triangles>0&&result.ownedReadable&&result.hidden&&result.hole&&result.rim&&result.outside&&result.corruption;
        return result;
    }
    public static Report Run(GameObject instance,int expectedMeshColliders=0,bool skinned=false) {
        var report=new Report{unityVersion=Application.unityVersion};
        var sockets=instance.GetComponent<GcadSockets>();var handle=sockets?sockets.Find("handle"):null;
        report.socket=handle&&(handle.position-new Vector3(-.01f,.035f,-.03f)).magnitude<1e-6f&&sockets.Find("missing")==null;
        var collider=instance.GetComponentsInChildren<BoxCollider>().Single();
        Physics.SyncTransforms();var center=collider.transform.TransformPoint(collider.center);
        RaycastHit hit;var ray=new Ray(center+collider.transform.forward, -collider.transform.forward);
        report.collision=collider.Raycast(ray,out hit,2)&&hit.distance>0&&hit.distance<1;
        report.hiddenCollision=!collider.GetComponent<Renderer>().enabled&&collider.enabled;
        report.meshCases=instance.GetComponentsInChildren<MeshCollider>().Select(Ring).ToArray();
        var group=instance.GetComponentsInChildren<LODGroup>().Single();var levels=group.GetLODs();
        report.lods=levels.Length==2&&Math.Abs(levels[0].screenRelativeTransitionHeight-.6f)<1e-6&&Math.Abs(levels[1].screenRelativeTransitionHeight-.15f)<1e-6
            &&levels[0].renderers.Length==1&&levels[0].renderers[0].name=="detail_high"&&levels[1].renderers.Length==1&&levels[1].renderers[0].name=="detail_low"
            &&levels.All(l=>l.renderers.All(r=>r.enabled))&&group.size>0&&group.fadeMode==LODFadeMode.None;
        if(handle) {var saved=handle.localPosition;handle.localPosition+=Vector3.right*.1f;report.corruption=(handle.position-new Vector3(-.01f,.035f,-.03f)).magnitude>.01f;handle.localPosition=saved;}
        if(skinned)report.skin=GcadConsumerSkinProbe.Run(instance);
        report.passed=report.socket&&report.collision&&report.hiddenCollision&&report.lods&&report.corruption&&report.meshCases.Length==expectedMeshColliders&&report.meshCases.All(c=>c.passed)&&(!skinned||report.skin.passed);return report;
    }
}
