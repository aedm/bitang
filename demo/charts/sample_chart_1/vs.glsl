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

layout (location = 5) out vec3 v_material_adjustment;

layout (set = 0, binding = 0) uniform Context {
    mat4 g_projection_from_world;
    mat4 g_projection_from_model;
    mat4 g_camera_from_model;
    mat4 g_camera_from_world;
    mat4 g_world_from_model;
    vec3 g_light_dir_worldspace_norm;
    float g_app_time;

    vec3 instance_move;
};

void main() {
    const int per_row = 8;
    vec3 mi = vec3(gl_InstanceIndex % per_row, gl_InstanceIndex / per_row, 0);
    vec3 move = instance_move * (mi - vec3((per_row -1.0)/ 2.0, 0, 0));
    v_pos_worldspace = (g_world_from_model * vec4(a_position, 1.0)).xyz + move;

    gl_Position = g_projection_from_world * vec4(v_pos_worldspace, 1.0);

    v_uv = a_uv;
    v_normal_worldspace = mat3(g_world_from_model) * a_normal;
    v_tangent_worldspace = mat3(g_world_from_model) * a_tangent;
    v_camera_pos_worldspace = calculate_camera_pos_worldspace(g_camera_from_world);

    v_material_adjustment = vec3(0.99 - mi.x / (per_row-1.0), mi.y / 2.0, 0.0);
}