#version 450

layout (location = 0) in vec2 v_uv;
layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 1) uniform sampler2D base_color;

layout (set = 1, binding = 0) uniform Uniforms {
    vec2 g_pixel_size;
    float do_clamp;
};

#include "gauss_params.glsl"

void main() {
    float uvstep = g_pixel_size.y;
    vec3 result = vec3(0, 0, 0);
    vec2 d = v_uv - vec2(0, uvstep * gauss_kernel_size);
    for (int i = 0; i < gauss_kernel_size * 2 + 1; ++i)
    {
        result += texture(base_color, d).rgb * gauss_weight[i];
        d.y += uvstep;
    }
    f_color = vec4(result, 1.0);
}
