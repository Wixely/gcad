using System;
using System.IO;
using UnityEngine;

public sealed class GcadRootMotionPlayerCheck : MonoBehaviour
{
    public GameObject actual;
    public GcadRootMotionProbe.Oracle oracle;
    void Start() {
        try {
            var report=GcadRootMotionProbe.Run(actual,oracle);
            File.WriteAllText(Path.Combine(Application.dataPath,"../root-motion-player-report.json"),JsonUtility.ToJson(report,true));
            Application.Quit(report.passed?0:1);
        } catch(Exception error){Debug.LogException(error);Application.Quit(1);}
    }
}
