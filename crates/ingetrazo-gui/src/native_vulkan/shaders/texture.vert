#version 450
layout(location=0)in vec3 position;
layout(location=1)in vec2 uv;
layout(location=0)out vec2 tex_uv;
layout(location=1)out vec3 world_position;
layout(push_constant)uniform Params{mat4 mvp;vec4 plane;uint clip_enabled;float opacity;float shade;uint cutout_height;}p;
void main(){gl_Position=p.mvp*vec4(position,1.0);tex_uv=vec2(uv.x,1.0-uv.y);world_position=position;}
