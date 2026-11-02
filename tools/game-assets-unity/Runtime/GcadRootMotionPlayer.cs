using System;
using UnityEngine;

// Absolute-time motion in Unity coordinates. No accumulated frame deltas.
[Serializable] public sealed class GcadRootMotionData
{
    public int schema;
    public double duration;
    public bool translationStep,rotationStep;
    public double[] translationTimes,rotationTimes;
    public Vector3[] translations;
    public Quaternion[] rotations;
    public Vector3 anchorPosition;
    public Quaternion anchorRotation=Quaternion.identity;
    public static bool Finite(double value) { return !double.IsNaN(value)&&!double.IsInfinity(value); }
    static void Times(double[] times,int count,double duration) {
        if(times==null||times.Length!=count||count<1||count>65536)throw new Exception("Invalid root-motion key count");
        for(int i=0;i<count;i++)if(!Finite(times[i])||times[i]<0||times[i]>duration||(i>0&&times[i]<=times[i-1]))
            throw new Exception("Invalid root-motion key times");
    }
    static bool Finite(Vector3 value) { return Finite(value.x)&&Finite(value.y)&&Finite(value.z); }
    static bool Unit(Quaternion value) {
        double length=(double)value.x*value.x+(double)value.y*value.y+(double)value.z*value.z+(double)value.w*value.w;
        return Finite(length)&&Math.Abs(length-1)<1e-5;
    }
    public void Validate() {
        if((schema!=1&&schema!=2)||!Finite(duration)||duration<=0||translations==null||!Finite(anchorPosition)||!Unit(anchorRotation))
            throw new Exception("Invalid root-motion description");
        if(translations!=null&&translations.Length>0) {
            Times(translationTimes,translations.Length,duration);
            foreach(var value in translations)if(!Finite(value))throw new Exception("Nonfinite root translation");
        } else if(schema==1||translationTimes==null||translationTimes.Length!=0)throw new Exception("Invalid translation track");
        if(schema==1) {
            if(translations[0]!=Vector3.zero||rotations==null||rotations.Length!=0||rotationTimes==null||rotationTimes.Length!=0)
                throw new Exception("Translation-only motion requires zero initial displacement and no rotations");
        } else {
            if(rotations==null)throw new Exception("Rigid root motion requires rotation keys");
            Times(rotationTimes,rotations.Length,duration);
            foreach(var value in rotations)if(!Unit(value))throw new Exception("Invalid root rotation");
        }
    }
    static void Interval(double[] times,double seconds,bool step,out int a,out int b,out float fraction) {
        int low=0,high=times.Length;
        while(low<high){int middle=low+(high-low)/2;if(times[middle]<=seconds)low=middle+1;else high=middle;}
        a=Math.Max(0,low-1);b=Math.Min(times.Length-1,low);
        fraction=a==b||step?0:(float)Math.Max(0,Math.Min(1,(seconds-times[a])/(times[b]-times[a])));
    }
    Vector3 Translation(double seconds) {
        if(translations.Length==0)return anchorPosition;
        int a,b;float fraction;Interval(translationTimes,seconds,translationStep,out a,out b,out fraction);
        return Vector3.LerpUnclamped(translations[a],translations[b],fraction);
    }
    Quaternion Rotation(double seconds) {
        int a,b;float fraction;Interval(rotationTimes,seconds,rotationStep,out a,out b,out fraction);
        return Quaternion.SlerpUnclamped(rotations[a],rotations[b],fraction).normalized;
    }
    Matrix4x4 Pose(double seconds) { return Matrix4x4.TRS(Translation(seconds),Rotation(seconds),Vector3.one); }
    public Matrix4x4 Anchor() { Validate();return schema==1?Matrix4x4.identity:Pose(0); }
    static Matrix4x4 Power(Matrix4x4 matrix,long cycles) {
        if(cycles<0)matrix=matrix.inverse;
        long exponent=Math.Abs(cycles);var result=Matrix4x4.identity;
        while(exponent>0){if((exponent&1)!=0)result=result*matrix;exponent>>=1;if(exponent>0)matrix=matrix*matrix;}
        return result;
    }
    public double Phase(double seconds,bool looping) {
        if(!Finite(seconds))throw new Exception("Nonfinite root-motion time");
        double cycles=looping?Math.Floor(seconds/duration):0;
        if(!Finite(cycles)||Math.Abs(cycles)>1000000)throw new Exception("Root-motion cycle limit exceeded");
        return looping?seconds-cycles*duration:Math.Max(0,Math.Min(duration,seconds));
    }
    public Matrix4x4 Evaluate(double seconds,bool looping) {
        Validate();double phase=Phase(seconds,looping);long cycles=looping?(long)Math.Floor(seconds/duration):0;
        Matrix4x4 result;
        if(schema==1)result=Matrix4x4.Translate(Translation(phase)+translations[translations.Length-1]*cycles);
        else {
            var inverse=Pose(0).inverse;
            result=Power(Pose(duration)*inverse,cycles)*(Pose(phase)*inverse);
        }
        for(int i=0;i<16;i++)if(!Finite(result[i]))throw new Exception("Root-motion transform overflow");
        return result;
    }
}

// Explicit manual playback. Call Sample each frame, or in any seek order.
[ExecuteAlways]
[RequireComponent(typeof(Animator))]
public sealed class GcadRootMotionPlayer : MonoBehaviour
{
    public Transform motionRoot;
    public AnimationClip inPlaceClip;
    public GcadRootMotionData motion;
    public GcadMotionTrack[] exactTracks;
    public bool exactTrackPlayback;
    public string LastError { get; private set; }
    bool captured,applied;
    Transform capturedRoot;
    Vector3 restPosition,restScale,inPlacePosition,inPlaceScale;
    Quaternion restRotation,inPlaceRotation;
    public bool Sample(double seconds,bool looping) {
        try {
            if(!isActiveAndEnabled||!motionRoot||!inPlaceClip||motion==null||!motionRoot.IsChildOf(transform))
                throw new Exception("Root-motion player is not ready");
            if(captured&&motionRoot!=capturedRoot)throw new Exception("Root-motion target changed after initialization");
            if(!inPlaceClip.legacy&&!GetComponent<Animator>())throw new Exception("Non-legacy motion sampling requires an Animator component");
            foreach(var animator in GetComponentsInChildren<Animator>(true))
                if(animator.enabled&&animator.runtimeAnimatorController)throw new Exception("Disable competing Animator playback before manual root-motion sampling");
            var delta=motion.Evaluate(seconds,looping);double phase=motion.Phase(seconds,looping);
            if(exactTrackPlayback&&(exactTracks==null||exactTracks.Length==0))throw new Exception("Exact motion tracks are missing");
            if(exactTrackPlayback)foreach(var track in exactTracks) {
                track.Validate(motion.duration);
                if(!track.target.IsChildOf(transform))throw new Exception("Motion track target left the adapted instance");
            }
            if(!captured){capturedRoot=motionRoot;restPosition=motionRoot.localPosition;restRotation=motionRoot.localRotation;restScale=motionRoot.localScale;captured=true;}
            motionRoot.localPosition=restPosition;motionRoot.localRotation=restRotation;motionRoot.localScale=restScale;
            if(exactTrackPlayback)foreach(var track in exactTracks)track.Sample(phase);
            else inPlaceClip.SampleAnimation(gameObject,(float)phase);
            inPlacePosition=motionRoot.localPosition;inPlaceRotation=motionRoot.localRotation;inPlaceScale=motionRoot.localScale;
            motionRoot.localPosition=delta.MultiplyPoint3x4(inPlacePosition);
            motionRoot.localRotation=(delta.rotation*inPlaceRotation).normalized;
            applied=true;LastError=null;return true;
        } catch(Exception error){LastError=error.Message;return false;}
    }
    void Restore() {
        if(applied&&capturedRoot){capturedRoot.localPosition=inPlacePosition;capturedRoot.localRotation=inPlaceRotation;capturedRoot.localScale=inPlaceScale;}
        applied=false;
    }
    void OnDisable(){Restore();}
    void OnDestroy(){Restore();}
}
