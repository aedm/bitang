layout (location = 0) in vec2 v_uv;

layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 0) uniform Uniforms {
    float fade;
    vec2 bah;
};

void main() {
    float a = clamp(length(v_uv) * (-2.5 + bah.y) - (-3.3 + bah.x), 0, 1);
    float b = clamp(1 - fade, 0, 1);
    f_color = vec4(0, 0, 0, 1 - (a * b * b));
}
