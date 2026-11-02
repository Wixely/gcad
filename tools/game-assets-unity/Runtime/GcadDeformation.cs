using System;
using UnityEngine;

// Explicit CPU fallback for gcad's inverse-transpose blended deformation basis.
// Install outside an Editor folder. No source assets are modified at runtime.
[DisallowMultipleComponent]
[DefaultExecutionOrder(10000)]
public sealed class GcadDeformation : MonoBehaviour
{
    public SkinnedMeshRenderer source;
    public Mesh restMesh;
    public string LastError { get; private set; }
    public Mesh CurrentMesh { get { return posed; } }
    Mesh posed;
    GameObject display;
    MeshRenderer renderer;
    Vector3[] positions,normals,outputPositions,outputNormals;
    Vector4[] tangents,outputTangents;
    BoneWeight[] weights;
    Matrix4x4[] binds,joints;
    bool sourceWasEnabled;

    public static void Validate(Mesh mesh,int boneCount) {
        if(!mesh || !mesh.isReadable || mesh.blendShapeCount!=0)throw new InvalidOperationException("Readable mesh without morph targets required");
        if(mesh.normals.Length!=mesh.vertexCount || mesh.tangents.Length!=mesh.vertexCount || mesh.boneWeights.Length!=mesh.vertexCount)
            throw new InvalidOperationException("Complete normals, tangents and skin weights required");
        if(mesh.bindposes.Length!=boneCount)throw new InvalidOperationException("Bone palette does not match bind poses");
        using(var counts=mesh.GetBonesPerVertex())foreach(var count in counts)
            if(count==0 || count>4)throw new InvalidOperationException("One to four influences per vertex required");
        foreach(var w in mesh.boneWeights) {
            float sum=0;
            for(int slot=0;slot<4;slot++) {
                float weight=Weight(w,slot);int index=Index(w,slot);
                if(float.IsNaN(weight)||float.IsInfinity(weight)||weight<0 || (weight>0 && (index<0 || index>=boneCount)))
                    throw new InvalidOperationException("Invalid skin influence");
                sum+=weight;
            }
            if(Mathf.Abs(sum-1)>1e-4f)throw new InvalidOperationException("Skin weights must sum to one");
        }
    }
    static float Weight(BoneWeight w,int slot) { return slot==0?w.weight0:slot==1?w.weight1:slot==2?w.weight2:w.weight3; }
    static int Index(BoneWeight w,int slot) { return slot==0?w.boneIndex0:slot==1?w.boneIndex1:slot==2?w.boneIndex2:w.boneIndex3; }
    static bool Finite(Vector3 v) { return !(float.IsNaN(v.x)||float.IsNaN(v.y)||float.IsNaN(v.z)||float.IsInfinity(v.x)||float.IsInfinity(v.y)||float.IsInfinity(v.z)); }
    void Initialize() {
        Validate(restMesh,source.bones.Length);
        positions=restMesh.vertices;normals=restMesh.normals;tangents=restMesh.tangents;
        weights=restMesh.boneWeights;binds=restMesh.bindposes;joints=new Matrix4x4[binds.Length];
        outputPositions=new Vector3[positions.Length];outputNormals=new Vector3[positions.Length];outputTangents=new Vector4[positions.Length];
        posed=Instantiate(restMesh);posed.name=restMesh.name+" posed";posed.MarkDynamic();
        display=new GameObject("gcad deformed surface");display.layer=source.gameObject.layer;
        display.transform.SetParent(source.transform,false);
        display.AddComponent<MeshFilter>().sharedMesh=posed;
        renderer=display.AddComponent<MeshRenderer>();renderer.sharedMaterials=source.sharedMaterials;
        renderer.shadowCastingMode=source.shadowCastingMode;renderer.receiveShadows=source.receiveShadows;
        renderer.enabled=false;sourceWasEnabled=source.enabled;
    }
    // Can be called after explicit AnimationClip.SampleAnimation in editor tooling.
    public bool Refresh() {
        if(!source || !restMesh)return false;
        try {
            if(!posed)Initialize();
            var bones=source.bones;var toLocal=source.transform.worldToLocalMatrix;
            if(bones.Length!=joints.Length)throw new InvalidOperationException("Bone palette changed");
            for(int j=0;j<joints.Length;j++) {
                if(!bones[j])throw new InvalidOperationException("Missing deformation bone");
                joints[j]=toLocal*bones[j].localToWorldMatrix*binds[j];
            }
            for(int v=0;v<positions.Length;v++) {
                var matrix=new Matrix4x4();
                for(int slot=0;slot<4;slot++) {
                    float weight=Weight(weights[v],slot);if(weight==0)continue;
                    var joint=joints[Index(weights[v],slot)];
                    for(int k=0;k<16;k++)matrix[k]+=joint[k]*weight;
                }
                float determinant=matrix.determinant;
                if(float.IsNaN(determinant)||float.IsInfinity(determinant)||Mathf.Abs(determinant)<1e-10f)
                    throw new InvalidOperationException("Singular blended deformation basis");
                var p=matrix.MultiplyPoint3x4(positions[v]);
                var n=matrix.inverse.transpose.MultiplyVector(normals[v]).normalized;
                var rest=tangents[v];var t=matrix.MultiplyVector(new Vector3(rest.x,rest.y,rest.z));
                t=(t-n*Vector3.Dot(n,t)).normalized;
                if(!Finite(p)||!Finite(n)||!Finite(t)||n.sqrMagnitude<.5f||t.sqrMagnitude<.5f)
                    throw new InvalidOperationException("Invalid deformed vertex basis");
                outputPositions[v]=p;outputNormals[v]=n;outputTangents[v]=new Vector4(t.x,t.y,t.z,rest.w*(determinant<0?-1:1));
            }
            // Publish only after the complete pose is valid; errors retain the last valid mesh.
            posed.vertices=outputPositions;posed.normals=outputNormals;posed.tangents=outputTangents;posed.RecalculateBounds();
            renderer.enabled=sourceWasEnabled;source.enabled=false;LastError=null;return true;
        } catch(Exception error) { LastError=error.Message;return false; }
    }
    void OnEnable() { if(source && restMesh)Refresh(); }
    void LateUpdate() {
        string previous=LastError;
        if(!Refresh() && LastError!=null && LastError!=previous)Debug.LogError("gcad deformation: "+LastError,this);
    }
    void OnDisable() { if(renderer)renderer.enabled=false;if(posed && source)source.enabled=sourceWasEnabled; }
    void OnDestroy() {
        if(Application.isPlaying) { if(display)Destroy(display);if(posed)Destroy(posed); }
        else { if(display)DestroyImmediate(display);if(posed)DestroyImmediate(posed); }
    }
}
