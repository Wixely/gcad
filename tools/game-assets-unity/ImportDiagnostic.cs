// Opt-in test-only changes to the copied fixture, never a production postprocessor.
using System;
using System.IO;
using UnityEditor;
using UnityEngine;

public static class GcadImportDiagnostic
{
    [Serializable] public class Settings {
        public string compression,tangents,skinWeights; public bool resampleCurves;
        public float minBoneWeight; public int maxBonesPerVertex;
        public Settings(ModelImporter importer) {
            compression=importer.animationCompression.ToString();
            tangents=importer.importTangents.ToString();resampleCurves=importer.resampleCurves;
            skinWeights=importer.skinWeights.ToString();minBoneWeight=importer.minBoneWeight;
            maxBonesPerVertex=importer.maxBonesPerVertex;
        }
    }
    [Serializable] public class Report { public string mode; public Settings before,after; }
    public static void Apply() {
        string mode=Environment.GetEnvironmentVariable("GCAD_IMPORT_DIAGNOSTIC");
        if(string.IsNullOrEmpty(mode))return;
        if(Environment.GetEnvironmentVariable("GCAD_SOURCE_RENDER_CHECK")!="1")throw new Exception("Import diagnostic requires source rendering");
        const string path="Assets/Fixture/model.fbx";
        var importer=(ModelImporter)AssetImporter.GetAtPath(path);
        var report=new Report { mode=mode,before=new Settings(importer) };
        switch(mode) {
            case "compression-off": importer.animationCompression=ModelImporterAnimationCompression.Off;break;
            case "source-tangents": importer.importTangents=ModelImporterTangents.Import;break;
            case "source-tangents-uncompressed":
                importer.importTangents=ModelImporterTangents.Import;
                importer.animationCompression=ModelImporterAnimationCompression.Off;break;
            case "source-tangents-uncompressed-zero-cutoff-request":
                // Unity may clamp this request. Report the effective setting after reimport.
                importer.importTangents=ModelImporterTangents.Import;
                importer.animationCompression=ModelImporterAnimationCompression.Off;
                importer.skinWeights=ModelImporterSkinWeights.Custom;
                importer.maxBonesPerVertex=4;
                importer.minBoneWeight=0;break;
            case "source-tangents-flip": importer.importTangents=ModelImporterTangents.Import;break;
            case "no-resampling": importer.resampleCurves=false;break;
            default: throw new Exception("Unknown fixture import diagnostic");
        }
        importer.SaveAndReimport();
        report.after=new Settings((ModelImporter)AssetImporter.GetAtPath(path));
        File.WriteAllText("import-diagnostic-report.json",JsonUtility.ToJson(report,true));
    }
}
