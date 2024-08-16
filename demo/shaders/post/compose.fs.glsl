#version 450

#include "gamma.glsl"

layout (location = 0) in vec2 v_uv;
layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 1) uniform sampler2D base_color;
layout (set = 1, binding = 2) uniform sampler2D glow_map;

layout (set = 1, binding = 0) uniform Uniforms {
    vec2 g_pixel_size;
    float do_clamp;
    vec3 tone_mapping;
    vec3 coloring;
    float glow;
    float glow_pow;
    float g_app_time;
};

void main() {
    vec3 color = textureLod(base_color, v_uv, 0.0).rgb;

    #if IMAGE_BOUND_TO_SAMPLER_GLOW
    vec3 glow_color = textureLod(glow_map, v_uv, 0.0).rgb;
    color = clamp(color, 0, 1) + pow(min(max(color_glow, 0) * glow, 1), vec3(glow_pow));
    #endif

    color = gamma_decompress(color);

    //    vec3 toned = pow(clamp(color, 0, 1), tone_mapping) * coloring;
    //    float dither = fract((v_uv.x * 32 + v_uv.y * 32 + g_app_time * 0.1) * 321.423) / 512;
    //    color = toned + dither;

    // hdr
    //    color = color / (color + vec3(1.0));

    f_color = vec4(color, 1);
}
