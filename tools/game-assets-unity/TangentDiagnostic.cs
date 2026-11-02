// Fixture attribute audit. Correspondence uses position/normal/UV, never tangents.
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEngine;

public static class GcadTangentDiagnostic
{
    sealed class Vertex { public Vector3 position,normal,tangent; public Vector2 uv; public float sign; public string material; }
    [Serializable] public class Report {
        public int sourceVertices,importedVertices,unmatched,ambiguous,oppositeHandedness,reversedTangents;
        public float maxTangentAngleDegrees,maxPositionError,maxUvError;
    }
    static IEnumerable<Vertex> Vertices(Mesh mesh,Transform transform,int submesh,string material) {
        var p=mesh.vertices;var n=mesh.normals;var t=mesh.tangents;var uv=mesh.uv;
        if(p.Length!=n.Length || p.Length!=t.Length || p.Length!=uv.Length)throw new Exception("Missing basis audit attributes");
        var matrix=transform.localToWorldMatrix;var normalMatrix=matrix.inverse.transpose;
        return mesh.GetIndices(submesh).Distinct().Select(i=>new Vertex {
            position=matrix.MultiplyPoint3x4(p[i]),normal=normalMatrix.MultiplyVector(n[i]).normalized,
            tangent=matrix.MultiplyVector(new Vector3(t[i].x,t[i].y,t[i].z)).normalized,
            uv=uv[i],sign=t[i].w*(matrix.determinant<0 ? -1 : 1),material=material
        });
    }
    public static void Run(GameObject source,GameObject imported) {
        var expected=source.GetComponentsInChildren<MeshFilter>().SelectMany(f=>Vertices(f.sharedMesh,f.transform,0,f.GetComponent<MeshRenderer>().sharedMaterial.name)).ToArray();
        var actual=new List<Vertex>();
        foreach(var skin in imported.GetComponentsInChildren<SkinnedMeshRenderer>()) {
            var baked=new Mesh();
            try {
                skin.BakeMesh(baked,false);
                for(int submesh=0;submesh<baked.subMeshCount;submesh++)actual.AddRange(Vertices(baked,skin.transform,submesh,skin.sharedMaterials[submesh].name));
            }
            finally { UnityEngine.Object.DestroyImmediate(baked); }
        }
        foreach(var filter in imported.GetComponentsInChildren<MeshFilter>()) {
            var renderer=filter.GetComponent<MeshRenderer>();
            if(!renderer || !renderer.enabled || filter.GetComponentInParent<GcadDeformation>())continue;
            for(int submesh=0;submesh<filter.sharedMesh.subMeshCount;submesh++)
                actual.AddRange(Vertices(filter.sharedMesh,filter.transform,submesh,renderer.sharedMaterials[submesh].name));
        }
        var report=new Report { sourceVertices=expected.Length,importedVertices=actual.Count };
        foreach(var vertex in actual) {
            var matches=expected.Where(v=>v.material==vertex.material && (v.position-vertex.position).magnitude<=1e-6f &&
                (v.uv-vertex.uv).magnitude<=1e-5f && Vector3.Dot(v.normal,vertex.normal)>=.99999f).ToArray();
            if(matches.Length==0){report.unmatched++;continue;}
            // All geometrically equivalent source corners must have the same tangent frame.
            var match=matches[0];
            if(matches.Any(v=>Vector3.Dot(v.tangent,match.tangent)<.99999f || v.sign*match.sign<0))
                {report.ambiguous++;continue;}
            report.maxPositionError=Mathf.Max(report.maxPositionError,(match.position-vertex.position).magnitude);
            report.maxUvError=Mathf.Max(report.maxUvError,(match.uv-vertex.uv).magnitude);
            report.maxTangentAngleDegrees=Mathf.Max(report.maxTangentAngleDegrees,Vector3.Angle(match.tangent,vertex.tangent));
            if(Vector3.Dot(match.tangent,vertex.tangent)<-.99999f)report.reversedTangents++;
            if(match.sign*vertex.sign<0)report.oppositeHandedness++;
        }
        File.WriteAllText("tangent-diagnostic-report.json",JsonUtility.ToJson(report,true));
    }
}
