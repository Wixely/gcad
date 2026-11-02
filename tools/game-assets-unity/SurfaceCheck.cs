// Numeric material evaluation, not lighting/raster-blend acceptance.
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEngine;

public static class GcadSurfaceCheck
{
    const int Size=16;
    [Serializable] public class Case { public int alphaMode,output; public float normalScale,facing,cutoff; public int failedPixels,discardedPixels; public double maximumError; }
    [Serializable] public class Report { public string unityVersion,graphicsApi; public Case[] cases; public int pixels; public double maximumError; public bool corruptionRejected,passed; }
    static readonly Color Clear=new Color(.07f,.11f,.13f,.17f);
    static double Linear(byte b) {
        double value=b/255.0;
        return value<=.04045 ? value/12.92 : Math.Pow((value+.055)/1.055,2.4);
    }
    static double Error(Color a,Color b) { return new[]{Math.Abs(a.r-b.r),Math.Abs(a.g-b.g),Math.Abs(a.b-b.b),Math.Abs(a.a-b.a)}.Max(); }
    static Color Expected(Color32 colour,Color32 numeric,Color32 normal,int output,float scale,float facing) {
        float alpha=colour.a/255f*.6f;
        if(output==0)return new Color((float)Linear(colour.r)*.3f,(float)Linear(colour.g)*.7f,(float)Linear(colour.b)*.2f,alpha);
        if(output==1)return new Color(numeric.b/255f*.4f,1-numeric.g/255f*.65f,1-.35f*(1-numeric.r/255f),1);
        if(output==2)return new Color((float)Linear(colour.r)*.2f,(float)Linear(colour.g)*1.3f,(float)Linear(colour.b)*2f,alpha);
        double nx=(normal.r/255.0*2-1)*scale,ny=(normal.g/255.0*2-1)*scale,nz=normal.b/255.0*2-1;
        double length=Math.Sqrt(nx*nx+ny*ny+nz*nz);
        return new Color((float)(nx/length*facing),(float)(ny/length*facing),(float)(nz/length*facing),1);
    }
    static Texture2D Texture(Color32[] pixels,bool linear) {
        var texture=new Texture2D(4,4,TextureFormat.RGBA32,false,linear);
        texture.SetPixels32(pixels);texture.Apply(false,false);texture.filterMode=FilterMode.Point;
        texture.wrapMode=TextureWrapMode.Clamp;return texture;
    }
    public static void Run() {
        PlayerSettings.colorSpace=ColorSpace.Linear;
        if(QualitySettings.activeColorSpace!=ColorSpace.Linear)throw new Exception("Linear colour space required");
        var shader=Shader.Find("Hidden/gcad/Surface Probe");
        if(!shader || !shader.isSupported)throw new Exception("Surface probe unavailable");
        var material=new Material(shader);
        var colour=new Color32[16];var numeric=new Color32[16];var normals=new Color32[16];
        byte[] values={0,10,128,255},alpha={0,64,128,255},nx={96,128,180,220},ny={40,110,160,200},nz={128,180,220,255};
        for(int y=0;y<4;y++)for(int x=0;x<4;x++) {
            int i=y*4+x;colour[i]=new Color32(values[x],values[y],values[(x+y)%4],alpha[x]);
            numeric[i]=new Color32(values[(x+1)%4],values[(y+1)%4],values[(x+y+1)%4],255);
            normals[i]=new Color32(nx[x],ny[y],nz[(x+y)%4],255);
        }
        var colourTexture=Texture(colour,false);var numericTexture=Texture(numeric,true);var normalTexture=Texture(normals,true);
        foreach(var slot in new[]{"_BaseMap","_EmissionMap"})material.SetTexture(slot,colourTexture);
        foreach(var slot in new[]{"_MetallicMap","_RoughnessMap","_OcclusionMap"})material.SetTexture(slot,numericTexture);
        material.SetTexture("_NormalMap",normalTexture);
        foreach(var slot in new[]{"_BaseMap","_EmissionMap","_MetallicMap","_RoughnessMap","_OcclusionMap","_NormalMap"})
            material.SetVector(slot+"_Filter",new Vector4(9728,9728,0,0));
        material.SetVector("_BaseFactor",new Vector4(.3f,.7f,.2f,.6f));material.SetVector("_EmissionFactor",new Vector4(.2f,1.3f,2,0));
        material.SetFloat("_Metallic",.4f);material.SetFloat("_Roughness",.65f);material.SetFloat("_OcclusionStrength",.35f);material.SetFloat("_HasNormal",1);
        var target=new RenderTexture(Size,Size,0,RenderTextureFormat.ARGBFloat,RenderTextureReadWrite.Linear);target.Create();
        var readback=new Texture2D(Size,Size,TextureFormat.RGBAFloat,false,true);
        var previous=RenderTexture.active;var srgb=GL.sRGBWrite;var cases=new List<Case>();bool corruptionRejected=false;
        try {
            GL.sRGBWrite=false;
            foreach(int mode in new[]{0,1,2})foreach(float scale in new[]{0f,.7f,2f})foreach(float facing in new[]{1f,-1f})
            foreach(float cutoff in new[]{.3f,.6f})for(int output=0;output<4;output++) {
                material.SetFloat("_AlphaMode",mode);material.SetFloat("_NormalScale",scale);material.SetFloat("_Facing",facing);
                material.SetFloat("_Cutoff",cutoff);material.SetFloat("_Output",output);
                RenderTexture.active=target;GL.Clear(false,true,Clear);
                Graphics.Blit(null,target,material);RenderTexture.active=target;
                readback.ReadPixels(new Rect(0,0,Size,Size),0,0);readback.Apply(false,false);
                var pixels=readback.GetPixels();var result=new Case { alphaMode=mode,normalScale=scale,facing=facing,cutoff=cutoff,output=output };
                for(int y=0;y<Size;y++)for(int x=0;x<Size;x++) {
                    int i=(y/4)*4+x/4;
                    bool discard=mode==1 && colour[i].a/255f*.6f<cutoff;
                    var expected=discard ? Clear : Expected(colour[i],numeric[i],normals[i],output,scale,facing);
                    if(discard)result.discardedPixels++;
                    double error=Error(pixels[y*Size+x],expected);
                    if(double.IsNaN(error)||error>0.001)result.failedPixels++;
                    result.maximumError=Math.Max(result.maximumError,error);
                    if(!discard && Error(pixels[y*Size+x],expected+new Color(.1f,0,0,0))>0.001)corruptionRejected=true;
                }
                cases.Add(result);
            }
        } finally {
            RenderTexture.active=previous;GL.sRGBWrite=srgb;target.Release();
            foreach(var item in new UnityEngine.Object[]{target,readback,material,colourTexture,numericTexture,normalTexture})UnityEngine.Object.DestroyImmediate(item);
        }
        var report=new Report { unityVersion=Application.unityVersion,graphicsApi=SystemInfo.graphicsDeviceType.ToString(),cases=cases.ToArray(),
            pixels=cases.Count*Size*Size,maximumError=cases.Max(c=>c.maximumError),corruptionRejected=corruptionRejected,
            passed=cases.All(c=>c.failedPixels==0)&&corruptionRejected };
        File.WriteAllText("surface-report.json",JsonUtility.ToJson(report,true));
        if(!report.passed)throw new Exception("Surface numeric checks failed; inspect surface-report.json");
        GcadRasterCheck.Run();
        GcadUvScrollCheck.Run();
    }
}
