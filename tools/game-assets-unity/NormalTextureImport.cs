// Explicit Unity import-settings adapter; no global AssetPostprocessor is installed.
using System;
using System.Collections.Generic;
using System.Linq;
using UnityEditor;
using UnityEngine;

public static class GcadNormalTextureImport
{
    public static void Configure(string modelPath)
    {
        var materials=AssetDatabase.LoadAllAssetsAtPath(modelPath).OfType<Material>().ToArray();
        if(materials.Length==0)throw new Exception("No imported model materials");
        var normals=new HashSet<string>();
        var other=new HashSet<string>();
        foreach(var material in materials)foreach(var property in material.GetTexturePropertyNames()) {
            var path=AssetDatabase.GetAssetPath(material.GetTexture(property));
            if(string.IsNullOrEmpty(path))continue;
            if(property=="_BumpMap")normals.Add(path);else other.Add(path);
        }
        // Never reinterpret a PNG that is also used as colour/numeric data.
        if(normals.Overlaps(other))throw new Exception("A PNG is shared between normal and other material slots; split the texture before configuring import");
        var importers=normals.Select(path=>AssetImporter.GetAtPath(path) as TextureImporter).ToArray();
        if(importers.Any(i=>!i))throw new Exception("Normal texture is not a standalone imported image");
        foreach(var importer in importers) {
            if(importer.textureType==TextureImporterType.NormalMap && !importer.sRGBTexture && !importer.convertToNormalmap)continue;
            importer.textureType=TextureImporterType.NormalMap;
            importer.sRGBTexture=false;
            importer.convertToNormalmap=false;
            importer.SaveAndReimport();
        }
    }
}
