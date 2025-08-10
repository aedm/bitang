#version 450

layout (location = 0) in vec3 a_position;
layout (location = 1) in vec3 a_normal;
layout (location = 2) in vec3 a_tangent;
layout (location = 3) in vec2 a_uv;

layout (location = 0) out vec3 v_ray_direction;

layout (set = 0, binding = 0) uniform Uniforms {
    mat4 g_projection_from_camera;
    mat4 g_camera_from_world;
} u;

vec3 calculate_backdrop_ray(vec2 uv) {
    vec2 fov = vec2(1 / u.g_projection_from_camera[0][0], 1 / u.g_projection_from_camera[1][1]);
    mat3 inverse_rotation = inverse(mat3(u.g_camera_from_world));
    return inverse_rotation * vec3(uv * fov, 1);
}

void main() {
    gl_Position = vec4(a_position.x, -a_position.z, 0, 1);
    vec2 screen_uv = a_uv * 2 - 1;
    v_ray_direction = calculate_backdrop_ray(screen_uv);
}