#version 450
layout(location=0) in vec4 color;
layout(location=1) in vec3 world_position;
layout(push_constant) uniform Clip {layout(offset=64) vec4 plane;layout(offset=80) uint enabled;layout(offset=84) float opacity;layout(offset=88) uint mode;layout(offset=96) vec4 front_color;layout(offset=112) vec4 back_color;} clip;
layout(location=0) out vec4 output_color;
void main(){if(clip.enabled!=0 && dot(vec4(world_position,1.0),clip.plane)>0.0) discard;vec3 rgb=color.rgb;if(clip.mode==1)rgb=clip.front_color.rgb;else if(clip.mode==2)rgb=gl_FrontFacing?clip.front_color.rgb:clip.back_color.rgb;else if(clip.mode==3)rgb=clip.back_color.rgb;output_color=vec4(rgb,clip.opacity);}
