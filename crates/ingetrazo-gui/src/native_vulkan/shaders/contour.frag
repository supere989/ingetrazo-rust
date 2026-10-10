#version 450
// Contours use their own 128-byte push layout, independent of face styles.
layout(location=0) in vec4 color;
layout(location=0) out vec4 output_color;
void main(){output_color=vec4(color.rgb,1.0);}
