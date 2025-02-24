use crate::render::image::BitangImage;
use crate::render::{SCREEN_COLOR_FORMAT, SCREEN_RENDER_TARGET_ID};
use crate::tool::content_renderer::ContentRenderer;
use crate::tool::ui::Ui;
use crate::tool::{
    FrameContext, GpuContext, Viewport, WindowContext, BORDERLESS_FULL_SCREEN, SCREEN_RATIO,
    START_IN_DEMO_MODE,
};
use anyhow::{Context, Result};
use std::cmp::max;
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info};
use wgpu::Surface;
use winit::keyboard::{Key, NamedKey};
// use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage};
// use vulkano::pipeline::graphics::viewport::Viewport;
// use vulkano::sync::GpuFuture;
// use vulkano_util::renderer::VulkanoWindowRenderer;
// use vulkano_util::window::{VulkanoWindows, WindowDescriptor};
use winit::dpi::{LogicalSize, PhysicalSize, Size};
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowAttributes};

pub struct WindowRunner {}

impl WindowRunner {
    pub fn run() -> Result<()> {
        let event_loop = EventLoop::new().unwrap();

        // TODO: review if this is needed
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut app = WinitAppWrapper::new();
        event_loop.run_app(&mut app);

        Ok(())
    }
}

struct WinitAppWrapper {
    app: Option<App>,
}

impl WinitAppWrapper {
    fn new() -> Self {
        Self { app: None }
    }
}

impl winit::application::ApplicationHandler for WinitAppWrapper {
    /// The `resumed` event here is considered to be equivalent to context creation.
    /// That assumption might not be true on mobile, but works fine on desktop.
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        assert!(self.app.is_none());

        let Ok(app) = App::new(event_loop) else {
            panic!("Failed to create app");
        };
        self.app = Some(app);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if let Some(app) = &mut self.app {
            app.handle_window_event(event_loop, event);
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(app) = &mut self.app {
            app.request_redraw();
        }
    }
}

struct App {
    // pub vulkan_context: Arc<WindowContext>,
    gpu_context: Arc<GpuContext>,
    is_fullscreen: bool,
    ui: Ui,
    // windows: VulkanoWindows,
    content_renderer: ContentRenderer,
    app_start_time: Instant,
    final_render_target: Arc<BitangImage>,
    demo_mode: bool,
    window: Arc<Window>,
    surface: Surface<'static>,
    surface_config: wgpu::SurfaceConfig,
}

pub enum PaintResult {
    None,
    EndReached,
}

impl App {
    const SAVE_SHORTCUT: egui::KeyboardShortcut =
        egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::S);
    const FULLSCREEN_SHORTCUT: egui::KeyboardShortcut =
        egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::F11);
    const RESET_SIMULATION_SHORTCUT: egui::KeyboardShortcut =
        egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::F1);
    const TOGGLE_SIMULATION_SHORTCUT: egui::KeyboardShortcut =
        egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::F2);

    fn new(event_loop: &winit::event_loop::ActiveEventLoop) -> Result<Self> {
        let gpu_context = GpuContext::new()?;

        let window = Arc::new(event_loop.create_window(WindowAttributes {
            inner_size: Some(Size::Logical(LogicalSize::new(1280.0, 1000.0))),
            title: "Bitang".to_string(),
            ..WindowAttributes::default()
        })?);
        let size = window.inner_size();

        let surface = gpu_context.instance.create_surface(window.clone())?;
        let mut surface_config = surface
            .get_default_config(&gpu_context.adapter, size.width, size.height)
            .context("No default config found")?;
        surface.configure(&gpu_context.device, &surface_config);

        let final_render_target =
            BitangImage::new_swapchain(SCREEN_RENDER_TARGET_ID, SCREEN_COLOR_FORMAT);
        // let vulkano_context = init_context.vulkano_context.clone();
        // let vulkan_context = init_context.into_vulkan_context(final_render_target);

        let mut app = ContentRenderer::new(&gpu_context)?;
        info!("Init DOOM refresh daemon...");
        app.reset_simulation(&gpu_context)?;

        // let event_loop = EventLoop::new();
        // let mut windows = VulkanoWindows::default();
        // let window_descriptor = WindowDescriptor {
        //     title: "Bitang".to_string(),
        //     width: 1280.,
        //     height: 1000.,
        //     ..WindowDescriptor::default()
        // };

        // windows.create_window(&event_loop, &vulkano_context, &window_descriptor, |ci| {
        //     ci.image_format = SCREEN_COLOR_FORMAT.vulkan_format();
        //     ci.min_image_count = ci.min_image_count.max(3);
        // });

        let ui = Ui::new(
            // &gpu_context,
            // &event_loop,
            // &windows.get_primary_renderer().unwrap().surface(),
        )?;

        let mut app = Self {
            is_fullscreen: false,
            ui,
            content_renderer: app,
            app_start_time: Instant::now(),
            gpu_context,
            final_render_target,
            demo_mode: START_IN_DEMO_MODE,
            window,
            surface,
            surface_config,
        };

        if app.demo_mode {
            // Start demo in fullscreen
            info!("Starting demo.");
            app.toggle_fullscreen();
            app.content_renderer.play();
        }

        Ok(app)
    }

    fn handle_hotkeys(&self, ctx: egui::Context) {
        todo!();
        // // Save
        // if ctx.input_mut(|i| i.consume_shortcut(&Self::SAVE_SHORTCUT)) {
        //     if let Some(project) = &self.ui_state.project {
        //         if let Err(err) = self.ui_state.control_repository.save_control_files(project) {
        //             error!("Failed to save controls: {err}");
        //         }
        //     }
        // }
    }

    fn handle_window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        self.ui.handle_window_event(&event);
        match event {
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                let new_size = self.window.inner_size();
                if new_size.width > 0 && new_size.height > 0 {
                    self.surface_config.width = new_size.width;
                    self.surface_config.height = new_size.height;
                    self.surface
                        .configure(&self.gpu_context.device, &self.surface_config);
                }
            }
            WindowEvent::CloseRequested => {
                info!("App closed.");
                event_loop.exit();
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                if key_event.state == winit::event::ElementState::Pressed {
                    match key_event.logical_key {
                        Key::Named(NamedKey::F11) => {
                            self.toggle_fullscreen();
                            self.demo_mode = false;
                        }
                        Key::Named(NamedKey::Escape) => {
                            if self.demo_mode {
                                info!("Exiting on user request.");
                                event_loop.exit();
                            } else if self.is_fullscreen {
                                self.toggle_fullscreen();
                                self.content_renderer.app_state.pause();
                            }
                        }
                        Key::Named(NamedKey::Space) => {
                            self.content_renderer.toggle_play();
                        }
                        Key::Named(NamedKey::F1) => {
                            if let Err(err) =
                                self.content_renderer.reset_simulation(&self.gpu_context)
                            {
                                error!("Failed to reset simulation: {:?}", err);
                            }
                            // Skip the time spent resetting the simulation
                            self.content_renderer
                                .set_last_render_time(self.app_start_time.elapsed().as_secs_f32());
                        }
                        Key::Named(NamedKey::F2) => {
                            self.content_renderer.app_state.is_simulation_enabled =
                                !self.content_renderer.app_state.is_simulation_enabled;
                        }
                        _ => (),
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(err) = self.render_frame_to_screen() {
                    error!("Failed to render frame: {err}");
                } else if self.has_timeline_ended() {
                    if self.demo_mode {
                        info!("Everything that has a beginning must have an end.");
                        event_loop.exit();
                    } else if self.is_fullscreen {
                        self.toggle_fullscreen();
                    }
                    self.content_renderer.stop();
                }
            }
            _ => (),
        }
    }

    fn render_frame_to_screen(&mut self) -> Result<()> {
        // Don't render anything if the window is minimized
        let window_size = self.window.inner_size();
        if window_size.width == 0 || window_size.height == 0 {
            return Ok(());
        }

        let swapchain_texture = self.surface.get_current_texture()?;

        // let before_future = self.get_renderer().acquire().unwrap();

        // Update swapchain target
        let swapchain_view = swapchain_texture.texture.create_view(&Default::default());
        self.final_render_target
            .set_swapchain_image_view(swapchain_view);
        // let target_image = self.get_renderer().swapchain_image_view();
        // self.vulkan_context
        //     .final_render_target
        //     .set_swapchain_image(target_image.clone());

        // Calculate viewport
        let scale_factor = self.window.scale_factor() as f32;
        let (width, height, top, left) = if self.is_fullscreen {
            if window_size.width * SCREEN_RATIO.1 > window_size.height * SCREEN_RATIO.0 {
                // Screen is too wide
                let height = window_size.height;
                let width = height * SCREEN_RATIO.0 / SCREEN_RATIO.1;
                let left = (window_size.width - width) / 2;
                let top = 0;
                (width, height, top, left)
            } else {
                // Screen is too tall
                let width = window_size.width;
                let height = width * SCREEN_RATIO.1 / SCREEN_RATIO.0;
                let left = 0;
                let top = (window_size.height - height) / 2;
                (width, height, top, left)
            }
        } else {
            let width = window_size.width;
            let height = width * SCREEN_RATIO.1 / SCREEN_RATIO.0;
            let left = 0;
            let top = 0;
            (width, height, top, left)
        };
        let ui_height = max(window_size.height as i32 - height as i32, 0) as f32;
        let screen_viewport = Viewport {
            // offset: [left as f32, top as f32],
            // extent: [width as f32, height as f32],
            x: left as f32,
            y: top as f32,
            width: width as f32,
            height: height as f32,
        };

        // // Make command buffer
        // let mut command_builder = AutoCommandBufferBuilder::primary(
        //     &self.vulkan_context.command_buffer_allocator,
        //     self.vulkan_context.gfx_queue.queue_family_index(),
        //     CommandBufferUsage::OneTimeSubmit,
        // )
        // .unwrap();

        // // Make render context
        // let mut render_context = FrameContext {
        //     vulkan_context: self.vulkan_context.clone(),
        //     screen_viewport,
        //     command_builder: &mut command_builder,
        //     globals: Default::default(),
        //     // simulation_elapsed_time_since_last_render: 0.0,
        // };
        // render_context.globals.app_time = self.app_start_time.elapsed().as_secs_f32();
        let mut encoder = self
            .gpu_context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        let mut frame_context = FrameContext {
            gpu_context: self.gpu_context.clone(),
            command_encoder: encoder,
            globals: Default::default(),
            simulation_elapsed_time_since_last_render: 0.0,
            screen_viewport,
        };
        frame_context.globals.app_time = self.app_start_time.elapsed().as_secs_f32();

        if self.content_renderer.reload_project(&self.gpu_context) {
            self.content_renderer
                .reset_simulation(&self.gpu_context)
                .unwrap();
            frame_context.globals.app_time = self.app_start_time.elapsed().as_secs_f32();
            self.content_renderer
                .set_last_render_time(frame_context.globals.app_time);
        }

        // Render content
        self.content_renderer.draw(&mut frame_context);

        // Render UI
        if !self.is_fullscreen && ui_height > 0.0 {
            self.ui.draw(
                &mut frame_context,
                ui_height,
                scale_factor,
                &mut self.content_renderer.app_state,
            );
        }

        // Execute commands and display the result
        self.gpu_context.queue.submit(Some(encoder.finish()));
        swapchain_texture.present();

        // let command_buffer = command_builder.build().unwrap();
        // let after_future = before_future
        //     .then_execute(self.vulkan_context.gfx_queue.clone(), command_buffer)
        //     .unwrap()
        //     .boxed();
        // // TODO: check if we really need to wait for the future
        // self.get_renderer().present(after_future, true);
        Ok(())
    }

    fn has_timeline_ended(&self) -> bool {
        let Some(project) = &self.content_renderer.app_state.project else {
            return false;
        };
        self.content_renderer.app_state.cursor_time >= project.length
    }

    fn toggle_fullscreen(&mut self) {
        // let renderer = self.windows.get_primary_renderer_mut().unwrap();
        self.is_fullscreen = !self.is_fullscreen;
        if self.is_fullscreen {
            if BORDERLESS_FULL_SCREEN {
                self.window
                    .set_fullscreen(Some(Fullscreen::Borderless(None)));
                self.window.set_cursor_visible(false);
            } else if let Some(monitor) = self.window.current_monitor() {
                let video_mode = monitor
                    .video_modes()
                    .find(|mode| mode.size() == PhysicalSize::new(1920, 1080));
                if let Some(video_mode) = video_mode {
                    self.window
                        .set_fullscreen(Some(Fullscreen::Exclusive(video_mode)));
                    self.window.set_cursor_visible(false);
                } else {
                    error!("Could not find 1920x1080 video mode");
                }
            } else {
                error!("Could not find current monitor");
            }
        } else {
            self.window.set_fullscreen(None);
            self.window.set_cursor_visible(true);
            self.window.focus_window();
        }
    }

    // fn get_renderer(&mut self) -> &mut VulkanoWindowRenderer {
    //     self.windows.get_primary_renderer_mut().unwrap()
    // }

    // fn get_window(&self) -> &Window {
    //     self.windows.get_primary_window().unwrap()
    // }

    fn request_redraw(&self) {
        self.window.request_redraw();
    }
}
