using System;
using System.IO;
using System.Linq;
using UnityEngine;

public static class GcadPositionMetricCheck
{
    [Serializable] class Report { public int oracleCases,largePointsPerSet; public double maxOracleDifference,largeMilliseconds; public bool invalidRejected,inputsPreserved,largeAnalyticMatch,passed; }
    static double Brute(Vector3[] a,Vector3[] b) {
        double maximum=0;
        foreach(var p in a) {
            double nearest=double.PositiveInfinity;
            foreach(var q in b) {
                double x=(double)p.x-q.x,y=(double)p.y-q.y,z=(double)p.z-q.z;
                nearest=Math.Min(nearest,x*x+y*y+z*z);
            }
            maximum=Math.Max(maximum,nearest);
        }
        return Math.Sqrt(maximum);
    }
    public static void Run() {
        var report=new Report { inputsPreserved=true,invalidRejected=true,largePointsPerSet=100000 };
        var random=new System.Random(40817);
        foreach(int count in new[]{1,2,31,127,511})foreach(int shape in new[]{0,1,2,3}) {
            var a=Enumerable.Range(0,count).Select(i=>new Vector3((float)random.NextDouble()*2-1,
                shape>=1?0:(float)random.NextDouble()*2-1,shape>=2?0:(float)random.NextDouble()*2-1)).ToArray();
            if(shape==3)for(int i=0;i<a.Length;i++)a[i]=Vector3.one;
            var b=a.Reverse().Take(Math.Max(1,count/2)).Select(p=>p+new Vector3(.01f,-.02f,.03f)).ToArray();
            var savedA=(Vector3[])a.Clone();var savedB=(Vector3[])b.Clone();
            double expected=Math.Max(Brute(a,b),Brute(b,a));double actual=GcadPositionMetric.Distance(a,b);
            report.maxOracleDifference=Math.Max(report.maxOracleDifference,Math.Abs(actual-expected));
            report.inputsPreserved&=a.SequenceEqual(savedA)&&b.SequenceEqual(savedB);report.oracleCases++;
        }
        foreach(var bad in new[]{new Vector3[0],new[]{new Vector3(float.NaN,0,0)},new[]{new Vector3(0,float.PositiveInfinity,0)}}) {
            bool rejected=false;try{GcadPositionMetric.Distance(new[]{Vector3.zero},bad);}catch(ArgumentException){rejected=true;}
            report.invalidRejected&=rejected;
        }
        // Unity's Vector3 == is approximate: nearby distinct points must not be collapsed.
        var tiny=Enumerable.Range(0,16).Select(i=>new Vector3(i*1e-7f,0,0)).ToArray();
        report.maxOracleDifference=Math.Max(report.maxOracleDifference,GcadPositionMetric.Distance(tiny,tiny.Reverse().ToArray()));
        report.oracleCases++;
        var large=Enumerable.Range(0,report.largePointsPerSet).Select(i=>new Vector3(i%100,(i/100)%100,i/10000)).ToArray();
        var shifted=large.Select(p=>p+new Vector3(.125f,.25f,.375f)).ToArray();
        var timer=System.Diagnostics.Stopwatch.StartNew();float distance=GcadPositionMetric.Distance(large,shifted);timer.Stop();
        report.largeMilliseconds=timer.Elapsed.TotalMilliseconds;
        report.largeAnalyticMatch=Math.Abs(distance-Math.Sqrt(.125*.125+.25*.25+.375*.375))<1e-7;
        report.passed=report.oracleCases==21 && report.maxOracleDifference<1e-7 && report.inputsPreserved && report.invalidRejected && report.largeAnalyticMatch;
        File.WriteAllText("position-metric-report.json",JsonUtility.ToJson(report,true));
        if(!report.passed)throw new Exception("Spatial position metric differs from independent reference");
    }
}
