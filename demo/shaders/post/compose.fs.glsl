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
};

void main() {
    vec3 color = textureLod(base_color, v_uv, 0.0).rgb;
    color = min(color, 1);
    //    color = gamma_decompress(color);

    #if IMAGE_BOUND_TO_SAMPLER_GLOW_MAP
    vec3 glow_color = textureLod(glow_map, v_uv, 0.0).rgb;
    //    glow_color = gamma_decompress(glow_color);
    //    glow_color = max(glow_color - COLOR_BASE_LEVEL, 0);
    glow_color = pow(glow_color, vec3(glow_pow)) * glow;
    //    color = clamp(color, 0, 1) + max(glow_color, 0);
    color += glow_color;
    #endif


    //    vec3 toned = pow(clamp(color, 0, 1), tone_mapping) * coloring;
    //    float dither = fract((v_uv.x * 32 + v_uv.y * 32 + g_app_time * 0.1) * 321.423) / 512;
    //    color = toned + dither;

    // hdr
    color = color / (color + vec3(0.4));

    f_color = vec4(color, 1);
}
