mod camera;
mod chart;
mod compute;
mod control;
mod core;
mod draw;
mod generate_mip_levels;
mod material;
mod pass;
mod project;
mod render_object;
mod scene;
mod graph;

pub use core::compute_call::ComputeCall;
pub use core::context::{
    ComputePassContext, FrameContext, GpuContext, RenderPassContext, RenderPassDrawBatch, Viewport,
};
pub use core::double_buffer::DoubleBuffer;
pub use core::draw_call::{BlendMode, DrawCall, DrawCallProps};
pub use core::globals::{GlobalType, Globals};
pub use core::image::{BitangImage, ImageSizeRule, PixelFormat};
pub use core::mesh::Mesh;
pub use core::mipmap_generator::MipmapGenerator;
pub use core::shader::{
    DescriptorResource, DescriptorSource, GlobalUniformMapping, ImageDescriptor,
    LocalUniformMapping, SamplerDescriptor, SamplerMode, Shader, ShaderKind,
};
pub use core::{Size2D, Vertex3};

pub use camera::Camera;
pub use chart::{Chart, ChartStep};
pub use compute::{Compute, Run};
pub use control::controls::{
    Control, ControlRepository, ControlSet, ControlSetBuilder, UsedControlsNode,
};
pub use control::spline::SplinePoint;
pub use control::{ControlId, ControlIdPartType};
pub use draw::{Draw, DrawItem};
pub use generate_mip_levels::GenerateMipLevels;
pub use material::Material;
pub use pass::{FramebufferInfo, Pass};
pub use project::{Cut, Project};
pub use render_object::RenderObject;
pub use scene::Scene;

/// How many times the simulation is updated per second.
/// Weird number on purpose so issues are easier to spot.
const SIMULATION_FREQUENCY_HZ: f32 = 53.526621;
pub const SIMULATION_STEP_SECONDS: f32 = 1.0 / SIMULATION_FREQUENCY_HZ;
