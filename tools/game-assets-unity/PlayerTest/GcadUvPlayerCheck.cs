using System;
using System.IO;
using System.Collections;
using UnityEngine;

// Test-only standalone player entry point.
public sealed class GcadUvPlayerCheck : MonoBehaviour
{
    public GameObject prefab;
    public bool allMaps;
    IEnumerator Start() {
        string output=Path.GetFullPath(Path.Combine(Application.dataPath,".."));
        var model=Instantiate(prefab);var animation=model.GetComponent<GcadUvScrollAnimation>();
        var renderer=model.GetComponentInChildren<Renderer>();var original=renderer.sharedMaterial;Material owned=null;
        string failure=null;
        try {
            if(!animation.Sample(0))throw new Exception(animation.LastError);
            owned=renderer.sharedMaterial;
            if(owned==original)throw new Exception("Player material was not cloned");
            var late=new GameObject("late deformation display");late.transform.SetParent(model.transform,false);
            var inherited=late.AddComponent<MeshRenderer>();inherited.sharedMaterial=owned;
            animation.enabled=false;
            if(renderer.sharedMaterial!=original)throw new Exception("Player disable did not restore material");
            if(inherited.sharedMaterial!=original)throw new Exception("Late renderer retained owned material");
        } catch(Exception error){failure=error.ToString();}
        yield return null;
        try {
            if(failure!=null)throw new Exception(failure);
            if(owned)throw new Exception("Deferred material destruction did not complete");
            animation.enabled=true;
            if(!animation.Sample(4)||renderer.sharedMaterial==original)throw new Exception("Player re-enable failed");
            if(animation.Sample(double.NaN)||animation.SampledSeconds!=4)throw new Exception("Player last-good state failed");
            Destroy(model);
        } catch(Exception error){failure=error.ToString();}
        yield return null;
        try {
            if(failure!=null)throw new Exception(failure);
            GcadUvRenderProbe.Run(prefab,output,allMaps);
            File.WriteAllText(Path.Combine(output,"uv-lifecycle-report.json"),"{\"passed\": true}");Application.Quit(0);
        } catch(Exception error){File.WriteAllText(Path.Combine(output,"uv-player-failure.txt"),error.ToString());Debug.LogException(error);Application.Quit(1);}
    }
}
