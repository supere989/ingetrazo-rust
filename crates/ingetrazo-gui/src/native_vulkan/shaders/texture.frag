#version 450
layout(constant_id=0)const bool pre_raster_clip=false;
layout(set=0,binding=0)uniform sampler2D source_texture;
layout(location=0)in vec2 tex_uv;
layout(location=1)in vec3 world_position;
layout(location=0)out vec4 color;
layout(push_constant)uniform Params{mat4 mvp;vec4 plane;uint clip_enabled;float opacity;float shade;uint cutout_height;layout(offset=112)vec3 context_background;float context_fade;}p;
const float BAYER[16]=float[16](.5/16.,8.5/16.,2.5/16.,10.5/16.,12.5/16.,4.5/16.,14.5/16.,6.5/16.,3.5/16.,11.5/16.,1.5/16.,9.5/16.,15.5/16.,7.5/16.,13.5/16.,5.5/16.);
void main(){if(!pre_raster_clip&&(p.clip_enabled&1u)!=0u&&dot(p.plane,vec4(world_position,1.))>0.)discard;
 if((p.clip_enabled&2u)!=0u){color=vec4(0.);return;} vec4 texel=texture(source_texture,tex_uv);
 if(p.opacity>.999&&texel.a<.5){if((p.cutout_height&1u)!=0u)discard;int x=int(gl_FragCoord.x)%4;int y=int(gl_FragCoord.y)%4;if(texel.a<BAYER[y*4+x])discard;color=vec4(texel.rgb*p.shade,1.);}else color=vec4(texel.rgb*p.shade,texel.a*p.opacity);
if(p.context_fade>0.){color.rgb=mix(color.rgb,p.context_background,p.context_fade);}
}
