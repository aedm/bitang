#version 450

layout (location = 0) in vec2 v_uv;

layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 1) uniform sampler2D original;

void main() {
    f_color = texture(original, v_uv);
}
