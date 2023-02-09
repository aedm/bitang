use crate::render::DemoApp;

use egui_winit_vulkano::Gui;
use std::cmp::max;
use std::sync::Arc;
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassContents,
};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::{Device, Queue};
use vulkano::format::Format;
use vulkano::image::{ImageUsage, ImageViewAbstract};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, Subpass};
use vulkano::swapchain::Surface;
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
    pub swapchain_format: Format,
    pub surface: Arc<Surface>,
    pub gfx_queue: Arc<Queue>,
}

// pub struct AppContext {
//     pub subpass: Subpass,
// }
//
// pub struct GuiContext {
//     pub gui: Gui,
//     pub subpass: Subpass,
// }

pub struct VulkanWindow {
    event_loop: EventLoop<()>,
    windows: VulkanoWindows,
    pub context: VulkanContext,
    // pub gui_context: GuiContext,
    // pub app_context: AppContext,
    app: Box<dyn VulkanApp>,
}

pub trait VulkanApp {
    fn init(&mut self, context: &VulkanContext, event_loop: &EventLoop<()>);
    fn paint(&mut self, context: &VulkanContext);
}

impl VulkanWindow {
    pub fn new(mut app: impl VulkanApp) -> Self {
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

        // let gui_context = GuiContext::new(vulkano_context.device(), renderer, &event_loop);
        // let app_context = AppContext::new(vulkano_context.device(), renderer);

        let context = VulkanContext {
            context: vulkano_context,
            command_buffer_allocator,
            descriptor_set_allocator,
            swapchain_format: renderer.swapchain_format(),
            surface: renderer.surface(),
            gfx_queue: renderer.graphics_queue(),
        };

        app.init(&context);

        Self {
            windows,
            event_loop,
            context,
            // gui_context,
            // app_context,
            app: Box::new(app),
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
                Event::RedrawRequested(_) => {
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
                        depth_image,
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
                                        ui.add_space(5.0);
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
                }
                Event::MainEventsCleared => {
                    renderer.window().request_redraw();
                }
                _ => (),
            }
        });
    }
}
