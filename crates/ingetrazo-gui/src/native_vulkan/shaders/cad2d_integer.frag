#version 450
layout(location=0) in float edge_coverage;
layout(location=0) out vec4 output_color;
layout(input_attachment_index=0,set=0,binding=0) uniform subpassInput destination;
layout(push_constant) uniform Push {vec2 scale;vec2 translation;vec2 screen;vec2 unused;vec4 rgba;} push;
uvec4 byte_mul(uvec4 pixel,uint coverage){uvec4 t=pixel*coverage;return (t+(t>>8u)+128u)>>8u;}
void main(){
 uint coverage=uint(floor(clamp(edge_coverage,0,1)*255.0+0.5));
 uvec4 color=uvec4(floor(clamp(push.rgba,0,1)*255.0+0.5));
 uvec4 src=byte_mul(color,coverage);
 // Opaque source never reads destination; admission validates every vertex/palette.
 if(src.a==255u){output_color=vec4(src)/255.0;return;}
 uvec4 dst=uvec4(floor(clamp(subpassLoad(destination),0,1)*255.0+0.5));
 output_color=vec4(min(src+byte_mul(dst,255u-src.a),uvec4(255u)))/255.0;
}
