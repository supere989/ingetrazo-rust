#version 450
layout(location=0)in vec2 tex_uv;
layout(location=0)out vec4 color;
layout(set=0,binding=0)uniform sampler2D picture;
layout(push_constant)uniform Params{mat4 mvp;vec4 origin_opacity;vec4 u;vec4 v;vec4 reserved;}p;
const float BAYER[16]=float[16](.5/16.,8.5/16.,2.5/16.,10.5/16.,12.5/16.,4.5/16.,14.5/16.,6.5/16.,3.5/16.,11.5/16.,1.5/16.,9.5/16.,15.5/16.,7.5/16.,13.5/16.,5.5/16.);
void main(){vec4 pixel=texture(picture,tex_uv);
 // Source clears hard-cutout for images, but basic.frag still retains its
 // ordinary Bayer branch for low-alpha opaque texture fragments.
 if(p.origin_opacity.w>.999&&pixel.a<.5){int x=int(gl_FragCoord.x)%4;int y=int(p.reserved.x-gl_FragCoord.y)%4;if(pixel.a<BAYER[y*4+x])discard;color=vec4(pixel.rgb,1.);}else color=vec4(pixel.rgb,pixel.a*p.origin_opacity.w);
}
