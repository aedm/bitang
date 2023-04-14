layout (location = 0) in vec2 v_uv;
layout (location = 1) in vec3 v_normal;
layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 0) uniform Uniforms {
    mat4 g_projection_from_model;
    mat4 g_camera_from_model;
    vec4 color;
} u;

layout (set = 1, binding = 1) uniform sampler2D tex;

void main() {
    vec3 light_dir = normalize(vec3(1.0, -1.0, -1.0));
    float intensity = dot(normalize(v_normal), light_dir);
    float specular = max(pow(dot(normalize(v_normal), light_dir), 5.0), 0.0);
    f_color = texture(tex, v_uv) * intensity * u.color + vec4(1.0, 1.0, 1.0, 0.0) * specular * 0.3;
}
