#include <metal_stdlib>
using namespace metal;
struct PackedVertex {packed_float2 xy;packed_float2 uv;uchar4 rgba;};
struct Vary {float4 position [[position]];float2 uv;float4 color;};
float3 decode(float3 x) {return select(x/12.92,pow((x+0.055)/1.055,float3(2.4)),x>0.04045);}
float3 encode(float3 x) {return select(12.92*x,1.055*pow(max(x,float3(0)),float3(1./2.4))-0.055,x>0.0031308);}
vertex Vary egui_vertex(uint id [[vertex_id]],constant PackedVertex* v [[buffer(0)]],constant float2& logical_size [[buffer(1)]]) {
    float4 c=float4(v[id].rgba)/255.;
    // Color32 stores gamma(linear straight RGB * alpha). Recover encoded
    // premultiplication before interpolation; quantized Color32 loss remains.
    if(c.a>0 && c.a<1)c.rgb=encode(decode(c.rgb)/c.a)*c.a;
    return {float4(2.*float2(v[id].xy)/logical_size-float2(1),0,1)*float4(1,-1,1,1),float2(v[id].uv),c};
}
fragment float4 egui_fragment(Vary in [[stage_in]],texture2d<float> image [[texture(0)]],sampler filter [[sampler(0)]]) {
    return in.color*image.sample(filter,in.uv);
}
