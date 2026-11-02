using System;
using System.IO;
using System.Linq;
using System.Runtime.Serialization;
using System.Runtime.Serialization.Json;
using System.Security.Cryptography;
using System.Xml;
using UnityEditor;
using UnityEngine;

// Editor boundary only. Player builds receive serialized, validated Unity data.
public static class GcadRootMotionAdapter
{
    [DataContract] public sealed class Pose {
        [DataMember(IsRequired=true)] public double[] translation;
        [DataMember(IsRequired=true)] public double[] rotation;
    }
    [DataContract] public sealed class Channel {
        [DataMember(IsRequired=true)] public string path;
        [DataMember(IsRequired=true)] public string interpolation;
        [DataMember(IsRequired=true)] public double[] times_seconds;
        [DataMember(IsRequired=true)] public double[][] values;
    }
    [DataContract] public sealed class ExactTrack {
        [DataMember(IsRequired=true)] public string node_name,path,interpolation;
        [DataMember(IsRequired=true)] public double[] times_seconds;
        [DataMember(IsRequired=true)] public double[][] values;
    }
    [DataContract] public sealed class Motion {
        [DataMember(IsRequired=true)] public int schema;
        [DataMember(IsRequired=true)] public string length_unit,up_axis,handedness;
        [DataMember(IsRequired=true)] public double duration_seconds;
        [DataMember(IsRequired=true)] public bool rotation_extracted;
        [DataMember(IsRequired=true)] public bool[] translation_axes;
        [DataMember] public string interpolation,mode,composition,loop_composition;
        [DataMember] public double[] times_seconds,loop_displacement;
        [DataMember] public double[][] displacements;
        [DataMember] public Pose anchor,loop_transform;
        [DataMember] public Channel[] channels;
        [DataMember] public int max_accumulated_cycles;
    }
    [DataContract] public sealed class Receipt {
        [DataMember(IsRequired=true)] public int version;
        [DataMember(IsRequired=true)] public string format,model_sha256,source_glb_sha256,source_clip,in_place_clip,node_name;
        [DataMember(IsRequired=true)] public Motion motion;
        [DataMember] public ExactTrack[] in_place_tracks;
        public GcadRootMotionData data;
    }
    static double[] Vector(double[] value,int length) {
        if(value==null||value.Length!=length||value.Any(v=>!GcadRootMotionData.Finite(v)||Math.Abs(v)>float.MaxValue))
            throw new Exception("Invalid motion vector");return value;
    }
    static Vector3 Position(double[] value) { Vector(value,3);return new Vector3(-(float)value[0],(float)value[1],(float)value[2]); }
    static Quaternion Rotation(double[] value) {
        Vector(value,4);if(Math.Abs(value.Sum(v=>v*v)-1)>1e-5)throw new Exception("Motion quaternion is not unit length");
        return new Quaternion((float)value[0],-(float)value[1],-(float)value[2],(float)value[3]).normalized;
    }
    static bool Step(string interpolation) {
        if(interpolation!="linear"&&interpolation!="step")throw new Exception("Unsupported root-motion interpolation");
        return interpolation=="step";
    }
    static Matrix4x4 Matrix(Pose pose) {
        if(pose==null)throw new Exception("Missing rigid motion pose");
        return Matrix4x4.TRS(Position(pose.translation),Rotation(pose.rotation),Vector3.one);
    }
    static void Same(Matrix4x4 a,Matrix4x4 b) {
        for(int i=0;i<16;i++)if(!GcadRootMotionData.Finite(a[i])||Math.Abs(a[i]-b[i])>1e-5*(1+Math.Max(Math.Abs(a[i]),Math.Abs(b[i]))))
            throw new Exception("Root-motion anchor or loop disagrees with channels");
    }
    public static Receipt Read(string modelPath) {
        string path=Path.Combine(Path.GetDirectoryName(modelPath),"gcad-root-motion.json");
        if(!File.Exists(path))return null;
        byte[] bytes;
        using(var stream=File.OpenRead(path)) {
            if(stream.Length<=0||stream.Length>16*1024*1024)throw new Exception("Root-motion receipt exceeds bound");
            bytes=new byte[(int)stream.Length];int offset=0;
            while(offset<bytes.Length){int count=stream.Read(bytes,offset,bytes.Length-offset);if(count==0)throw new Exception("Receipt changed during read");offset+=count;}
            if(stream.ReadByte()!=-1)throw new Exception("Receipt changed during read");
        }
        Receipt receipt;
        var serializer=new DataContractJsonSerializer(typeof(Receipt),new DataContractJsonSerializerSettings{MaxItemsInObjectGraph=1000000});
        using(var reader=JsonReaderWriterFactory.CreateJsonReader(bytes,new XmlDictionaryReaderQuotas{MaxDepth=32,MaxArrayLength=65536,MaxStringContentLength=4096,MaxBytesPerRead=4096,MaxNameTableCharCount=65536}))
            receipt=(Receipt)serializer.ReadObject(reader);
        if(receipt==null||(receipt.version!=1&&receipt.version!=2)||receipt.format!="gcad-root-motion"||receipt.motion==null)throw new Exception("Unsupported root-motion receipt");
        foreach(var name in new[]{receipt.source_clip,receipt.in_place_clip,receipt.node_name})
            if(string.IsNullOrWhiteSpace(name)||name.Length>4096)throw new Exception("Invalid root-motion reference name");
        if(receipt.source_clip==receipt.in_place_clip)throw new Exception("Source and in-place clips must differ");
        string digest;using(var hash=SHA256.Create())using(var stream=File.OpenRead(modelPath))digest=BitConverter.ToString(hash.ComputeHash(stream)).Replace("-","").ToLowerInvariant();
        if(receipt.model_sha256!=digest)throw new Exception("Root-motion receipt does not match this FBX");
        if(receipt.source_glb_sha256==null||receipt.source_glb_sha256.Length!=64||receipt.source_glb_sha256.Any(c=>!Uri.IsHexDigit(c)))throw new Exception("Invalid source digest");
        var m=receipt.motion;
        if(m.length_unit!="metre"||m.up_axis!="Y"||m.handedness!="right"||m.translation_axes==null||m.translation_axes.Length!=3)
            throw new Exception("Unsupported root-motion coordinates");
        var data=new GcadRootMotionData{schema=m.schema,duration=m.duration_seconds,translations=new Vector3[0],translationTimes=new double[0],rotations=new Quaternion[0],rotationTimes=new double[0]};
        if(m.schema==1) {
            if(m.rotation_extracted||!m.translation_axes.Any(v=>v)||m.displacements==null||m.displacements.Length==0||m.displacements.Length>65536)
                throw new Exception("Invalid translation-only motion");
            data.translationStep=Step(m.interpolation);data.translationTimes=m.times_seconds;
            if(Vector(m.displacements[0],3).Any(v=>v!=0))throw new Exception("Initial displacement must be zero");
            foreach(var value in m.displacements){Vector(value,3);for(int k=0;k<3;k++)if(!m.translation_axes[k]&&value[k]!=0)throw new Exception("Motion outside selected axes");}
            data.translations=m.displacements.Select(Position).ToArray();
            if(!Vector(m.loop_displacement,3).SequenceEqual(m.displacements.Last()))throw new Exception("Invalid loop displacement");
        } else if(m.schema==2) {
            if(!m.rotation_extracted||m.mode!="rigid"||!m.translation_axes.All(v=>v)||m.composition!="left-multiply-in-place-root"||m.loop_composition!="endpoint-power-times-phase"||m.max_accumulated_cycles!=1000000||m.channels==null||m.channels.Length<1||m.channels.Length>2||m.anchor==null)
                throw new Exception("Unsupported rigid motion convention");
            data.anchorPosition=Position(m.anchor.translation);data.anchorRotation=Rotation(m.anchor.rotation);
            if(m.channels.Select(c=>c.path).Distinct().Count()!=m.channels.Length)throw new Exception("Duplicate motion channel");
            foreach(var channel in m.channels) {
                if(channel.values==null||channel.values.Length>65536)throw new Exception("Invalid motion channel values");
                bool step=Step(channel.interpolation);
                if(channel.path=="translation"){data.translationStep=step;data.translationTimes=channel.times_seconds;data.translations=channel.values.Select(Position).ToArray();}
                else if(channel.path=="rotation"){data.rotationStep=step;data.rotationTimes=channel.times_seconds;data.rotations=channel.values.Select(Rotation).ToArray();}
                else throw new Exception("Unsupported motion channel");
            }
            Same(data.Anchor(),Matrix(m.anchor));Same(data.Evaluate(data.duration,false),Matrix(m.loop_transform));
        } else throw new Exception("Unsupported root-motion schema");
        data.Validate();
        if(data.duration!=(double)(float)data.duration||data.translationTimes.Any(t=>t!=(double)(float)t)||data.rotationTimes.Any(t=>t!=(double)(float)t))
            throw new NotSupportedException("Root-motion times differ from the float32 clip timeline; regenerate the root-motion GLB and package");
        // Unity's FBX importer moves STEP boundaries early, even with curve
        // resampling disabled. The trajectory alone cannot repair unextracted
        // root channels in motion schema 1. Old receipts must be re-exported.
        if(receipt.version==1&&(data.translationStep||data.rotationStep))
            throw new NotSupportedException("STEP root-motion playback requires receipt v2 exact tracks; re-export this package");
        if(receipt.version==2&&(receipt.in_place_tracks==null||receipt.in_place_tracks.Length==0||receipt.in_place_tracks.Length>4096))
            throw new Exception("Exact in-place tracks required by motion receipt v2");
        receipt.data=data;return receipt;
    }
    public static GcadRootMotionPlayer Attach(GameObject instance,string modelPath,Receipt receipt) {
        if(receipt==null)return null;
        var roots=instance.GetComponentsInChildren<Transform>(true).Where(t=>t.name==receipt.node_name).ToArray();
        if(roots.Length!=1||instance.GetComponent<GcadRootMotionPlayer>())throw new Exception("Missing/ambiguous root or existing motion player");
        var clips=AssetDatabase.LoadAllAssetsAtPath(modelPath).OfType<AnimationClip>().Where(c=>!c.name.StartsWith("__preview__")).ToArray();
        var source=clips.Single(c=>c.name==receipt.source_clip);var inPlace=clips.Single(c=>c.name==receipt.in_place_clip);
        if(Math.Abs(source.length-receipt.data.duration)>1e-4||Math.Abs(inPlace.length-receipt.data.duration)>1e-4)
            throw new Exception("Root-motion duration does not match imported clips");
        var player=instance.AddComponent<GcadRootMotionPlayer>();player.motionRoot=roots[0];player.inPlaceClip=inPlace;player.motion=receipt.data;
        if(receipt.version==2) {
            player.exactTrackPlayback=true;
            int keys=0;var seen=new System.Collections.Generic.HashSet<string>();
            player.exactTracks=receipt.in_place_tracks.Select(track=> {
                if(track==null||string.IsNullOrWhiteSpace(track.node_name)||track.node_name.Length>4096||track.values==null||track.values.Length>65536)
                    throw new Exception("Invalid exact motion reference");
                var targets=instance.GetComponentsInChildren<Transform>(true).Where(t=>t.name==track.node_name).ToArray();
                if(targets.Length!=1||!seen.Add(track.node_name+"\n"+track.path))throw new Exception("Ambiguous/duplicate exact motion channel");
                keys+=track.values.Length;if(keys>65536)throw new Exception("Exact motion key limit exceeded");
                var result=new GcadMotionTrack{target=targets[0],path=track.path,step=Step(track.interpolation),times=track.times_seconds};
                result.values=track.values.Select(value=> {
                    if(track.path=="rotation"){var q=Rotation(value);return new Vector4(q.x,q.y,q.z,q.w);}
                    Vector(value,3);
                    if(track.path!="translation"&&track.path!="scale")throw new Exception("Unsupported exact motion path");
                    return new Vector4((float)value[0]*(track.path=="translation"?-1:1),(float)value[1],(float)value[2],0);
                }).ToArray();
                result.Validate(receipt.data.duration);return result;
            }).ToArray();
        }
        return player;
    }
}
