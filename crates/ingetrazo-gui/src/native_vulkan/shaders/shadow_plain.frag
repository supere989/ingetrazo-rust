#version 450
layout(constant_id=0)const bool pre_raster_clip=false;

layout(location=0)in vec2 tex_uv;
layout(location=1)in vec3 world_position;
layout(location=0)out vec4 packed_depth;
layout(push_constant)uniform Params{mat4 light_vp;vec4 clip;uint clip_enabled;uint use_texture;}p;
void main(){
 if(!pre_raster_clip&&p.clip_enabled!=0u&&dot(vec4(world_position,1.),p.clip)>0.)discard;

 float d=gl_FragCoord.z;
 float slope=max(abs(dFdx(d)),abs(dFdy(d)));
 d=min(d+min(slope*2.,.01)+.0004,.999999);
 vec3 e=fract(vec3(1.,255.,65025.)*d);
 e.xy-=e.yz*(1./255.);
 packed_depth=vec4(e,1.);
}
