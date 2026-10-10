#version 450
layout(location=0) in float edge_coverage;
layout(location=0) out vec4 output_color;
layout(push_constant) uniform Push {vec2 scale;vec2 translation;vec2 screen;vec2 unused;vec4 rgba;} push;
void main(){output_color=push.rgba*clamp(edge_coverage,0,1);}
