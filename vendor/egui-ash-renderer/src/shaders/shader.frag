#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec4 oColor;
layout(location = 1) in vec2 oUV;

layout(binding = 0, set = 0) uniform sampler2D fontsSampler;

layout(location = 0) out vec4 finalColor;

layout(constant_id = 0) const bool SRGB_FRAMEBUFFER = false;

layout(push_constant) uniform TextureEncoding {layout(offset=64) uint encoded_opaque;} encoding;
vec3 SRGBtoLINEAR(vec3 color){return mix(color/12.92,pow((color+0.055)/1.055,vec3(2.4)),greaterThan(color,vec3(0.04045)));}

void main() {
    // Managed images are UNORM encoded RGBA; the native world texture uses
    // the same encoded contract. Blend directly into UNORM presentation.
    vec4 encodedColor = oColor * texture(fontsSampler, oUV);
    finalColor = SRGB_FRAMEBUFFER
        ? vec4(SRGBtoLINEAR(encodedColor.rgb), encodedColor.a)
        : encodedColor;
}
