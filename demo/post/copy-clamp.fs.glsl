#version 450

layout (location = 0) in vec2 v_uv;

layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 1) uniform sampler2D original;

void main() {
    vec3 c = texture(original, v_uv).rgb;
    c = clamp(c-vec3(1), 0, 10);
    f_color = vec4(c, 1);
}
