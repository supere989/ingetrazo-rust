#include <metal_stdlib>
using namespace metal;
uint4 pp_byte_mul(uint4 c,uint a){uint4 t=c*a;return(t+(t>>8)+128)>>8;}
uint4 pp_pixel(device const uint*image,int x,int y,constant int*p){x=clamp(x,0,p[20]-1);y=clamp(y,0,p[21]-1);uint c=image[uint(y)*uint(p[20])+uint(x)];uint4 out=uint4(c&255,(c>>8)&255,(c>>16)&255,c>>24);uint4 pm=pp_byte_mul(out,out.a);pm.a=out.a;return pm;}
bool pp_source(int2 q,device const uint*image,device const int*rows,constant int*p,thread uint4&source){
 int2 g=q+int2(p[2],p[3]);if(g.x<p[4]||g.y<p[5]||g.x>=p[6]||g.y>=p[7]||g.y<p[19]||g.y-p[19]>=p[22])return false;
 uint right=p[10]==0?65536:uint(p[10]);uint bottom=p[11]==0?65536:uint(p[11]);
 uint w=65536-(g.x==p[4]?uint(p[8]):0)-(g.x==p[6]-1?65536-right:0);uint h=65536-(g.y==p[5]?uint(p[9]):0)-(g.y==p[7]-1?65536-bottom:0);
 uint product=w*h;uint coverage=(w==65536&&h==65536)?255:(((product>>16)*255+(((product&65535)*255)>>16))>>16);
 int fx=g.x==p[12]?p[16]:(g.x==p[14]?p[18]:p[17]+p[15]*(g.x-p[13]));int fy=rows[g.y-p[19]];int x=fx>>16;int y=fy>>16;uint dx=(uint(fx)&65535)>>8;uint dy=(uint(fy)&65535)>>8;
 uint4 left=(pp_pixel(image,x,y,p)*(256-dy)+pp_pixel(image,x,y+1,p)*dy)>>8;
 uint4 other=(pp_pixel(image,x+1,y,p)*(256-dy)+pp_pixel(image,x+1,y+1,p)*dy)>>8;
 source=pp_byte_mul((left*(256-dx)+other*dx)>>8,coverage);return true;
}
uint4 pp_over(uint4 previous,uint4 source){return min(source+pp_byte_mul(previous,255-source.a),uint4(255));}
#ifndef PP_COMPUTE_ONLY
struct PPVertex{float4 position [[position]];};
vertex PPVertex pp_vertex(uint index[[vertex_id]]){const float2 v[3]={float2(-1,-1),float2(3,-1),float2(-1,3)};return{float4(v[index],0,1)};}
struct PPOutput{float4 rgba [[color(0),raster_order_group(0)]];};
fragment PPOutput pp_fragment(PPVertex in[[stage_in]],device const uint*image[[buffer(0)]],device const int*rows[[buffer(1)]],constant int*p[[buffer(2)]],float4 previous[[color(0),raster_order_group(0)]]){uint4 source;uint4 prior=uint4(floor(clamp(previous,0.0f,1.0f)*255.0f+0.5f));bool hit=pp_source(int2(in.position.xy),image,rows,p,source);return{float4(hit?pp_over(prior,source):prior)/255.0f};}

#endif
kernel void pp_compute(device const uint*image[[buffer(0)]],device const int*rows[[buffer(1)]],constant int*p[[buffer(2)]],texture2d<uint,access::read_write>canvas[[texture(0)]],uint2 point[[thread_position_in_grid]]){
 if(point.x>=uint(p[0])||point.y>=uint(p[1]))return;uint4 source;if(!pp_source(int2(point),image,rows,p,source))return;point+=uint2(uint(p[23])&65535,uint(p[23])>>16);uint word=canvas.read(point).x;uint4 prior=uint4(word&255,(word>>8)&255,(word>>16)&255,word>>24);uint4 out=pp_over(prior,source);canvas.write(uint4(out.r|(out.g<<8)|(out.b<<16)|(out.a<<24),0,0,0),point);
}
