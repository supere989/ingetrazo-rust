#version 450
layout(location=0)in vec3 position;

layout(location=0)out vec2 tex_uv;
layout(location=1)out vec3 world_position;
layout(push_constant)uniform Params{mat4 light_vp;vec4 clip;uint clip_enabled;uint use_texture;}p;
void main(){world_position=position;tex_uv=vec2(0.);gl_Position=p.light_vp*vec4(position,1.);gl_Position.z=(gl_Position.z+gl_Position.w)*.5;}
