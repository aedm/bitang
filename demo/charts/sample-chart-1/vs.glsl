#version 450

#include "/shaders/math.glsl"

layout (location = 0) in vec3 a_position;
layout (location = 1) in vec3 a_normal;
layout (location = 2) in vec3 a_tangent;
layout (location = 3) in vec2 a_uv;

layout (location = 0) out vec2 v_uv;
layout (location = 1) out vec3 v_normal_worldspace;
layout (location = 2) out vec3 v_tangent_worldspace;
layout (location = 3) out vec3 v_pos_worldspace;
layout (location = 4) out vec3 v_camera_pos_worldspace;

layout (set = 0, binding = 0) uniform Context {
    mat4 g_projection_from_model;
    mat4 g_camera_from_model;
    mat4 g_camera_from_world;
    mat4 g_world_from_model;
    vec3 g_light_dir_worldspace_norm;
    float g_app_time;
//    float scale;
};
//
//vec3 pal(in float t, in vec3 a, in vec3 b, in vec3 c, in vec3 d)
//{
//    return a + b*cos(6.28318*(c*t+d));
//}

void main() {
    gl_Position = g_projection_from_model * vec4(a_position, 1.0);

    v_uv = a_uv;
    v_normal_worldspace = mat3(g_world_from_model) * a_normal;
    v_tangent_worldspace = mat3(g_world_from_model) * a_tangent;
    v_pos_worldspace = (g_world_from_model * vec4(a_position, 1.0)).xyz;
    v_camera_pos_worldspace = calculate_camera_pos_worldspace(g_camera_from_world);
}