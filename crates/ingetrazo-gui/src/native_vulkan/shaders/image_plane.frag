#version 450
layout(location=0)in vec2 tex_uv;
layout(location=0)out vec4 color;
layout(set=0,binding=0)uniform sampler2D picture;
layout(push_constant)uniform Params{mat4 mvp;vec4 origin_opacity;vec4 u;vec4 v;vec4 reserved;}p;
void main(){vec4 pixel=texture(picture,tex_uv);color=vec4(pixel.rgb,pixel.a*p.origin_opacity.w);}
