#version 450
layout(location=0)out vec2 tex_uv;
layout(push_constant)uniform Params{mat4 mvp;vec4 origin_opacity;vec4 u;vec4 v;vec4 reserved;}p;
void main(){
 const vec2 corners[6]=vec2[](vec2(0,0),vec2(1,0),vec2(1,1),vec2(0,0),vec2(1,1),vec2(0,1));
 vec2 uv=corners[gl_VertexIndex];
 // Match Qt corners: addition order o+u+v is retained explicitly.
 vec3 corner=p.origin_opacity.xyz;
 if(uv.x>0.5)corner+=p.u.xyz;
 if(uv.y>0.5)corner+=p.v.xyz;
 gl_Position=p.mvp*vec4(corner,1.0);tex_uv=uv;
}
