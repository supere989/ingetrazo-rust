#version 450
layout(push_constant) uniform EdgeColor{layout(offset=96)vec4 rgba;} edge;
layout(location=0)out vec4 output_color;
// Section clipping happens before line rasterization. A fragment-center plane
// discard would remove partially covered endpoint samples kept by source GL.
void main(){output_color=vec4(edge.rgba.rgb,1.0);}
