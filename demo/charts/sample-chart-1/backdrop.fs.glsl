#version 460

//#include "/shaders/image_based_lighting.glsl"
layout (set = 1, binding = 2) uniform sampler envmap_sampler;

const float PI = 3.1415926535f;

// Center of image is mapped to z-forward
vec2 direction_wn_to_spherical_envmap_uv(vec3 direction_wn) {
    // Calculate the azimuthal angle (phi) and the polar angle (theta)
    float phi = atan(direction_wn.z, direction_wn.x);
    float theta = acos(direction_wn.y);

    // Convert angles to UV coordinates
    float u = phi / (2.0 * PI) + 0.25;
    float v = theta / PI;

    return vec2(u, v);
}

vec4 sample_environment_map(vec3 direction_wn, float bias, texture2D envmap) {
    int levels = textureQueryLevels(envmap);
    float adjust = pow(1.0 - bias, 4.0);
    float mipLevel = max(float(levels) - 3.5 - adjust * 7.0, 0.0);
    vec2 uv = direction_wn_to_spherical_envmap_uv(direction_wn);
    return textureLod(envmap, uv, mipLevel);
}

vec3 sample_srgb_as_linear(sampler2D map, vec2 uv) {
    vec3 v = texture(map, uv).rgb;
    return pow(v, vec3(1.0 / 2.2));
}

vec3 apply_normal_map_amount(sampler2D normal_map, vec2 uv, vec3 normal_n, vec3 tangent_n, float normal_strength) {
    mat3 normal_space = mat3(tangent_n, cross(normal_n, tangent_n), normal_n);
    vec3 n = sample_srgb_as_linear(normal_map, uv).rgb;
    n = normalize(n * 2.0 - 1.0);
    n = normal_space * n;
    return normalize(mix(normal_n, n, normal_strength));
}

mat3 make_lightspace_from_worldspace_transformation(vec3 light_dir_worldspace_n) {
    // Axes of lightspace expressed in worldspace
    vec3 light_z = light_dir_worldspace_n;
    vec3 light_x = normalize(cross(light_z, vec3(0, 1, 0)));
    vec3 light_y = cross(light_x, light_z);

    // Orthonormal transformation from lightspace to worldspace
    mat3 world_from_light = mat3(light_x, light_y, light_z);

    // We need the inverse to transform worldspace vectors to lightspace.
    // For orthonormal vectors, transpose is the same as inverse but cheaper.
    mat3 light_from_world = transpose(world_from_light);

    return light_from_world;
}


layout (location = 0) in vec3 v_ray_direction;

layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 0) uniform Uniforms {
    float g_app_time;
    vec3 g_light_dir;
    vec4 args;
    vec4 args2;
    vec3 col1;
    vec3 col2;
};

layout (set = 1, binding = 1) uniform texture2D envmap;

void main() {
    //    f_color = vec4(1, 0, 1, 1);

    vec4 c = sample_environment_map(normalize(v_ray_direction), -1, envmap);
    f_color = vec4(c.rgb * args.r, 1);
}
