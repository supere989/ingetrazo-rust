#version 450
layout(location=0) in vec2 position;
layout(location=1) in float coverage;
layout(location=2) in vec4 ab;
layout(location=3) in vec2 c;
layout(location=4) in vec4 bounds;
layout(location=0) out float edge_coverage;
layout(location=1) flat out vec4 source_ab;
layout(location=2) flat out vec2 source_c;
layout(location=3) flat out vec4 source_bounds;
layout(push_constant) uniform Push {vec2 scale;vec2 translation;vec2 screen;uvec2 gates;vec4 rgba;uint composition;float density;} push;
vec2 physical(vec2 p){precise vec2 product=p*push.scale;precise vec2 translated=product+push.translation;precise vec2 result=translated*push.density;return result;}
void main(){vec2 p=position*push.scale+push.translation;gl_Position=vec4(2*p/push.screen-1,0,1);edge_coverage=coverage;source_ab=vec4(physical(ab.xy),physical(ab.zw));source_c=physical(c);source_bounds=vec4(physical(bounds.xy),physical(bounds.zw));}
