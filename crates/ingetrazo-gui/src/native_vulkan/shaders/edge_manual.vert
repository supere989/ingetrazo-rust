#version 450
layout(location=0) in vec3 endpoint_a;
layout(location=1) in vec3 endpoint_b;
layout(push_constant) uniform Projection{mat4 mvp;layout(offset=64)vec4 plane;layout(offset=80)uint enabled;} projection;
layout(location=0) out vec4 color;
layout(location=1) out vec3 world_position;
void main(){
 vec3 a=endpoint_a,b=endpoint_b;
 if(projection.enabled!=0u){float da=dot(vec4(a,1),projection.plane),db=dot(vec4(b,1),projection.plane);
  if(da>0.0&&db>0.0){gl_Position=vec4(2,2,2,1);world_position=a;color=vec4(0);return;}
  if(da>0.0||db>0.0){vec3 crossing=mix(a,b,da/(da-db));if(da>0.0)a=crossing;else b=crossing;}
 }
 world_position=gl_VertexIndex==0?a:b;gl_Position=projection.mvp*vec4(world_position,1.0);color=vec4(0.0);
}
