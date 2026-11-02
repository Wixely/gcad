using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using UnityEngine;

public static class GcadFbxMaterialContract
{
    [Serializable] public class Map {
        public string slot, path;
        public int WrapU, WrapV, MagFilter, MinFilter, UvSet, channel;
    }
    [Serializable] public class Surface {
        public string name, AlphaMode;
        public float Roughness, Metallic, NormalScale, OcclusionStrength, AlphaCutoff, BaseAlpha;
        public bool DoubleSided;
        public float[] BaseColour, Emission;
        public Map[] maps;
    }
    [Serializable] public class Contract { public int protocol, materialSchema; public Surface[] materials; }
    [Serializable] class Identity {
        public int protocol, material_schema;
        public string helper, assimp_diff_sha256;
        public string[] capabilities;
    }
    static string Bounded(StreamReader reader,int limit) {
        var result=new StringBuilder();var block=new char[4096];int count;
        while((count=reader.Read(block,0,block.Length))!=0) {
            if(result.Length+count>limit)throw new Exception("Helper output exceeds limit");
            result.Append(block,0,count);
        }
        return result.ToString();
    }
    internal static string Run(string helper,string arguments,int limit) {
        using(var process=new Process()) {
            process.StartInfo=new ProcessStartInfo(helper,arguments) {
                UseShellExecute=false,CreateNoWindow=true,RedirectStandardOutput=true,RedirectStandardError=true
            };
            process.Start();
            var stdout=Task.Run(()=>Bounded(process.StandardOutput,limit));
            var stderr=Task.Run(()=>Bounded(process.StandardError,65536));
            var clock=Stopwatch.StartNew();
            try {
                while(!process.WaitForExit(25)) {
                    if(clock.Elapsed.TotalSeconds>60 || stdout.IsFaulted || stderr.IsFaulted)
                        throw new Exception("Helper timed out or exceeded output bounds");
                }
                if(!Task.WaitAll(new Task[]{stdout,stderr},5000))throw new Exception("Helper streams did not close");
                if(process.ExitCode!=0)throw new Exception("Material helper failed: "+stderr.Result);
                return stdout.Result;
            } finally { if(!process.HasExited){process.Kill();process.WaitForExit();} }
        }
    }
    public static Contract Read(string helper,string model) {
        if(!Path.IsPathRooted(helper) || !File.Exists(helper))throw new Exception("Select an absolute helper executable");
        var identity=JsonUtility.FromJson<Identity>(Run(helper,"--handshake",65536));
        if(identity.protocol!=1 || identity.helper!="gcad-fbx-helper" || identity.material_schema!=3 ||
            identity.assimp_diff_sha256!="783bf12bee4103422f8e8d2f74b2f17ae4dd93109b25266cd67baa9838340bf9" ||
            identity.capabilities==null || !identity.capabilities.Contains("material-inspection-v1-experimental"))
            throw new Exception("Rebuild the reviewed v12 material-inspection helper");
        model=Path.GetFullPath(model);
        if(model.Contains("\"") || model.Contains("\n") || model.Contains("\r"))throw new Exception("Unsupported model path characters");
        // The absolute filename ends in .fbx, so no trailing slash needs Windows quote escaping.
        if(!model.EndsWith(".fbx",StringComparison.OrdinalIgnoreCase))throw new Exception("FBX input required");
        var contract=JsonUtility.FromJson<Contract>(Run(helper,"--inspect-materials-v1 \""+model+"\"",4*1024*1024));
        if(contract.protocol!=1 || contract.materialSchema!=3 || contract.materials==null || contract.materials.Length==0)
            throw new Exception("Invalid material inspection response");
        return contract;
    }
}
