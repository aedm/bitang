mod app_config;
mod app_state;
pub mod content_renderer;
mod music_player;
mod runners;
mod spline_editor;
mod timer;
mod ui;

use crate::control::controls::Globals;
use crate::render::image::BitangImage;
use crate::render::SCREEN_COLOR_FORMAT;
use crate::tool::runners::frame_dump_runner::FrameDumpRunner;
use crate::tool::runners::window_runner::WindowRunner;
use anyhow::Result;
use std::default::Default;
use std::sync::Arc;
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::{Device, Queue};
use vulkano::format::Format;
use vulkano::instance::{InstanceCreateInfo, InstanceExtensions};
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano_util::context::{VulkanoConfig, VulkanoContext};

const START_IN_DEMO_MODE: bool = false;
const BORDERLESS_FULL_SCREEN: bool = true;

pub const FRAMEDUMP_MODE: bool = false;
pub const FRAMEDUMP_WIDTH: u32 = 3840;
pub const FRAMEDUMP_HEIGHT: u32 = 2160;
pub const FRAMEDUMP_FPS: u32 = 60;

const SCREEN_RATIO: (u32, u32) = (16, 9);

pub struct InitContext {
    pub vulkano_context: Arc<VulkanoContext>,
    pub command_buffer_allocator: StandardCommandBufferAllocator,
    pub descriptor_set_allocator: StandardDescriptorSetAllocator,
}

impl InitContext {
    fn into_vulkan_context(self, final_render_target: Arc<BitangImage>) -> Arc<VulkanContext> {
        Arc::new(VulkanContext {
            command_buffer_allocator: self.command_buffer_allocator,
            descriptor_set_allocator: self.descriptor_set_allocator,
            memory_allocator: self.vulkano_context.memory_allocator().clone(),
            gfx_queue: self.vulkano_context.graphics_queue().clone(),
            device: self.vulkano_context.device().clone(),
            swapchain_format: SCREEN_COLOR_FORMAT.vulkan_format(),
            final_render_target,
        })
    }
}

pub struct VulkanContext {
    pub device: Arc<Device>,
    pub command_buffer_allocator: StandardCommandBufferAllocator,
    pub descriptor_set_allocator: StandardDescriptorSetAllocator,
    pub memory_allocator: Arc<StandardMemoryAllocator>,
    pub gfx_queue: Arc<Queue>,
    pub swapchain_format: Format,
    pub final_render_target: Arc<BitangImage>,
}

pub struct RenderContext<'a> {
    pub vulkan_context: Arc<VulkanContext>,
    pub screen_viewport: Viewport,
    // TODO: rename to command queue
    pub command_builder: &'a mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    pub globals: Globals,
    pub simulation_elapsed_time_since_last_render: f32,
}

pub fn run_app() -> Result<()> {
    let vulkano_context = Arc::new(VulkanoContext::new(VulkanoConfig {
        instance_create_info: InstanceCreateInfo {
            enabled_extensions: InstanceExtensions {
                // TODO: implement debug flag
                // ext_debug_utils: true,
                ..InstanceExtensions::empty()
            },
            ..InstanceCreateInfo::default()
        },
        ..Default::default()
    }));

    let command_buffer_allocator =
        StandardCommandBufferAllocator::new(vulkano_context.device().clone(), Default::default());

    let descriptor_set_allocator =
        StandardDescriptorSetAllocator::new(vulkano_context.device().clone(), Default::default());

    let init_context = InitContext {
        vulkano_context,
        command_buffer_allocator,
        descriptor_set_allocator,
    };

    if FRAMEDUMP_MODE {
        FrameDumpRunner::run(init_context)
    } else {
        WindowRunner::run(init_context)
    }
}
