#version 450
layout(location=0) in vec3 position;
layout(location=1) in vec3 normal;
layout(location=2) in vec4 color;
layout(location=3) in vec2 uv;
layout(push_constant) uniform Matrices {mat4 mvp;vec4 plane;uint clip_enabled;} camera;
layout(location=0) out vec4 out_color;
layout(location=1) out vec3 world_position;
void main(){gl_Position=camera.mvp*vec4(position,1.0);gl_ClipDistance[0]=camera.clip_enabled!=0u?-dot(camera.plane,vec4(position,1.)):1.;out_color=color;world_position=position;}
