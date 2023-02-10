use crate::render::vulkan_window::VulkanContext;
use egui_winit_vulkano::Gui;
use std::cmp::max;
use std::sync::Arc;
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassContents,
};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::Device;
use vulkano::format::Format;
use vulkano::image::{ImageUsage, ImageViewAbstract};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, Subpass};
use vulkano::sync::GpuFuture;
use vulkano_util::renderer::{SwapchainImageView, VulkanoWindowRenderer};
use vulkano_util::{
    context::{VulkanoConfig, VulkanoContext},
    window::{VulkanoWindows, WindowDescriptor},
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

pub struct Ui {
    pub gui: Gui,
    pub subpass: Subpass,
}

impl Ui {
    pub fn new(context: &VulkanContext, event_loop: &EventLoop<()>) -> Ui {
        let render_pass = vulkano::single_pass_renderpass!(
            context.context.device().clone(),
            attachments: {
                color: {
                    load: DontCare,
                    store: Store,
                    format: context.swapchain_format,
                    samples: 1,
                }
            },
            pass:
                { color: [color], depth_stencil: {} }
        )
        .unwrap();
        let subpass = Subpass::from(render_pass, 0).unwrap();

        let gui = Gui::new_with_subpass(
            event_loop,
            context.surface.clone(),
            Some(vulkano::format::Format::B8G8R8A8_SRGB),
            context.gfx_queue.clone(),
            subpass.clone(),
        );

        Ui { gui, subpass }
    }

    pub fn render(
        &mut self,
        context: &VulkanContext,
        before_future: Box<dyn GpuFuture>,
        target_image: SwapchainImageView,
        bottom_panel_height: f32,
    ) -> Box<dyn GpuFuture> {
        self.gui.immediate_ui(|gui| {
            let ctx = gui.context();
            egui::TopBottomPanel::bottom("my_panel")
                .height_range(bottom_panel_height..=bottom_panel_height)
                .show(&ctx, |ui| {
                    ui.with_layout(
                        egui::Layout::top_down_justified(egui::Align::Center),
                        |ui| {
                            ui.add_space(5.0);
                            let _ = ui.button("Some button");
                            let _ = ui.button("Another button");
                            ui.allocate_space(ui.available_size());
                        },
                    );
                    // ui.label("Hello World!");
                });
        });

        self.render_gui(context, before_future, target_image)
    }

    fn render_gui(
        &mut self,
        context: &VulkanContext,
        before_future: Box<dyn GpuFuture>,
        target_image: SwapchainImageView,
    ) -> Box<dyn GpuFuture> {
        let mut builder = AutoCommandBufferBuilder::primary(
            &context.command_buffer_allocator,
            context.context.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let dimensions = target_image.dimensions().width_height();
        let framebuffer = Framebuffer::new(
            self.subpass.render_pass().clone(),
            FramebufferCreateInfo {
                attachments: vec![target_image],
                ..Default::default()
            },
        )
        .unwrap();

        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![None],
                    ..RenderPassBeginInfo::framebuffer(framebuffer)
                },
                SubpassContents::SecondaryCommandBuffers,
            )
            .unwrap();

        let gui_commands = self.gui.draw_on_subpass_image(dimensions);
        builder.execute_commands(gui_commands).unwrap();

        builder.end_render_pass().unwrap();
        let command_buffer = builder.build().unwrap();

        let after_future = before_future
            .then_execute(context.context.graphics_queue().clone(), command_buffer)
            .unwrap()
            .boxed();

        after_future
    }

    pub fn handle_window_event(&mut self, event: &WindowEvent) {
        let _pass_events_to_game = !self.gui.update(&event);
    }
}
