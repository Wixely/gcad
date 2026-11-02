Shader "Hidden/gcad/Surface Probe" {
Properties {
    _GcadUvTransform ("UV scale/offset", Vector) = (1,1,0,0)
    _GcadUvEnabled ("UV transform enabled", Float) = 0
    _BaseMap ("Base",2D)="white" {}
    _RoughnessMap ("Roughness",2D)="white" {}
    _MetallicMap ("Metallic",2D)="white" {}
    _NormalMap ("Normal",2D)="bump" {}
    _OcclusionMap ("Occlusion",2D)="white" {}
    _EmissionMap ("Emission",2D)="white" {}
}
SubShader {
    Cull Off ZWrite Off ZTest Always
    Pass {
        CGPROGRAM
        #pragma target 3.0
        #pragma vertex vert_img
        #pragma fragment frag
        #include "UnityCG.cginc"
        #include "UnityPBSLighting.cginc"
        #include "GcadPbr.cginc"
        float _Output,_Facing,_ProbeFrozen;
        float4 _ProbeFrozenTransform;
        float4 frag(v2f_img input):SV_Target {
            Input surfaceInput;
            surfaceInput.uv_BaseMap=input.uv;surfaceInput.facing=_Facing;
            if(_ProbeFrozen>0.5)surfaceInput.uv_BaseMap=input.uv*_ProbeFrozenTransform.xy+_ProbeFrozenTransform.zw;
            SurfaceOutputStandard surface=(SurfaceOutputStandard)0;
            surf(surfaceInput,surface);
            if(_Output==0)return float4(surface.Albedo,surface.Alpha);
            if(_Output==1)return float4(surface.Metallic,surface.Smoothness,surface.Occlusion,1);
            if(_Output==2)return float4(surface.Emission,surface.Alpha);
            return float4(surface.Normal,1);
        }
        ENDCG
    }
}
Fallback Off
}
