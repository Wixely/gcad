// Independent CPU texel reference versus the exact sampling code used by gcad PBR.
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEngine;

public static class GcadSamplerCheck
{
    const int Width=64,Height=64;
    [Serializable] public class Case { public string name; public double maximumError; public int failedPixels; }
    [Serializable] public class Report { public string unityVersion,graphicsApi; public int pixels; public Case[] cases; public double maximumError; public bool corruptionRejected,passed; }
    static int Address(int index,int size,TextureWrapMode mode) {
        if(mode==TextureWrapMode.Clamp)return Math.Max(0,Math.Min(size-1,index));
        var period=mode==TextureWrapMode.Mirror ? 2*size : size;
        index=((index%period)+period)%period;
        return index<size ? index : period-1-index;
    }
    static Color Texel(Color[][] levels,int mip,int x,int y,TextureWrapMode u,TextureWrapMode v) {
        int w=Math.Max(1,8>>mip),h=Math.Max(1,4>>mip);
        return levels[mip][Address(y,h,v)*w+Address(x,w,u)];
    }
    static Color Level(Color[][] levels,int mip,double u,double v,bool linear,TextureWrapMode wrapU,TextureWrapMode wrapV) {
        int w=Math.Max(1,8>>mip),h=Math.Max(1,4>>mip);
        if(!linear)return Texel(levels,mip,(int)Math.Floor(u*w),(int)Math.Floor(v*h),wrapU,wrapV);
        double x=u*w-.5,y=v*h-.5;int ix=(int)Math.Floor(x),iy=(int)Math.Floor(y);
        double fx=x-ix,fy=y-iy;
        var result=new Color(0,0,0,0);
        // Explicit four-texel weighted sum, separate from the shader's nested lerps.
        for(int row=0;row<2;row++)for(int col=0;col<2;col++)
            result+=Texel(levels,mip,ix+col,iy+row,wrapU,wrapV)*(float)((col==0 ? 1-fx : fx)*(row==0 ? 1-fy : fy));
        return result;
    }
    static Color Reference(Color[][] levels,double u,double v,double scale,int mag,int min,TextureWrapMode wrapU,TextureWrapMode wrapV) {
        double footprint=Math.Max(8*Math.Abs(scale)/Width,4*Math.Abs(scale)*.63/Height);
        if(footprint<=1)return Level(levels,0,u,v,mag==9729,wrapU,wrapV);
        bool linear=min==9729 || min==9985 || min==9987;
        if(min<9984)return Level(levels,0,u,v,linear,wrapU,wrapV);
        double mip=Math.Min(3,Math.Log(footprint,2));
        if(min==9984 || min==9985)return Level(levels,(int)Math.Floor(mip+.5),u,v,linear,wrapU,wrapV);
        int low=(int)Math.Floor(mip),high=Math.Min(3,low+1);
        return Level(levels,low,u,v,linear,wrapU,wrapV)*(float)(1-(mip-low))+
            Level(levels,high,u,v,linear,wrapU,wrapV)*(float)(mip-low);
    }
    static double Error(Color a,Color b) {
        return new[]{Math.Abs(a.r-b.r),Math.Abs(a.g-b.g),Math.Abs(a.b-b.b),Math.Abs(a.a-b.a)}.Max();
    }
    public static void Run() {
        var shader=Shader.Find("Hidden/gcad/Sampler Probe");
        if(!shader || !shader.isSupported)throw new Exception("Sampler probe shader unavailable");
        var material=new Material(shader);
        var texture=new Texture2D(8,4,TextureFormat.RGBAFloat,true,true);
        var levels=new Color[texture.mipmapCount][];
        for(int mip=0;mip<levels.Length;mip++) {
            int w=Math.Max(1,8>>mip),h=Math.Max(1,4>>mip);
            var colours=new Color[w*h];
            for(int y=0;y<h;y++)for(int x=0;x<w;x++)
                colours[y*w+x]=new Color((x+1)/16f,(y+1)/8f,mip/4f,((x+2*y)%4)/4f);
            texture.SetPixels(colours,mip);levels[mip]=colours;
        }
        texture.Apply(false,false);texture.filterMode=FilterMode.Point;
        var target=new RenderTexture(Width,Height,0,RenderTextureFormat.ARGBFloat,RenderTextureReadWrite.Linear);
        target.Create();var readback=new Texture2D(Width,Height,TextureFormat.RGBAFloat,false,true);
        material.SetTexture("_ProbeTex",texture);material.SetVector("_Offset",new Vector4(-.231f,.127f,0,0));
        var cases=new List<Case>();bool corruptionRejected=false;
        var previous=RenderTexture.active;var previousSrgb=GL.sRGBWrite;
        try {
            GL.sRGBWrite=false;
            var wraps=new[]{TextureWrapMode.Repeat,TextureWrapMode.Clamp,TextureWrapMode.Mirror};
            for(int wrap=0;wrap<3;wrap++)foreach(int mag in new[]{9728,9729})
            foreach(int min in new[]{9728,9729,9984,9985,9986,9987})foreach(float scale in new[]{.7f,8f,12.75f,32f,-12.75f,256f}) {
                texture.wrapModeU=wraps[wrap];texture.wrapModeV=wraps[(wrap+1)%3];
                material.SetVector("_Scale",new Vector4(scale,scale*.63f,0,0));
                material.SetVector("_Filter",new Vector4(mag,min,texture.mipmapCount-1,0));
                Graphics.Blit(null,target,material);RenderTexture.active=target;
                readback.ReadPixels(new Rect(0,0,Width,Height),0,0);readback.Apply(false,false);
                var pixels=readback.GetPixels();
                var result=new Case { name=wraps[wrap]+"/"+wraps[(wrap+1)%3]+" mag="+mag+" min="+min+" scale="+scale };
                for(int y=0;y<Height;y++)for(int x=0;x<Width;x++) {
                    double u=(x+.5)/Width*scale-.231f,v=(y+.5)/Height*(scale*.63f)+.127f;
                    var expected=Reference(levels,u,v,scale,mag,min,wraps[wrap],wraps[(wrap+1)%3]);
                    double error=Error(pixels[y*Width+x],expected);
                    if(double.IsNaN(error) || error>2e-4)result.failedPixels++;
                    result.maximumError=Math.Max(result.maximumError,error);
                    if(!corruptionRejected && Error(pixels[y*Width+x],expected+new Color(.1f,0,0,0))>2e-4)corruptionRejected=true;
                }
                cases.Add(result);
            }
        } finally {
            RenderTexture.active=previous;GL.sRGBWrite=previousSrgb;
            target.Release();UnityEngine.Object.DestroyImmediate(target);UnityEngine.Object.DestroyImmediate(readback);
            UnityEngine.Object.DestroyImmediate(texture);UnityEngine.Object.DestroyImmediate(material);
        }
        var report=new Report { unityVersion=Application.unityVersion,graphicsApi=SystemInfo.graphicsDeviceType.ToString(),
            cases=cases.ToArray(),pixels=cases.Count*Width*Height,maximumError=cases.Max(c=>c.maximumError),
            corruptionRejected=corruptionRejected,passed=cases.All(c=>c.failedPixels==0)&&corruptionRejected };
        File.WriteAllText("sampler-report.json",JsonUtility.ToJson(report,true));
        if(!report.passed)throw new Exception("Numeric sampler validation failed; inspect sampler-report.json");
    }
}
