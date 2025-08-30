import package::shaders::quaternion::Quaternion;

struct Particle {
    position: vec3<f32>,
    velocity: vec3<f32>,
    rotation_quat: Quaternion,
}
