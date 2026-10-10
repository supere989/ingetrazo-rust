#include <metal_stdlib>
using namespace metal;
struct Span {int x0;int y0;int x1;int y1;uint coverage;uint palette;};
struct Tile {int y;int x;uint first;uint count;};
struct Push {int2 translation;uint2 size;uint4 clip;uint tile_count;uint pad0;uint pad1;uint pad2;};
uint4 byte_mul(uint4 pixel,uint coverage){uint4 t=pixel*coverage;return (t+(t>>8)+128)>>8;}
int tile_coordinate(int p){return p>=0?p/8:(p-7)/8;}
kernel void span_compose(const device Span* spans [[buffer(0)]],const device Tile* tiles [[buffer(1)]],const device uint* references [[buffer(2)]],const device uint4* palette [[buffer(3)]],constant Push& p [[buffer(4)]],texture2d<float,access::read_write> output [[texture(0)]],uint3 group [[threadgroup_position_in_grid]],uint3 local [[thread_position_in_threadgroup]]){
 if(group.x>=p.tile_count)return;
 Tile tile=tiles[group.x];int2 point=int2(tile.x*8,tile.y*8)+int2(local.xy);int2 physical=point+p.translation;
 if(any(physical<0)||any(uint2(physical)>=p.size))return;uint2 pixel=uint2(physical);
 if(pixel.x<p.clip.x||pixel.y<p.clip.y||pixel.x>=p.clip.z||pixel.y>=p.clip.w)return;
 uint4 destination=uint4(round(output.read(pixel)*255.0f));
 // Unique source tile owns this pixel. References preserve original batch/index
 // order; no races, atomics, feedback, image copies or floating blend.
 for(uint j=0;j<tile.count;j++){Span s=spans[references[tile.first+j]];if(point.x>=s.x0&&point.x<s.x1&&point.y>=s.y0&&point.y<s.y1){uint4 source=byte_mul(palette[s.palette],s.coverage);destination=source+byte_mul(destination,255-source.a);}}
 output.write(float4(destination)/255.0f,pixel);
}
