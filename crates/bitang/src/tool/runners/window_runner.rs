use std::cmp::max;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use eframe::egui;
use egui::ViewportBuilder;
use egui_wgpu::{WgpuSetup, WgpuSetupCreateNew};
use parking_lot::Mutex;
use tracing::{debug, error, info};
use wgpu::Backends;

use crate::engine::{
    BitangImage, FrameContext, FramebufferInfo, GpuContext, PixelFormat, RenderStage, Size2D,
    Viewport,
};
use crate::tool::content_renderer::ContentRenderer;
use crate::tool::ui::Ui;
use crate::tool::{SCREEN_RATIO, START_IN_DEMO_MODE};

pub struct WindowRunner {}

impl WindowRunner {
    pub fn run() -> Result<()> {
        let wgpu_configuration = egui_wgpu::WgpuConfiguration {
            #[cfg(windows)]
            present_mode: wgpu::PresentMode::Mailbox,
            desired_maximum_frame_latency: Some(2),
            wgpu_setup: WgpuSetup::CreateNew(WgpuSetupCreateNew {
                instance_descriptor: wgpu::InstanceDescriptor {
                    #[cfg(windows)]
                    backends: Backends::DX12,
                    ..Default::default()
                },
                device_descriptor: Arc::new(|_adapter| wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::FLOAT32_FILTERABLE
                        | wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER
                        | wgpu::Features::VERTEX_WRITABLE_STORAGE,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let viewport_builder = ViewportBuilder::default().with_title("Bitang");
        let viewport_builder = if START_IN_DEMO_MODE {
            viewport_builder.with_fullscreen(true)
        } else {
            viewport_builder.with_inner_size(egui::vec2(1280.0, 1000.0))
        };
        let native_options = eframe::NativeOptions {
            wgpu_options: wgpu_configuration,
            viewport: viewport_builder,
            ..eframe::NativeOptions::default()
        };

        eframe::run_native(
            "Bitang",
            native_options,
            Box::new(|cc| {
                // TODO: no unwrap
                Ok(App::new(cc).unwrap())
            }),
        )
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to run app: {e:?}"),
            )
        })?;
        Ok(())
    }
}

struct AppInner {
    gpu_context: Arc<GpuContext>,
    is_fullscreen: bool,
    ui: Ui,
    content_renderer: ContentRenderer,
    app_start_time: Instant,
    demo_mode: bool,

    viewport: Viewport,
    ui_height: f32,
}

impl AppInner {
    fn compute_viewport(&mut self, window_size: Size2D) {
        // Calculate viewport
        let (width, height, top, left) = if self.is_fullscreen {
            if window_size[0] * SCREEN_RATIO.1 >= window_size[1] * SCREEN_RATIO.0 {
                (window_size[0], window_size[1], 0, 0)
            } else {
                // Window is too tall
                let width = window_size[0];
                let height = width * SCREEN_RATIO.1 / SCREEN_RATIO.0;
                let left = 0;
                let top = (window_size[1] - height) / 2;
                (width, height, top, left)
            }
        } else {
            if window_size[0] * SCREEN_RATIO.1 > window_size[1] * SCREEN_RATIO.0 {
                // Window is too wide
                let height = window_size[1];
                let width = height * SCREEN_RATIO.0 / SCREEN_RATIO.1;
                let left = (window_size[0] - width) / 2;
                let top = 0;
                (width, height, top, left)
            } else {
                // Window is too tall
                let width = window_size[0];
                let height = width * SCREEN_RATIO.1 / SCREEN_RATIO.0;
                let left = 0;
                let top = 0;
                (width, height, top, left)
            }
        };
        self.ui_height = max(window_size[1] as i32 - height as i32, 0) as f32;
        self.viewport = Viewport {
            x: left,
            y: top,
            size: [width, height],
        };
    }

    fn render_offscreen_content(
        &mut self,
        cb: &CustomRenderCallback,
        command_encoder: &mut wgpu::CommandEncoder,
    ) {
        // Don't render anything if the window is minimized
        if cb.window_size[0] == 0 || cb.window_size[1] == 0 {
            return;
        }

        self.compute_viewport(cb.window_size);

        // Create frame context
        // TODO: create it content_renderer
        let mut frame_context = FrameContext {
            gpu_context: self.gpu_context.clone(),
            render_stage: RenderStage::Offscreen(command_encoder),
            globals: Default::default(),
            screen_viewport: self.viewport,
        };
        frame_context.globals.app_time = cb.render_time;

        // Reload project
        // TODO: start render function with this block
        if self.content_renderer.reload_project(&self.gpu_context) {
            self.content_renderer.reset_simulation(&self.gpu_context).unwrap();
            frame_context.globals.app_time = self.app_start_time.elapsed().as_secs_f32();
            self.content_renderer.unset_last_render_time();
        }

        // Render content
        self.content_renderer.draw(&mut frame_context);
    }

    fn render_onscreen_content(
        &mut self,
        cb: &CustomRenderCallback,
        render_pass: &mut wgpu::RenderPass<'static>,
    ) {
        // Don't render anything if the window is minimized
        if cb.window_size[0] == 0 || cb.window_size[1] == 0 {
            return;
        }

        // Create frame context
        // TODO: create it content_renderer
        let mut frame_context = FrameContext {
            gpu_context: self.gpu_context.clone(),
            render_stage: RenderStage::Onscreen(render_pass),
            globals: Default::default(),
            screen_viewport: self.viewport,
        };
        frame_context.globals.app_time = cb.render_time;

        // Render content
        self.content_renderer.draw(&mut frame_context);
    }

    fn has_timeline_ended(&self) -> bool {
        let Some(project) = &self.content_renderer.app_state.project else {
            return false;
        };
        self.content_renderer.app_state.cursor_time >= project.length
    }

    fn render_ui(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_hotkeys(&ctx);

        // let scale_factor = ctx.pixels_per_point();
        // let size = ctx.input(|i: &egui::InputState| i.screen_rect()).size() * scale_factor;
        let size = ctx.input(|i: &egui::InputState| i.screen_rect()).size();
        self.compute_viewport([size.x as u32, size.y as u32]);
        // let pixels_per_point =
        //     if scale_factor > 1.0 { scale_factor } else { 1.15f32 * scale_factor };
        // let bottom_panel_height = self.ui_height / pixels_per_point;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                let size =
                    egui::Vec2::new(self.viewport.size[0] as f32, self.viewport.size[1] as f32);
                let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::hover());
                ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                    rect,
                    CustomRenderCallback {
                        window_size: self.viewport.size,
                        render_time: self.app_start_time.elapsed().as_secs_f32(),
                    },
                ));

                // ctx.set_pixels_per_point(pixels_per_point);
                self.ui.draw(ui, &mut self.content_renderer.app_state);
            });
        });

        if self.demo_mode && self.has_timeline_ended() {
            // Avoid logging twice as eframe doesn't close the viewport immediately.
            self.demo_mode = false;
            info!("End of demo.");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    const SAVE_SHORTCUT: egui::KeyboardShortcut =
        egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::S);
    const FULLSCREEN_SHORTCUT: egui::KeyboardShortcut =
        egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F11);
    const RESET_SIMULATION_SHORTCUT: egui::KeyboardShortcut =
        egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F1);
    const TOGGLE_SIMULATION_SHORTCUT: egui::KeyboardShortcut =
        egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F2);
    const TOGGLE_PLAY_SHORTCUT: egui::KeyboardShortcut =
        egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Space);
    const STOP_SHORTCUT: egui::KeyboardShortcut =
        egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Escape);

    fn handle_hotkeys(&mut self, ctx: &egui::Context) {
        // Save
        if ctx.input_mut(|i| i.consume_shortcut(&Self::SAVE_SHORTCUT)) {
            if let Some(project) = &self.content_renderer.app_state.project {
                if let Err(err) =
                    self.content_renderer.app_state.control_repository.save_control_files(project)
                {
                    error!("Failed to save controls: {err}");
                }
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&Self::FULLSCREEN_SHORTCUT)) {
            self.toggle_fullscreen(ctx);
            self.demo_mode = false;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&Self::RESET_SIMULATION_SHORTCUT)) {
            self.content_renderer.reset_simulation(&self.gpu_context).unwrap();
            self.content_renderer.unset_last_render_time();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&Self::TOGGLE_SIMULATION_SHORTCUT)) {
            self.content_renderer.app_state.is_simulation_enabled =
                !self.content_renderer.app_state.is_simulation_enabled;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&Self::TOGGLE_PLAY_SHORTCUT)) {
            self.content_renderer.toggle_play();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&Self::STOP_SHORTCUT)) {
            if self.demo_mode {
                info!("Exiting on user request.");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else if self.is_fullscreen {
                self.toggle_fullscreen(ctx);
                self.content_renderer.app_state.pause();
            }
        }
    }

    fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        self.is_fullscreen = !self.is_fullscreen;
        debug!("Setting fullscreen to {}", self.is_fullscreen);
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.is_fullscreen));
    }
}

struct CustomRenderCallback {
    // app: Arc<Mutex<AppInner>>,
    window_size: Size2D,
    render_time: f32,
}

impl egui_wgpu::CallbackTrait for CustomRenderCallback {
    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let inner: &Arc<Mutex<AppInner>> = callback_resources.get().unwrap();
        let mut inner = inner.lock();
        inner.render_onscreen_content(self, render_pass);
    }

    fn prepare(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let inner: &Arc<Mutex<AppInner>> = callback_resources.get().unwrap();
        let mut inner = inner.lock();
        inner.render_offscreen_content(self, egui_encoder);
        Vec::new()
    }
}

struct App {
    inner: Arc<Mutex<AppInner>>,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let mut inner = self.inner.lock();
        inner.render_ui(ctx, frame);
        ctx.request_repaint();
    }
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Result<Box<dyn eframe::App>> {
        let render_state = cc.wgpu_render_state.as_ref().context("No WGPU render state")?;

        let adapter_info = render_state.adapter.get_info();
        info!(
            "WGPU adapter: {:?} on {}",
            adapter_info.backend, adapter_info.name
        );

        let swapchain_pixel_format = PixelFormat::from_wgpu_format(render_state.target_format)?;
        let final_render_target = BitangImage::new_swapchain("__screen", swapchain_pixel_format);

        let swapchain_framebuffer_info = FramebufferInfo {
            color_buffer_formats: vec![final_render_target.pixel_format],
            depth_buffer_format: None,
        };

        let gpu_context = Arc::new(GpuContext {
            adapter: render_state.adapter.clone(),
            queue: render_state.queue.clone(),
            device: render_state.device.clone(),
            final_render_target,
            swapchain_framebuffer_info,
        });

        let mut content_renderer = ContentRenderer::new(&gpu_context)?;
        info!("Init DOOM refresh daemon...");
        content_renderer.reset_simulation(&gpu_context)?;

        let ui = Ui::new()?;

        let app_inner = AppInner {
            gpu_context,
            is_fullscreen: START_IN_DEMO_MODE,
            ui,
            content_renderer,
            app_start_time: Instant::now(),
            demo_mode: START_IN_DEMO_MODE,
            viewport: Viewport::default(),
            ui_height: 0.0,
        };

        let inner = Arc::new(Mutex::new(app_inner));

        render_state.renderer.write().callback_resources.insert(inner.clone());

        let app = Self { inner };

        if START_IN_DEMO_MODE {
            // Start demo in fullscreen
            info!("Starting demo.");
            app.inner.lock().content_renderer.play();
        }

        Ok(Box::new(app))
    }
}
