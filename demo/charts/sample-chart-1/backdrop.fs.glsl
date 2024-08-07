#version 450

layout (location = 0) in vec3 v_ray_direction;

layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 1) uniform sampler2D env;

layout (set = 1, binding = 0) uniform Uniforms {
    float g_app_time;
    vec3 g_light_dir;
    vec4 args;
    vec4 args2;
    vec3 col1;
    vec3 col2;
};

void main() {
    
    #if IMAGE_BOUND_TO_SAMPLER_ENV
        vec4 c = textureLod(env, v_ray_direction.zx, 7.5);
        f_color = vec4(c.rgb, 1);
    #else 
        float d = dot(normalize(v_ray_direction), g_light_dir);
        d = (d + 1) * 0.5;
        vec3 col = mix(col1, col2, d);
        col = pow(col, args2.rgb);

        f_color = vec4(col, 1);
    #endif
}
