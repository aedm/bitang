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

pub use camera::Camera;
pub use chart::Chart;
pub use chart::ChartStep;
pub use compute::Compute;
pub use compute::Run;
pub use draw::Draw;
pub use draw::DrawItem;
pub use generate_mip_levels::GenerateMipLevels;
pub use pass::FramebufferInfo;
pub use pass::Pass;
pub use project::Cut;
pub use project::Project;
pub use render_object::RenderObject;
pub use scene::Scene;

pub use material::Material;

pub const SCREEN_RENDER_TARGET_ID: &str = "screen";

pub use core::compute_call::ComputeCall;
pub use core::double_buffer::DoubleBuffer;
pub use core::draw_call::DrawCall;
pub use core::draw_call::DrawCallProps;
pub use core::image::PixelFormat;
pub use core::mesh::Mesh;
pub use core::mipmap_generator::MipmapGenerator;
pub use core::shader::Shader;
pub use core::shader::ShaderKind;
pub use core::Size2D;

pub use core::image::BitangImage;
pub use core::image::ImageSizeRule;
pub use core::image::SwapchainImage;

pub use core::draw_call::BlendMode;
pub use core::Vertex3;

// DescriptorResource, DescriptorSource, ImageDescriptor, LocalUniformMapping, SamplerDescriptor,
//     Shader, ShaderKind,

pub use core::shader::DescriptorResource;
pub use core::shader::DescriptorSource;
pub use core::shader::GlobalUniformMapping;
pub use core::shader::ImageDescriptor;
pub use core::shader::LocalUniformMapping;
pub use core::shader::SamplerDescriptor;
pub use core::shader::SamplerMode;

pub use core::SIMULATION_STEP_SECONDS;

pub use core::context::{
    ComputePassContext, FrameContext, GpuContext, RenderPassContext, Viewport,
};

pub use core::globals::{GlobalType, Globals};

pub use control::controls::Control;
pub use control::controls::ControlRepository;
pub use control::controls::ControlSet;
pub use control::controls::ControlSetBuilder;
pub use control::ControlId;
pub use control::ControlIdPartType;

pub use control::spline::SplinePoint;

pub use control::controls::UsedControlsNode;
