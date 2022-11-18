#version 450

layout (location = 0) in vec3 position;
layout (location = 1) in vec3 normal;
layout (location = 2) in vec3 tangent;
layout (location = 3) in vec2 uv;

layout (set = 0, binding = 0) uniform Data {
    mat4 model_to_projection;
    mat4 model_to_camera;
} uniforms;

layout (location = 0) out vec2 v_uv;
layout (location = 1) out vec3 v_normal;

void main() {
    vec2 xy = position.xy;
    gl_Position = uniforms.model_to_projection * vec4(position, 1.0);
    v_uv = uv;
    v_normal = (uniforms.model_to_camera * vec4(normal, 0.0)).xyz;
}