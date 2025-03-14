struct Particle {
    position: vec3<f32>,
    velocity: vec3<f32>,
    upvector: vec3<f32>,
}

@group(0) @binding(1)
var<storage, read_write> particles: array<Particle>;

fn init_particle(iv: f32) -> Particle {
    let pos = vec3<f32>(cos(iv), 0.0, sin(iv * 2.2342));
    let vel = vec3<f32>(0.0, 0.0, 0.0);
    let up = vec3<f32>(0.0, 1.0, 0.0);
    
    var particle: Particle;
    particle.position = pos;
    particle.velocity = vel;
    particle.upvector = up;
    
    return particle;
}

@compute @workgroup_size(64, 1, 1)
fn cs_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    
    // Get the length of the particles array
    let num_particles = arrayLength(&particles);
    
    if (index >= num_particles) {
        return;
    }
    
    particles[index] = init_particle(f32(index));
}
