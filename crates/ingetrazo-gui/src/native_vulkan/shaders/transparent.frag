#version 450
layout(constant_id=0)const bool pre_raster_clip=false;
layout(location=0) in vec4 color;
layout(location=1) in vec3 world_position;
layout(push_constant) uniform Clip {layout(offset=64) vec4 plane;layout(offset=80) uint enabled;layout(offset=84) float opacity;} clip;
layout(location=0) out vec4 output_color;
void main(){if(!pre_raster_clip&&(clip.enabled&1u)!=0u && dot(vec4(world_position,1.0),clip.plane)>0.0) discard;output_color=vec4(color.rgb,clip.opacity);}
