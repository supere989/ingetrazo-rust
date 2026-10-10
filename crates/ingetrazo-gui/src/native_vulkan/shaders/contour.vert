#version 450
// Original viewport._section_cut_segments/_draw_section_cut_edges, GPU assembled.
layout(set=0,binding=0,std430) readonly buffer Vertices {float vertices[];};
layout(set=0,binding=1,std430) readonly buffer Indices {uint indices[];};
layout(push_constant) uniform Parameters {mat4 mvp;layout(offset=64) vec4 plane;layout(offset=80) uint enabled;layout(offset=84) uint stride;layout(offset=88) uint implicit_indices;layout(offset=96) vec3 eye;layout(offset=108) float width_factor;layout(offset=112) vec4 edge_color;} p;
layout(location=0) out vec4 color;
layout(location=1) out vec3 world_position;
vec3 position(uint id){uint base=id*p.stride;return vec3(vertices[base],vertices[base+1],vertices[base+2]);}
void main(){
 vec3 points[3];float d[3];for(int i=0;i<3;i++){points[i]=position(p.implicit_indices!=0?uint(gl_InstanceIndex*3+i):indices[gl_InstanceIndex*3+i]);d[i]=dot(vec4(points[i],1),p.plane);}
 vec3 hits[3];int count=0;for(int i=0;i<3;i++){int j=(i+1)%3;if(d[i]*d[j]<0){hits[count++]=points[i]+(points[j]-points[i])*(d[i]/(d[i]-d[j]));}}
 color=p.edge_color;world_position=vec3(0);gl_Position=vec4(2,2,2,1);
 if(count!=2)return;vec3 tangent=hits[1]-hits[0];float length_t=length(tangent);if(length_t<=1e-9)return;
 vec3 side=cross(p.plane.xyz,tangent/length_t);float width=max(length((hits[0]+hits[1])*0.5-p.eye)*p.width_factor,1.25e-6);vec3 lift=p.plane.xyz*(0.6*width);
 vec3 quad[4]=vec3[4](hits[0]-side*width+lift,hits[1]-side*width+lift,hits[1]+side*width+lift,hits[0]+side*width+lift);
 int order[6]=int[6](0,1,2,0,2,3);world_position=quad[order[gl_VertexIndex]];gl_Position=p.mvp*vec4(world_position,1);gl_Position.y=-gl_Position.y;
}
