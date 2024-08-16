#version 450

layout (location = 0) in vec2 v_uv;
layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 1) uniform sampler2D base_color;

layout (set = 1, binding = 0) uniform Uniforms {
    vec2 g_pixel_size;
};

#include "gauss_params.glsl"

void main() {
    float base_color_pixel_size = 1.0 / textureSize(base_color, 0).x;
    float uvstep = base_color_pixel_size * POW2MIP;
    vec3 result = vec3(0.0, 0.0, 0.0);
    vec2 d = v_uv - vec2(uvstep * gauss_kernel_size, 0);
    for (int i = 0; i < gauss_kernel_size * 2 + 1; ++i)
    {
        vec3 c = textureLod(base_color, d, MIP).rgb * gauss_weight[i];
        result += c;
        d.x += uvstep;
    }
    f_color = vec4(result, 1.0);
}
