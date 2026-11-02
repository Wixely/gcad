// Explicit Built-In pipeline adapter. Original FBX/PNGs and their .meta files are untouched.
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;
using UnityEditor;
using UnityEngine;
using UnityEngine.Rendering;

public static class GcadMaterialAdapter
{
    static readonly Dictionary<string,string> Slots=new Dictionary<string,string> {
        {"BaseColour","_BaseMap"},{"Roughness","_RoughnessMap"},{"Metallic","_MetallicMap"},
        {"Normal","_NormalMap"},{"Occlusion","_OcclusionMap"},{"Emission","_EmissionMap"}
    };
    static TextureWrapMode Wrap(int value) {
        if(value==10497)return TextureWrapMode.Repeat;
        if(value==33071)return TextureWrapMode.Clamp;
        if(value==33648)return TextureWrapMode.Mirror;
        throw new Exception("Unsupported texture wrap");
    }
    static bool Finite(float x) { return !float.IsNaN(x) && !float.IsInfinity(x); }
    static Vector4 Colour(float[] values,float alpha) {
        if(values==null || values.Length!=3 || values.Any(v=>!Finite(v)||v<0))throw new Exception("Invalid linear colour factor");
        return new Vector4(values[0],values[1],values[2],alpha);
    }
    public static GameObject Create(string helper,string modelPath,string output,bool preserveDeformationBasis=false) {
        if(GraphicsSettings.currentRenderPipeline!=null)throw new Exception("gcad adapter currently targets Unity's Built-In render pipeline");
        if(QualitySettings.activeColorSpace!=ColorSpace.Linear)throw new Exception("gcad PBR adapter requires Linear project colour space");
        if(!modelPath.StartsWith("Assets/",StringComparison.Ordinal) || modelPath.Split('/').Any(p=>p==".." || p==".") || !modelPath.EndsWith(".fbx",StringComparison.OrdinalIgnoreCase))
            throw new Exception("Select an FBX under Assets");
        if(!Regex.IsMatch(output,@"^Assets/[A-Za-z0-9_/-]+$") || output.Contains("//") || output.EndsWith("/") || Directory.Exists(output) || File.Exists(output) || File.Exists(output+".meta"))
            throw new Exception("Select a new Assets output directory");
        var contract=GcadFbxMaterialContract.Read(helper,modelPath);
        var shaderReceipt=GcadUvScrollAdapter.Read(modelPath);
        var motionReceipt=GcadRootMotionAdapter.Read(modelPath);
        var consumerReceipt=GcadConsumerAdapter.Read(modelPath);
        if(consumerReceipt!=null&&!((ModelImporter)AssetImporter.GetAtPath(modelPath)).preserveHierarchy)
            throw new Exception("Consumer profiles require Preserve Hierarchy enabled on the FBX importer");
        var skinContract=preserveDeformationBasis?GcadFbxSkinWeights.Read(helper,modelPath):null;
        var source=AssetDatabase.LoadAssetAtPath<GameObject>(modelPath);
        if(!source)throw new Exception("FBX must already be imported");
        var names=new HashSet<string>();
        var package=Path.GetDirectoryName(Path.GetFullPath(modelPath));
        var opaque=Shader.Find("gcad/PBR Opaque Mask");
        var blend=Shader.Find("gcad/PBR Blend");
        if(!opaque || !blend || ShaderUtil.ShaderHasError(opaque) || ShaderUtil.ShaderHasError(blend))
            throw new Exception("Install the gcad PBR shaders without compilation errors first");
        // Validate every contract/source path before creating output assets.
        foreach(var surface in contract.materials) {
            if(string.IsNullOrEmpty(surface.name) || !names.Add(surface.name))throw new Exception("Material names must be unique");
            Colour(surface.BaseColour,surface.BaseAlpha);Colour(surface.Emission,1);
            if(new[]{surface.Roughness,surface.Metallic,surface.OcclusionStrength,surface.AlphaCutoff,surface.BaseAlpha}.Any(v=>!Finite(v)||v<0||v>1) || !Finite(surface.NormalScale)||surface.NormalScale<0)
                throw new Exception("Invalid material factors");
            if(!new[]{"OPAQUE","MASK","BLEND"}.Contains(surface.AlphaMode))throw new Exception("Invalid alpha mode");
            var slots=new HashSet<string>();
            foreach(var map in surface.maps) {
                if(!Slots.ContainsKey(map.slot) || !slots.Add(map.slot) || map.UvSet!=0)throw new Exception("Unsupported/duplicate slot or UV set");
                if(!Regex.IsMatch(map.path,@"^textures/image_(0|[1-9][0-9]{0,3})\.png$") || !File.Exists(Path.Combine(package,map.path)))
                    throw new Exception("Missing or noncanonical package texture");
                Wrap(map.WrapU);Wrap(map.WrapV);
                if(!new[]{-1,9728,9729}.Contains(map.MagFilter) || !new[]{-1,9728,9729,9984,9985,9986,9987}.Contains(map.MinFilter))throw new Exception("Unsupported sampler filter");
                if((map.slot=="Roughness" && map.channel!=1) || (map.slot=="Metallic" && map.channel!=2) || (map.slot=="Occlusion" && map.channel!=0))throw new Exception("Unsupported source channel layout");
            }
        }
        foreach(var renderer in source.GetComponentsInChildren<Renderer>(true))
            if(renderer.sharedMaterials.Any(m=>!m || !names.Contains(m.name)))throw new Exception("FBX material assignments do not match contract");
        if(shaderReceipt!=null) {
            var usedNames=new HashSet<string>(source.GetComponentsInChildren<Renderer>(true).SelectMany(r=>r.sharedMaterials).Select(m=>m.name));
            if(shaderReceipt.materials.Any(entry=>!names.Contains(entry.material_name)||!usedNames.Contains(entry.material_name)))
                throw new Exception("Shader receipt material is absent from imported FBX");
        }
        if(preserveDeformationBasis) {
            var importer=(ModelImporter)AssetImporter.GetAtPath(modelPath);
            if(importer.importNormals!=ModelImporterNormals.Import || importer.importTangents!=ModelImporterTangents.Import)
                throw new Exception("Basis-preserving adaptation requires normals and tangents imported from the v12 FBX; configure these import options explicitly first");
        }
        // Resolve/validate exact animation targets and values before publishing
        // any material assets. A malformed motion receipt must leave no output.
        if(motionReceipt!=null) {
            var probe=UnityEngine.Object.Instantiate(source);
            try {GcadRootMotionAdapter.Attach(probe,modelPath,motionReceipt);}
            finally {UnityEngine.Object.DestroyImmediate(probe);}
        }
        GcadConsumerAdapter.Validate(source,consumerReceipt);
        Directory.CreateDirectory(output);
        try {
        AssetDatabase.Refresh(ImportAssetOptions.ForceSynchronousImport);
        var replacements=new Dictionary<string,Material>();
        for(int i=0;i<contract.materials.Length;i++) {
            var surface=contract.materials[i];
            var material=new Material(surface.AlphaMode=="BLEND" ? blend : opaque) { name=surface.name };
            material.SetVector("_BaseFactor",Colour(surface.BaseColour,surface.BaseAlpha));
            material.SetVector("_EmissionFactor",Colour(surface.Emission,1));
            material.SetFloat("_Metallic",surface.Metallic);material.SetFloat("_Roughness",surface.Roughness);
            material.SetFloat("_NormalScale",surface.NormalScale);material.SetFloat("_OcclusionStrength",surface.OcclusionStrength);
            material.SetFloat("_AlphaMode",surface.AlphaMode=="MASK" ? 1 : surface.AlphaMode=="BLEND" ? 2 : 0);
            material.SetFloat("_Cutoff",surface.AlphaCutoff);material.SetFloat("_Cull",surface.DoubleSided ? 0 : 2);
            if(surface.AlphaMode=="MASK") { material.renderQueue=2450;material.SetOverrideTag("RenderType","TransparentCutout"); }
            foreach(var map in surface.maps) {
                var target=output+"/material_"+i+"_"+map.slot+".png";
                File.Copy(Path.Combine(package,map.path),target,false);
                AssetDatabase.ImportAsset(target,ImportAssetOptions.ForceSynchronousImport);
                var importer=(TextureImporter)AssetImporter.GetAtPath(target);
                importer.textureType=TextureImporterType.Default; // Preserve all three normal components.
                importer.sRGBTexture=map.slot=="BaseColour" || map.slot=="Emission";
                importer.textureCompression=TextureImporterCompression.Uncompressed;
                importer.npotScale=TextureImporterNPOTScale.None;importer.maxTextureSize=8192;
                importer.alphaSource=TextureImporterAlphaSource.FromInput;importer.alphaIsTransparency=false;
                importer.mipmapEnabled=map.MinFilter>=9984;importer.streamingMipmaps=false;
                importer.filterMode=FilterMode.Point; // Shader implements distinct min/mag filters.
                importer.wrapModeU=Wrap(map.WrapU);importer.wrapModeV=Wrap(map.WrapV);
                importer.SaveAndReimport();
                var texture=AssetDatabase.LoadAssetAtPath<Texture2D>(target);
                var property=Slots[map.slot];material.SetTexture(property,texture);
                material.SetVector(property+"_Filter",new Vector4(map.MagFilter<0 ? 9729 : map.MagFilter,map.MinFilter<0 ? 9729 : map.MinFilter,texture.mipmapCount-1,0));
                if(map.slot=="Normal")material.SetFloat("_HasNormal",1);
            }
            AssetDatabase.CreateAsset(material,output+"/material_"+i+".mat");
            replacements.Add(surface.name,material);
        }
        var instance=(GameObject)PrefabUtility.InstantiatePrefab(source);
        try {
            foreach(var renderer in instance.GetComponentsInChildren<Renderer>(true))
                renderer.sharedMaterials=renderer.sharedMaterials.Select(m=>replacements[m.name]).ToArray();
            if(preserveDeformationBasis) {
                int meshIndex=0;
                foreach(var skin in instance.GetComponentsInChildren<SkinnedMeshRenderer>(true)) {
                    var originalMesh=skin.sharedMesh;
                    if(originalMesh.blendShapeCount!=0)throw new Exception("Deformation adapter does not support morph targets");
                    using(var counts=originalMesh.GetBonesPerVertex())foreach(var count in counts)
                        if(count==0 || count>4)throw new Exception("Deformation adapter requires one to four influences");
                    // Editor access can read importer meshes whose player Read/Write flag is off.
                    // Construct a readable asset without changing that original import flag.
                    var owned=new Mesh { name=originalMesh.name,indexFormat=originalMesh.indexFormat,
                        vertices=originalMesh.vertices,normals=originalMesh.normals,tangents=originalMesh.tangents,
                        colors=originalMesh.colors,boneWeights=GcadFbxSkinWeights.Recover(skinContract,skin,((ModelImporter)AssetImporter.GetAtPath(modelPath)).minBoneWeight),bindposes=originalMesh.bindposes,
                        subMeshCount=originalMesh.subMeshCount };
                    for(int channel=0;channel<8;channel++) {
                        var uv=new List<Vector4>();originalMesh.GetUVs(channel,uv);if(uv.Count>0)owned.SetUVs(channel,uv);
                    }
                    for(int submesh=0;submesh<originalMesh.subMeshCount;submesh++)
                        owned.SetIndices(originalMesh.GetIndices(submesh),originalMesh.GetTopology(submesh),submesh);
                    owned.RecalculateBounds();GcadDeformation.Validate(owned,skin.bones.Length);
                    AssetDatabase.CreateAsset(owned,output+"/skin_"+(meshIndex++)+".asset");
                    var deformation=skin.gameObject.AddComponent<GcadDeformation>();
                    deformation.source=skin;deformation.restMesh=owned;
                }
                foreach(var animator in instance.GetComponentsInChildren<Animator>(true))animator.cullingMode=AnimatorCullingMode.AlwaysAnimate;
            }
            GcadUvScrollAdapter.Attach(instance,shaderReceipt,replacements);
            GcadRootMotionAdapter.Attach(instance,modelPath,motionReceipt);
            GcadConsumerAdapter.Apply(instance,consumerReceipt,output);
            var result=PrefabUtility.SaveAsPrefabAsset(instance,output+"/model.prefab");
            if(!result)throw new Exception("Failed to save adapted prefab");
            AssetDatabase.SaveAssets();return result;
        } finally { UnityEngine.Object.DestroyImmediate(instance); }
        } catch(Exception failure) {
            // The destination was absent before this call. Never clean source
            // assets or an existing destination; report cleanup failures too.
            try {
                AssetDatabase.DeleteAsset(output);
                if(Directory.Exists(output)||File.Exists(output)||File.Exists(output+".meta"))
                    throw new IOException("Incomplete adapter output cleanup: "+output);
            } catch(Exception cleanup) {throw new AggregateException("Adaptation failed and output cleanup failed",failure,cleanup);}
            throw;
        }
    }
}
