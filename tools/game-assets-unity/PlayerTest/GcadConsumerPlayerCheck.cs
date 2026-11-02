using System;
using System.IO;
using UnityEngine;

public sealed class GcadConsumerPlayerCheck : MonoBehaviour
{
    public GameObject instance;
    public int expectedMeshColliders;
    public bool render;
    public bool skinned;
    public Shader lodShader;
    void Start() {
        try {
            var report=GcadConsumerProbe.Run(instance,expectedMeshColliders,skinned);File.WriteAllText(Path.Combine(Application.dataPath,"../consumer-player-report.json"),JsonUtility.ToJson(report,true));
            bool passed=report.passed;
            if(render){var rendered=GcadConsumerLodProbe.Run(instance,lodShader,Path.Combine(Application.dataPath,"../ConsumerLod"));File.WriteAllText(Path.Combine(Application.dataPath,"../consumer-lod-report.json"),JsonUtility.ToJson(rendered,true));passed&=rendered.passed;}
            if(render&&skinned){var posed=GcadConsumerLodProbe.RunPosed(instance,lodShader,Path.Combine(Application.dataPath,"../ConsumerLodPosed"));File.WriteAllText(Path.Combine(Application.dataPath,"../consumer-lod-posed-report.json"),JsonUtility.ToJson(posed,true));passed&=posed.passed;}
            Application.Quit(passed?0:1);
        }
        catch(Exception error){Debug.LogException(error);Application.Quit(1);}
    }
}
