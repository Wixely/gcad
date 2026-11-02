Shader "gcad/PBR Opaque Mask" {
Properties {
    _GcadUvTransform ("UV scale/offset", Vector) = (1,1,0,0)
    _GcadUvEnabled ("UV transform enabled", Float) = 0
    _BaseMap ("Base colour", 2D) = "white" {}
    _RoughnessMap ("Roughness G", 2D) = "white" {}
    _MetallicMap ("Metallic B", 2D) = "white" {}
    _NormalMap ("Normal RGB", 2D) = "bump" {}
    _OcclusionMap ("Occlusion R", 2D) = "white" {}
    _EmissionMap ("Emission", 2D) = "white" {}
    _BaseFactor ("Base factor (linear)", Vector) = (1,1,1,1)
    _EmissionFactor ("Emission factor (linear)", Vector) = (0,0,0,0)
    _Roughness ("Roughness", Float) = 1
    _Metallic ("Metallic", Float) = 1
    _NormalScale ("Normal scale", Float) = 1
    _OcclusionStrength ("Occlusion strength", Float) = 1
    _AlphaMode ("Alpha mode", Float) = 0
    _Cutoff ("Alpha cutoff", Float) = 0.5
    _Cull ("Cull", Float) = 2
    _HasNormal ("Has normal", Float) = 0
    _BaseMap_Filter ("Base filters", Vector) = (9729,9729,0,0)
    _RoughnessMap_Filter ("Roughness filters", Vector) = (9729,9729,0,0)
    _MetallicMap_Filter ("Metallic filters", Vector) = (9729,9729,0,0)
    _NormalMap_Filter ("Normal filters", Vector) = (9729,9729,0,0)
    _OcclusionMap_Filter ("Occlusion filters", Vector) = (9729,9729,0,0)
    _EmissionMap_Filter ("Emission filters", Vector) = (9729,9729,0,0)
}
SubShader {
    Tags { "RenderType"="Opaque" }
    Cull [_Cull]
    CGPROGRAM
    #pragma target 3.0
    #pragma surface surf Standard fullforwardshadows addshadow
    #include "GcadPbr.cginc"
    ENDCG
}
Fallback Off
}
