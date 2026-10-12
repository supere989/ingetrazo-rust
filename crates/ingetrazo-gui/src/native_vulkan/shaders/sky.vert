#version 450
layout(push_constant) uniform Background {vec4 colors[5]; float horizon;} background;
layout(location=0) out vec4 out_color;
void main(){
 int i=gl_VertexIndex;
 if(i>=12){gl_Position=vec4(i==12?-1.0:1.0,background.horizon,0.0,1.0);out_color=background.colors[4];return;}
 const vec2 corners[6]=vec2[6](vec2(-1,0),vec2(1,0),vec2(1,1),vec2(-1,0),vec2(1,1),vec2(-1,1));
 vec2 p=corners[i%6];
 bool sky=i<6;
 float y=sky?mix(background.horizon,1.0,p.y):mix(-1.0,background.horizon,p.y);
 gl_Position=vec4(p.x,y,0.0,1.0);
 out_color=sky?mix(background.colors[0],background.colors[1],p.y):mix(background.colors[3],background.colors[2],p.y);
}
