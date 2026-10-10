#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec2 vPosition;
layout(location = 1) in vec2 vUV;
layout(location = 2) in vec4 vColor;

layout(push_constant) uniform Matrices {
    mat4 ortho;
} matrices;

layout(location = 0) out vec4 oColor;
layout(location = 1) out vec2 oUV;

vec3 decodeSRGB(vec3 c){return mix(c/12.92,pow((c+0.055)/1.055,vec3(2.4)),greaterThan(c,vec3(0.04045)));}
vec3 encodeSRGB(vec3 c){return mix(c*12.92,1.055*pow(c,vec3(1.0/2.4))-0.055,greaterThan(c,vec3(0.0031308)));}
vec4 encodedPremultiplied(vec4 c){
    // Color32 stores gamma-encoded *linear* premultiplication. Undo that
    // representation before interpolating source encoded-premultiplied RGB.
    // Alpha-zero colors retain egui additive semantics; opaque colors are direct.
    if(c.a==0.0 || c.a==1.0)return c;
    return vec4(encodeSRGB(clamp(decodeSRGB(c.rgb)/c.a,0.0,1.0))*c.a,c.a);
}
void main() {
    oColor = encodedPremultiplied(vColor);
    oUV = vUV;

    gl_Position = matrices.ortho*vec4(vPosition.x, vPosition.y, 0.0, 1.0);
}
