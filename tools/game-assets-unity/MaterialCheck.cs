// Read-only, source-derived material audit for the isolated reference fixture.
// Passing these settings checks would still not prove shader/image equivalence.
using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text;
using UnityEditor;
using UnityEngine;

public static class GcadMaterialCheck
{
    [Serializable] public class TextureSlot { public int index; public float scale = 1, strength = 1; }
    [Serializable] public class Pbr {
        public float[] baseColorFactor;
        public float metallicFactor = 1, roughnessFactor = 1;
        public TextureSlot baseColorTexture, metallicRoughnessTexture;
    }
    [Serializable] public class SourceMaterial {
        public string name, alphaMode;
        public float alphaCutoff = 0.5f;
        public bool doubleSided;
        public float[] emissiveFactor;
        public Pbr pbrMetallicRoughness;
        public TextureSlot normalTexture, occlusionTexture, emissiveTexture;
    }
    [Serializable] public class SourceTexture { public int source; }
    [Serializable] public class Source { public SourceMaterial[] materials; public SourceTexture[] textures; }
    [Serializable] public class Check { public string material, property, expected, actual; public bool matched; }
    [Serializable] public class Report {
        public string unityVersion;
        public string scope = "Source GLB versus ordinary FBX import settings; no shader or texture packing equivalence claim";
        public Check[] checks;
        public int mismatches;
        public bool passed;
    }

    public static Source ReadSource()
    {
        using(var reader = new BinaryReader(File.OpenRead("material-reference.glb"))) {
            if(reader.BaseStream.Length > 64 * 1024 * 1024 || reader.BaseStream.Length < 20 ||
                reader.ReadUInt32() != 0x46546c67 || reader.ReadUInt32() != 2 ||
                reader.ReadUInt32() != reader.BaseStream.Length)
                throw new Exception("Invalid/bounded GLB reference required");
            var count = reader.ReadUInt32();
            if(reader.ReadUInt32() != 0x4e4f534a || count > reader.BaseStream.Length - 20)
                throw new Exception("GLB first chunk must be bounded JSON");
            var result = JsonUtility.FromJson<Source>(Encoding.UTF8.GetString(reader.ReadBytes((int)count)));
            if(result.materials == null || result.materials.Length != 3 || result.textures == null)
                throw new Exception("Expected the three-material source fixture");
            return result;
        }
    }

    public static void Run()
    {
        GcadImportCheck.Run();
        var source = ReadSource();
        var materials = AssetDatabase.LoadAllAssetsAtPath("Assets/Fixture/model.fbx").OfType<Material>().ToArray();
        var checks = new List<Check>();
        for(int i=0;i<source.materials.Length;i++) {
            var expected = source.materials[i];
            var name = string.IsNullOrEmpty(expected.name) ? "materials_" + i : expected.name;
            var material = materials.SingleOrDefault(m => m.name == name);
            if(!material) throw new Exception("Missing/ambiguous imported material " + name);
            Action<string,string,string,bool> check = (p,e,a,ok) => checks.Add(new Check {
                material=name, property=p, expected=e, actual=a, matched=ok
            });
            Action<string,float> scalar = (p,e) => {
                var found=material.HasProperty(p);
                var a=found ? material.GetFloat(p) : float.NaN;
                check(p,e.ToString("R",CultureInfo.InvariantCulture),found ? a.ToString("R",CultureInfo.InvariantCulture) : "absent", found && Math.Abs(a-e)<1e-5f);
            };
            Action<string,float[]> colour = (p,e) => {
                if(e==null) throw new Exception("Fixture requires explicit colour factors");
                var a=material.GetColor(p);
                var values=new[]{a.r,a.g,a.b,a.a}.Take(e.Length).ToArray();
                check(p,string.Join(",",e.Select(v=>v.ToString("R",CultureInfo.InvariantCulture))),
                    string.Join(",",values.Select(v=>v.ToString("R",CultureInfo.InvariantCulture))),
                    values.Zip(e,(x,y)=>Math.Abs(x-y)<1e-5f).All(v=>v));
            };
            Action<string,TextureSlot> map = (p,slot) => {
                if(slot==null)return;
                if(slot.index<0 || slot.index>=source.textures.Length)throw new Exception("Invalid source texture index");
                var e="Assets/Fixture/textures/image_"+source.textures[slot.index].source+".png";
                var a=AssetDatabase.GetAssetPath(material.GetTexture(p));
                check(p,e,a,e==a);
            };
            check("shader","Standard",material.shader.name,material.shader.name=="Standard");
            colour("_Color",expected.pbrMetallicRoughness.baseColorFactor);
            scalar("_Metallic",expected.pbrMetallicRoughness.metallicFactor);
            // Only a scalar roughness can be compared directly with scalar smoothness.
            // Textured roughness requires pixel repacking and separate validation.
            if(expected.pbrMetallicRoughness.metallicRoughnessTexture==null)
                scalar("_Glossiness",1-expected.pbrMetallicRoughness.roughnessFactor);
            scalar("_Mode",expected.alphaMode=="MASK" ? 1 : expected.alphaMode=="BLEND" ? 3 : 0);
            if(expected.alphaMode=="MASK") {
                scalar("_Cutoff",expected.alphaCutoff);
                check("alpha-test keyword","enabled",material.IsKeywordEnabled("_ALPHATEST_ON").ToString(),material.IsKeywordEnabled("_ALPHATEST_ON"));
            }
            if(expected.doubleSided) {
                // Double-sided GI is not a raster culling switch.
                check("double-sided raster","culling off","Standard shader back-face culling",false);
            }
            if(expected.normalTexture!=null)scalar("_BumpScale",expected.normalTexture.scale);
            if(expected.occlusionTexture!=null)scalar("_OcclusionStrength",expected.occlusionTexture.strength);
            colour("_EmissionColor",expected.emissiveFactor);
            map("_MainTex",expected.pbrMetallicRoughness.baseColorTexture);
            map("_BumpMap",expected.normalTexture);
            map("_OcclusionMap",expected.occlusionTexture);
            map("_EmissionMap",expected.emissiveTexture);
            if(expected.pbrMetallicRoughness.metallicRoughnessTexture!=null)
                check("metallic/roughness texture adaptation","validated metallic R / smoothness A texture",
                    AssetDatabase.GetAssetPath(material.GetTexture("_MetallicGlossMap")),false);
        }
        var report=new Report { unityVersion=Application.unityVersion, checks=checks.ToArray(),
            mismatches=checks.Count(c=>!c.matched), passed=checks.All(c=>c.matched) };
        File.WriteAllText("material-report.json",JsonUtility.ToJson(report,true));
        if(!report.passed)throw new Exception("Source material contract did not survive ordinary FBX import; inspect material-report.json");
    }
}
