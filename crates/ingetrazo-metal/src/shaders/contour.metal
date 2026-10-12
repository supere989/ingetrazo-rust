#include <metal_stdlib>
using namespace metal;
// Original _section_cut_segments + _draw_section_cut_edges. Cached source
// buffers remain immutable; plane intersections and camera widths execute here.
struct Parameters {float4 eye_width;float4 color;uint4 info;};
struct Section {float4 plane;uint enabled;uint padding[3];};
struct Out {float4 position [[position]];float4 color;};
vertex Out section_contour(uint corner [[vertex_id]],uint instance [[instance_id]],
 const device float* vertices [[buffer(0)]],const device float4x4* models [[buffer(1)]],
 constant float4x4& camera [[buffer(2)]],constant Section& section [[buffer(3)]],
 const device uint* indices [[buffer(4)]],constant Parameters& p [[buffer(5)]]) {
 uint triangle=instance%p.info.w;uint placement=instance/p.info.w;
 float3 points[3];float distances[3];
 for(uint i=0;i<3;i++){uint index=p.info.y!=0?p.info.z+triangle*3+i:indices[p.info.z+triangle*3+i];uint base=index*p.info.x;
  points[i]=(models[placement]*float4(vertices[base],vertices[base+1],vertices[base+2],1.)).xyz;
  distances[i]=dot(float4(points[i],1.),-section.plane);}
 Out out;out.position=float4(2.,2.,2.,1.);out.color=float4(p.color.rgb,1.);
 float3 hits[3];uint count=0;
 for(uint i=0;i<3;i++){uint j=(i+1)%3;if(distances[i]*distances[j]<0.)hits[count++]=points[i]+(points[j]-points[i])*(distances[i]/(distances[i]-distances[j]));}
 if(count!=2)return out;
 float3 tangent=hits[1]-hits[0];float len=length(tangent);if(len<=1e-9)return out;
 float3 normal=-section.plane.xyz;float3 side=cross(normal,tangent/len);
 float width=max(length((hits[0]+hits[1])*.5-p.eye_width.xyz)*p.eye_width.w,1.25e-6);
 float3 lift=normal*(.6*width);
 float3 quad[4]={hits[0]-side*width+lift,hits[1]-side*width+lift,hits[1]+side*width+lift,hits[0]+side*width+lift};
 const uint order[6]={0,1,2,0,2,3};out.position=camera*float4(quad[order[corner]],1.);
 out.position.z=(out.position.z+out.position.w)*.5;
#if SOURCE_REFLECT_Y
 out.position.y=-out.position.y;
#endif
 return out;
}
fragment float4 section_contour_color(Out in [[stage_in]]){return in.color;}
