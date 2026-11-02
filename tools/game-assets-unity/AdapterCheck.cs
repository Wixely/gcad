using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEngine;

public static class GcadAdapterCheck
{
    [Serializable] public class Report { public string unityVersion; public int materials,textures; public string[] failures; public bool passed; }
    public static void Run()
    {
        PlayerSettings.colorSpace=ColorSpace.Linear;
        GcadImportCheck.Run();
        GcadImportDiagnostic.Apply();
        const string model="Assets/Fixture/model.fbx",output="Assets/GcadAdapted";
        var original=Directory.GetFiles("Assets/Fixture","*",SearchOption.AllDirectories).ToDictionary(p=>p,File.ReadAllBytes);
        var prefab=GcadMaterialAdapter.Create(Environment.GetEnvironmentVariable("GCAD_FBX_HELPER"),model,output,
            Environment.GetEnvironmentVariable("GCAD_DEFORMATION_ADAPTER")=="1");
        var source=GcadMaterialCheck.ReadSource();
        var failures=new List<string>();
        Action<bool,string> check=(ok,message)=>{if(!ok)failures.Add(message);};
        int textures=0;
        for(int i=0;i<source.materials.Length;i++) {
            var expected=source.materials[i];
            var material=AssetDatabase.LoadAssetAtPath<Material>(output+"/material_"+i+".mat");
            check(material!=null,"missing material "+i);if(!material)continue;
            check(material.shader.name=="gcad/PBR Opaque Mask","fixture mask shader "+i);
            check(!ShaderUtil.ShaderHasError(material.shader),"shader compilation "+i);
            Action<string,float> scalar=(p,v)=>check(Math.Abs(material.GetFloat(p)-v)<1e-6,"factor "+i+":"+p);
            scalar("_Metallic",expected.pbrMetallicRoughness.metallicFactor);
            scalar("_Roughness",expected.pbrMetallicRoughness.roughnessFactor);
            scalar("_NormalScale",expected.normalTexture.scale);scalar("_OcclusionStrength",expected.occlusionTexture.strength);
            scalar("_Cutoff",expected.alphaCutoff);scalar("_AlphaMode",1);scalar("_Cull",0);
            var colour=material.GetVector("_BaseFactor");var emission=material.GetVector("_EmissionFactor");
            for(int c=0;c<4;c++)check(Math.Abs(colour[c]-expected.pbrMetallicRoughness.baseColorFactor[c])<1e-6,"base factor "+i+":"+c);
            for(int c=0;c<3;c++)check(Math.Abs(emission[c]-expected.emissiveFactor[c])<1e-6,"emission factor "+i+":"+c);
            var slots=new[]{"BaseColour","Roughness","Metallic","Normal","Occlusion","Emission"};
            var properties=new[]{"_BaseMap","_RoughnessMap","_MetallicMap","_NormalMap","_OcclusionMap","_EmissionMap"};
            var references=new[]{expected.pbrMetallicRoughness.baseColorTexture,expected.pbrMetallicRoughness.metallicRoughnessTexture,
                expected.pbrMetallicRoughness.metallicRoughnessTexture,expected.normalTexture,expected.occlusionTexture,expected.emissiveTexture};
            for(int s=0;s<slots.Length;s++) {
                var target=output+"/material_"+i+"_"+slots[s]+".png";
                check(AssetDatabase.GetAssetPath(material.GetTexture(properties[s]))==target,"map connection "+i+":"+slots[s]);
                var image=source.textures[references[s].index].source;
                check(File.ReadAllBytes(target).SequenceEqual(File.ReadAllBytes("Assets/Fixture/textures/image_"+image+".png")),"PNG bytes "+i+":"+slots[s]);
                var importer=(TextureImporter)AssetImporter.GetAtPath(target);
                check(importer.sRGBTexture==(s==0 || s==5),"colour space "+target);
                check(importer.filterMode==FilterMode.Point && !importer.mipmapEnabled,"fixture sampler storage "+target);
                check(importer.wrapModeU==TextureWrapMode.Clamp && importer.wrapModeV==TextureWrapMode.Mirror,"fixture wrapping "+target);
                check(importer.textureType==TextureImporterType.Default && importer.textureCompression==TextureImporterCompression.Uncompressed,"raw PNG interpretation "+target);
                var filter=material.GetVector(properties[s]+"_Filter");
                check(filter.x==9728 && filter.y==9729 && filter.z==0,"distinct min/mag filters "+target);
                textures++;
            }
        }
        foreach(var entry in original)check(File.ReadAllBytes(entry.Key).SequenceEqual(entry.Value),"original changed: "+entry.Key);
        check(prefab.GetComponentsInChildren<Renderer>(true).SelectMany(r=>r.sharedMaterials)
            .All(m=>AssetDatabase.GetAssetPath(m).StartsWith(output+"/",StringComparison.Ordinal)),"prefab material assignment");
        var report=new Report { unityVersion=Application.unityVersion,materials=source.materials.Length,textures=textures,failures=failures.ToArray(),passed=failures.Count==0 };
        File.WriteAllText("adapter-report.json",JsonUtility.ToJson(report,true));
        if(!report.passed)throw new Exception("Adapter contract check failed");
        if(Environment.GetEnvironmentVariable("GCAD_SOURCE_RENDER_CHECK")=="1")GcadPositionMetricCheck.Run();
        if(Environment.GetEnvironmentVariable("GCAD_DEFORMATION_ADAPTER")=="1") {
            GcadSkinWeightRecoveryCheck.Run();GcadDeformationCheck.Run();
        }
        if(Environment.GetEnvironmentVariable("GCAD_DEFORMATION_ADAPTER")=="1" &&
            Environment.GetEnvironmentVariable("GCAD_SOURCE_RENDER_CHECK")=="1")GcadSkinWeightDiagnostic.Run();
        if(Environment.GetEnvironmentVariable("GCAD_UNITY_ADAPTER_RENDER")=="1")GcadAdapterRenderCheck.Run();
        if(Environment.GetEnvironmentVariable("GCAD_SOURCE_RENDER_CHECK")=="1")GcadSourceRenderCheck.Run();
        if(!string.IsNullOrEmpty(Environment.GetEnvironmentVariable("GCAD_LOD_REFERENCE")))GcadSourceRenderCheck.RunLodQuality();
        if(Environment.GetEnvironmentVariable("GCAD_PLAYER_CHECK")=="1")GcadPlayerBuildCheck.Run();
    }
}
