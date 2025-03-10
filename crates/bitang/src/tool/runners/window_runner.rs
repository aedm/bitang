use crate::render::image::{BitangImage, PixelFormat, SwapchainImage};
use crate::render::{Size2D, SCREEN_COLOR_FORMAT, SCREEN_RENDER_TARGET_ID};
use crate::tool::content_renderer::ContentRenderer;
use crate::tool::ui::Ui;
use crate::tool::{
    FrameContext, GpuContext, Viewport, BORDERLESS_FULL_SCREEN, SCREEN_RATIO, START_IN_DEMO_MODE,
};
use anyhow::{Context, Result};
use eframe::egui;
use egui::ViewportId;
use egui_wgpu::BackgroundRenderProps;
use egui_wgpu::{WgpuSetup, WgpuSetupCreateNew, WgpuSetupExisting};
use std::cmp::max;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{error, info};
use wgpu::{Backends, Surface, SurfaceConfiguration};

pub struct WindowRunner {}

impl WindowRunner {
    pub fn run() -> Result<()> {
        let inner = Arc::new(Mutex::new(None::<AppInner>));
        let inner_clone = inner.clone();

        let wgpu_configuration = egui_wgpu::WgpuConfiguration {
            // present_mode: wgpu::PresentMode::FifoRelaxed,
            present_mode: wgpu::PresentMode::Mailbox,
            desired_maximum_frame_latency: Some(2),
            wgpu_setup: WgpuSetup::CreateNew(WgpuSetupCreateNew {
                instance_descriptor: wgpu::InstanceDescriptor {
                    // TODO: DX12 then Vulkan then Metal
                    backends: Backends::DX12,
                    // backends: Backends::VULKAN,
                    ..Default::default()
                },
                device_descriptor: Arc::new(|_adapter| wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::FLOAT32_FILTERABLE
                        | wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            on_draw_background: Some(Rc::new(move |props| {
                let mut inner = inner_clone.lock().unwrap();
                let Some(app_inner) = inner.as_mut() else {
                    error!("Failed to get app inner");
                    return;
                };
                app_inner.render_frame_to_screen(props).unwrap();
            })),
            ..Default::default()
        };
        let native_options = eframe::NativeOptions {
            wgpu_options: wgpu_configuration,
            ..eframe::NativeOptions::default()
        };

        // TODO: no unwrap
        eframe::run_native(
            "Bitang",
            native_options,
            Box::new(|cc| {
                // TODO: unwrap
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

    fn render_frame_to_screen(&mut self, props: egui_wgpu::BackgroundRenderProps) -> Result<()> {
        // Don't render anything if the window is minimized
        if props.surface_size[0] == 0 || props.surface_size[1] == 0 {
            return Ok(());
        }

        self.compute_viewport(props.surface_size);

        // Update swapchain target
        let swapchain_view = props.surface_view;
        let swapchain_image = Some(SwapchainImage {
            texture_view: swapchain_view,
            size: props.surface_size,
        });
        self.gpu_context.final_render_target.set_swapchain_image_view(swapchain_image);

        // Create frame context
        let command_encoder = self
            .gpu_context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let mut frame_context = FrameContext {
            gpu_context: self.gpu_context.clone(),
            command_encoder,
            globals: Default::default(),
            simulation_elapsed_time_since_last_render: 0.0,
            screen_viewport: self.viewport,
            canvas_size: props.surface_size,
        };
        frame_context.globals.app_time = self.app_start_time.elapsed().as_secs_f32();

        // Reload project
        // TODO: start render function with this block
        if self.content_renderer.reload_project(&self.gpu_context) {
            self.content_renderer.reset_simulation(&self.gpu_context).unwrap();
            frame_context.globals.app_time = self.app_start_time.elapsed().as_secs_f32();
            self.content_renderer.set_last_render_time(frame_context.globals.app_time);
        }

        // Render content
        self.content_renderer.draw(&mut frame_context);

        // Execute commands and display the result
        self.gpu_context.queue.submit(Some(frame_context.command_encoder.finish()));

        // Set swapchain image view to None, DX12 would fail without this
        self.gpu_context.final_render_target.set_swapchain_image_view(None);

        Ok(())
    }

    fn has_timeline_ended(&self) -> bool {
        let Some(project) = &self.content_renderer.app_state.project else {
            return false;
        };
        self.content_renderer.app_state.cursor_time >= project.length
    }

    fn render_ui(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let scale_factor = ctx.pixels_per_point();
        let size = ctx.input(|i: &egui::InputState| i.screen_rect()).size() * scale_factor;
        self.compute_viewport([size.x as u32, size.y as u32]);
        let pixels_per_point =
            if scale_factor > 1.0 { scale_factor } else { 1.15f32 * scale_factor };
        let bottom_panel_height = self.ui_height / pixels_per_point;

        if bottom_panel_height <= 0.0 {
            return;
        }

        ctx.set_pixels_per_point(pixels_per_point);
        self.ui.draw(
            ctx,
            &mut self.content_renderer.app_state,
            bottom_panel_height,
        );

        self.handle_hotkeys(&ctx);
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
            self.toggle_fullscreen();
            self.demo_mode = false;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&Self::RESET_SIMULATION_SHORTCUT)) {
            self.content_renderer.reset_simulation(&self.gpu_context).unwrap();
            self.content_renderer.set_last_render_time(self.app_start_time.elapsed().as_secs_f32());
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
                // event_loop.exit();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else if self.is_fullscreen {
                self.toggle_fullscreen();
                self.content_renderer.app_state.pause();
            }
        }
    }

    fn toggle_fullscreen(&mut self) {
        todo!();
        // self.is_fullscreen = !self.is_fullscreen;
        // self.window.set_fullscreen(self.is_fullscreen);
    }

    // fn toggle_fullscreen(&mut self) {
    //     todo!()
    //     // let renderer = self.windows.get_primary_renderer_mut().unwrap();
    //     // if self.window.fullscreen().is_none() {
    //     //     self.inner.lock().unwrap().is_fullscreen = true;
    //     //     if BORDERLESS_FULL_SCREEN {
    //     //         self.window.set_fullscreen(Some(Fullscreen::Borderless(None)));
    //     //         self.window.set_cursor_visible(false);
    //     //     } else if let Some(monitor) = self.window.current_monitor() {
    //     //         let video_mode =
    //     //             monitor.video_modes().find(|mode| mode.size() == PhysicalSize::new(1920, 1080));
    //     //         if let Some(video_mode) = video_mode {
    //     //             self.window.set_fullscreen(Some(Fullscreen::Exclusive(video_mode)));
    //     //             self.window.set_cursor_visible(false);
    //     //         } else {
    //     //             error!("Could not find 1920x1080 video mode");
    //     //         }
    //     //     } else {
    //     //         error!("Could not find current monitor");
    //     //     }
    //     // } else {
    //     //     self.inner.lock().unwrap().is_fullscreen = false;
    //     //     self.window.set_fullscreen(None);
    //     //     self.window.set_cursor_visible(true);
    //     //     self.window.focus_window();
    //     // }
    // }
}

struct App {
    inner: Arc<Mutex<Option<AppInner>>>,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(inner) = &mut *inner {
            inner.render_ui(ctx, frame);
            // ctx.request_repaint();
        }
    }
}

pub enum PaintResult {
    None,
    EndReached,
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
        let final_render_target =
            BitangImage::new_swapchain(SCREEN_RENDER_TARGET_ID, swapchain_pixel_format);

        let gpu_context = Arc::new(GpuContext {
            adapter: render_state.adapter.clone(),
            queue: render_state.queue.clone(),
            device: render_state.device.clone(),
            final_render_target,
        });

        let mut content_renderer = ContentRenderer::new(&gpu_context)?;
        info!("Init DOOM refresh daemon...");
        content_renderer.reset_simulation(&gpu_context)?;

        let ui = Ui::new()?;

        let app_inner = AppInner {
            gpu_context,
            is_fullscreen: false,
            ui,
            content_renderer,
            app_start_time: Instant::now(),
            demo_mode: START_IN_DEMO_MODE,
            viewport: Viewport::default(),
            ui_height: 0.0,
        };

        *inner.lock().unwrap() = Some(app_inner);

        let mut app = Self { inner };

        if START_IN_DEMO_MODE {
            // Start demo in fullscreen
            info!("Starting demo.");
            // TODO: fugly
            let mut lock = app.inner.lock().unwrap();
            let inner = lock.as_mut().unwrap();
            inner.toggle_fullscreen();
            inner.content_renderer.play();
        }

        Ok(Box::new(app))
    }

    fn has_timeline_ended(&self) -> bool {
        false // TODO
    }
}
