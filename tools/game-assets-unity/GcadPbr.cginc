// Original gcad Built-In pipeline adapter. Source PNG channels remain unchanged.
sampler2D _BaseMap, _RoughnessMap, _MetallicMap, _NormalMap, _OcclusionMap, _EmissionMap;
float4 _BaseMap_TexelSize, _RoughnessMap_TexelSize, _MetallicMap_TexelSize;
float4 _NormalMap_TexelSize, _OcclusionMap_TexelSize, _EmissionMap_TexelSize;
float4 _BaseMap_Filter, _RoughnessMap_Filter, _MetallicMap_Filter;
float4 _NormalMap_Filter, _OcclusionMap_Filter, _EmissionMap_Filter;
float4 _BaseFactor, _EmissionFactor;
float _Roughness, _Metallic, _NormalScale, _OcclusionStrength, _AlphaMode, _Cutoff;
float _HasNormal;
float4 _GcadUvTransform;
float _GcadUvEnabled;
struct Input { float2 uv_BaseMap; float facing : VFACE; };

#include "GcadSampling.cginc"
void surf(Input IN,inout SurfaceOutputStandard o) {
    float2 uv=IN.uv_BaseMap;
    if(_GcadUvEnabled>0.5)uv=uv*_GcadUvTransform.xy+_GcadUvTransform.zw;
    float4 base=gcadSample(_BaseMap,uv,_BaseMap_TexelSize,_BaseMap_Filter)*_BaseFactor;
    if(_AlphaMode==1)clip(base.a-_Cutoff);
    o.Albedo=base.rgb;
    o.Alpha=base.a;
    o.Metallic=gcadSample(_MetallicMap,uv,_MetallicMap_TexelSize,_MetallicMap_Filter).b*_Metallic;
    o.Smoothness=1-gcadSample(_RoughnessMap,uv,_RoughnessMap_TexelSize,_RoughnessMap_Filter).g*_Roughness;
    o.Occlusion=lerp(1,gcadSample(_OcclusionMap,uv,_OcclusionMap_TexelSize,_OcclusionMap_Filter).r,_OcclusionStrength);
    o.Emission=gcadSample(_EmissionMap,uv,_EmissionMap_TexelSize,_EmissionMap_Filter).rgb*_EmissionFactor.rgb;
    float3 normal=float3(0,0,1);
    if(_HasNormal>0) {
        normal=gcadSample(_NormalMap,uv,_NormalMap_TexelSize,_NormalMap_Filter).rgb*2-1;
        normal.xy*=_NormalScale;
        normal=normalize(normal);
    }
    o.Normal=normal*(IN.facing>=0 ? 1 : -1);
}
