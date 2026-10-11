#include <metal_stdlib>
using namespace metal;
struct Vertex { packed_float3 position; packed_float3 normal; packed_float4 color; packed_float2 uv; };
struct Out { float4 position [[position]]; float4 color; };
vertex Out world_vertex(uint id [[vertex_id]],uint instance [[instance_id]],const device Vertex* v [[buffer(0)]],const device float4x4* model [[buffer(1)]],constant float4x4& camera [[buffer(2)]]) {
 Out o; o.position=camera*model[instance]*float4(v[id].position,1.0);
 // OrbitCamera uses OpenGL clip depth; Metal raster depth is [0,w].
 o.position.z=(o.position.z+o.position.w)*0.5; o.color=float4(v[id].color); return o;
}
struct Paint {float4 front;float4 back;uint mode;float opacity;uint padding1;uint padding2;};
// Frozen source RGBA8 blend factor conversion uses nearest-even UNORM8.
// Preserve straight RGB; only the attachment blend factor is encoded here.
float source_blend_alpha(float alpha){return rint(clamp(alpha,0.,1.)*255.)/255.;}
fragment float4 world_fragment(Out in [[stage_in]],bool front [[front_facing]],constant Paint& paint [[buffer(0)]]) {
 if(paint.mode==1||paint.mode==3)return paint.front;
 if(paint.mode==4)return front?in.color:paint.back;
 if(paint.mode==2)return front?paint.front:paint.back;
 // Source u_opacity is uniform per run, not perspective-interpolated alpha.
 return float4(in.color.rgb,source_blend_alpha(paint.opacity));
}

// Source basic.frag sampled policy: bottom-up UV asset rows, linear repeat,
// trilinear mips unless sparse alpha selects source hard-cutout mode.
struct TextureVertex {packed_float3 position;packed_float2 uv;};
struct TextureOut {float4 position [[position]];float2 uv;};
vertex TextureOut world_texture_vertex(uint id [[vertex_id]],uint instance [[instance_id]],const device TextureVertex* v [[buffer(0)]],const device float4x4* model [[buffer(1)]],constant float4x4& camera [[buffer(2)]]) {
 TextureOut o;o.position=camera*model[instance]*float4(v[id].position,1.);o.position.z=(o.position.z+o.position.w)*.5;o.uv=float2(v[id].uv);return o;
}
struct TexturePaint {float opacity;float shade;uint hard_cutout;uint height;float4 mean;};
constant float source_bayer[16]={0.5/16.,8.5/16.,2.5/16.,10.5/16.,12.5/16.,4.5/16.,14.5/16.,6.5/16.,3.5/16.,11.5/16.,1.5/16.,9.5/16.,15.5/16.,7.5/16.,13.5/16.,5.5/16.};
fragment float4 world_texture_fragment(TextureOut in [[stage_in]],texture2d<float> image [[texture(0)]],sampler filter [[sampler(0)]],constant TexturePaint& paint [[buffer(0)]]) {
 if(paint.mean.w!=0.)return float4(paint.mean.rgb,source_blend_alpha(paint.opacity));
 float4 texel=image.sample(filter,in.uv);
 if(paint.opacity>.999&&texel.a<.5){if(paint.hard_cutout!=0)discard_fragment();uint x=uint(in.position.x)%4;uint y=uint(float(paint.height)-in.position.y)%4;if(texel.a<source_bayer[y*4+x])discard_fragment();return float4(texel.rgb*paint.shade,1.);}
 return float4(texel.rgb*paint.shade,source_blend_alpha(texel.a*paint.opacity));
}

// World-space ghosts deliberately omit section clipping and ordinary paint policy.
struct GhostVertex {packed_float3 position;packed_float3 color;};
vertex Out world_ghost_vertex(uint id [[vertex_id]],const device GhostVertex* v [[buffer(0)]],constant float4x4& camera [[buffer(2)]]) {
 Out o;o.position=camera*float4(v[id].position,1.);o.position.z=(o.position.z+o.position.w)*.5;o.color=float4(v[id].color,1.);return o;
}
struct GhostPaint {float4 edge;uint height;uint mode;uint padding1;uint padding2;};
fragment float4 world_ghost_fragment(Out in [[stage_in]],constant GhostPaint& paint [[buffer(0)]]) {
 float x=in.position.x,y=float(paint.height)-in.position.y;
 if(paint.mode==0){if(fmod(x,4.)>=1.&&fmod(y,4.)>=1.)discard_fragment();return float4(in.color.rgb,1.);}
 if(paint.mode==1&&fmod(x+y,6.)>=3.)discard_fragment();return float4(paint.edge.rgb,1.);
}
