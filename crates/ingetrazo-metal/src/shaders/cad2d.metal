#include <metal_stdlib>
using namespace metal;
// Immutable packed12-byte source vertex, matching Rust cad2d::Vertex.
struct Vertex {float x;float y;float coverage;};
struct Push {float2 scale;float2 translation;float2 screen;float2 pad;float4 rgba;uint4 glyph;};
struct Vary {float4 position [[position]];float coverage;};
struct Framebuffer {float4 rgba [[color(0),raster_order_group(0)]];};
vertex Vary cad_vertex(uint id [[vertex_id]],const device Vertex* vertices [[buffer(0)]],constant Push& p [[buffer(1)]]) {
 float2 point=float2(vertices[id].x,vertices[id].y)*p.scale+p.translation;
 Vary out;out.position=float4(point.x/p.screen.x*2-1,1-point.y/p.screen.y*2,0,1);out.coverage=vertices[id].coverage;return out;
}
uint4 mul16(uint4 c,uint a){uint4 t=c*a+32768;return (t+(t>>16))>>16;}
uint4 store8(uint4 c){uint4 t=c+128;return (t-(t>>8))>>8;}
uint4 byte_mul(uint4 pixel,uint coverage){uint4 t=pixel*coverage;return (t+(t>>8)+128)>>8;}
fragment Framebuffer cad_fragment(Vary in [[stage_in]],Framebuffer previous,constant Push& p [[buffer(1)]]
#ifdef ANNOTATION
,const device AnnotationDraw& annotation [[buffer(2)]],const device uint* hidden [[buffer(3)]]
#endif
) {
#ifdef ANNOTATION
 if(!annotation_visible(annotation,hidden,int2(floor(in.position.xy))))discard_fragment();
#endif
 uint coverage=uint(floor(clamp(in.coverage,0.0f,1.0f)*255.0f+0.5f));
 uint4 source=byte_mul(uint4(floor(clamp(p.rgba,0.0f,1.0f)*255.0f+0.5f)),coverage);
 uint4 destination=uint4(floor(clamp(previous.rgba,0.0f,1.0f)*255.0f+0.5f));
 Framebuffer out;
 uint op=p.glyph.x&255u;
 uint4 straight=uint4(p.glyph.y&255u,(p.glyph.y>>8)&255u,(p.glyph.y>>16)&255u,p.glyph.y>>24);
 if(op==2u){float3 mask=float3((p.glyph.x>>8)&255u,(p.glyph.x>>16)&255u,p.glyph.x>>24)/255.0f*(float(straight.a)/255.0f);out.rgba=float4(float3(straight.rgb)/255.0f*mask+previous.rgba.rgb*(1.0f-mask),previous.rgba.a);return out;}
 if(op==1u){uint4 dst=destination*257u;uint4 c=straight*257u;uint4 src=mul16(c,c.a);src.a=c.a;uint m=coverage*257u;uint4 result=c.a==65535u?mul16(src,m)+mul16(dst,65535u-m):mul16(src+mul16(dst,65535u-src.a),m)+mul16(dst,65535u-m);out.rgba=float4(store8(result))/255.0f;return out;}
 out.rgba=float4(source+byte_mul(destination,255-source.a))/255.0f;return out;
}
