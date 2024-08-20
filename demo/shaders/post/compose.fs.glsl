#version 450

#include "gamma.glsl"

layout (location = 0) in vec2 v_uv;
layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 1) uniform sampler2D base_color;
layout (set = 1, binding = 2) uniform sampler2D glow_map;

layout (set = 1, binding = 0) uniform Uniforms {
    vec2 g_pixel_size;
    vec3 tone_mapping;
    vec3 coloring;
    float glow;
    float glow_pow;
    float g_app_time;
    float hdr_adjust;
};

float noise2d(vec2 co) {
    return fract(sin(dot(co.xy, vec2(12.9898, 78.233))) * 43758.5453);
}

void main() {
    // base color
    vec3 color = textureLod(base_color, v_uv, 0.0).rgb;
    color = min(color, 1);

    // glow
    #if IMAGE_BOUND_TO_SAMPLER_GLOW_MAP
    vec3 glow_color = textureLod(glow_map, v_uv, 0.0).rgb;
    glow_color = gamma_decompress(glow_color);
    glow_color = pow(glow_color, vec3(glow_pow)) * glow;
    color += glow_color;
    #endif

    // tone mapping
    color = pow(clamp(color, 0, 1), tone_mapping) * coloring;

    // hdr
    color = color / (color + vec3(1-hdr_adjust));

    // dither
    float dither = noise2d(v_uv * 512 + g_app_time*10) / 512;
    color += dither;

    f_color = vec4(color, 1);
}
