layout (location = 0) in vec3 a_position;
layout (location = 1) in vec3 a_normal;
layout (location = 2) in vec3 a_tangent;
layout (location = 3) in vec2 a_uv;

layout (location = 0) out vec2 v_uv;

layout (set = 0, binding = 0) uniform Uniforms {
    float g_aspect_ratio;
};

void main() {
    gl_Position = vec4(a_position.x, -a_position.z, 0, 1);
    v_uv = vec2(a_position.x, -a_position.z / g_aspect_ratio);
}