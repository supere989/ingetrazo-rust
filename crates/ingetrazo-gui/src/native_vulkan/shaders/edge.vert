#version 450
layout(location=0) in vec3 endpoint_a;
layout(location=1) in vec3 endpoint_b;
layout(push_constant) uniform Projection{mat4 mvp;layout(offset=64)vec4 plane;layout(offset=80)uint enabled;} projection;
layout(location=0) out vec4 color;
layout(location=1) out vec3 world_position;
void main(){world_position=gl_VertexIndex==0?endpoint_a:endpoint_b;gl_Position=projection.mvp*vec4(world_position,1.0);if((projection.enabled&0x80000000u)!=0u)gl_Position.z=(gl_Position.z+gl_Position.w)*0.5;gl_ClipDistance[0]=(projection.enabled&1u)!=0u?-dot(vec4(world_position,1),projection.plane):1.0;color=vec4(0.0);}
