using System;
using System.IO;
using System.Linq;
using UnityEngine;

// Decode actual images; PNG byte equality is a separate, stricter diagnostic.
public static class GcadEditorPlayerParityCheck
{
    [Serializable] public class View {public int pose,view,changedPixels,visiblePixels;public double meanRgbError;public bool exact,passed;}
    [Serializable] public class Report {public bool passed,allExact;public View[] views;}
    static Color32[] Load(string path) {
        var texture=new Texture2D(2,2,TextureFormat.RGBA32,false);
        try {if(!texture.LoadImage(File.ReadAllBytes(path))||texture.width!=512||texture.height!=512)throw new Exception("512-square rendered image required");return texture.GetPixels32();}
        finally {UnityEngine.Object.DestroyImmediate(texture);}
    }
    public static void Run() {
        var report=new Report();
        File.WriteAllText("source-editor-player-report.json",JsonUtility.ToJson(report,true));
        var editor=JsonUtility.FromJson<GcadSourceRenderCheck.AnimationReport>(File.ReadAllText("source-animation-report.json"));
        var player=JsonUtility.FromJson<GcadPlayerRenderCheck.Report>(File.ReadAllText("Player/player-render-report.json"));
        if(!editor.passed||!player.passed||editor.poses.Length!=21||player.views.Length!=84)throw new Exception("Successful complete editor/player reference checks required");
        var results=new System.Collections.Generic.List<View>();
        for(int pose=0;pose<21;pose++)for(int view=0;view<4;view++) {
            var a=Load("source-renders/pose-"+pose+"/imported-"+view+".png");
            var b=Load("Player/player-renders/"+pose+"-imported-"+view+".png");
            int changed=0;long sum=0;bool exact=true;
            for(int i=0;i<a.Length;i++) {
                int delta=Math.Abs(a[i].r-b[i].r)+Math.Abs(a[i].g-b[i].g)+Math.Abs(a[i].b-b[i].b);
                sum+=delta;if(delta>6)changed++;if(delta!=0)exact=false;
            }
            int visible=editor.poses[pose].views[view].visiblePixels;
            double mean=sum/(a.Length*3.0*255);
            // Identical gate to the source-reference comparisons, with no pixel exclusions.
            results.Add(new View{pose=pose,view=view,changedPixels=changed,visiblePixels=visible,meanRgbError=mean,exact=exact,
                passed=visible>1000&&changed<=visible*.01&&mean<=.002});
        }
        report.views=results.ToArray();report.passed=results.All(v=>v.passed);report.allExact=results.All(v=>v.exact);
        File.WriteAllText("source-editor-player-report.json",JsonUtility.ToJson(report,true));
        if(!report.passed)throw new Exception("Editor/player pixel parity failed");
    }
}
