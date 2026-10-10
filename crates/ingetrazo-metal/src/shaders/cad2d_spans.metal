#include <metal_stdlib>
using namespace metal;
struct Span {int x0;int y0;int x1;int y1;uint coverage;uint palette;};
struct Tile {int y;int x;uint first;uint count;};
struct Push {int2 translation;uint2 size;uint4 clear;uint tile_count;uint pad0;uint pad1;uint pad2;};
uint4 byte_mul(uint4 pixel,uint coverage){uint4 t=pixel*coverage;return (t+(t>>8)+128)>>8;}
int tile_coordinate(int p){return p>=0?p/8:(p-7)/8;}
kernel void span_compute(const device Span* spans [[buffer(0)]],const device Tile* tiles [[buffer(1)]],const device uint* references [[buffer(2)]],const device uint4* palette [[buffer(3)]],constant Push& p [[buffer(4)]],texture2d<float,access::write> output [[texture(0)]],uint2 pixel [[thread_position_in_grid]]){
 if(any(pixel>=p.size))return;
 int2 point=int2(pixel)-p.translation;int x=tile_coordinate(point.x),y=tile_coordinate(point.y);
 uint lo=0,hi=p.tile_count;while(lo<hi){uint mid=lo+(hi-lo)/2;Tile t=tiles[mid];if(t.y<y||(t.y==y&&t.x<x))lo=mid+1;else hi=mid;}
 uint4 destination=p.clear;
 if(lo<p.tile_count&&tiles[lo].y==y&&tiles[lo].x==x){Tile tile=tiles[lo];
  // IDs are emitted in original batch/index order. One invocation owns this
  // pixel; no framebuffer feedback, race, atomics or approximate float blend.
  for(uint j=0;j<tile.count;j++){Span s=spans[references[tile.first+j]];if(point.x>=s.x0&&point.x<s.x1&&point.y>=s.y0&&point.y<s.y1){uint4 source=byte_mul(palette[s.palette],s.coverage);destination=source+byte_mul(destination,255-source.a);}}
 }
 output.write(float4(destination)/255.0f,pixel);
}
