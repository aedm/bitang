use crate::render::image::Image;
use crate::render::{SCREEN_COLOR_FORMAT, SCREEN_RENDER_TARGET_ID};
use crate::tool::content_renderer::ContentRenderer;
use crate::tool::timer::Timer;
use crate::tool::ui::Ui;
use crate::tool::{
    InitContext, RenderContext, VulkanContext, BORDERLESS_FULL_SCREEN, SCREEN_RATIO,
    START_IN_DEMO_MODE,
};
use anyhow::Result;
use std::cmp::max;
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage};
use vulkano::image::ImageViewAbstract;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::sync::GpuFuture;
use vulkano_util::renderer::VulkanoWindowRenderer;
use vulkano_util::window::{VulkanoWindows, WindowDescriptor};
use winit::dpi::PhysicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window};

pub struct WindowRunner {
    pub vulkan_context: Arc<VulkanContext>,
    is_fullscreen: bool,
    ui: Ui,
    windows: VulkanoWindows,
    app: ContentRenderer,
    app_start_time: Instant,
}

pub enum PaintResult {
    None,
    EndReached,
}

impl WindowRunner {
    pub fn run(init_context: InitContext) -> Result<()> {
        let final_render_target =
            Image::new_swapchain(SCREEN_RENDER_TARGET_ID, SCREEN_COLOR_FORMAT);

        let vulkano_context = init_context.vulkano_context.clone();
        let vulkan_context = init_context.into_vulkan_context(final_render_target);

        let app = ContentRenderer::new(&vulkan_context)?;

        let event_loop = EventLoop::new();

        let mut windows = VulkanoWindows::default();
        let window_descriptor = WindowDescriptor {
            title: "Bitang".to_string(),
            width: 1280.,
            height: 1000.,
            ..WindowDescriptor::default()
        };

        windows.create_window(&event_loop, &vulkano_context, &window_descriptor, |ci| {
            ci.image_format = Some(SCREEN_COLOR_FORMAT.vulkan_format());
            ci.min_image_count = ci.min_image_count.max(3);
        });

        let ui = Ui::new(
            &vulkan_context,
            &event_loop,
            &windows.get_primary_renderer().unwrap().surface(),
        )?;

        let window_runner = Self {
            vulkan_context,
            is_fullscreen: false,
            ui,
            windows,
            app,
            app_start_time: Instant::now(),
        };

        window_runner.run_inner(event_loop);
        Ok(())
    }

    fn run_inner(mut self, event_loop: EventLoop<()>) {
        let mut demo_mode = START_IN_DEMO_MODE;
        if demo_mode {
            info!("Starting demo.");
            self.toggle_fullscreen();
            self.app.play();
        }

        event_loop.run(move |event, _, control_flow| {
            match event {
                Event::WindowEvent { event, window_id } if window_id == self.get_window().id() => {
                    self.ui.handle_window_event(&event);
                    match event {
                        WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                            self.get_renderer().resize();
                        }
                        WindowEvent::CloseRequested => {
                            *control_flow = ControlFlow::Exit;
                            info!("App closed.");
                        }
                        WindowEvent::KeyboardInput { input, .. } => {
                            if input.state == winit::event::ElementState::Pressed {
                                match input.virtual_keycode {
                                    Some(winit::event::VirtualKeyCode::F11) => {
                                        self.toggle_fullscreen();
                                        demo_mode = false;
                                    }
                                    Some(winit::event::VirtualKeyCode::Escape) => {
                                        if demo_mode {
                                            *control_flow = ControlFlow::Exit;
                                            info!("Exiting on user request.");
                                        } else if self.is_fullscreen {
                                            self.toggle_fullscreen();
                                            // self.app.stop();
                                            self.app.app_state.pause();
                                        }
                                    }
                                    Some(winit::event::VirtualKeyCode::Space) => {
                                        self.app.toggle_play();
                                    }
                                    Some(winit::event::VirtualKeyCode::F1) => {
                                        self.app.reset_simulation();
                                    }
                                    _ => (),
                                }
                            }
                        }
                        _ => (),
                    }
                }
                Event::RedrawRequested(_) => {
                    let result = self.render_frame_to_screen();
                    match result {
                        PaintResult::None => {}
                        PaintResult::EndReached => {
                            if demo_mode {
                                *control_flow = ControlFlow::Exit;
                                info!("Everything that has a beginning must have an end.");
                            } else if self.is_fullscreen {
                                self.toggle_fullscreen();
                            }
                            self.app.stop();
                        }
                    }
                }
                Event::MainEventsCleared => {
                    self.get_window().request_redraw();
                }
                _ => (),
            };
        });
    }

    fn render_frame_to_screen(&mut self) -> PaintResult {
        // Don't render anything if the window is minimized
        let window_size = self.get_window().inner_size();
        if window_size.width == 0 || window_size.height == 0 {
            return PaintResult::None;
        }

        let before_future = self.get_renderer().acquire().unwrap();

        // Update swapchain target
        let target_image = self.get_renderer().swapchain_image_view();
        self.vulkan_context
            .final_render_target
            .set_swapchain_image(target_image.clone());

        // Calculate viewport
        let window_size = target_image.dimensions();
        let scale_factor = self.get_window().scale_factor() as f32;
        let (width, height, top, left) = if self.is_fullscreen {
            if window_size.width() * SCREEN_RATIO.1 > window_size.height() * SCREEN_RATIO.0 {
                // Screen is too wide
                let height = window_size.height();
                let width = height * SCREEN_RATIO.0 / SCREEN_RATIO.1;
                let left = (window_size.width() - width) / 2;
                let top = 0;
                (width, height, top, left)
            } else {
                // Screen is too tall
                let width = window_size.width();
                let height = width * SCREEN_RATIO.1 / SCREEN_RATIO.0;
                let left = 0;
                let top = (window_size.height() - height) / 2;
                (width, height, top, left)
            }
        } else {
            let width = window_size.width();
            let height = width * SCREEN_RATIO.1 / SCREEN_RATIO.0;
            let left = 0;
            let top = 0;
            (width, height, top, left)
        };
        let ui_height = max(window_size.height() as i32 - height as i32, 0) as f32;
        let screen_viewport = Viewport {
            origin: [left as f32, top as f32],
            dimensions: [width as f32, height as f32],
            depth_range: 0.0..1.0,
        };

        // Make command buffer
        let mut command_builder = AutoCommandBufferBuilder::primary(
            &self.vulkan_context.command_buffer_allocator,
            self.vulkan_context.gfx_queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        // Make render context
        let mut render_context = RenderContext {
            vulkan_context: self.vulkan_context.clone(),
            screen_viewport,
            command_builder: &mut command_builder,
            globals: Default::default(),
            simulation_elapsed_time_since_last_render: 0.0,
        };
        render_context.globals.app_time = self.app_start_time.elapsed().as_secs_f32();

        // Render content
        self.app.reload_project(&self.vulkan_context);
        self.app.issue_render_commands(&mut render_context);

        // Render UI
        if !self.is_fullscreen && ui_height > 0.0 {
            self.ui.draw(
                &mut render_context,
                ui_height,
                scale_factor,
                &mut self.app.app_state,
            );
        }

        // Execute commands and display the result
        let command_buffer = command_builder.build().unwrap();
        let after_future = before_future
            .then_execute(self.vulkan_context.gfx_queue.clone(), command_buffer)
            .unwrap()
            .boxed();
        self.get_renderer().present(after_future, true);

        if let Some(project) = &self.app.app_state.project {
            if self.app.app_state.cursor_time >= project.length {
                return PaintResult::EndReached;
            }
        }
        PaintResult::None
    }

    fn toggle_fullscreen(&mut self) {
        let renderer = self.windows.get_primary_renderer_mut().unwrap();
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
    }

    fn get_renderer(&mut self) -> &mut VulkanoWindowRenderer {
        self.windows.get_primary_renderer_mut().unwrap()
    }

    fn get_window(&self) -> &Window {
        self.windows.get_primary_window().unwrap()
    }
}
