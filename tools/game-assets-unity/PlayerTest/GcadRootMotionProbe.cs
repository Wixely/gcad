using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;

// Shared editor/player acceptance against direct-GLB poses frozen by the editor.
public static class GcadRootMotionProbe
{
    [Serializable] public class Sample {public double seconds;public bool looping;public float maxPositionError;}
    [Serializable] public class Report {public string unityVersion,reference="direct-glb-trs-on-imported-mesh";public int schema,controls,skinnedMeshes;public Sample[] samples;public bool lifecycle,invalidTime,invalidTrack,passed;}
    [Serializable] public class Expected {public double seconds;public bool looping;public Vector3[] points;}
    [Serializable] public class Oracle {public Expected[] samples;public Vector3[] inPlace;}
    static Matrix4x4 Local(Transform t) {return Matrix4x4.TRS(t.localPosition,t.localRotation,Vector3.one);}
    static Vector3[] Points(GameObject root) {
        var points=root.GetComponentsInChildren<MeshFilter>().SelectMany(f=>f.sharedMesh.vertices.Select(v=>f.transform.TransformPoint(v))).ToList();
        foreach(var skin in root.GetComponentsInChildren<SkinnedMeshRenderer>()) {
            var baked=new Mesh();
            try {skin.BakeMesh(baked);if(baked.vertexCount==0)throw new Exception("Empty skinned motion fixture");points.AddRange(baked.vertices.Select(v=>skin.transform.TransformPoint(v)));}
            finally {if(Application.isPlaying)UnityEngine.Object.Destroy(baked);else UnityEngine.Object.DestroyImmediate(baked);}
        }
        if(points.Count==0)throw new Exception("Empty motion fixture");return points.ToArray();
    }
    static Matrix4x4 Power(Matrix4x4 value,int count) {
        if(count<0)value=value.inverse;
        var result=Matrix4x4.identity;for(int i=0;i<Math.Abs(count);i++)result=result*value;return result;
    }
    public static Oracle CreateOracle(GameObject expected,int schema,double duration,string nodeName,bool[] axes,string source,string inPlace,Action<GameObject,string,double> sample,double[] keyTimes) {
        var root=expected.GetComponentsInChildren<Transform>(true).Single(t=>t.name==nodeName);
        var samples=new List<Expected>();
        sample(expected,source,0);var start=Local(root);
        sample(expected,source,duration);var end=Local(root);
        var cycle=end*start.inverse;
        Vector3 displacement=end.GetColumn(3)-start.GetColumn(3);
        for(int k=0;k<3;k++)if(!axes[k])displacement[k]=0;
        var times=new[]{-4.25,-.1,0,.125,.7,.99999,.99999999,1,1.00000001,1.99999,1.99999999,2,2.00000001,2.1,4.25,8.7,.7,.7,0}
            .Concat(keyTimes.SelectMany(t=>new[]{t-1e-8,t,t+1e-8,t+duration-1e-8,t+duration,t+duration+1e-8})).ToArray();
        foreach(bool looping in new[]{false,true})foreach(double seconds in times) {
            int cycles=looping?(int)Math.Floor(seconds/duration):0;
            double phase=looping?seconds-cycles*duration:Math.Max(0,Math.Min(duration,seconds));
            sample(expected,source,phase);
            if(looping) {
                if(schema==1)root.localPosition+=cycles*displacement;
                else {var pose=Power(cycle,cycles)*Local(root);root.localPosition=pose.GetColumn(3);root.localRotation=pose.rotation;}
            }
            samples.Add(new Expected{seconds=seconds,looping=looping,points=Points(expected)});
        }
        sample(expected,inPlace,.7);
        return new Oracle{samples=samples.ToArray(),inPlace=Points(expected)};
    }
    public static Report Run(GameObject actual,Oracle oracle) {
        var player=actual.GetComponent<GcadRootMotionPlayer>();
        if(!player||oracle==null||oracle.samples==null||oracle.samples.Length<38)throw new Exception("Motion player and source oracle required");
        var samples=new List<Sample>();int controls=0;
        var report=new Report{unityVersion=Application.unityVersion,schema=player.motion.schema,skinnedMeshes=actual.GetComponentsInChildren<SkinnedMeshRenderer>().Length};
        foreach(var expected in oracle.samples) {
            if(!player.Sample(expected.seconds,expected.looping))throw new Exception(player.LastError);
            var points=expected.points;float error=GcadPositionMetric.Distance(points,Points(actual));
            samples.Add(new Sample{seconds=expected.seconds,looping=expected.looping,maxPositionError=error});
            var position=player.motionRoot.localPosition;player.motionRoot.localPosition+=Vector3.right*.1f;
            if(GcadPositionMetric.Distance(points,Points(actual))<=.00001f)throw new Exception("Motion corruption control was accepted");
            player.motionRoot.localPosition=position;controls++;
        }
        if(!player.Sample(.7,true))throw new Exception(player.LastError);
        var last=Points(actual);
        report.invalidTime=!player.Sample(double.NaN,true)&&!player.Sample(player.motion.duration*1000001,true)&&GcadPositionMetric.Distance(last,Points(actual))==0;
        if(player.exactTrackPlayback) {
            var tracks=player.exactTracks;player.exactTracks=null;
            bool missing=!player.Sample(.7,true)&&GcadPositionMetric.Distance(last,Points(actual))==0;
            player.exactTracks=tracks;double time=tracks[0].times[0];tracks[0].times[0]=double.NaN;
            report.invalidTrack=missing&&!player.Sample(.7,true)&&GcadPositionMetric.Distance(last,Points(actual))==0;
            tracks[0].times[0]=time;
        } else report.invalidTrack=true;
        player.enabled=false;
        bool restored=GcadPositionMetric.Distance(oracle.inPlace,Points(actual))<=.00001f;
        player.enabled=true;bool resumed=player.Sample(.7,true);
        report.lifecycle=restored&&resumed&&GcadPositionMetric.Distance(last,Points(actual))<=.00001f;
        report.samples=samples.ToArray();report.controls=controls;
        report.passed=samples.Count==oracle.samples.Length&&samples.All(s=>s.maxPositionError<=.00001f)&&report.invalidTime&&report.invalidTrack&&report.lifecycle&&controls==samples.Count;
        return report;
    }
}
