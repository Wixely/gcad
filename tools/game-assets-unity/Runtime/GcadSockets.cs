using System;
using UnityEngine;

public sealed class GcadSockets : MonoBehaviour
{
    [Serializable] public sealed class Entry {public string name;public Transform node;}
    public Entry[] entries;
    public Transform Find(string name) {
        if(entries!=null)foreach(var entry in entries)if(entry.name==name)return entry.node;
        return null;
    }
}
