// Headless consumer check. Copy into an isolated Unity project's Assets/Editor.
using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEngine;

public static class GcadImportCheck
{
    [Serializable] public class Map { public string property, asset; }
    [Serializable] public class Surface { public string name, shader; public Map[] maps; }
    [Serializable] public class Report
    {
        public string unityVersion;
        public int meshes, triangles, skinnedRenderers, bones, pngs, duplicateBoneReferences;
        public string[] boneNames;
        public int baseColourMaps, normalMaps;
        public string[] clips;
        public Surface[] materials;
        public string animationType;
        public GcadSourceMeshReference.Counts sourceCounts;
        public bool passed;
    }
    public static void Run()
    {
        const string path = "Assets/Fixture/model.fbx";
        AssetDatabase.ImportAsset(path, ImportAssetOptions.ForceSynchronousImport | ImportAssetOptions.ForceUpdate);
        var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(path);
        if (!prefab) throw new Exception("FBX did not import as a model");
        var assets = AssetDatabase.LoadAllAssetsAtPath(path);
        var meshes = assets.OfType<Mesh>().ToArray();
        var skins = prefab.GetComponentsInChildren<SkinnedMeshRenderer>(true);
        var materials = assets.OfType<Material>().ToArray();
        var importer = (ModelImporter)AssetImporter.GetAtPath(path);
        var report = new Report {
            unityVersion = Application.unityVersion,
            meshes = meshes.Length,
            triangles = skins.Sum(s=>s.sharedMesh.triangles.Length/3)+prefab.GetComponentsInChildren<MeshFilter>(true).Sum(f=>f.sharedMesh.triangles.Length/3),
            skinnedRenderers = skins.Length,
            bones = skins.Sum(s => s.bones.Length),
            duplicateBoneReferences = skins.Sum(s => s.bones.Length - s.bones.Distinct().Count()),
            boneNames = skins.SelectMany(s => s.bones).Select(b => b ? b.name : "missing").ToArray(),
            pngs = Directory.GetFiles("Assets/Fixture/textures", "*.png").Length,
            clips = assets.OfType<AnimationClip>().Where(c => !c.name.StartsWith("__preview__"))
                .Select(c => c.name).OrderBy(n => n).ToArray(),
            animationType = importer.animationType.ToString(),
            materials = materials.Select(m => new Surface {
                name = m.name, shader = m.shader ? m.shader.name : "missing",
                maps = m.GetTexturePropertyNames().Select(p => new Map {
                    property = p, asset = AssetDatabase.GetAssetPath(m.GetTexture(p))
                }).ToArray()
            }).ToArray()
        };
        // The reference fixture has 1,276 triangles, three materials/clips, and
        // fifteen external PNGs. Record default importer behavior before adapters.
        report.baseColourMaps = report.materials.Count(m => m.maps.Any(p =>
            p.property == "_MainTex" && p.asset.StartsWith("Assets/Fixture/textures/")));
        report.normalMaps = report.materials.Count(m => m.maps.Any(p =>
            p.property == "_BumpMap" && p.asset.StartsWith("Assets/Fixture/textures/")));
        var counts=new GcadSourceMeshReference.Counts { triangles=1276,skinnedNodes=1,bones=2,images=15,clips=new[]{"bend","stretch","travel"} };
        int materialCount=3,baseMaps=3,normalMaps=3;
        if(File.Exists("material-reference.glb")) {
            counts=GcadSourceMeshReference.Describe("material-reference.glb");
            var source=GcadMaterialCheck.ReadSource();materialCount=source.materials.Length;
            baseMaps=source.materials.Count(m=>m.pbrMetallicRoughness.baseColorTexture!=null);
            normalMaps=source.materials.Count(m=>m.normalTexture!=null);
        }
        report.sourceCounts=counts;
        report.passed = report.triangles == counts.triangles && report.materials.Length == materialCount &&
            report.clips.SequenceEqual(counts.clips) && report.pngs == counts.images && report.skinnedRenderers == counts.skinnedNodes &&
            report.bones == counts.bones && report.duplicateBoneReferences == 0 &&
            report.baseColourMaps == baseMaps && report.normalMaps == normalMaps;
        File.WriteAllText("import-report.json", JsonUtility.ToJson(report, true));
        Debug.Log("GCAD_IMPORT_REPORT_WRITTEN");
        if (!report.passed) throw new Exception("Default Unity import did not satisfy the reference fixture contract; inspect import-report.json");
    }
}
