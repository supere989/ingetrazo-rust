#version 450
layout(location=0) in float edge_coverage;
layout(location=0) out vec4 output_color;
layout(push_constant) uniform Push {vec2 scale;vec2 translation;vec2 screen;vec2 unused;vec4 rgba;} push;
// Qt6.12 qdrawhelper_p.h BYTE_MUL: round each premultiplied channel
// before SourceOver. Coverage is an 8-bit raster span, not brush opacity.
uvec4 byte_mul(uvec4 pixel,uint coverage){uvec4 t=pixel*coverage;return (t+(t>>8u)+128u)>>8u;}
void main(){
 uint coverage=uint(floor(clamp(edge_coverage,0,1)*255.0+0.5));
 uvec4 premultiplied=uvec4(floor(clamp(push.rgba,0,1)*255.0+0.5));
 output_color=vec4(byte_mul(premultiplied,coverage))/255.0;
}
