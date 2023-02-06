use crate::render::DemoApp;

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

pub struct VulkanContext {
    // TODO: expand and remove
    pub context: VulkanoContext,
    pub command_buffer_allocator: StandardCommandBufferAllocator,
    pub descriptor_set_allocator: StandardDescriptorSetAllocator,
}

pub struct AppContext {
    pub subpass: Subpass,
}

pub struct GuiContext {
    pub gui: Gui,
    pub subpass: Subpass,
}

pub struct VulkanWindow {
    event_loop: EventLoop<()>,
    windows: VulkanoWindows,
    pub context: VulkanContext,
    pub gui_context: GuiContext,
    pub app_context: AppContext,
}

impl VulkanWindow {
    pub fn new() -> Self {
        let event_loop = EventLoop::new();

        let vulkano_context = VulkanoContext::new(VulkanoConfig::default());

        let mut windows = VulkanoWindows::default();
        let window_descriptor = WindowDescriptor {
            title: "bitang".to_string(),
            width: 1000.,
            height: 720.,
            ..WindowDescriptor::default()
        };

        windows.create_window(&event_loop, &vulkano_context, &window_descriptor, |ci| {
            ci.image_format = Some(Format::B8G8R8A8_SRGB)
        });

        let renderer = windows.get_primary_renderer_mut().unwrap();
        renderer.add_additional_image_view(
            1,
            Format::D16_UNORM,
            ImageUsage {
                depth_stencil_attachment: true,
                ..ImageUsage::empty()
            },
        );

        let command_buffer_allocator = StandardCommandBufferAllocator::new(
            vulkano_context.device().clone(),
            Default::default(),
        );
        let descriptor_set_allocator =
            StandardDescriptorSetAllocator::new(vulkano_context.device().clone());

        let gui_context = GuiContext::new(&vulkano_context.device(), renderer, &event_loop);
        let app_context = AppContext::new(&vulkano_context.device(), renderer);

        let context = VulkanContext {
            context: vulkano_context,
            command_buffer_allocator,
            descriptor_set_allocator,
        };

        Self {
            windows,
            event_loop,
            context,
            gui_context,
            app_context,
        }
    }

    pub fn main_loop(mut self, mut app: DemoApp) {
        self.event_loop.run(move |event, _, control_flow| {
            let scale_factor = self.windows.get_primary_window().unwrap().scale_factor() as f32;
            let renderer = self.windows.get_primary_renderer_mut().unwrap();
            match event {
                Event::WindowEvent { event, window_id } if window_id == renderer.window().id() => {
                    // Update Egui integration so the UI works!
                    let _pass_events_to_game = !self.gui_context.gui.update(&event);
                    match event {
                        WindowEvent::Resized(_) => {
                            renderer.resize();
                        }
                        WindowEvent::ScaleFactorChanged { .. } => {
                            renderer.resize();
                        }
                        WindowEvent::CloseRequested => {
                            *control_flow = ControlFlow::Exit;
                        }
                        _ => (),
                    }
                }
                Event::RedrawRequested(window_id) if window_id == window_id => {
                    let before_future = renderer.acquire().unwrap();
                    let image = renderer.swapchain_image_view();
                    let size = image.dimensions();
                    let movie_height = (size.width() * 9 / 16) as i32;
                    let bottom_panel_height =
                        max(size.height() as i32 - movie_height, 0) as f32 / scale_factor;

                    let render_viewport = Viewport {
                        origin: [0.0, 0.0],
                        dimensions: [size.width() as f32, movie_height as f32],
                        depth_range: 0.0..1.0,
                    };

                    let depth_image = renderer.get_additional_image_view(1);
                    // Render app
                    let app_finished_future = app.draw(
                        &self.context,
                        &self.app_context,
                        image.clone(),
                        depth_image.clone(),
                        render_viewport,
                        before_future,
                    );

                    // Draw UI
                    self.gui_context.gui.immediate_ui(|gui| {
                        let ctx = gui.context();
                        egui::TopBottomPanel::bottom("my_panel")
                            .height_range(bottom_panel_height..=bottom_panel_height)
                            .show(&ctx, |ui| {
                                ui.with_layout(
                                    egui::Layout::top_down_justified(egui::Align::Center),
                                    |ui| {
                                        let _ = ui.add_space(5.0);
                                        let _ = ui.button("Some button");
                                        let _ = ui.button("Another button");
                                        ui.allocate_space(ui.available_size());
                                    },
                                );
                                // ui.label("Hello World!");
                            });
                    });

                    // Render UI
                    let gui_finished_future =
                        self.gui_context
                            .render_gui(&self.context, app_finished_future, image);

                    renderer.present(gui_finished_future, true);

                    // Present swapchain

                    // let mut command_buffer_builder = AutoCommandBufferBuilder::primary(
                    //     self.allocators.command_buffers.as_ref(),
                    //     self.gfx_queue.queue_family_index(),
                    //     CommandBufferUsage::OneTimeSubmit,
                    // )
                    // .unwrap();
                    // let render_viewport = Viewport {
                    //     origin: [0.0, 0.0],
                    //     dimensions: [size.width as f32, movie_height as f32],
                    //     depth_range: 0.0..1.0,
                    // };
                    // app.draw(
                    //     &self.context,
                    //     &mut command_buffer_builder,
                    //     framebuffer,
                    //     render_viewport,
                    // );
                }
                Event::MainEventsCleared => {
                    renderer.window().request_redraw();
                }
                _ => (),
            }
        });
    }
}

impl GuiContext {
    fn new(
        device: &Arc<Device>,
        renderer: &mut VulkanoWindowRenderer,
        event_loop: &EventLoop<()>,
    ) -> GuiContext {
        let render_pass = vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    load: DontCare,
                    store: Store,
                    format: renderer.swapchain_format(),
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
            renderer.surface(),
            Some(vulkano::format::Format::B8G8R8A8_SRGB),
            renderer.graphics_queue(),
            subpass.clone(),
        );

        GuiContext { gui, subpass }
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
}

impl AppContext {
    fn new(device: &Arc<Device>, renderer: &mut VulkanoWindowRenderer) -> AppContext {
        let render_pass = vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: renderer.swapchain_format(),
                    samples: 1,
                },
                depth: {
                    load: Clear,
                    store: DontCare,
                    format: Format::D16_UNORM,
                    samples: 1,
                }
            },
            pass:
                { color: [color], depth_stencil: {depth} }
        )
        .unwrap();

        let subpass = Subpass::from(render_pass, 0).unwrap();

        AppContext { subpass }
    }
}
