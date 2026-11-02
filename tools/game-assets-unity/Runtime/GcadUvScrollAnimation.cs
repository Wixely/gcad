using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;

// Attach to an adapted prefab. Sample explicitly with the animation clock.
// Each instance owns its materials; the saved prefab/material assets remain static.
[DisallowMultipleComponent]
[ExecuteAlways]
public sealed class GcadUvScrollAnimation : MonoBehaviour
{
    [Serializable] public class Binding {public Material material;public Vector2 speed;public double frozenSeconds;}
    public Binding[] bindings;
    public string LastError {get;private set;}
    public double SampledSeconds {get;private set;}
    readonly Dictionary<Material,Material> owned=new Dictionary<Material,Material>();
    readonly List<Renderer> touched=new List<Renderer>();
    public bool Sample(double seconds) {
        try {
            if(!isActiveAndEnabled)throw new InvalidOperationException("Enable the UV scroll component before sampling");
            if(bindings==null||bindings.Length==0)throw new ArgumentException("UV scroll bindings missing");
            if(bindings.Any(b=>b==null||!b.material||!b.material.HasProperty("_GcadUvTransform")||!b.material.HasProperty("_GcadUvEnabled")) ||
                bindings.Select(b=>b.material).Distinct().Count()!=bindings.Length)throw new ArgumentException("Unique gcad material bindings required");
            var transforms=bindings.Select(b=>GcadUvScroll.Frozen(b.speed,b.frozenSeconds,seconds)).ToArray();
            for(int i=0;i<bindings.Length;i++) {
                var source=bindings[i].material;Material instance;
                if(!owned.TryGetValue(source,out instance)) {
                    instance=new Material(source){name=source.name+" gcad UV instance"};owned.Add(source,instance);
                }
                GcadUvScroll.Apply(instance,transforms[i]);
            }
            // Also discovers a deformation display renderer created after the first sample.
            var instances=new HashSet<Material>(owned.Values);
            foreach(var renderer in GetComponentsInChildren<Renderer>(true)) {
                var slots=renderer.sharedMaterials;bool changed=false,usesOwned=false;
                for(int i=0;i<slots.Length;i++) {
                    Material replacement;
                    if(slots[i]&&owned.TryGetValue(slots[i],out replacement)){slots[i]=replacement;changed=true;}
                    if(slots[i]&&instances.Contains(slots[i]))usesOwned=true;
                }
                if(changed)renderer.sharedMaterials=slots;
                if(usesOwned&&!touched.Contains(renderer))touched.Add(renderer);
            }
            SampledSeconds=seconds;LastError=null;return true;
        } catch(Exception error){LastError=error.Message;return false;}
    }
    void Release() {
        var reverse=owned.ToDictionary(p=>p.Value,p=>p.Key);
        // A display may copy an owned material and be disabled before another sample.
        foreach(var renderer in touched.Concat(GetComponentsInChildren<Renderer>(true)).Distinct())if(renderer) {
            var slots=renderer.sharedMaterials;bool changed=false;
            for(int i=0;i<slots.Length;i++){Material original;if(slots[i]&&reverse.TryGetValue(slots[i],out original)){slots[i]=original;changed=true;}}
            if(changed)renderer.sharedMaterials=slots;
        }
        foreach(var material in owned.Values)if(material){if(Application.isPlaying)Destroy(material);else DestroyImmediate(material);}
        touched.Clear();owned.Clear();
    }
    void OnDisable(){Release();}
    void OnDestroy(){Release();}
}
