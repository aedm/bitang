use std::cmp::max;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{Context, Result};
use eframe::egui;
use egui::{Pos2, ViewportBuilder};
use egui_wgpu::{WgpuSetup, WgpuSetupCreateNew};
use smallvec::SmallVec;
use tracing::{debug, error, info};
#[cfg(windows)]
use wgpu::Backends;

use crate::engine::{
    BitangImage, FrameContext, GpuContext, PixelFormat, RenderPassDrawBatch, Size2D,
    SwapchainImage, Viewport,
};
use crate::tool::app_config::AppConfig;
use crate::tool::content_renderer::ContentRenderer;
use crate::tool::ui::Ui;
use crate::tool::SCREEN_RATIO;

pub struct WindowRunner {}

impl WindowRunner {
    pub fn run() -> Result<()> {
        let inner = Arc::new(Mutex::new(None::<AppInner>));
        let inner_clone = inner.clone();

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
            // on_draw_background: Some(Rc::new(move |props| {
            //     let mut inner = inner_clone.lock().unwrap();
            //     let Some(app_inner) = inner.as_mut() else {
            //         error!("Failed to get app inner");
            //         return;
            //     };
            //     app_inner.render_frame_to_screen(props).unwrap();
            // })),
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

        // TODO: no unwrap
        eframe::run_native(
            "Bitang",
            native_options,
            Box::new(|cc| {
                // TODO: no unwrap
                Ok(App::new(cc, inner).unwrap())
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
            x: left,
            y: top,
            size: [width, height],
        };
    }

    fn render_frame_to_screen(&mut self) -> RenderPassDrawBatch {
        // self.compute_viewport(props.surface_size);

        // Update swapchain target
        // let swapchain_view = props.surface_view;
        // let swapchain_image = Some(SwapchainImage {
        //     texture_view: swapchain_view,
        //     size: props.surface_size,
        // });
        // self.gpu_context.final_render_target.set_swapchain_image_view(swapchain_image);

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

        // Set swapchain image view to None, DX12 would fail without this
        // self.gpu_context.final_render_target.set_swapchain_image_view(None);

        frame_context.screen_pass_draw_batch
    }

    // fn render_frame_to_screen(&mut self, props: egui_wgpu::BackgroundRenderProps) -> Result<()> {
    //     // Don't render anything if the window is minimized
    //     if props.surface_size[0] == 0 || props.surface_size[1] == 0 {
    //         return Ok(());
    //     }

    //     self.compute_viewport(props.surface_size);

    //     // Update swapchain target
    //     let swapchain_view = props.surface_view;
    //     let swapchain_image = Some(SwapchainImage {
    //         texture_view: swapchain_view,
    //         size: props.surface_size,
    //     });
    //     self.gpu_context.final_render_target.set_swapchain_image_view(swapchain_image);

    //     // Create frame context
    //     // TODO: create it content_renderer
    //     let command_encoder = self
    //         .gpu_context
    //         .device
    //         .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    //     let mut frame_context = FrameContext {
    //         gpu_context: self.gpu_context.clone(),
    //         command_encoder,
    //         globals: Default::default(),
    //         screen_viewport: self.viewport,
    //     };
    //     frame_context.globals.app_time = self.app_start_time.elapsed().as_secs_f32();

    //     // Reload project
    //     // TODO: start render function with this block
    //     if self.content_renderer.reload_project(&self.gpu_context) {
    //         self.content_renderer.reset_simulation(&self.gpu_context).unwrap();
    //         frame_context.globals.app_time = self.app_start_time.elapsed().as_secs_f32();
    //         self.content_renderer.unset_last_render_time();
    //     }

    //     // Render content
    //     self.content_renderer.draw(&mut frame_context);

    //     // Execute commands and display the result
    //     self.gpu_context.queue.submit(Some(frame_context.command_encoder.finish()));

    //     // Set swapchain image view to None, DX12 would fail without this
    //     self.gpu_context.final_render_target.set_swapchain_image_view(None);

    //     Ok(())
    // }

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
        // println!("scale factor: {}", scale_factor);
        let size = ctx.input(|i: &egui::InputState| i.screen_rect()).size();
        // println!("size: {}x{}", size.x, size.y);

        // Don't render anything if the window is minimized
        if size[0] <= 0.0 || size[1] <= 0.0 {
            return;
        }

        // println!(
        //     "viewport size: {}x{}",
        //     self.viewport.size[0], self.viewport.size[1]
        // );

        // let rect = egui::Rect::from_min_size(
        //     egui::Pos2::new(0.0, 0.0),
        //     egui::Vec2::new(self.viewport.size[0] as f32, self.viewport.size[1] as f32),
        // );
        // ctx.set_pixels_per_point(1.0);
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

        // let pixels_per_point =
        //     if scale_factor > 1.0 { scale_factor } else { 1.15f32 * scale_factor };
        // let bottom_panel_height = self.ui_height / pixels_per_point;

        // if bottom_panel_height > 0.0 {
        //     ctx.set_pixels_per_point(pixels_per_point);
        //     self.ui.draw(
        //         ctx,
        //         &mut self.content_renderer.app_state,
        //         bottom_panel_height,
        //     );
        // }
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
    screen_pass_batch: RenderPassDrawBatch,
}

impl egui_wgpu::CallbackTrait for CustomRenderCallback {
    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        self.screen_pass_batch.render(render_pass);
    }
}

struct App {
    inner: Arc<Mutex<Option<AppInner>>>,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(inner) = &mut *inner {
            inner.render_ui(ctx, frame);
            ctx.request_repaint();
        }
    }
}

impl App {
    fn new(
        cc: &eframe::CreationContext<'_>,
        inner: Arc<Mutex<Option<AppInner>>>,
    ) -> Result<Box<dyn eframe::App>> {
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
        let app_inner = AppInner {
            gpu_context,
            is_fullscreen: demo_mode,
            ui,
            content_renderer,
            app_start_time: Instant::now(),
            demo_mode,
            viewport: Viewport::default(),
            ui_height: 0.0,
        };

        *inner.lock().unwrap() = Some(app_inner);

        let app = Self { inner };

        if demo_mode {
            // Start demo in fullscreen
            info!("Starting demo.");
            // TODO: this is not pretty

            let mut lock = app.inner.lock().unwrap();
            let inner = lock.as_mut().unwrap();
            inner.content_renderer.play();
        }

        Ok(Box::new(app))
    }
}
