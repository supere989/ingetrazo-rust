#include <metal_stdlib>
using namespace metal;
struct Raster{float4 position [[position]];};
vertex Raster canvas_vertex(uint id [[vertex_id]]){
 const float2 positions[3]={float2(-1,1),float2(3,1),float2(-1,-3)};
 return {float4(positions[id],0,1)};
}
fragment float4 canvas_fragment(Raster in [[stage_in]],texture2d<float,access::read> canvas [[texture(0)]]){
 return canvas.read(uint2(in.position.xy));
}
