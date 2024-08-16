#version 450

#include "gamma.glsl"

layout (location = 0) in vec2 v_uv;
layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 0) uniform sampler2D base_color;

void main() {
    vec3 color = textureLod(base_color, v_uv, 0.0).rgb;
    color = min(color, 4.0);
    color = max(color - COLOR_BASE_LEVEL, 0);
    color = gamma_compress(color);
    f_color = vec4(color, 1);
}
