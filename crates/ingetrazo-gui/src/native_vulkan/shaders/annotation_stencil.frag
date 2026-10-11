#version 450
layout(location=0) in float edge_coverage;
layout(location=1) flat in vec4 source_ab;
layout(location=2) flat in vec2 source_c;
layout(location=3) flat in vec4 source_bounds;
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
 if((push.gates.x!=0xffffffffu&&hidden[push.gates.x]!=0u)||(push.gates.y!=0xffffffffu&&hidden[push.gates.y]!=0u))discard;
 ivec2 p=ivec2(roundEven(gl_FragCoord.xy*256.0));
 if(!source_rectangle(source_bounds.xy,source_bounds.zw,p)||!source_triangle(ivec2(roundEven(source_ab.xy*256.0)),ivec2(roundEven(source_ab.zw*256.0)),ivec2(roundEven(source_c*256.0)),p))discard;
 output_color=vec4(0);
}
