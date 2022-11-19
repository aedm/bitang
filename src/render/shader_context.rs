use bytemuck::{Pod, Zeroable};
use glam::Mat4;

#[derive(Default, Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct ContextUniforms {
    pub model_to_projection: Mat4,
    pub model_to_camera: Mat4,
}
