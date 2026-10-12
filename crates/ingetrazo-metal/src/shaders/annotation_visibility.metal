#include <metal_stdlib>
using namespace metal;
struct Query {int2 pixel;float2 normalized;float threshold;uint active;uint2 padding;};
struct Parameters {float4x4 inverse;float4 eye;float4 forward;uint4 extent_count_sample;};
uint occluded(Query q,float depth,constant Parameters& p){
 if(!q.active||depth>=1.)return 0;
 float4 v=float4(q.normalized,depth*2.-1.,1.);
 // Match explicit Qt f32 sequence rather than fused matrix multiply.
 float4 r=p.inverse[0]*v.x+p.inverse[1]*v.y+p.inverse[2]*v.z+p.inverse[3]*v.w;
 if(abs(r.w)<as_type<float>(0x2b8cbccdu))return 0;
 float3 delta=r.xyz/r.w-p.eye.xyz;
 float along=delta.x*p.forward.x+delta.y*p.forward.y+delta.z*p.forward.z;
 return along<q.threshold;
}
kernel void visibility_single(uint id [[thread_position_in_grid]],const device Query* queries [[buffer(0)]],device uint* hidden [[buffer(1)]],constant Parameters& p [[buffer(2)]],depth2d<float,access::read> depth [[texture(0)]]){
 if(id>=p.extent_count_sample.z)return;Query q=queries[id];float maximum=0.;
 if(q.active){for(int y=max(q.pixel.y-1,0);y<=min(q.pixel.y+1,int(p.extent_count_sample.y)-1);y++){for(int x=max(q.pixel.x-1,0);x<=min(q.pixel.x+1,int(p.extent_count_sample.x)-1);x++){maximum=max(maximum,depth.read(uint2(x,((p.extent_count_sample.w&0x80000000u)?y:int(p.extent_count_sample.y)-1-y))));}}}
 hidden[id]=occluded(q,maximum,p);
}
kernel void visibility_ms(uint id [[thread_position_in_grid]],const device Query* queries [[buffer(0)]],device uint* hidden [[buffer(1)]],constant Parameters& p [[buffer(2)]],depth2d_ms<float,access::read> depth [[texture(0)]]){
 if(id>=p.extent_count_sample.z)return;Query q=queries[id];float maximum=0.;
 if(q.active){for(int y=max(q.pixel.y-1,0);y<=min(q.pixel.y+1,int(p.extent_count_sample.y)-1);y++){for(int x=max(q.pixel.x-1,0);x<=min(q.pixel.x+1,int(p.extent_count_sample.x)-1);x++){maximum=max(maximum,depth.read(uint2(x,((p.extent_count_sample.w&0x80000000u)?y:int(p.extent_count_sample.y)-1-y)),p.extent_count_sample.w&0x7fffffffu));}}}
 hidden[id]=occluded(q,maximum,p);
}
