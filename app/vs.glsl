layout (location = 0) in vec3 a_position;
layout (location = 1) in vec3 a_normal;
layout (location = 2) in vec3 a_tangent;
layout (location = 3) in vec2 a_uv;

layout (location = 0) out vec2 v_uv;
layout (location = 1) out vec3 v_normal;

layout (set = 0, binding = 0) uniform Context {
    mat4 g_projection_from_model;
    mat4 g_camera_from_model;
    float extrude;
} u;

void main() {
    vec3 pos = a_position + a_normal * u.extrude * 0.25;
    gl_Position = u.g_projection_from_model * vec4(pos, 1.0);
    v_uv = a_uv;
    v_normal = (u.g_camera_from_model * vec4(a_normal, 0.0)).xyz;
}