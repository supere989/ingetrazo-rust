#version 450
layout(location=0) in vec2 position;
layout(location=1) in float coverage;
layout(location=0) out float edge_coverage;
layout(push_constant) uniform Push {vec2 scale;vec2 translation;vec2 screen;vec2 unused;vec4 rgba;} push;
void main(){vec2 p=position*push.scale+push.translation;gl_Position=vec4(2*p/push.screen-1,0,1);edge_coverage=coverage;}
