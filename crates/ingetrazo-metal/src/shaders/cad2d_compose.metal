#include <metal_stdlib>
using namespace metal;
struct Span {int x0;int y0;int x1;int y1;uint coverage;uint palette;};
struct Palette {uint4 color;uint composition;uint r;uint g;uint b;uint a;uint pad0;uint pad1;uint pad2;};
struct Tile {int y;int x;uint first;uint count;};
struct Push {int2 translation;uint2 size;uint4 clip;uint tile_count;uint pad0;uint pad1;uint pad2;};
uint4 mul16(uint4 c,uint a){uint4 t=c*a+32768;return (t+(t>>16))>>16;}
uint4 store8(uint4 c){uint4 t=c+128;return (t-(t>>8))>>8;}
uint4 glyph_compose(Palette p,uint coverage,uint4 destination){
 uint op=p.composition&255u;uint4 c=uint4(p.r,p.g,p.b,p.a);
 if(op==2u){float3 mask=float3((p.composition>>8)&255u,(p.composition>>16)&255u,p.composition>>24)/255.0f*(float(c.a)/255.0f);float3 result=float3(c.rgb)*mask+float3(destination.rgb)*(1.0f-mask);return uint4(uint3(floor(result+0.5f)),destination.a);}
 uint4 dst=destination*257u;c*=257u;uint4 src=mul16(c,c.a);src.a=c.a;uint m=coverage*257u;return store8(c.a==65535u?mul16(src,m)+mul16(dst,65535u-m):mul16(src+mul16(dst,65535u-src.a),m)+mul16(dst,65535u-m));
}
uint4 byte_mul(uint4 pixel,uint coverage){uint4 t=pixel*coverage;return (t+(t>>8)+128)>>8;}
int tile_coordinate(int p){return p>=0?p/8:(p-7)/8;}
kernel void span_compose(const device Span* spans [[buffer(0)]],const device Tile* tiles [[buffer(1)]],const device uint* references [[buffer(2)]],const device Palette* palette [[buffer(3)]],constant Push& p [[buffer(4)]],texture2d<uint,access::read_write> output [[texture(0)]],uint3 group [[threadgroup_position_in_grid]],uint3 local [[thread_position_in_threadgroup]]){
 if(group.x>=p.tile_count)return;
 Tile tile=tiles[group.x];int2 point=int2(tile.x*8,tile.y*8)+int2(local.xy);int2 physical=point+p.translation;
 if(any(physical<0)||any(uint2(physical)>=p.size))return;uint2 pixel=uint2(physical);
 if(pixel.x<p.clip.x||pixel.y<p.clip.y||pixel.x>=p.clip.z||pixel.y>=p.clip.w)return;
 uint packed=output.read(pixel).r;
 uint4 destination=uint4(packed&255u,(packed>>8)&255u,(packed>>16)&255u,packed>>24);
 // Unique source tile owns this pixel. References preserve original batch/index
 // order; no races, atomics, feedback, image copies or floating blend.
 for(uint j=0;j<tile.count;j++){Span s=spans[references[tile.first+j]];if(point.x>=s.x0&&point.x<s.x1&&point.y>=s.y0&&point.y<s.y1){Palette p=palette[s.palette];if(p.composition!=0){destination=glyph_compose(p,s.coverage,destination);}else{uint4 source=byte_mul(p.color,s.coverage);destination=source+byte_mul(destination,255-source.a);}}}
 uint encoded=destination.r|(destination.g<<8)|(destination.b<<16)|(destination.a<<24);
 output.write(uint4(encoded,0,0,0),pixel);
}
