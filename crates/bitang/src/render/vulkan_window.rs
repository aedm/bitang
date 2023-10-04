use crate::control::controls::Globals;
use crate::render::image::{Image, ImageSizeRule};
use crate::render::{SCREEN_COLOR_FORMAT, SCREEN_RENDER_TARGET_ID};
use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{error, info};
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::Queue;
use vulkano::format::Format;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::swapchain::Surface;
use vulkano_util::renderer::VulkanoWindowRenderer;
use vulkano_util::{
    context::{VulkanoConfig, VulkanoContext},
    window::{VulkanoWindows, WindowDescriptor},
};
use winit::dpi::PhysicalSize;
use winit::window::{Fullscreen, Window, WindowBuilder};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

const START_IN_DEMO_MODE: bool = false;
const BORDERLESS_FULL_SCREEN: bool = true;

pub const FRAMEDUMP_MODE: bool = false;
pub const FRAMEDUMP_WIDTH: u32 = 3840;
pub const FRAMEDUMP_HEIGHT: u32 = 2160;
pub const FRAMEDUMP_FPS: u32 = 60;

pub struct VulkanContext {
    pub vulkano_context: VulkanoContext,
    pub command_buffer_allocator: StandardCommandBufferAllocator,
    pub descriptor_set_allocator: StandardDescriptorSetAllocator,
    pub gfx_queue: Arc<Queue>,
    pub final_render_target: Arc<Image>,
    pub swapchain_format: Format,
}

pub struct RenderContext<'a> {
    pub vulkan_context: Arc<VulkanContext>,
    pub screen_viewport: Viewport,
    pub command_builder: &'a mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    pub globals: Globals,
    // pub surface: Arc<Surface>,
}

pub struct VulkanWindow {
    pub context: Arc<VulkanContext>,
    pub event_loop: Option<EventLoop<()>>,
    // windows: VulkanoWindows,
    is_fullscreen: bool,
}

pub enum PaintResult {
    None,
    EndReached,
}

pub trait VulkanApp {
    fn paint(
        &mut self,
        context: &Arc<VulkanContext>,
        renderer: &mut VulkanoWindowRenderer,
    ) -> PaintResult;
    fn handle_window_event(&mut self, event: &WindowEvent);
    fn play(&mut self);
    fn stop(&mut self);
    fn set_fullscreen(&mut self, fullscreen: bool);

    fn init_with_surface(
        &mut self,
        context: &Arc<VulkanContext>,
        event_loop: &EventLoop<()>,
        surface: &Arc<Surface>,
    ) -> Result<()>;
}

impl VulkanWindow {
    pub fn new() -> Result<Self> {
        let event_loop = EventLoop::new();

        let vulkano_context = VulkanoContext::new(VulkanoConfig::default());

        // let window = Arc::new(
        //     WindowBuilder::new()
        //         // .with_visible(false)
        //         .build(&event_loop)
        //         .unwrap(),
        // );
        // // window.set_visible(false);
        // let surface = Surface::from_window(instance.clone(), window.clone()).unwrap();

        let final_render_target = if FRAMEDUMP_MODE {
            let size = ImageSizeRule::Fixed(FRAMEDUMP_WIDTH, FRAMEDUMP_HEIGHT);
            Image::new_attachment(SCREEN_RENDER_TARGET_ID, SCREEN_COLOR_FORMAT, size)
        } else {
            Image::new_swapchain(SCREEN_RENDER_TARGET_ID, SCREEN_COLOR_FORMAT)
        };

        let command_buffer_allocator = StandardCommandBufferAllocator::new(
            vulkano_context.device().clone(),
            Default::default(),
        );
        let descriptor_set_allocator =
            StandardDescriptorSetAllocator::new(vulkano_context.device().clone());

        let context = Arc::new(VulkanContext {
            command_buffer_allocator,
            descriptor_set_allocator,
            gfx_queue: vulkano_context.graphics_queue().clone(),
            vulkano_context,
            final_render_target,
            // swapchain_format: renderer.swapchain_format(),
            swapchain_format: SCREEN_COLOR_FORMAT.vulkan_format(),
        });

        Ok(Self {
            event_loop: Some(event_loop),
            context,
            is_fullscreen: false,
        })
    }

    fn toggle_fullscreen(&mut self, renderer: &VulkanoWindowRenderer, app: &mut impl VulkanApp) {
        // let renderer = self.windows.get_primary_renderer_mut().unwrap();
        let window = renderer.window();
        self.is_fullscreen = !self.is_fullscreen;
        if self.is_fullscreen {
            if BORDERLESS_FULL_SCREEN {
                window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                window.set_cursor_visible(false);
            } else if let Some(monitor) = window.current_monitor() {
                let video_mode = monitor
                    .video_modes()
                    .find(|mode| mode.size() == PhysicalSize::new(1920, 1080));
                if let Some(video_mode) = video_mode {
                    window.set_fullscreen(Some(Fullscreen::Exclusive(video_mode)));
                    window.set_cursor_visible(false);
                } else {
                    error!("Could not find 1920x1080 video mode");
                }
            } else {
                error!("Could not find current monitor");
            }
        } else {
            window.set_fullscreen(None);
            window.set_cursor_visible(true);
            window.focus_window();
        }
        app.set_fullscreen(self.is_fullscreen);
    }

    fn get_renderer(windows: &mut VulkanoWindows) -> &mut VulkanoWindowRenderer {
        windows.get_primary_renderer_mut().unwrap()
    }

    fn get_window(windows: &VulkanoWindows) -> &Window {
        windows.get_primary_window().unwrap()
    }

    pub fn run(mut self, mut app: impl VulkanApp + 'static) {
        let event_loop = self.event_loop.take().unwrap();

        let mut windows = VulkanoWindows::default();
        let window_descriptor = WindowDescriptor {
            title: "Bitang".to_string(),
            width: 1280.,
            height: 1000.,
            ..WindowDescriptor::default()
        };

        windows.create_window(
            &event_loop,
            &self.context.vulkano_context,
            &window_descriptor,
            |ci| {
                ci.image_format = Some(SCREEN_COLOR_FORMAT.vulkan_format());
                ci.min_image_count = ci.min_image_count.max(3);
            },
        );

        app.init_with_surface(
            &self.context,
            &event_loop,
            &Self::get_renderer(&mut windows).surface(),
        )
        .unwrap();

        app.set_fullscreen(self.is_fullscreen);
        if START_IN_DEMO_MODE {
            info!("Starting demo.");
            self.toggle_fullscreen(Self::get_renderer(&mut windows), &mut app);
            app.play();
        }
        let mut demo_mode = START_IN_DEMO_MODE;

        event_loop.run(move |event, _, control_flow| {
            match event {
                Event::WindowEvent { event, window_id }
                    if window_id == Self::get_window(&windows).id() =>
                {
                    app.handle_window_event(&event);
                    match event {
                        WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                            Self::get_renderer(&mut windows).resize();
                        }
                        WindowEvent::CloseRequested => {
                            *control_flow = ControlFlow::Exit;
                            info!("App closed.");
                        }
                        WindowEvent::KeyboardInput { input, .. } => {
                            if input.state == winit::event::ElementState::Pressed {
                                match input.virtual_keycode {
                                    Some(winit::event::VirtualKeyCode::F11) => {
                                        self.toggle_fullscreen(
                                            Self::get_renderer(&mut windows),
                                            &mut app,
                                        );
                                        demo_mode = false;
                                    }
                                    Some(winit::event::VirtualKeyCode::Escape) => {
                                        if demo_mode {
                                            *control_flow = ControlFlow::Exit;
                                            info!("Exiting on user request.");
                                        } else if self.is_fullscreen {
                                            self.toggle_fullscreen(
                                                Self::get_renderer(&mut windows),
                                                &mut app,
                                            );
                                            app.stop();
                                        }
                                    }
                                    _ => (),
                                }
                            }
                        }
                        _ => (),
                    }
                }
                Event::RedrawRequested(_) => {
                    let result = app.paint(&self.context, Self::get_renderer(&mut windows));
                    match result {
                        PaintResult::None => {}
                        PaintResult::EndReached => {
                            if demo_mode || FRAMEDUMP_MODE {
                                *control_flow = ControlFlow::Exit;
                                info!("Everything that has a beginning must have an end.");
                            } else if self.is_fullscreen {
                                self.toggle_fullscreen(Self::get_renderer(&mut windows), &mut app);
                            }
                            app.stop();
                        }
                    }
                }
                Event::MainEventsCleared => {
                    Self::get_window(&windows).request_redraw();
                }
                _ => (),
            };
        });
    }
}
