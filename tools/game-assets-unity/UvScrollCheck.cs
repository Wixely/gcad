using System;
using System.IO;
using UnityEditor;
using UnityEngine;

public static class GcadUvScrollCheck
{
    const int Size=16;
    [Serializable] class Report {public string unityVersion,graphicsApi;public int cases,pixels,failedPixels,rejectedControls;public double maximumError;public bool invalidRejected,lastGoodPreserved,passed;}
    static int Index(double uv,int mode) {
        int i=(int)Math.Floor(uv*4);
        if(mode==0)return ((i%4)+4)%4;
        if(mode==1)return Math.Max(0,Math.Min(3,i));
        int at=((i%8)+8)%8;return at<4?at:7-at;
    }
    static Color Expected(Color32 value,int output) {
        var c=(Color)value;
        if(output==0||output==2)return c;
        if(output==1)return new Color(c.b,1-c.g,c.r,1);
        var n=new Vector3(c.r*2-1,c.g*2-1,c.b*2-1).normalized;return new Color(n.x,n.y,n.z,1);
    }
    static double Error(Color a,Color b) {return Math.Max(Math.Max(Math.Abs(a.r-b.r),Math.Abs(a.g-b.g)),Math.Max(Math.Abs(a.b-b.b),Math.Abs(a.a-b.a)));}
    static bool Reject(Action action){try{action();return false;}catch(ArgumentException){return true;}}
    public static void Run() {
        PlayerSettings.colorSpace=ColorSpace.Linear;
        var report=new Report {unityVersion=Application.unityVersion,graphicsApi=SystemInfo.graphicsDeviceType.ToString()};
        var material=new Material(Shader.Find("Hidden/gcad/Surface Probe"));
        var texture=new Texture2D(4,4,TextureFormat.RGBA32,false,true);
        var values=new Color32[16];for(int y=0;y<4;y++)for(int x=0;x<4;x++)values[y*4+x]=new Color32((byte)(20+x*60),(byte)(30+y*55),(byte)(40+((x+2*y)%4)*50),255);
        texture.SetPixels32(values);texture.Apply();texture.filterMode=FilterMode.Point;
        foreach(var slot in new[]{"_BaseMap","_RoughnessMap","_MetallicMap","_OcclusionMap","_EmissionMap","_NormalMap"}) {
            material.SetTexture(slot,texture);material.SetVector(slot+"_Filter",new Vector4(9728,9728,0,0));
        }
        material.SetVector("_BaseFactor",Vector4.one);material.SetVector("_EmissionFactor",Vector4.one);
        foreach(var name in new[]{"_Metallic","_Roughness","_OcclusionStrength","_NormalScale","_HasNormal","_Facing"})material.SetFloat(name,1);
        var target=new RenderTexture(Size,Size,0,RenderTextureFormat.ARGBFloat,RenderTextureReadWrite.Linear);target.Create();
        var readback=new Texture2D(Size,Size,TextureFormat.RGBAFloat,false,true);var previous=RenderTexture.active;bool srgb=GL.sRGBWrite;
        var scale=new Vector2(1.5f,.75f);var offset=new Vector2(.125f,-.25f);var speed=new Vector2(.125f,.25f);
        Func<Color[]> capture=()=>{Graphics.Blit(null,target,material);RenderTexture.active=target;readback.ReadPixels(new Rect(0,0,Size,Size),0,0);readback.Apply();return readback.GetPixels();};
        try {
            GL.sRGBWrite=false;
            for(int wrap=0;wrap<3;wrap++) {
                texture.wrapMode=wrap==0?TextureWrapMode.Repeat:wrap==1?TextureWrapMode.Clamp:TextureWrapMode.Mirror;
                foreach(double seconds in new[]{0.0,.5,2.0,4.0,0.0})for(int output=0;output<4;output++)for(int frozen=0;frozen<2;frozen++) {
                    material.SetFloat("_Output",output);material.SetFloat("_ProbeFrozen",frozen);
                    // Independent representation of UVs frozen at two seconds.
                    material.SetVector("_ProbeFrozenTransform",new Vector4(1.5f,.75f,.375f,0));
                    var transform=frozen==0?GcadUvScroll.Original(scale,offset,speed,seconds):GcadUvScroll.Frozen(speed,2,seconds);
                    GcadUvScroll.Apply(material,transform);var actual=capture();
                    var expected=new Color[Size*Size];
                    for(int y=0;y<Size;y++)for(int x=0;x<Size;x++) {
                        double u=(x+.5)/Size,sourceV=1-(y+.5)/Size;
                        double sourceU=u*1.5+.125+seconds*.125,transformedV=sourceV*.75-.25+seconds*.25;
                        expected[y*Size+x]=Expected(values[Index(1-transformedV,wrap)*4+Index(sourceU,wrap)],output);
                        double error=Error(actual[y*Size+x],expected[y*Size+x]);
                        if(double.IsNaN(error)||error>.001)report.failedPixels++;
                        report.maximumError=Math.Max(report.maximumError,error);
                    }
                    transform.z+=.25f;GcadUvScroll.Apply(material,transform);var corrupt=capture();
                    bool rejected=false;for(int i=0;i<corrupt.Length;i++)rejected|=Error(corrupt[i],expected[i])>.001;
                    if(rejected)report.rejectedControls++;report.cases++;report.pixels+=actual.Length;
                }
            }
            var saved=material.GetVector("_GcadUvTransform");
            report.invalidRejected=Reject(()=>GcadUvScroll.Apply(material,new Vector4(1,1,float.NaN,0))) &&
                Reject(()=>GcadUvScroll.Original(Vector2.one,Vector2.zero,speed,double.NaN)) &&
                Reject(()=>GcadUvScroll.Frozen(speed,-1,2));
            report.lastGoodPreserved=material.GetVector("_GcadUvTransform")==saved;
        } finally {
            RenderTexture.active=previous;GL.sRGBWrite=srgb;target.Release();
            foreach(var obj in new UnityEngine.Object[]{target,readback,texture,material})UnityEngine.Object.DestroyImmediate(obj);
        }
        report.passed=report.cases==120&&report.failedPixels==0&&report.rejectedControls==120&&report.invalidRejected&&report.lastGoodPreserved;
        File.WriteAllText("uv-scroll-report.json",JsonUtility.ToJson(report,true));
        if(!report.passed)throw new Exception("UV scroll checks failed; inspect uv-scroll-report.json");
    }
}
