#version 450

#include "/shaders/common.glsl"

layout (location = 0) in vec2 v_uv;
layout (location = 1) in vec3 v_normal;
layout (location = 2) in vec3 v_model_pos;
layout (location = 3) in vec3 v_tangent;
layout (location = 4) in vec3 v_light_dir;
layout (location = 5) in vec3 v_world_normal;
layout (location = 6) in vec3 v_world_pos;
layout (location = 7) in vec3 v_world_eye;
layout (location = 8) in vec3 v_world_tangent;
layout (location = 9) in vec3 v_camera_pos;
layout (location = 10) in vec3 v_color;
layout (location = 11) in float v_size;

layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 0) uniform Uniforms {
    mat4 g_light_projection_from_world;
    mat4 g_camera_from_world;
    mat4 g_projection_from_camera;
    float g_chart_time;
    float g_app_time;
    vec3 g_light_dir;

    vec4 color;
    float shadow_bias;
    float ambient;
    float normal_factor;

    vec3 specular_color;
};

layout (set = 1, binding = 1) uniform sampler2D envmap;
layout (set = 1, binding = 2) uniform sampler2D shadow;


const float DISTANCE_MAX = 100;
const float SURFACE_PROXIMITY = 0.0001;

float calculate_light(vec3 world_pos) {
    vec3 lightspace_pos = (g_light_projection_from_world * vec4(world_pos, 1.0)).xyz;
    lightspace_pos.xy = lightspace_pos.xy * 0.5 + 0.5;
    float shadow_z = texture(shadow, lightspace_pos.xy).r;
    return (shadow_z + shadow_bias*0.001 > lightspace_pos.z) ? 1.0 : 0.0;
}

vec3 light_pixel(vec3 pos, vec3 normal, float ambient_occlusion, float ambient, vec3 base_color, float light_min) {
    float light = calculate_light(pos);
    light = max(light, light_min);
    //    light *= ambient_occlusion * ambient_occlusion;

    // Calculate environment map
    vec3 eye_dir = normalize(pos - v_world_eye);
    vec3 reflect_dir = reflect(eye_dir, normal);
    vec2 reflect_uv = reflect_dir.xy * 0.5 + 0.5;
    vec3 env_color = vec3(1);//texture(env_map, reflect_uv).rgb;

    vec3 envmap_color = sample_environment_map(normal, 6.5, envmap).rgb;

    // Light components
    vec3 light_dir = normalize(g_light_dir);
    float diffuse = max(dot(normal, light_dir), 0) * light;
    float specular = max(pow(dot(normal, light_dir), 5.0), 0.00) * (light + 0.1);

    vec3 final_color = base_color * envmap_color * (ambient + (1-ambient) * diffuse) + specular * env_color * specular_color;

    return final_color;
}

void main() {
    //    f_color = vec4(v_world_normal, 1);
    //    return;

    vec3 base_color = color.rgb;

    // Calculate final color
    vec3 color = light_pixel(v_world_pos, v_world_normal, 1.0, ambient, base_color, 0.0);
    f_color = vec4(color, 1.0);
}
