#include <metal_stdlib>
using namespace metal;
struct Color { float4 value [[color(0)]]; };
// Probe only: reconstruct original color slots without writing the imageblock.
kernel void observe_iso_color(imageblock<Color> block,ushort2 p [[thread_position_in_threadgroup]],uint2 tile [[threadgroup_position_in_grid]],constant uint2* points [[buffer(0)]],device float4* values [[buffer(1)]]) {
 uint2 global=tile*uint2(16)+uint2(p);
 for(uint i=0;i<3;i++)if(all(global==points[i])) {
  float4 samples[4]={float4(-1),float4(-1),float4(-1),float4(-1)};
  ushort covered=0;ushort count=block.get_num_colors(p);
  for(ushort c=0;c<count;c++) {
   float4 v=block.read(p,c,imageblock_data_rate::color).value;
   ushort mask=block.get_color_coverage_mask(p,c)&ushort(15);
   if(covered&mask)return;covered|=mask;
   for(uint slot=0;slot<4;slot++)if(mask&(1u<<slot))samples[slot]=v;
  }
  if(covered!=15)return;
  for(uint slot=0;slot<4;slot++)values[i*4+slot]=samples[slot];
 }
}
kernel void observe_iso_depth(depth2d_ms<float,access::read> image [[texture(0)]],constant uint2* points [[buffer(0)]],device float* values [[buffer(1)]],uint id [[thread_position_in_grid]]) {if(id<12)values[id]=image.read(points[id/4],id%4);}
