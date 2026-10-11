#version 450
layout(constant_id=0)const bool pre_raster_clip=false;
layout(set=0,binding=0)uniform sampler2D source_texture;
layout(location=0)in vec2 tex_uv;
layout(location=1)in vec3 world_position;
layout(location=0)out vec4 color;
layout(push_constant)uniform Params{mat4 mvp;vec4 plane;uint clip_enabled;float opacity;float shade;uint cutout_height;}p;
const float BAYER[16]=float[16](.5/16.,8.5/16.,2.5/16.,10.5/16.,12.5/16.,4.5/16.,14.5/16.,6.5/16.,3.5/16.,11.5/16.,1.5/16.,9.5/16.,15.5/16.,7.5/16.,13.5/16.,5.5/16.);
// Source viewport/basic.frag shadow_light, kept in source operation order.
layout(set=1,binding=0)uniform sampler2D shadow_map;
layout(set=1,binding=1,std140)uniform Shadow{mat4 light_vp;vec4 sun_bias;vec4 parameters;}shadow;
float shadow_light() {
    vec4 lp = shadow.light_vp * vec4(world_position, 1.0);
    vec3 uvz = lp.xyz / lp.w * 0.5 + 0.5;
    if (uvz.x <= 0.0 || uvz.x >= 1.0 || uvz.y <= 0.0 || uvz.y >= 1.0
            || uvz.z >= 1.0) {
        return 1.0;
    }
    vec2 texel = 1.0 / vec2(textureSize(shadow_map, 0));
    // Receiver-plane depth correction (Isidoro): each PCF tap compares the
    // stored depth against the receiver PLANE evaluated AT the tap, not
    // against the centre fragment. The plane's depth gradient in MAP UV
    // space comes from the screen derivatives through the inverse Jacobian
    // — both components, so diagonal taps on a surface lying oblique to
    // the sun stay exact (a per-axis slope estimate under-covered them by
    // up to 2× and striped grazing faces with moiré acne). The clamp keeps
    // a degenerate 2×2 quad (silhouette pixels) from blowing the plane up.
    vec3 ddx = dFdx(uvz);
    vec3 ddy = dFdy(uvz);
    float det = ddx.x * ddy.y - ddx.y * ddy.x;
    vec2 grad = (abs(det) > 1e-12)
        ? vec2(ddy.y * ddx.z - ddx.y * ddy.z,
               ddx.x * ddy.z - ddy.x * ddx.z) / det
        : vec2(0.0);
    float glen = length(grad);
    if (glen > 8.0) {
        // Cap: a face seen edge-on by the sun has a near-infinite gradient;
        // uncapped it inflates the margin and paints a wide false-lit band
        // where such a face meets its roof. 8 covers every face that
        // actually SHOWS its lighting (the truly parallel ones are dark by
        // face shading anyway) while keeping the band under ~15 cm.
        grad *= 8.0 / glen;
        glen = 8.0;
    }
    // Margin: constant term + 1.5 texels of slope along the gradient
    // (covers the within-texel offset of both receiver and caster raster).
    float bias = shadow.sun_bias.w + glen * texel.x * 1.5;
    float lit = 0.0;
    for (int dx = -1; dx <= 1; dx++) {
        for (int dy = -1; dy <= 1; dy++) {
            vec2 off = vec2(dx, dy) * texel;
            vec3 e = texture(shadow_map, uvz.xy + off).rgb;
            float d = dot(e, vec3(1.0, 1.0 / 255.0, 1.0 / 65025.0));
            lit += (uvz.z + dot(grad, off) - bias <= d) ? 1.0 : 0.0;
        }
    }
    return lit / 9.0;
}
vec3 shadow_surface(vec3 rgb){
 vec3 normal=normalize(cross(dFdx(world_position),dFdy(world_position)));
 float ndl=dot(normal,shadow.sun_bias.xyz);
 float lit=ndl>0.?shadow_light():0.;
 return rgb*mix(shadow.parameters.x,1.,clamp(ndl*1.5,0.,1.)*lit);
}

void main(){if(!pre_raster_clip&&(p.clip_enabled&1u)!=0u&&dot(p.plane,vec4(world_position,1.))>0.)discard;
 if((p.clip_enabled&2u)!=0u){color=vec4(0.);return;} vec4 texel=texture(source_texture,tex_uv);
 if(p.opacity>.999&&texel.a<.5){if((p.cutout_height&1u)!=0u)discard;int x=int(gl_FragCoord.x)%4;int y=int(float(p.cutout_height>>1u)-gl_FragCoord.y)%4;if(texel.a<BAYER[y*4+x])discard;color=vec4(texel.rgb*p.shade,1.);}else color=vec4(texel.rgb*p.shade,texel.a*p.opacity);
color.rgb=shadow_surface(color.rgb);}
