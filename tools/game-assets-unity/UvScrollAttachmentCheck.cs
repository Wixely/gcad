using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEngine;

public static class GcadUvScrollAttachmentCheck
{
    static void Require(bool value,string message){if(!value)throw new Exception(message);}
    static void Reject(Action action){bool rejected=false;try{action();}catch{rejected=true;}Require(rejected,"Invalid receipt accepted");}
    public static void Run() {
        PlayerSettings.colorSpace=ColorSpace.Linear;
        const string model="Assets/Fixture/model.fbx", receiptPath="Assets/Fixture/gcad-shaders.json";
        var original=File.ReadAllBytes(model);var receiptText=File.ReadAllText(receiptPath);
        var prefab=GcadMaterialAdapter.Create(Environment.GetEnvironmentVariable("GCAD_FBX_HELPER"),model,"Assets/GcadAdapted");
        var binding=prefab.GetComponent<GcadUvScrollAnimation>();
        Require(binding&&binding.bindings.Length==1,"Missing prefab shader binding");
        var source=binding.bindings[0].material;
        Require(binding.bindings[0].speed==new Vector2(.125f,.25f)&&binding.bindings[0].frozenSeconds==2,"Wrong fixture description");
        var a=UnityEngine.Object.Instantiate(prefab);var b=UnityEngine.Object.Instantiate(prefab);
        try {
            var first=a.GetComponent<GcadUvScrollAnimation>();var second=b.GetComponent<GcadUvScrollAnimation>();
            var ra=a.GetComponentsInChildren<Renderer>(true).First();var rb=b.GetComponentsInChildren<Renderer>(true).First();
            Require(first.Sample(0)&&second.Sample(4),"Sampling failed");
            Require(ra.sharedMaterial!=rb.sharedMaterial&&ra.sharedMaterial!=source&&rb.sharedMaterial!=source,"Instances share mutable material");
            Require(ra.sharedMaterial.GetVector("_GcadUvTransform")==new Vector4(1,1,-.25f,.5f),"Incorrect zero-time transform");
            Require(rb.sharedMaterial.GetVector("_GcadUvTransform")==new Vector4(1,1,.25f,-.5f),"Incorrect later transform");
            Require(source.GetFloat("_GcadUvEnabled")==0,"Saved material mutated");
            var late=new GameObject("late deformation display");late.transform.SetParent(a.transform,false);
            var inherited=late.AddComponent<MeshRenderer>();inherited.sharedMaterial=ra.sharedMaterial;
            Require(first.Sample(2)&&ra.sharedMaterial.GetVector("_GcadUvTransform")==new Vector4(1,1,0,0),"Frozen-time identity failed");
            Require(!first.Sample(double.NaN)&&first.SampledSeconds==2&&ra.sharedMaterial.GetVector("_GcadUvTransform")==new Vector4(1,1,0,0),"Invalid sample changed last-good state");
            first.enabled=false;Require(ra.sharedMaterial==source,"Disable did not restore source");
            Require(inherited.sharedMaterial==source,"Late renderer retained destroyed material");
            first.enabled=true;Require(first.Sample(0),"Resample failed");
            Require(ra.sharedMaterial.GetVector("_GcadUvTransform")==new Vector4(1,1,-.25f,.5f),"Seek back failed");
        } finally {UnityEngine.Object.DestroyImmediate(a);UnityEngine.Object.DestroyImmediate(b);}
        try {
            var receipt=GcadUvScrollAdapter.Read(model);
            receipt.model_sha256=new string('0',64);File.WriteAllText(receiptPath,JsonUtility.ToJson(receipt));
            Reject(()=>GcadUvScrollAdapter.Read(model));
            File.WriteAllText(receiptPath,receiptText);receipt=GcadUvScrollAdapter.Read(model);
            receipt.materials[0].description.template="unknown";File.WriteAllText(receiptPath,JsonUtility.ToJson(receipt));
            Reject(()=>GcadUvScrollAdapter.Read(model));
            File.WriteAllText(receiptPath,receiptText);receipt=GcadUvScrollAdapter.Read(model);
            receipt.materials[0].material_name="absent";
            Reject(()=>GcadUvScrollAdapter.Attach(prefab,receipt,new System.Collections.Generic.Dictionary<string,Material>{{source.name,source}}));
            File.WriteAllText(receiptPath,JsonUtility.ToJson(receipt));
            Reject(()=>GcadMaterialAdapter.Create(Environment.GetEnvironmentVariable("GCAD_FBX_HELPER"),model,"Assets/InvalidShaderOutput"));
            Require(!Directory.Exists("Assets/InvalidShaderOutput"),"Invalid binding created partial assets");
        } finally {File.WriteAllText(receiptPath,receiptText);}
        Require(original.SequenceEqual(File.ReadAllBytes(model)),"Source FBX mutated");
        File.WriteAllText("uv-attachment-report.json","{\"passed\": true, \"independent_instances\": 2, \"invalid_receipt_controls\": 3, \"last_good\": true, \"source_preserved\": true}");
    }
}
