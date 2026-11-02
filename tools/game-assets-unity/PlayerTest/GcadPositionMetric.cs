// Exact bidirectional nearest-point metric shared by editor and player probes.
// This is validation tooling, not part of the runtime deformation adapter.
using System;
using System.Collections.Generic;
using UnityEngine;

public static class GcadPositionMetric
{
    struct Node { public Vector3 point; public int axis,left,right; }
    sealed class AxisComparer : IComparer<Vector3> {
        readonly int axis;
        public AxisComparer(int axis) { this.axis=axis; }
        public int Compare(Vector3 a,Vector3 b) { return a[axis].CompareTo(b[axis]); }
    }
    sealed class Tree {
        static readonly IComparer<Vector3>[] Comparers={new AxisComparer(0),new AxisComparer(1),new AxisComparer(2)};
        readonly Node[] nodes;
        int used;
        public Tree(Vector3[] input) {
            if(input==null || input.Length==0)throw new ArgumentException("Nonempty position sets required");
            foreach(var p in input)for(int axis=0;axis<3;axis++)
                if(float.IsNaN(p[axis])||float.IsInfinity(p[axis]))throw new ArgumentException("Finite positions required");
            nodes=new Node[input.Length];Build((Vector3[])input.Clone(),0,input.Length);
        }
        int Build(Vector3[] points,int start,int count) {
            if(count==0)return -1;
            var low=points[start];var high=low;
            for(int i=start+1;i<start+count;i++) { low=Vector3.Min(low,points[i]);high=Vector3.Max(high,points[i]); }
            int axis=0;
            for(int a=1;a<3;a++)if((double)high[a]-low[a]>(double)high[axis]-low[axis])axis=a;
            int node=used++;
            if(high.x==low.x && high.y==low.y && high.z==low.z) { nodes[node]=new Node { point=points[start],axis=axis,left=-1,right=-1 };return node; }
            Array.Sort(points,start,count,Comparers[axis]);int middle=start+count/2;
            var point=points[middle];
            int left=Build(points,start,middle-start),right=Build(points,middle+1,start+count-middle-1);
            nodes[node]=new Node { point=point,axis=axis,left=left,right=right };return node;
        }
        public double NearestSquared(Vector3 point) {
            double best=double.PositiveInfinity;Search(0,point,ref best);return best;
        }
        void Search(int index,Vector3 point,ref double best) {
            if(index<0)return;
            var node=nodes[index];
            double x=(double)point.x-node.point.x,y=(double)point.y-node.point.y,z=(double)point.z-node.point.z;
            best=Math.Min(best,x*x+y*y+z*z);if(best==0)return;
            double delta=(double)point[node.axis]-node.point[node.axis];
            int near=delta<0?node.left:node.right,far=delta<0?node.right:node.left;
            Search(near,point,ref best);
            if(delta*delta<best)Search(far,point,ref best);
        }
    }
    public static float Distance(Vector3[] a,Vector3[] b) {
        var first=new Tree(a);var second=new Tree(b);double maximum=0;
        foreach(var p in a)maximum=Math.Max(maximum,second.NearestSquared(p));
        foreach(var p in b)maximum=Math.Max(maximum,first.NearestSquared(p));
        float result=(float)Math.Sqrt(maximum);
        if(float.IsInfinity(result))throw new ArgumentException("Position distance exceeds float range");
        return result;
    }
}
