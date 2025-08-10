#version 450

layout(local_size_x = 64, local_size_y = 1, local_size_z = 1) in;

layout (set = 0, binding = 0) uniform Context {
    float g_simulation_step_seconds;
    float time;

    vec3 rot1_center;
    vec3 rot1_axis;
    float rot1_speed;

    vec3 grav1_center;
    vec3 grav1_strength;

    vec3 grav2_center;
    vec3 grav2_strength;

    vec4 flux1_a;
    vec4 flux1_b;
    float grav_center_speed;

    float brake;
};

struct Particle {
    vec3 position;
    // 1 pad byte

    vec3 velocity;
    // 1 pad byte

    vec3 upvector;
    // 1 pad byte
};


layout (set = 0, binding = 1) buffer ParticlesCurrent {
    Particle buf[];
} particles_current;

layout (set = 0, binding = 2) buffer writeonly ParticlesNext {
    Particle buf[];
} particles_next;

#include "/shaders/math.glsl"

vec3 rotator_force(vec3 pos, vec3 center, vec3 axis, float speed) {
    vec3 p = pos - center;
    mat3 rot = mat3(rotate_axis_matrix(axis, speed * g_simulation_step_seconds));
    vec3 p2 = rot * p;
    float d = length(p);
    float f = d / (d+1);
    return (p2 - p) * f;
}

vec3 pull_force(vec3 pos, vec3 center, vec3 strength) {
    vec3 p = pos - center;
    float d = length(p);
    //float f = max(d/strength.y, 1);
    float f = 1 - pow((d+0.1) / (d+1), strength.y);
    return p * -d * f * strength.x * g_simulation_step_seconds;
}

vec3 repel_force(vec3 pos, vec3 center, vec3 strength) {
    vec3 p = pos - center;
    float d = length(p);
    float f = pow(max(1 - d/strength.y, 0), strength.z + 1) * strength.x;
    return p / d * f * g_simulation_step_seconds;
}

vec3 flux_force(vec3 pos, vec4 a, vec4 b, float index) {
    vec3 p = pos;
    float vx = cos(p.y * a.x + p.z * a.y + index);
    float vy = cos(p.z * a.x + p.x * a.y + index);
    float vz = cos(p.x * a.x + p.y * a.y + index);
    return vec3(vx, vy, vz) * a.w  * g_simulation_step_seconds;
}

Particle step(Particle p, float index) {
    vec3 pos = p.position;
    vec3 vel = p.velocity;

    float t = time;
    vec3 c = vec3(cos(grav2_center *t)) * grav_center_speed;

    vel += rotator_force(pos, c, rot1_axis, rot1_speed);
    vel += pull_force(pos, c, grav1_strength);
    vel += repel_force(pos, c, grav2_strength);
    vel += flux_force(pos, flux1_a, flux1_b, index);

    pos += vel;
    vel *= (1-brake);

    vec3 up = p.upvector;

    return Particle(pos, vel, up);
}


void main() {
    uint num_particles = particles_current.buf.length();

    // Calculate the new position and velocity of the particle.
    uint index = gl_GlobalInvocationID.x;
    if (index >= num_particles) {
        return;
    }

//    vec3 pos = particles_current.buf[index].position;
//    vec3 vel = particles_current.buf[index].velocity;
//    vec3 up = particles_current.buf[index].upvector;

    Particle p = particles_current.buf[index];

    Particle next = step(p, index);

    particles_next.buf[index] = next;
}


