#version 450
layout(constant_id=0)const bool pre_raster_clip=false;
layout(location=1)in vec3 world_position;
layout(location=0)out vec4 color;
layout(push_constant)uniform Params{mat4 mvp;vec4 plane;uint clip_enabled;float opacity;float shade;uint cutout_height;vec4 mean;layout(offset=112)vec3 context_background;float context_fade;}p;
void main(){
 if(!pre_raster_clip&&(p.clip_enabled&1u)!=0u&&dot(p.plane,vec4(world_position,1.))>0.)discard;
 if((p.clip_enabled&2u)!=0u){color=vec4(0.);return;}
 // Source basic.frag applies u_shade only while sampling a texture.
 // Shaded _set_color uses the mean directly, despite the caller setting shade.
 color=vec4(p.mean.rgb,p.opacity);
if(p.context_fade>0.){color.rgb=mix(color.rgb,p.context_background,p.context_fade);}
}
