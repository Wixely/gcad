// Test-only observer, active solely for the isolated fixture plus explicit marker.
using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.AssetImporters;
using UnityEngine;

public class GcadMaterialDescriptionAudit : AssetPostprocessor
{
    [Serializable] public class Property { public string name, kind, value; }
    [Serializable] public class Description { public string material; public Property[] properties; }
    [Serializable] public class Report { public string unityVersion; public Description[] materials; public string[] missing; public bool passed; }
    const string Marker="gcad-material-description-audit.enabled";
    const string Model="Assets/Fixture/model.fbx";
    bool Active { get { return assetPath==Model && File.Exists(Marker); } }
    void OnPreprocessModel()
    {
        if(Active)((ModelImporter)assetImporter).materialImportMode=ModelImporterMaterialImportMode.ImportViaMaterialDescription;
    }
    void OnPreprocessMaterialDescription(MaterialDescription description, Material material, AnimationClip[] animations)
    {
        if(!Active)return;
        var properties=new List<Property>();
        var names=new List<string>();
        description.GetFloatPropertyNames(names);
        foreach(var name in names) {
            float value;
            if(description.TryGetProperty(name,out value))properties.Add(new Property { name=name, kind="float", value=value.ToString("R",CultureInfo.InvariantCulture) });
        }
        names.Clear(); description.GetVector4PropertyNames(names);
        foreach(var name in names) {
            Vector4 value;
            if(description.TryGetProperty(name,out value))properties.Add(new Property { name=name, kind="vector", value=string.Join(",",new[]{value.x,value.y,value.z,value.w}.Select(v=>v.ToString("R",CultureInfo.InvariantCulture))) });
        }
        names.Clear(); description.GetStringPropertyNames(names);
        foreach(var name in names) {
            string value;
            if(description.TryGetProperty(name,out value))properties.Add(new Property { name=name, kind="string", value=value });
        }
        names.Clear(); description.GetTexturePropertyNames(names);
        foreach(var name in names) {
            TexturePropertyDescription value;
            if(description.TryGetProperty(name,out value))properties.Add(new Property { name=name, kind="texture", value=AssetDatabase.GetAssetPath(value.texture) });
        }
        var entry=new Description { material=description.materialName, properties=properties.OrderBy(p=>p.name).ThenBy(p=>p.kind).ToArray() };
        if(!new[]{"materials_0","materials_1","materials_2"}.Contains(entry.material))throw new Exception("Unexpected audit fixture material");
        Directory.CreateDirectory("description-audit");
        File.WriteAllText("description-audit/"+entry.material+".json",JsonUtility.ToJson(entry,true));
    }
    public static void Run()
    {
        if(!File.Exists(Marker))throw new Exception("Explicit description audit marker required");
        AssetDatabase.ImportAsset(Model,ImportAssetOptions.ForceUpdate|ImportAssetOptions.ForceSynchronousImport);
        var materials=Enumerable.Range(0,3).Select(i=>JsonUtility.FromJson<Description>(File.ReadAllText("description-audit/materials_"+i+".json"))).ToArray();
        var required=new[]{"gcad|PbrVersion","gcad|Roughness","gcad|Metallic","gcad|BaseColour","gcad|BaseAlpha","gcad|AlphaMode","gcad|AlphaCutoff","gcad|DoubleSided","gcad|NormalScale","gcad|OcclusionStrength","gcad|Emission"};
        var missing=materials.SelectMany(m=>required.Where(key=>!m.properties.Any(p=>p.name==key)).Select(key=>m.material+":"+key)).ToArray();
        var report=new Report { unityVersion=Application.unityVersion, materials=materials, missing=missing, passed=missing.Length==0 };
        File.WriteAllText("description-report.json",JsonUtility.ToJson(report,true));
        if(!report.passed)throw new Exception("Required FBX custom properties are absent from Unity material descriptions");
    }
}
