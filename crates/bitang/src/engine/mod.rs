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

pub const SCREEN_RENDER_TARGET_ID: &str = "screen";

pub use core::compute_call::ComputeCall;
pub use core::context::{
    ComputePassContext, FrameContext, GpuContext, RenderPassContext, Viewport,
};
pub use core::double_buffer::DoubleBuffer;
pub use core::draw_call::{BlendMode, DrawCall, DrawCallProps};
pub use core::globals::{GlobalType, Globals};
pub use core::image::{BitangImage, ImageSizeRule, PixelFormat, SwapchainImage};
pub use core::mesh::Mesh;
pub use core::mipmap_generator::MipmapGenerator;
pub use core::shader::{
    DescriptorResource, DescriptorSource, GlobalUniformMapping, ImageDescriptor,
    LocalUniformMapping, SamplerDescriptor, SamplerMode, Shader, ShaderKind,
};
pub use core::{Size2D, Vertex3, SIMULATION_STEP_SECONDS};

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
