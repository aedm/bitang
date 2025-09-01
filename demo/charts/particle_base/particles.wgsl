import package::shaders::quaternion::q_from_axis_angle;
import super::particle::Particle;

struct Context {
    g_simulation_step_seconds: f32,
    time: f32,
    
    rot1_center: vec3<f32>,
    rot1_axis: vec3<f32>,
    rot1_speed: f32,
    
    grav1_center: vec3<f32>,
    grav1_strength: vec3<f32>,
    
    grav2_center: vec3<f32>,
    grav2_strength: vec3<f32>,
    
    flux1_a: vec4<f32>,
    flux1_b: vec4<f32>,
    grav_center_speed: f32,
    
    brake: f32,
    _pad: vec4<f32>,
}

@group(0) @binding(0) var<uniform> context: Context;
@group(0) @binding(1) var<storage, read> particles_current: array<Particle>;
@group(0) @binding(2) var<storage, read_write> particles_next: array<Particle>;


fn rotate_axis_matrix(axis: vec3<f32>, angle: f32) -> mat3x3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    let t = 1.0 - c;
    let x = axis.x;
    let y = axis.y;
    let z = axis.z;

    return mat3x3<f32>(
        vec3<f32>(t*x*x + c, t*x*y - s*z, t*x*z + s*y),
        vec3<f32>(t*x*y + s*z, t*y*y + c, t*y*z - s*x),
        vec3<f32>(t*x*z - s*y, t*y*z + s*x, t*z*z + c)
    );
}

fn rotator_force(pos: vec3<f32>, center: vec3<f32>, axis: vec3<f32>, speed: f32) -> vec3<f32> {
    let p = pos - center;
    let rot = rotate_axis_matrix(axis, speed * context.g_simulation_step_seconds);
    let p2 = rot * p;
    let d = length(p);
    let f = d / (d + 1.0);
    return (p2 - p) * f;
}

fn pull_force(pos: vec3<f32>, center: vec3<f32>, strength: vec3<f32>) -> vec3<f32> {
    let p = pos - center;
    let d = length(p);
    let f = 1.0 - pow((d + 0.1) / (d + 1.0), strength.y);
    return p * -d * f * strength.x * context.g_simulation_step_seconds;
}

fn repel_force(pos: vec3<f32>, center: vec3<f32>, strength: vec3<f32>) -> vec3<f32> {
    let p = pos - center;
    let d = length(p);
    let f = pow(max(1.0 - d/strength.y, 0.0), strength.z + 1.0) * strength.x;
    return p / d * f * context.g_simulation_step_seconds;
}

fn flux_force(pos: vec3<f32>, a: vec4<f32>, b: vec4<f32>, index: f32) -> vec3<f32> {
    let p = pos;
    let vx = cos(p.y * a.x + p.z * a.y + index);
    let vy = cos(p.z * a.x + p.x * a.y + index);
    let vz = cos(p.x * a.x + p.y * a.y + index);
    return vec3<f32>(vx, vy, vz) * a.w * context.g_simulation_step_seconds;
}

fn step_particle(p: Particle, index: f32) -> Particle {
    var pos = p.position;
    var vel = p.velocity;

    let t = context.time;
    let c = vec3<f32>(cos(context.grav2_center * t)) * context.grav_center_speed;

    vel += rotator_force(pos, c, context.rot1_axis, context.rot1_speed);
    vel += pull_force(pos, c, context.grav1_strength);
    vel += repel_force(pos, c, context.grav2_strength);
    vel += flux_force(pos, context.flux1_a, context.flux1_b, index);

    pos += vel;
    vel *= (1.0 - context.brake);

    // let pos = vec3<f32>(cos(index+t), 0.0, sin(index * 2.2342 + t));
    let rot = q_from_axis_angle(vec3<f32>(0.0, 1.0, 0.0), index * 2.2342 + t);

    var result: Particle;
    result.position = pos;
    result.velocity = vel;
    result.rotation_quat = rot;
    return result;
}

@compute @workgroup_size(64, 1, 1)
fn cs_simulate(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    let num_particles = arrayLength(&particles_current);

    if (index >= num_particles) {
        return;
    }

    let p = particles_current[index];
    let next = step_particle(p, f32(index));
    particles_next[index] = next;
}

fn init_particle(iv: f32) -> Particle {
    let pos = vec3<f32>(cos(iv), 0.0, sin(iv * 2.2342));
    let vel = vec3<f32>(0.0, 0.0, 0.0);
    let up = vec3<f32>(0.0, 1.0, 0.0);
    let rotation_quat = q_from_axis_angle(up, 0.0);
    
    var particle: Particle;
    particle.position = pos;
    particle.velocity = vel;
    particle.rotation_quat = rotation_quat;
    
    return particle;
}

@compute @workgroup_size(64, 1, 1)
fn cs_init(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    
    // Get the length of the particles array
    let num_particles = arrayLength(&particles_next);
    
    if (index >= num_particles) {
        return;
    }
    
    particles_next[index] = init_particle(f32(index));
}