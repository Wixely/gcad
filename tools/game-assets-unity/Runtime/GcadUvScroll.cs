// Explicit clock for the selected gcad template. No wall-clock sampling or asset mutation.
using System;
using UnityEngine;

public static class GcadUvScroll
{
    static bool Finite(double value) {return !double.IsNaN(value)&&!double.IsInfinity(value);}
    static void Time(double seconds) {
        if(!Finite(seconds)||seconds<0||seconds>1000000)throw new ArgumentException("Shader time outside supported bounds");
    }
    static void Speed(Vector2 speed) {
        if(!Finite(speed.x)||!Finite(speed.y)||Math.Abs(speed.x)>10000||Math.Abs(speed.y)>10000)
            throw new ArgumentException("Shader speed outside supported bounds");
    }
    static Vector4 Checked(double sx,double sy,double ox,double oy) {
        if(!Finite(ox)||!Finite(oy)||Math.Abs(ox)>1000000||Math.Abs(oy)>1000000)
            throw new ArgumentException("Evaluated shader offset exceeds supported bounds");
        return new Vector4((float)sx,(float)sy,(float)ox,(float)oy);
    }
    // Convert top-left gcad UV semantics into Unity's bottom-left sampling convention.
    public static Vector4 Original(Vector2 scale,Vector2 offset,Vector2 speed,double seconds) {
        Time(seconds);Speed(speed);
        if(!Finite(scale.x)||!Finite(scale.y)||scale.x<.0001f||scale.y<.0001f||scale.x>10000||scale.y>10000 ||
            !Finite(offset.x)||!Finite(offset.y)||Math.Abs(offset.x)>1000000||Math.Abs(offset.y)>1000000)
            throw new ArgumentException("Shader scale/offset outside supported bounds");
        double u=offset.x+seconds*speed.x,v=offset.y+seconds*speed.y;
        Checked(scale.x,scale.y,u,v);
        return Checked(scale.x,scale.y,u,1-scale.y-v);
    }
    // Frozen FBX UVs already contain scale and offset at frozenSeconds. Apply only
    // the remaining scroll delta; applying Original again would transform twice.
    public static Vector4 Frozen(Vector2 speed,double frozenSeconds,double seconds) {
        Time(frozenSeconds);Time(seconds);Speed(speed);
        double dt=seconds-frozenSeconds;
        return Checked(1,1,dt*speed.x,-dt*speed.y);
    }
    // Caller supplies an owned material instance, using the same clock as animation.
    public static void Apply(Material owned,Vector4 transform) {
        if(!owned||!owned.HasProperty("_GcadUvTransform")||!owned.HasProperty("_GcadUvEnabled"))
            throw new ArgumentException("gcad UV-enabled material required");
        if(!Finite(transform.x)||!Finite(transform.y)||transform.x<.0001f||transform.y<.0001f||transform.x>10000||transform.y>10000)
            throw new ArgumentException("Invalid UV scale");
        Checked(transform.x,transform.y,transform.z,transform.w);
        owned.SetVector("_GcadUvTransform",transform);owned.SetFloat("_GcadUvEnabled",1);
    }
}
