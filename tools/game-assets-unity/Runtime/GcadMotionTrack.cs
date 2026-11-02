using System;
using UnityEngine;

// Exact authored TRS channels in Unity coordinates. FBX import curves may move
// STEP boundaries or bake quaternion rotation, so manual motion uses these keys.
[Serializable] public sealed class GcadMotionTrack
{
    public Transform target;
    public string path;
    public bool step;
    public double[] times;
    public Vector4[] values;
    public void Validate(double duration) {
        if(!target||times==null||values==null||times.Length!=values.Length||times.Length<1||times.Length>65536||
            (path!="translation"&&path!="rotation"&&path!="scale"))throw new Exception("Invalid exact motion track");
        for(int i=0;i<times.Length;i++) {
            if(!GcadRootMotionData.Finite(times[i])||times[i]<0||times[i]>duration+1e-6||(i>0&&times[i]<=times[i-1]))throw new Exception("Invalid exact motion times");
            for(int k=0;k<4;k++)if(!GcadRootMotionData.Finite(values[i][k]))throw new Exception("Nonfinite exact motion value");
            if(path=="rotation"&&Math.Abs(values[i].sqrMagnitude-1)>1e-5)throw new Exception("Invalid exact motion quaternion");
            if(path=="scale"&&(Math.Abs(values[i].x)<=1e-12||Math.Abs(values[i].y)<=1e-12||Math.Abs(values[i].z)<=1e-12))throw new Exception("Singular exact motion scale");
        }
    }
    public void Sample(double seconds) {
        int low=0,high=times.Length;
        while(low<high){int middle=low+(high-low)/2;if(times[middle]<=seconds)low=middle+1;else high=middle;}
        int a=Math.Max(0,low-1),b=Math.Min(times.Length-1,low);
        float t=step||a==b?0:(float)Math.Max(0,Math.Min(1,(seconds-times[a])/(times[b]-times[a])));
        if(path=="rotation") {
            var x=values[a];var y=values[b];
            target.localRotation=Quaternion.SlerpUnclamped(new Quaternion(x.x,x.y,x.z,x.w),new Quaternion(y.x,y.y,y.z,y.w),t).normalized;
        } else {
            Vector3 value=Vector4.LerpUnclamped(values[a],values[b],t);
            if(path=="translation")target.localPosition=value;else target.localScale=value;
        }
    }
}
