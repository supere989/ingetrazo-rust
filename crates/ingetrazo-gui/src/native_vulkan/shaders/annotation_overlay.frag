#version 450
layout(location=0) in float edge_coverage;
layout(location=0) out vec4 output_color;
layout(input_attachment_index=0,set=0,binding=0) uniform subpassInput destination;
layout(push_constant) uniform Push {vec2 scale;vec2 translation;vec2 screen;uvec2 gates;vec4 rgba;uint composition;} push;
uvec4 byte_mul(uvec4 pixel,uint coverage){uvec4 t=pixel*coverage;return (t+(t>>8u)+128u)>>8u;}
layout(set=0,binding=1,std430) readonly buffer Visibility {uint hidden[];};
// Qt6.12 QRgba64 multiplyAlpha65535: exact integer intermediates, no float blend.
uvec4 mul16(uvec4 value,uint alpha){uvec4 t=value*alpha+32768u;return (t+(t>>16u))>>16u;}
uvec4 store8(uvec4 value){uvec4 t=value+128u;return (t-(t>>8u))>>8u;}
void main(){
 if((push.gates.x!=0xffffffffu&&hidden[push.gates.x]!=0u)||(push.gates.y!=0xffffffffu&&hidden[push.gates.y]!=0u)){discard;}
 uint coverage=uint(floor(clamp(edge_coverage,0,1)*255.0+0.5));
 uvec4 color=uvec4(floor(clamp(push.rgba,0,1)*255.0+0.5));
 if(push.composition==1u){
  if(color.a==255u&&coverage==255u){output_color=vec4(color)/255.0;return;}
  // The native annotation pass targets the opaque retained framebuffer.
  // Source Format_RGBA8888 glyph blit uses QRgba64 even though span fills use BYTE_MUL.
  uvec4 dst=uvec4(floor(clamp(subpassLoad(destination),0,1)*255.0+0.5))*257u;
  uvec4 straight=color*257u;
  uvec4 src=mul16(straight,straight.a);src.a=straight.a;
  uint mask=coverage*257u;
  uvec4 result;
  if(straight.a==65535u){result=mul16(src,mask)+mul16(dst,65535u-mask);}
  else{
   // Nonopaque text color is composed first, then that result is interpolated by glyph coverage.
   uvec4 colored=src+mul16(dst,65535u-src.a);
   result=mul16(colored,mask)+mul16(dst,65535u-mask);
  }
  output_color=vec4(store8(result))/255.0;return;
 }
 uvec4 src=byte_mul(color,coverage);
 // Opaque source never reads destination; admission validates every vertex/palette.
 if(src.a==255u){output_color=vec4(src)/255.0;return;}
 uvec4 dst=uvec4(floor(clamp(subpassLoad(destination),0,1)*255.0+0.5));
 output_color=vec4(min(src+byte_mul(dst,255u-src.a),uvec4(255u)))/255.0;
}
