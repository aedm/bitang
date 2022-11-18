#version 450

layout (location = 0) in vec2 v_uv;
layout (location = 1) in vec3 v_normal;
layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 0) uniform sampler2D tex;

void main() {
    float intensity = dot(normalize(v_normal), vec3(0.0, 0.0, 1.0));
    float specular = max(pow(dot(normalize(v_normal), normalize(vec3(1.0, 1.0, 1.5))), 5.0), 0.0);
    f_color = texture(tex, v_uv) * intensity + vec4(1.0, 1.0, 1.0, 0.0) * specular * 0.3;
}