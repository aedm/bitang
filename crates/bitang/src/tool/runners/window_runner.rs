use crate::render::image::{BitangImage, PixelFormat};
use crate::render::{Size2D, SCREEN_COLOR_FORMAT, SCREEN_RENDER_TARGET_ID};
use crate::tool::content_renderer::ContentRenderer;
use crate::tool::ui::Ui;
use crate::tool::{
    FrameContext, GpuContext, Viewport, BORDERLESS_FULL_SCREEN, SCREEN_RATIO, START_IN_DEMO_MODE,
};
use anyhow::{Context, Result};
use egui::ViewportId;
use egui_wgpu::{WgpuSetup, WgpuSetupCreateNew, WgpuSetupExisting};
use std::cmp::max;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{error, info};
use wgpu::{Surface, SurfaceConfiguration};
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
        event_loop.run_app(&mut app)?;

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

        let app = match App::new(event_loop) {
            Ok(app) => app,
            Err(err) => {
                error!("Failed to create app: {err:?}");
                return;
            }
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

struct AppInner {
    gpu_context: Arc<GpuContext>,
    is_fullscreen: bool,
    ui: Ui,
    content_renderer: ContentRenderer,
    app_start_time: Instant,
    demo_mode: bool,
}

struct BackgroundRenderProps {
    surface_view: wgpu::TextureView, 
    surface_size: [u32; 2],
    scale_factor: f32,
}

impl AppInner {
    fn render_frame_to_screen(&mut self, props: BackgroundRenderProps) -> Result<()> {
        // Don't render anything if the window is minimized
        // let window_size = self.window.inner_size();
        if props.surface_size[0] == 0 || props.surface_size[1] == 0 {
            return Ok(());
        }

        // let swapchain_texture = self.surface.get_current_texture()?;
        // let swapchain_size = swapchain_texture.texture.size();
        let swapchain_size = props.surface_size;
        

        // let before_future = self.get_renderer().acquire().unwrap();

        // Update swapchain target
        let swapchain_view = props.surface_view;
        self.gpu_context
            .final_render_target
            .set_swapchain_image_view(swapchain_view, swapchain_size);
        // let target_image = self.get_renderer().swapchain_image_view();
        // self.vulkan_context
        //     .final_render_target
        //     .set_swapchain_image(target_image.clone());

        // Calculate viewport
        let scale_factor = props.scale_factor;
        let (width, height, top, left) = if self.is_fullscreen {
            if swapchain_size[0] * SCREEN_RATIO.1 > swapchain_size[1] * SCREEN_RATIO.0 {
                // Screen is too wide
                let height = swapchain_size[1];
                let width = height * SCREEN_RATIO.0 / SCREEN_RATIO.1;
                let left = (swapchain_size[0] - width) / 2;
                let top = 0;
                (width, height, top, left)
            } else {
                // Screen is too tall
                let width = swapchain_size[0];
                let height = width * SCREEN_RATIO.1 / SCREEN_RATIO.0;
                let left = 0;
                let top = (swapchain_size[1] - height) / 2;
                (width, height, top, left)
            }
        } else {
            let width = swapchain_size[0];
            let height = width * SCREEN_RATIO.1 / SCREEN_RATIO.0;
            let left = 0;
            let top = 0;
            (width, height, top, left)
        };
        let ui_height = max(swapchain_size[1] as i32 - height as i32, 0) as f32;
        let screen_viewport = Viewport {
            // offset: [left as f32, top as f32],
            // extent: [width as f32, height as f32],
            x: left,
            y: top,
            size: [width, height],
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
            canvas_size: swapchain_size,
        };
        frame_context.globals.app_time = self.app_start_time.elapsed().as_secs_f32();

        if self.content_renderer.reload_project(&self.gpu_context) {
            self.content_renderer.reset_simulation(&self.gpu_context).unwrap();
            frame_context.globals.app_time = self.app_start_time.elapsed().as_secs_f32();
            self.content_renderer.set_last_render_time(frame_context.globals.app_time);
        }

        // Render content
        self.content_renderer.draw(&mut frame_context);

        // TODO: gui
        // // Render UI
        // if !self.is_fullscreen && ui_height > 0.0 {
        //     let pixels_per_point =
        //         if scale_factor > 1.0 { scale_factor } else { 1.15f32 * scale_factor };
        //     let bottom_panel_height = ui_height / pixels_per_point;

        //     let full_outout = self.egui_context.run(new_input, |ctx| {
        //         ctx.set_pixels_per_point(pixels_per_point);
        //         self.ui.draw(
        //             ctx,
        //             &mut self.content_renderer.app_state,
        //             bottom_panel_height,
        //         );
        //     });

        //     let clipped_primitives = self.egui_context.tessellate(shapes, pixels_per_point);

        //     let user_cmd_bufs = {
        //         let renderer = &mut self.egui_wgpu_renderer;
        //         for (id, image_delta) in &textures_delta.set {
        //             renderer.update_texture(
        //                 &self.gpu_context.device,
        //                 &self.gpu_context.queue,
        //                 *id,
        //                 image_delta,
        //             );
        //         }

        //         renderer.update_buffers(
        //             &self.gpu_context.device,
        //             &self.gpu_context.queue,
        //             &mut encoder,
        //             clipped_primitives,
        //             &screen_descriptor,
        //         )
        //     };

        // }

        // Execute commands and display the result
        self.gpu_context.queue.submit(Some(frame_context.command_encoder.finish()));
        // swapchain_texture.present();

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
}



struct App {
    inner: Arc<Mutex<AppInner>>,
    // pub vulkan_context: Arc<WindowContext>,
    // windows: VulkanoWindows,
    window: Arc<Window>,
    // surface: Surface<'static>,
    // surface_config: SurfaceConfiguration,
    egui_context: egui::Context,
    egui_wgpu_painter: egui_wgpu::winit::Painter,
    viewport_id: ViewportId,
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
        let window_attributes = Window::default_attributes()
            .with_title("Bitang".to_string())
            .with_inner_size(Size::Logical(LogicalSize::new(1280.0, 1000.0)));
        let window = Arc::new(event_loop.create_window(window_attributes)?);
        let size = window.inner_size();

        let mut egui_context = egui::Context::default();

        let wgpu_configuration = egui_wgpu::WgpuConfiguration {
            // wgpu_setup: WgpuSetup::Existing(WgpuSetupExisting {
            //     instance: gpu_context.instance.clone(),
            //     adapter: gpu_context.adapter.clone(),
            //     device: gpu_context.device.clone(),
            //     queue: gpu_context.queue.clone(),
            // }),
            wgpu_setup: WgpuSetup::CreateNew(WgpuSetupCreateNew {
                device_descriptor: Arc::new(|adapter| wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::FLOAT32_FILTERABLE,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        let viewport_id = ViewportId(egui::Id::from("main_window"));

        let mut egui_wgpu_painter = tokio::runtime::Runtime::new()?.block_on(async {
            let mut painter = egui_wgpu::winit::Painter::new(
                egui_context.clone(),
                wgpu_configuration,
                1,
                None,
                false,
                // TODO: dithering?
                false,
            )
            .await;

            painter
                .set_window(
                    viewport_id,
                    Some(window.clone()),
                )
                .await?;

            Ok::<_, egui_wgpu::WgpuError>(painter)
        })?;

        let render_state = egui_wgpu_painter.render_state().context("No WGPU render state")?;

        let swapchain_pixel_format = PixelFormat::from_wgpu_format(render_state.target_format)?;
        let final_render_target =
            BitangImage::new_swapchain(SCREEN_RENDER_TARGET_ID, swapchain_pixel_format);

        let gpu_context = Arc::new(GpuContext {
            adapter: render_state.adapter.clone(),
            queue: render_state.queue.clone(),
            device: render_state.device.clone(),
            final_render_target,
        });

        // let surface = gpu_context.instance.create_surface(window.clone())?;
        // let surface_config = surface
        //     .get_default_config(&gpu_context.adapter, size.width, size.height)
        //     .context("No default config found")?;
        // surface.configure(&gpu_context.device, &surface_config);

        // let vulkano_context = init_context.vulkano_context.clone();
        // let vulkan_context = init_context.into_vulkan_context(final_render_target);

        let mut content_renderer = ContentRenderer::new(&gpu_context)?;
        info!("Init DOOM refresh daemon...");
        content_renderer.reset_simulation(&gpu_context)?;

        let ui = Ui::new()?;

        let inner = AppInner {
            gpu_context,
            is_fullscreen: false,
            ui,
            content_renderer,
            app_start_time: Instant::now(),
            demo_mode: START_IN_DEMO_MODE,
        };

        let inner = Arc::new(Mutex::new(inner));

        let mut app = Self {
            inner: inner.clone(),
            window,
            // surface,
            // surface_config,
            egui_context,
            egui_wgpu_painter,
            viewport_id,
        };

        if START_IN_DEMO_MODE {
            // Start demo in fullscreen
            info!("Starting demo.");
            app.toggle_fullscreen();
            // TODO: fugly
            let mut lock = app.inner.lock().unwrap();
            lock.content_renderer.play();
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
        // self.ui.handle_window_event(&event);
        match event {
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                let new_size = self.window.inner_size();
                if let Some(width) = NonZeroU32::new(new_size.width) {
                    if let Some(height) = NonZeroU32::new(new_size.height) {
                        self.egui_wgpu_painter.on_window_resized(self.viewport_id, width, height);
                    }   
                }
                // if new_size.width > 0 && new_size.height > 0 {
                //     // self.surface_config.width = new_size.width;
                //     // self.surface_config.height = new_size.height;
                //     // self.surface.configure(&self.gpu_context.device, &self.surface_config);
                // }
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
                            self.inner.lock().unwrap().demo_mode = false;
                        }
                        Key::Named(NamedKey::Escape) => {
                            // if self.demo_mode {
                            //     info!("Exiting on user request.");
                            //     event_loop.exit();
                            // } else if self.is_fullscreen {
                            //     self.toggle_fullscreen();
                            //     self.content_renderer.app_state.pause();
                            // }
                        }
                        Key::Named(NamedKey::Space) => {
                            self.inner.lock().unwrap().content_renderer.toggle_play();
                            // self.content_renderer.toggle_play();
                        }
                        Key::Named(NamedKey::F1) => {
                            // if let Err(err) =
                            //     self.content_renderer.reset_simulation(&self.gpu_context)
                            // {
                            //     error!("Failed to reset simulation: {:?}", err);
                            // }
                            // // Skip the time spent resetting the simulation
                            // self.content_renderer
                            //     .set_last_render_time(self.app_start_time.elapsed().as_secs_f32());
                        }
                        Key::Named(NamedKey::F2) => {
                            // self.content_renderer.app_state.is_simulation_enabled =
                            //     !self.content_renderer.app_state.is_simulation_enabled;
                        }
                        _ => (),
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(err) = self.render_frame_to_screen() {
                    error!("Failed to render frame: {err}");
                } else if self.has_timeline_ended() {
                    // if self.demo_mode {
                    //     info!("Everything that has a beginning must have an end.");
                    //     event_loop.exit();
                    // } else if self.is_fullscreen {
                    //     self.toggle_fullscreen();
                    // }
                    // self.content_renderer.stop();
                }
            }
            _ => (),
        }
    }

    fn render_frame_to_screen(&mut self) -> Result<()> {
        // TODO: gui
        // Render UI
        // if self.window.fullscreen().is_none() {}

        // if !self.is_fullscreen && ui_height > 0.0 {
        //     let pixels_per_point =
        //         if scale_factor > 1.0 { scale_factor } else { 1.15f32 * scale_factor };
        //     let bottom_panel_height = ui_height / pixels_per_point;

        //     let full_outout = self.egui_context.run(new_input, |ctx| {
        //         ctx.set_pixels_per_point(pixels_per_point);
        //         self.ui.draw(
        //             ctx,
        //             &mut self.content_renderer.app_state,
        //             bottom_panel_height,
        //         );
        //     });

        //     let clipped_primitives = self.egui_context.tessellate(shapes, pixels_per_point);

        //     let user_cmd_bufs = {
        //         let renderer = &mut self.egui_wgpu_renderer;
        //         for (id, image_delta) in &textures_delta.set {
        //             renderer.update_texture(
        //                 &self.gpu_context.device,
        //                 &self.gpu_context.queue,
        //                 *id,
        //                 image_delta,
        //             );
        //         }

        //         renderer.update_buffers(
        //             &self.gpu_context.device,
        //             &self.gpu_context.queue,
        //             &mut encoder,
        //             clipped_primitives,
        //             &screen_descriptor,
        //         )
        //     };

        // }

        Ok(())
    }   

    fn has_timeline_ended(&self) -> bool {
        false // TODO
    }

    fn toggle_fullscreen(&mut self) {
        // let renderer = self.windows.get_primary_renderer_mut().unwrap();
        if self.window.fullscreen().is_none() {
            self.inner.lock().unwrap().is_fullscreen = true;
            if BORDERLESS_FULL_SCREEN {
                self.window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                self.window.set_cursor_visible(false);
            } else if let Some(monitor) = self.window.current_monitor() {
                let video_mode =
                    monitor.video_modes().find(|mode| mode.size() == PhysicalSize::new(1920, 1080));
                if let Some(video_mode) = video_mode {
                    self.window.set_fullscreen(Some(Fullscreen::Exclusive(video_mode)));
                    self.window.set_cursor_visible(false);
                } else {
                    error!("Could not find 1920x1080 video mode");
                }
            } else {
                error!("Could not find current monitor");
            }
        } else {
            self.inner.lock().unwrap().is_fullscreen = false;
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
