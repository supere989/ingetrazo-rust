#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec4 oColor;
layout(location = 1) in vec2 oUV;

layout(binding = 0, set = 0) uniform sampler2D fontsSampler;

layout(location = 0) out vec4 finalColor;

layout(constant_id = 0) const bool SRGB_FRAMEBUFFER = false;

layout(push_constant) uniform TextureEncoding {layout(offset=64) uint encoded_opaque;} encoding;
vec3 SRGBtoLINEAR(vec3 color){return mix(color/12.92,pow((color+0.055)/1.055,vec3(2.4)),greaterThan(color,vec3(0.04045)));}

const float GAMMA = 2.2;
const float INV_GAMMA = 1.0 / GAMMA;

vec3 LINEARtoSRGB(vec3 color) {
    return pow(color, vec3(INV_GAMMA));
}
vec4 LINEARtoSRGB(vec4 color) {
    return vec4(LINEARtoSRGB(color.rgb), color.a);
}
void main() {
    if(encoding.encoded_opaque!=0){vec4 sampleColor=oColor*texture(fontsSampler,oUV);finalColor=SRGB_FRAMEBUFFER?vec4(SRGBtoLINEAR(sampleColor.rgb),sampleColor.a):sampleColor;return;}
    if (SRGB_FRAMEBUFFER) {
        finalColor = oColor * texture(fontsSampler, oUV);
    } else {
        finalColor = LINEARtoSRGB(oColor * texture(fontsSampler, oUV));
    }
}
