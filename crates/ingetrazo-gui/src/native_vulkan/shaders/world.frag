#version 450
layout(location=0) in vec4 color;
layout(location=0) out vec4 output_color;
void main(){output_color=vec4(pow(color.rgb,vec3(2.2)),color.a);}
