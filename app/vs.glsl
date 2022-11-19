layout (location = 0) in vec3 a_position;
layout (location = 1) in vec3 a_normal;
layout (location = 2) in vec3 a_tangent;
layout (location = 3) in vec2 a_uv;

layout (location = 0) out vec2 v_uv;
layout (location = 1) out vec3 v_normal;

void main() {
    gl_Position = cx.model_to_projection * vec4(a_position, 1.0);
    v_uv = a_uv;
    v_normal = (cx.model_to_camera * vec4(a_normal, 0.0)).xyz;
}