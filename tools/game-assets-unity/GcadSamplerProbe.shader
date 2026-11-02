Shader "Hidden/gcad/Sampler Probe" {
Properties { _ProbeTex ("Probe",2D)="white" {} }
SubShader {
    Cull Off ZWrite Off ZTest Always
    Pass {
        CGPROGRAM
        #pragma target 3.0
        #pragma vertex vert_img
        #pragma fragment frag
        #include "UnityCG.cginc"
        #include "GcadSampling.cginc"
        sampler2D _ProbeTex;
        float4 _ProbeTex_TexelSize,_Filter,_Scale,_Offset;
        float4 frag(v2f_img input):SV_Target {
            return gcadSample(_ProbeTex,input.uv*_Scale.xy+_Offset.xy,_ProbeTex_TexelSize,_Filter);
        }
        ENDCG
    }
}
Fallback Off
}
