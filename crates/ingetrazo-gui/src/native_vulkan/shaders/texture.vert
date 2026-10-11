#version 450
layout(location=0)in vec3 position;
layout(location=1)in vec2 uv;
layout(location=0)out vec2 tex_uv;
layout(location=1)out vec3 world_position;
layout(push_constant)uniform Params{mat4 mvp;vec4 plane;uint clip_enabled;float opacity;float shade;uint cutout_height;}p;
void main(){gl_Position=p.mvp*vec4(position,1.0);if((p.clip_enabled&0x80000000u)!=0u)gl_Position.z=(gl_Position.z+gl_Position.w)*0.5;tex_uv=uv;world_position=position;}
