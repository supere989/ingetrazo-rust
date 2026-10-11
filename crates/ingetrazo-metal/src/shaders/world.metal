#include <metal_stdlib>
using namespace metal;
struct Vertex { packed_float3 position; packed_float3 normal; packed_float4 color; packed_float2 uv; };
struct Out { float4 position [[position]]; float4 color; };
vertex Out world_vertex(uint id [[vertex_id]],const device Vertex* v [[buffer(0)]],constant float4x4& mvp [[buffer(1)]]) {
 Out o; o.position=mvp*float4(v[id].position,1.0);
 // OrbitCamera uses OpenGL clip depth; Metal raster depth is [0,w].
 o.position.z=(o.position.z+o.position.w)*0.5; o.color=float4(v[id].color); return o;
}
struct Paint {float4 front;float4 back;uint mode;uint padding0;uint padding1;uint padding2;};
fragment float4 world_fragment(Out in [[stage_in]],bool front [[front_facing]],constant Paint& paint [[buffer(0)]]) {
 if(paint.mode==1||paint.mode==3)return paint.front;
 if(paint.mode==2)return front?paint.front:paint.back;
 return in.color;
}
