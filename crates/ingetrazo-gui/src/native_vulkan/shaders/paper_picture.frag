#version 450
layout(location=0) out vec4 output_color;
layout(input_attachment_index=0,set=0,binding=0) uniform subpassInput destination;
layout(std430,set=0,binding=1) readonly buffer Pixels { uint rgba[]; } pixels;
layout(std430,set=0,binding=2) readonly buffer Rows { int fy[]; } rows;
layout(push_constant) uniform Push { ivec4 target;ivec4 bounds;ivec4 fractions;ivec4 x;ivec4 fx;ivec4 image; } p;
uvec4 byte_mul(uvec4 pixel,uint alpha){uvec4 t=pixel*alpha;return (t+(t>>8u)+128u)>>8u;}
uvec4 pixel(int x,int y){
 ivec2 q=clamp(ivec2(x,y),ivec2(0),p.image.xy-1);
 uint raw=pixels.rgba[q.y*p.image.x+q.x];
 uvec4 color=uvec4(raw&255u,(raw>>8u)&255u,(raw>>16u)&255u,raw>>24u);
 return uvec4(byte_mul(color,color.a).rgb,color.a);
}
void main(){
 ivec2 g=ivec2(gl_FragCoord.xy)+p.target.zw;
 int row=g.y-p.fx.w;
 if(row<0||row>=p.image.z||g.x<p.bounds.x||g.x>=p.bounds.z)discard;
 uint w=65536u,h=65536u;
 if(g.x==p.bounds.x)w-=uint(p.fractions.x);
 if(g.y==p.bounds.y)h-=uint(p.fractions.y);
 if(g.x==p.bounds.z-1&&p.fractions.z!=0)w-=65536u-uint(p.fractions.z);
 if(g.y==p.bounds.w-1&&p.fractions.w!=0)h-=65536u-uint(p.fractions.w);
 // Exact floor(w*h*255/2^32), including the fully covered 65536^2 pixel.
 uint high,low,carry,unused;
 umulExtended(w,h,high,low);umulExtended(low,255u,carry,unused);
 uint coverage=high*255u+carry;
 if(coverage==0u)discard;
 int fx=g.x==p.x.x?p.fx.x:(g.x==p.x.z?p.fx.z:p.fx.y+p.x.w*(g.x-p.x.y));
 int fy=rows.fy[row];
 int x=fx>>16,y=fy>>16;
 uint dx=(uint(fx)&65535u)>>8u,dy=(uint(fy)&65535u)>>8u;
 uvec4 a=pixel(x,y),b=pixel(x+1,y),c=pixel(x,y+1),d=pixel(x+1,y+1);
 uvec4 left=(a*(256u-dy)+c*dy)>>8u;
 uvec4 right=(b*(256u-dy)+d*dy)>>8u;
 uvec4 src=byte_mul((left*(256u-dx)+right*dx)>>8u,coverage);
 if(src.a==255u){output_color=vec4(src)/255.0;return;}
 uvec4 dst=uvec4(floor(clamp(subpassLoad(destination),0,1)*255.0+0.5));
 output_color=vec4(min(src+byte_mul(dst,255u-src.a),uvec4(255u)))/255.0;
}
