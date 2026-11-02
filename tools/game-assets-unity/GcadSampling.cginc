float4 gcadLevel(sampler2D map,float2 uv,float2 dimensions,float level,bool linearFilter) {
    if(!linearFilter)return tex2Dlod(map,float4(uv,0,level));
    float2 size=max(floor(dimensions/exp2(level)),1);
    float2 at=uv*size-0.5, base=floor(at), blend=frac(at);
    float4 a=tex2Dlod(map,float4((base+float2(0.5,0.5))/size,0,level));
    float4 b=tex2Dlod(map,float4((base+float2(1.5,0.5))/size,0,level));
    float4 c=tex2Dlod(map,float4((base+float2(0.5,1.5))/size,0,level));
    float4 d=tex2Dlod(map,float4((base+float2(1.5,1.5))/size,0,level));
    return lerp(lerp(a,b,blend.x),lerp(c,d,blend.x),blend.y);
}
float4 gcadSample(sampler2D map,float2 uv,float4 texelSize,float4 filter) {
    float2 dx=ddx(uv)*texelSize.zw,dy=ddy(uv)*texelSize.zw;
    float lod=0.5*log2(max(max(dot(dx,dx),dot(dy,dy)),1e-8));
    if(lod<=0)return gcadLevel(map,uv,texelSize.zw,0,filter.x==9729);
    bool linearFilter=filter.y==9729 || filter.y==9985 || filter.y==9987;
    if(filter.y<9984)return gcadLevel(map,uv,texelSize.zw,0,linearFilter);
    lod=clamp(lod,0,filter.z);
    if(filter.y==9984 || filter.y==9985)return gcadLevel(map,uv,texelSize.zw,floor(lod+0.5),linearFilter);
    float low=floor(lod),high=min(low+1,filter.z);
    return lerp(gcadLevel(map,uv,texelSize.zw,low,linearFilter),gcadLevel(map,uv,texelSize.zw,high,linearFilter),frac(lod));
}
