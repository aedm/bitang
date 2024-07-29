#version 450

#include "/shaders/math.glsl"

layout (location = 0) in vec3 a_position;
layout (location = 1) in vec3 a_normal;
layout (location = 2) in vec3 a_tangent;
layout (location = 3) in vec2 a_uv;

layout (location = 0) out vec2 v_uv;
layout (location = 1) out vec3 v_normal;
layout (location = 2) out vec3 v_model_pos;
layout (location = 3) out vec3 v_tangent;
layout (location = 4) out vec3 v_light_dir;
layout (location = 5) out vec3 v_world_normal;
layout (location = 6) out vec3 v_world_pos;
layout (location = 7) out vec3 v_world_eye;
layout (location = 8) out vec3 v_world_tangent;
layout (location = 9) out vec3 v_camera_pos;

layout (location = 10) out vec3 v_color;
layout (location = 11) out float v_size;


layout (set = 0, binding = 0) uniform Context {
    mat4 g_projection_from_model;
    mat4 g_camera_from_model;
    mat4 g_camera_from_world;
    mat4 g_world_from_model;
    float g_app_time;
    vec3 g_light_dir;
    float scale;
};

vec3 pal(in float t, in vec3 a, in vec3 b, in vec3 c, in vec3 d)
{
    return a + b*cos(6.28318*(c*t+d));
}

void main() {
    vec3 move = vec3(0);

    vec3 rot = vec3(0, g_app_time * 0.0, 0);
    mat4 tr_mat = translate_matrix(move);
    mat4 rot_mat = rotate_xyz_matrix(rot);

    vec3 pos = a_position;
    vec4 new_pos = tr_mat * rot_mat * vec4(pos, 1.0);
    vec3 new_normal = mat3(tr_mat) * mat3(rot_mat) * a_normal;
    vec3 new_tangent = mat3(tr_mat) * mat3(rot_mat) * a_tangent;

    gl_Position = g_projection_from_model * new_pos;
    v_uv = a_uv;

    v_normal = mat3(g_camera_from_model) * new_normal;
    v_tangent = mat3(g_camera_from_model) * new_tangent;
    v_model_pos = new_pos.xyz;

    v_light_dir = mat3(g_camera_from_world) * g_light_dir;

    mat3 inverse_camera_from_model = mat3(g_camera_from_world);
    v_world_eye = inverse_camera_from_model * -g_camera_from_world[3].xyz;
    v_world_pos = (g_world_from_model * new_pos).xyz;
    v_world_normal = mat3(g_world_from_model) * new_normal;
    v_world_tangent = mat3(g_world_from_model) * new_tangent;
    v_camera_pos = (g_camera_from_model * new_pos).xyz;

    //    v_color = pal(gl_InstanceIndex * 0.1, vec3(0.5,0.5,0.5),vec3(0.5,0.5,0.5),vec3(1.0,1.0,1.0),vec3(0.0,0.10,0.20) );
    //    vec2 pad = particles_current.buf[gl_InstanceIndex]._pad;
    //    v_color = vec3(1, 1-pad.x, 1-pad.y);
    v_color = vec3(1, 1, 1);
}