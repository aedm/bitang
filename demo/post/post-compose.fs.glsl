#version 450

layout (location = 0) in vec2 v_uv;
layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 1) uniform sampler2D original;
layout (set = 1, binding = 2) uniform sampler2D blurred;

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
    vec4 c1 = texture(original, v_uv);
    vec4 c2 = texture(blurred, v_uv);
    vec4 combined = clamp(c1, 0, 1) + pow(min(max(c2, 0) * glow, 1), vec4(glow_pow));
    vec3 toned = pow(clamp(combined.rgb, 0, 1), tone_mapping) * coloring;
    float dither = fract((v_uv.x * 32 + v_uv.y * 32 + g_app_time * 0.1) * 321.423) / 512;
    f_color = vec4(toned + dither, 1);
}
