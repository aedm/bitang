import package::shaders::quaternion::{q_from_axis_angle, q_rotate};
import super::particle::Particle;

struct Context {
    g_simulation_step_seconds: f32,
    time: f32,

    seed_size: vec3f,
    
    selector_1_ty: f32,
    selector_1_center: vec3<f32>,
    selector_1_strength: vec3<f32>,
    selector_1_args: vec4<f32>,

    field_1_ty: f32,
    field_1_center: vec3<f32>,
    field_1_args1: vec4<f32>,
    field_1_args2: vec4<f32>,


    
    brake: f32,
    _pad: vec4<f32>,
}

@group(0) @binding(0) var<uniform> context: Context;
@group(0) @binding(1) var<storage, read> particles_current: array<Particle>;
@group(0) @binding(2) var<storage, read_write> particles_next: array<Particle>;




// Calculates selector strength for a given distance.
// strength.x: damping (0.0 = no damping)
// strength.y: max distance
// strength.z: falloff exponent
fn calc_selector_strength(distance: f32, strength: vec3f) -> f32 {
    let amplify = strength.x;
    let distance_max = strength.y;
    let exponent = strength.z + 1.0;
    let value = pow(max(0.0, 1.0 - distance / distance_max), exponent);
    return value * amplify;
}

// Selector function. Defines a strength for each point in space.
// Returns a value between 0 and 1 for each particle.
const SELECTOR_EVERYWHERE: u32 = 0;
const SELECTOR_SPHERE: u32 = 1;  // args: x=radius
const SELECTOR_PLANE: u32 = 2;  // args: xyz=normal
const SELECTOR_TUBE: u32 = 3;   // args: xyz=axis, w=radius
fn selector(particle_pos: vec3f, ty: u32, center: vec3f, strength: vec3f, args: vec4f) -> f32 {
    switch ty {
        case SELECTOR_EVERYWHERE: { return strength.x; }
        case SELECTOR_SPHERE: {
            let radius = args.x;
            // let distance = max(0.0, 1.0 - length(particle_pos - center) / radius);
            let distance = length(particle_pos - center);
            return calc_selector_strength(distance, strength);
        }
        case SELECTOR_PLANE: {
            let normal = normalize(args.xyz);
            let distance = abs(dot(particle_pos - center, normal));
            return calc_selector_strength(distance, strength);
        }
        case SELECTOR_TUBE: {
            let axis = normalize(args.xyz);
            let radius = args.w;
            let distance = max(0.0, length(cross(particle_pos - center, axis)) - radius);
            return calc_selector_strength(distance, strength);
        }
        default: { return 1.0; }
    }
}


// Field function
// Returns the force applied to a particle at a given position.
const FIELD_ROTATE_AXIS: u32 = 0;  // args1: xyz=axis
const FIELD_ATTRACT_POINT: u32 = 1;
const FIELD_ATTRACT_AXIS: u32 = 2; // args1: xyz=axis
const FIELD_ATTRACT_PLANE: u32 = 3; // args1: xyz=normal
const FIELD_FLUX: u32 = 4;  // args1, args2
const FIELD_DIRECTION: u32 = 5; // args1: xyz=direction
fn field(particle_pos: vec3f, ty: u32, center: vec3f, args1: vec4f, args2: vec4f, selector_strength: f32,sim_step: f32) -> vec3f {
    switch ty {
        case FIELD_ROTATE_AXIS: { 
            let axis = normalize(args1.xyz);
            let pull_to_center = args1.w;
            // let rotation_quat = q_from_axis_angle(axis, speed * sim_step);

            let center_to_projected = dot(particle_pos - center, axis) * axis;
            let axis_to_point = particle_pos - center_to_projected;

            // let rotated_pos = q_rotate(axis_to_point, rotation_quat) * pull_to_center;
            // return (rotated_pos - axis_to_point);
            
            let rotation_dir = cross(axis, normalize(axis_to_point));
            return rotation_dir - pull_to_center * axis_to_point;
        }
        case FIELD_ATTRACT_POINT: { 
            let direction = normalize(particle_pos - center);
            return direction;
        }
        case FIELD_ATTRACT_AXIS: { 
            let axis = normalize(args1.xyz);
            let center_to_projected = dot(particle_pos - center, axis) * axis;
            let axis_to_point = particle_pos - center_to_projected;
            return normalize(axis_to_point);
        }
        case FIELD_ATTRACT_PLANE: { 
            let normal = normalize(args1.xyz);
            let normal_projected = dot(particle_pos - center, normal) * normal;
            return normalize(normal_projected);
        }
        case FIELD_FLUX: {
            let p = particle_pos;
            let a = args1;
            let index = 0.0;
            let vx = cos(p.y * a.x + p.z * a.y + index);
            let vy = cos(p.z * a.x + p.x * a.y + index);
            let vz = cos(p.x * a.x + p.y * a.y + index);
            return vec3<f32>(vx, vy, vz) * a.w;
        }
        case FIELD_DIRECTION: { 
            return normalize(args1.xyz);
        }
        default: { return vec3<f32>(0.0, 0.0, 0.0); }
    };
}


fn step_particle(p: Particle, index: f32) -> Particle {
    var pos = p.position;
    var vel = p.velocity;
    var rot = p.rotation_quat;

    let selector_strength = selector(pos, u32(context.selector_1_ty), context.selector_1_center, 
        context.selector_1_strength, context.selector_1_args);
    let force = field(pos, u32(context.field_1_ty), context.field_1_center, context.field_1_args1, 
        context.field_1_args2, selector_strength, context.g_simulation_step_seconds);

    vel += force * selector_strength;
    pos += vel * context.g_simulation_step_seconds;

    var result: Particle;
    result.position = pos;
    result.velocity = vel;
    result.rotation_quat = rot;
    return result;
}


@compute @workgroup_size(64, 1, 1)
fn cs_simulate(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= arrayLength(&particles_next)) {
        return;
    }

    let p = particles_current[index];
    let next = step_particle(p, f32(index));
    particles_next[index] = next;
}

@compute @workgroup_size(64, 1, 1)
fn cs_init(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= arrayLength(&particles_next)) {
        return;
    }

    let iv = f32(index);
    let pos = vec3<f32>(cos(iv), cos(iv * 2.913424), sin(iv * 2.2342)) * context.seed_size;
    let vel = vec3<f32>(0.0, 0.0, 0.0);
    let up = vec3<f32>(0.0, 1.0, 0.0);
    let rotation_quat = q_from_axis_angle(up, 0.0);
    
    var particle: Particle;
    particle.position = pos;
    particle.velocity = vel;
    particle.rotation_quat = rotation_quat;

    particles_next[index] = particle;
}