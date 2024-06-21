mat4 translate(vec3 translateVector) {
    mat4 matrix;
    matrix[0] = vec4(1, 0, 0, 0);
    matrix[1] = vec4(0, 1, 0, 0);
    matrix[2] = vec4(0, 0, 1, 0);
    matrix[3] = vec4(translateVector, 1);
    return matrix;
}

mat4 rotate_x(float angle) {
    float s = sin(angle);
    float c = cos(angle);
    mat4 matrix;
    matrix[0] = vec4(1, 0, 0, 0);
    matrix[1] = vec4(0, c, s, 0);
    matrix[2] = vec4(0, -s, c, 0);
    matrix[3] = vec4(0, 0, 0, 1);
    return matrix;
}

mat4 rotate_y(float angle) {
    float s = sin(angle);
    float c = cos(angle);
    mat4 matrix;
    matrix[0] = vec4(c, 0, s, 0);
    matrix[1] = vec4(0, 1, 0, 0);
    matrix[2] = vec4(-s, 0, c, 0);
    matrix[3] = vec4(0, 0, 0, 1);
    return matrix;
}

mat4 rotate_z(float angle) {
    float s = sin(angle);
    float c = cos(angle);
    mat4 matrix;
    matrix[0] = vec4(c, s, 0, 0);
    matrix[1] = vec4(-s, c, 0, 0);
    matrix[2] = vec4(0, 0, 1, 0);
    matrix[3] = vec4(0, 0, 0, 1);
    return matrix;
}

mat4 rotate(vec3 angle) {
    return rotate_x(angle.x) * rotate_y(angle.y) * rotate_z(angle.z);
}

void calculate_ray(mat4 projection_from_camera, mat4 camera_from_world, vec2 uv, out vec3 eye, out vec3 dir) {
    vec2 fov = vec2(1 / projection_from_camera[0][0], 1 / projection_from_camera[1][1]);
    mat3 inverse_rotation = inverse(mat3(camera_from_world));
    eye = inverse_rotation * -camera_from_world[3].xyz;
    dir = inverse_rotation * vec3(uv * fov, 1);
}

float depth_sample_to_z(float z_near, float buffer_value) {
    return z_near / (1 - buffer_value);
}


float dist_from_plane(vec3 eye, vec3 dir, vec3 plane_normal, vec3 plane_center, float circle_radius, float buffer_depth) {
    float dist_from_plane = dot(plane_normal, plane_center - eye) / dot(plane_normal, dir);
    if (dist_from_plane < 0 || dist_from_plane > buffer_depth) {
        return 10000;
    }
    vec3 plane_intersection = eye + dist_from_plane * dir;
    float dist_from_center = length(plane_intersection - plane_center);
    return dist_from_center - circle_radius;
}

vec4 traced_color(vec3 eye, vec3 dir, vec3 circle_plane_normal, vec3 circle_center, vec4 circle_args, vec4 circle_color, float buffer_depth, out float dist) {
    dist = dist_from_plane(eye, dir, circle_plane_normal, circle_center, circle_args.x, buffer_depth);
    float intensity = 0;
    if (dist < 0) {
        intensity = -dist * circle_args.y;
    } else {
        intensity = dist * circle_args.z;
    }
    intensity = clamp(1-intensity, 0, 1);
    intensity = pow(intensity, circle_args.w);

    return vec4(circle_color.rgb, intensity * circle_color.a);
}

const float gauss_extrude = 0.5;
const int gauss_kernel_size = 20;
float gauss_weight[gauss_kernel_size * 2 + 1] = float[]
(0.0003,	0.0004,	0.0007,	0.0012,	0.0019,	0.0029,	0.0044,	0.0064,	0.0090,	0.0124,	0.0166,	0.0216,	0.0274,	0.0337,	0.0404,	0.0470,	0.0532,	0.0587,	0.0629,	0.0655,	0.0665,	0.0655,	0.0629,	0.0587,	0.0532,	0.0470,	0.0404,	0.0337,	0.0274,	0.0216,	0.0166,	0.0124,	0.0090,	0.0064,	0.0044,	0.0029,	0.0019,	0.0012,	0.0007,	0.0004,	0.0003);



mat4 sdf_model_from_camera;
float l_obj_sphere_radius;
float l_obj_transition;

float sd_round_box(vec3 pos_ms, vec3 size, float radius) {
    vec3 q = abs(pos_ms) - size;
    return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0) - radius;
}

float sd_sphere(vec3 pos_ms, float radius) {
    float sd_sphere = length(pos_ms) - radius;
    float sd_box = sd_round_box(pos_ms, vec3(radius * 0.67), 0.02);
    return sd_sphere + l_obj_transition * (sd_box - sd_sphere);
}

float sd(vec3 pos_cs) {
    vec3 pos_ms = (sdf_model_from_camera * vec4(pos_cs, 1)).xyz;
    return sd_sphere(pos_ms, l_obj_sphere_radius);
}