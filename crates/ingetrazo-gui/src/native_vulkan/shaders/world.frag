#version 450
layout(location=0) in vec4 color;
layout(location=1) in vec3 world_position;
layout(push_constant) uniform Clip {layout(offset=64) vec4 plane;layout(offset=80) uint enabled;} clip;
layout(location=0) out vec4 output_color;
void main(){if(clip.enabled!=0 && dot(vec4(world_position,1.0),clip.plane)>0.0) discard;output_color=vec4(pow(color.rgb,vec3(2.2)),color.a);}
