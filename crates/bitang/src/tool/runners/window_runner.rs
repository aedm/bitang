use std::cmp::max;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use eframe::egui;
use egui::ViewportBuilder;
use egui_wgpu::{WgpuSetup, WgpuSetupCreateNew};
use smallvec::SmallVec;
use tracing::{debug, error, info};
#[cfg(windows)]
use wgpu::Backends;

use crate::engine::{
    BitangImage, FrameContext, GpuContext, PixelFormat, RenderPassDrawBatch, Size2D, Viewport,
};
use crate::tool::app_config::AppConfig;
use crate::tool::content_renderer::ContentRenderer;
use crate::tool::ui::Ui;
use crate::tool::SCREEN_RATIO;

pub struct WindowRunner {
    gpu_context: Arc<GpuContext>,
    is_fullscreen: bool,
    ui: Ui,
    content_renderer: ContentRenderer,
    app_start_time: Instant,
    demo_mode: bool,

    viewport: Viewport,
    ui_height: f32,
}

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
        let viewport_builder = if AppConfig::get().start_in_demo_mode {
            viewport_builder.with_fullscreen(true)
        } else {
            viewport_builder.with_inner_size(egui::vec2(960.0, 800.0))
        };
        let native_options = eframe::NativeOptions {
            wgpu_options: wgpu_configuration,
            viewport: viewport_builder,
            ..eframe::NativeOptions::default()
        };

        eframe::run_native(
            "Bitang",
            native_options,
            Box::new(|cc| Ok(Box::new(Self::new(cc)?))),
        )
        .map_err(|e| anyhow::anyhow!("Failed to run app: {e:?}"))
    }

    fn new(cc: &eframe::CreationContext<'_>) -> Result<Self> {
        let render_state = cc.wgpu_render_state.as_ref().context("No WGPU render state")?;

        let adapter_info = render_state.adapter.get_info();
        info!(
            "WGPU adapter: {:?} on {}",
            adapter_info.backend, adapter_info.name
        );

        let swapchain_pixel_format = PixelFormat::from_wgpu_format(render_state.target_format)?;
        let final_render_target = BitangImage::new_swapchain("__screen", swapchain_pixel_format);

        let gpu_context = Arc::new(GpuContext {
            adapter: render_state.adapter.clone(),
            queue: render_state.queue.clone(),
            device: render_state.device.clone(),
            final_render_target,
        });

        let mut content_renderer = ContentRenderer::new(&gpu_context)?;
        content_renderer.reset_simulation(&gpu_context)?;

        let ui = Ui::new()?;

        let demo_mode = AppConfig::get().start_in_demo_mode;

        if demo_mode {
            // Start demo in fullscreen
            info!("Starting demo.");
            content_renderer.play();
        }

        Ok(Self {
            gpu_context,
            is_fullscreen: demo_mode,
            ui,
            content_renderer,
            app_start_time: Instant::now(),
            demo_mode,
            viewport: Viewport::default(),
            ui_height: 0.0,
        })
    }

    fn compute_viewport(&mut self, swapchain_size: Size2D) {
        // Calculate viewport
        // TODO: simplify, the is_fullscreen flag is probably not used
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
                let top = 0;
                (width, height, top, left)
            }
        };
        self.ui_height = max(swapchain_size[1] as i32 - height as i32, 0) as f32;
        self.viewport = Viewport {
            _x: left,
            _y: top,
            size: [width, height],
        };
    }

    fn render_frame_to_screen(&mut self) -> RenderPassDrawBatch {
        // Create frame context
        // TODO: create it content_renderer
        let command_encoder = self
            .gpu_context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let mut frame_context = FrameContext {
            gpu_context: self.gpu_context.clone(),
            command_encoder,
            globals: Default::default(),
            screen_size: self.viewport.size,
            screen_pass_draw_batch: RenderPassDrawBatch {
                draw_commands: SmallVec::new(),
            },
        };
        frame_context.globals.app_time = self.app_start_time.elapsed().as_secs_f32();

        // Reload project
        // TODO: start render function with this block
        if self.content_renderer.reload_project(&self.gpu_context) {
            self.content_renderer.reset_simulation(&self.gpu_context).unwrap();
            frame_context.globals.app_time = self.app_start_time.elapsed().as_secs_f32();
            self.content_renderer.unset_last_render_time();
        }

        // Render content
        self.content_renderer.draw(&mut frame_context);

        // Execute commands and display the result
        self.gpu_context.queue.submit(Some(frame_context.command_encoder.finish()));

        frame_context.screen_pass_draw_batch
    }

    fn has_timeline_ended(&self) -> bool {
        let Some(project) = &self.content_renderer.app_state.project else {
            return false;
        };
        self.content_renderer.app_state.cursor_time >= project.length
    }

    fn render_ui(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_hotkeys(&ctx);

        if self.demo_mode {
            if self.has_timeline_ended() {
                // Avoid logging twice as eframe doesn't close the viewport immediately.
                self.demo_mode = false;
                info!("End of demo.");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        }

        let scale_factor = ctx.pixels_per_point();
        let size = ctx.input(|i: &egui::InputState| i.screen_rect()).size();

        // Don't render anything if the window is minimized
        if size[0] <= 0.0 || size[1] <= 0.0 {
            return;
        }

        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.vertical(|ui| {
                let panel_size = ui.max_rect().size();
                self.compute_viewport([
                    (panel_size.x * scale_factor) as u32,
                    (panel_size.y * scale_factor) as u32,
                ]);
                let screen_pass_batch = self.render_frame_to_screen();
                let custom_callback = CustomRenderCallback { screen_pass_batch };

                let desired_size = egui::Vec2::new(
                    self.viewport.size[0] as f32 / scale_factor,
                    self.viewport.size[1] as f32 / scale_factor,
                );
                let sense = egui::Sense::all();
                let (rect, _response) = ui.allocate_exact_size(desired_size, sense);
                ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                    rect,
                    custom_callback,
                ));
                self.ui.draw(ui, &mut self.content_renderer.app_state);
            });
        });
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

impl eframe::App for WindowRunner {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.render_ui(ctx, frame);
        ctx.request_repaint();
    }
}

struct CustomRenderCallback {
    screen_pass_batch: RenderPassDrawBatch,
}

impl egui_wgpu::CallbackTrait for CustomRenderCallback {
    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        _callback_resources: &egui_wgpu::CallbackResources,
    ) {
        self.screen_pass_batch.render(render_pass);
    }
}
