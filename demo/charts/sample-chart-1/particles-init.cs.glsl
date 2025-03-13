#version 450

layout(local_size_x = 64, local_size_y = 1, local_size_z = 1) in;

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
} particles;


Particle init_particle(float iv) {
    vec3 pos = vec3(cos(iv), 0, sin(iv*2.2342));
    vec3 vel = vec3(0, 0, 0);
    vec3 up = vec3(0, 1, 0);
    return Particle(pos, vel, up);
}

void main() {
    uint num_particles = particles.buf.length();

    // Calculate the new position and velocity of the particle.
    uint index = gl_GlobalInvocationID.x;
    if (index >= num_particles) {
        return;
    }

    particles.buf[index] = init_particle(index);
}
