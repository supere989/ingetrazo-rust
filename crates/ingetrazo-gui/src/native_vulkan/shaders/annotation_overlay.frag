#version 450
layout(location=0) in float edge_coverage;
layout(location=0) out vec4 output_color;
layout(input_attachment_index=0,set=0,binding=0) uniform subpassInput destination;
layout(push_constant) uniform Push {vec2 scale;vec2 translation;vec2 screen;uvec2 gates;vec4 rgba;uint composition;float ax;float ay;float bx;float by;float cx;float cy;float dx;float dy;float ex;float ey;float fx;float fy;float gx;float gy;float hx;float hy;} push;
uvec4 byte_mul(uvec4 pixel,uint coverage){uvec4 t=pixel*coverage;return (t+(t>>8u)+128u)>>8u;}
layout(set=0,binding=1,std430) readonly buffer Visibility {uint hidden[];};
// Qt6.12 QRgba64 multiplyAlpha65535: exact integer intermediates, no float blend.
uvec4 mul16(uvec4 value,uint alpha){uvec4 t=value*alpha+32768u;return (t+(t>>16u))>>16u;}
uvec4 store8(uvec4 value){uvec4 t=value+128u;return (t-(t>>8u))>>8u;}
// Source NVIDIA GL_SUBPIXEL_BITS=8. Signed 32x32 products are retained
// as two words, so edge tests do not lose integer ownership to float rounding
// and no shaderInt64 capability is required.
uvec2 product64(int a,int b){int hi;int lo;imulExtended(a,b,hi,lo);return uvec2(uint(lo),uint(hi));}
uvec2 subtract64(uvec2 a,uvec2 b){return uvec2(a.x-b.x,a.y-b.y-uint(a.x<b.x));}
uvec2 edge64(ivec2 a,ivec2 b,ivec2 p){ivec2 d=b-a;ivec2 q=p-a;return subtract64(product64(d.x,q.y),product64(d.y,q.x));}
bool positive64(uvec2 v){return int(v.y)>0||(v.y==0u&&v.x!=0u);}
bool source_edge(ivec2 a,ivec2 b,ivec2 sample_point){
 ivec2 d=b-a;uvec2 edge=edge64(a,b,sample_point);
 return positive64(edge)||(all(equal(edge,uvec2(0)))&&(d.y<0||(d.y==0&&d.x<0)));
}
bool source_triangle(ivec2 a,ivec2 b,ivec2 c,ivec2 p){
 uvec2 area=edge64(a,b,c);if(all(equal(area,uvec2(0))))return false;
 if(!positive64(area)){ivec2 tmp=b;b=c;c=tmp;}
 return source_edge(a,b,p)&&source_edge(b,c,p)&&source_edge(c,a,p);
}
bool source_rectangle(vec2 lo,vec2 hi,ivec2 p){
 ivec2 a=ivec2(roundEven(lo*256.0));ivec2 d=ivec2(roundEven(hi*256.0));ivec2 b=ivec2(d.x,a.y);ivec2 c=ivec2(a.x,d.y);
 return source_triangle(a,b,c,p)||source_triangle(c,b,d,p);
}
void main(){
 if((push.gates.x!=0xffffffffu&&hidden[push.gates.x]!=0u)||(push.gates.y!=0xffffffffu&&hidden[push.gates.y]!=0u)){discard;}
 if(push.composition==5u){
  ivec2 p=ivec2(floor(gl_FragCoord.xy))*256+ivec2(128);
  if(!source_rectangle(vec2(push.dx,push.dy),vec2(push.ex,push.ey),p)||!source_triangle(ivec2(roundEven(vec2(push.ax,push.ay)*256.0)),ivec2(roundEven(vec2(push.bx,push.by)*256.0)),ivec2(roundEven(vec2(push.cx,push.cy)*256.0)),p))discard;
  output_color=vec4(0.);return;
 }
 if(push.composition==6u){
  ivec2 p=ivec2(floor(gl_FragCoord.xy))*256+ivec2(128);
  if(!source_rectangle(vec2(push.ax,push.ay),vec2(push.bx,push.by),p))discard;
  output_color=vec4(push.rgba.rgb*push.rgba.a,push.rgba.a);return;
 }
 if(push.composition==4u){
  vec2 vertices[8]=vec2[8](vec2(push.ax,push.ay),vec2(push.bx,push.by),vec2(push.cx,push.cy),vec2(push.dx,push.dy),vec2(push.ex,push.ey),vec2(push.fx,push.fy),vec2(push.gx,push.gy),vec2(push.hx,push.hy));
  ivec2 p=ivec2(floor(gl_FragCoord.xy))*256+ivec2(128);
  bool covered=false;
  for(int i=0;i<6;i++){covered=covered||source_triangle(ivec2(roundEven(vertices[i]*256.0)),ivec2(roundEven(vertices[i+1]*256.0)),ivec2(roundEven(vertices[i+2]*256.0)),p);}
  if(!covered)discard;
  // Qt solid brush premultiplies before standard fixed-function SourceOver.
  // One source operation owns this fragment even where stroker triangles overlap.
  output_color=vec4(push.rgba.rgb*push.rgba.a,push.rgba.a);return;
 }
 if(push.composition==3u){
  ivec2 a=ivec2(roundEven(vec2(push.ax,push.ay)*256.0));
  ivec2 b=ivec2(roundEven(vec2(push.bx,push.by)*256.0));
  ivec2 c=ivec2(roundEven(vec2(push.cx,push.cy)*256.0));
  uvec2 area=edge64(a,b,c);
  if(all(equal(area,uvec2(0)))){discard;}
  if(!positive64(area)){ivec2 temporary=b;b=c;c=temporary;}
  ivec2 sample_point=ivec2(floor(gl_FragCoord.xy))*256+ivec2(128);
  if(!source_edge(a,b,sample_point)||!source_edge(b,c,sample_point)||!source_edge(c,a,sample_point)){discard;}
  output_color=push.rgba;return;
 }
 uint coverage=uint(floor(clamp(edge_coverage,0,1)*255.0+0.5));
 uvec4 color=uvec4(floor(clamp(push.rgba,0,1)*255.0+0.5));
 if((push.composition&255u)==2u){
  vec3 mask=vec3((push.composition>>8u)&255u,(push.composition>>16u)&255u,(push.composition>>24u)&255u)/255.0;
  mask*=push.rgba.a;
  // The dedicated same-pass LCD pipeline performs actual fixed-function
  // CONSTANT_COLOR / ONE_MINUS_SRC_COLOR, as the original GL engine does.
  output_color=vec4(mask,1.);return;
 }
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
