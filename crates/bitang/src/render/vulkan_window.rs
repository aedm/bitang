use std::sync::Arc;
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::Queue;
use vulkano::format::Format;
use vulkano::image::ImageUsage;
use vulkano::swapchain::Surface;
use vulkano_util::renderer::VulkanoWindowRenderer;
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

pub struct VulkanWindow {
    pub context: VulkanContext,
    pub event_loop: EventLoop<()>,
    windows: VulkanoWindows,
}

pub trait VulkanApp {
    fn paint(&mut self, context: &VulkanContext, renderer: &mut VulkanoWindowRenderer);
    fn handle_window_event(&mut self, event: &WindowEvent);
}

impl VulkanWindow {
    pub fn new() -> Self {
        let event_loop = EventLoop::new();

        let vulkano_context = VulkanoContext::new(VulkanoConfig::default());

        let mut windows = VulkanoWindows::default();
        let window_descriptor = WindowDescriptor {
            title: "bitang".to_string(),
            width: 1000.,
            height: 920.,
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

        let context = VulkanContext {
            context: vulkano_context,
            command_buffer_allocator,
            descriptor_set_allocator,
            swapchain_format: renderer.swapchain_format(),
            surface: renderer.surface(),
            gfx_queue: renderer.graphics_queue(),
        };

        Self {
            windows,
            event_loop,
            context,
        }
    }

    pub fn run(mut self, mut app: impl VulkanApp + 'static) {
        self.event_loop.run(move |event, _, control_flow| {
            let renderer = self.windows.get_primary_renderer_mut().unwrap();
            match event {
                Event::WindowEvent { event, window_id } if window_id == renderer.window().id() => {
                    app.handle_window_event(&event);
                    match event {
                        WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                            renderer.resize();
                        }
                        WindowEvent::CloseRequested => {
                            *control_flow = ControlFlow::Exit;
                        }
                        _ => (),
                    }
                }
                Event::RedrawRequested(_) => {
                    app.paint(&self.context, renderer);
                }
                Event::MainEventsCleared => {
                    renderer.window().request_redraw();
                }
                _ => (),
            }
        });
    }
}
