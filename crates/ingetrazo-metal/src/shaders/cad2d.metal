#include <metal_stdlib>
using namespace metal;
// Immutable packed12-byte source vertex, matching Rust cad2d::Vertex.
struct Vertex {float x;float y;float coverage;};
struct Push {float2 scale;float2 translation;float2 screen;float2 pad;float4 rgba;};
struct Vary {float4 position [[position]];float coverage;};
struct Framebuffer {float4 rgba [[color(0),raster_order_group(0)]];};
vertex Vary cad_vertex(uint id [[vertex_id]],const device Vertex* vertices [[buffer(0)]],constant Push& p [[buffer(1)]]) {
 float2 point=float2(vertices[id].x,vertices[id].y)*p.scale+p.translation;
 Vary out;out.position=float4(point.x/p.screen.x*2-1,1-point.y/p.screen.y*2,0,1);out.coverage=vertices[id].coverage;return out;
}
uint4 byte_mul(uint4 pixel,uint coverage){uint4 t=pixel*coverage;return (t+(t>>8)+128)>>8;}
fragment Framebuffer cad_fragment(Vary in [[stage_in]],Framebuffer previous,constant Push& p [[buffer(1)]]) {
 uint coverage=uint(floor(clamp(in.coverage,0.0f,1.0f)*255.0f+0.5f));
 uint4 source=byte_mul(uint4(floor(clamp(p.rgba,0.0f,1.0f)*255.0f+0.5f)),coverage);
 uint4 destination=uint4(floor(clamp(previous.rgba,0.0f,1.0f)*255.0f+0.5f));
 Framebuffer out;out.rgba=float4(source+byte_mul(destination,255-source.a))/255.0f;return out;
}
