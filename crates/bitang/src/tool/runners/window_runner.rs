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
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{error, info};
use wgpu::{Backends, Surface, SurfaceConfiguration};
// use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage};
// use vulkano::pipeline::graphics::viewport::Viewport;
// use vulkano::sync::GpuFuture;
// use vulkano_util::renderer::VulkanoWindowRenderer;
// use vulkano_util::window::{VulkanoWindows, WindowDescriptor};
use eframe::egui;
use egui_wgpu::BackgroundRenderProps;

pub struct WindowRunner {}

impl WindowRunner {
    pub fn run() -> Result<()> {
        let inner = Arc::new(Mutex::new(None::<AppInner>));
        let inner_clone = inner.clone();

        let wgpu_configuration = egui_wgpu::WgpuConfiguration {
            // present_mode: wgpu::PresentMode::Mailbox,
            desired_maximum_frame_latency: Some(1),
            wgpu_setup: WgpuSetup::CreateNew(WgpuSetupCreateNew {
                instance_descriptor: wgpu::InstanceDescriptor {
                    backends: Backends::VULKAN,
                    ..Default::default()
                },
                device_descriptor: Arc::new(|_adapter| wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::FLOAT32_FILTERABLE,
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
            })
        ).unwrap();
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
        self.ui_height = max(swapchain_size[1] as i32 - height as i32, 0) as f32;
        self.viewport = Viewport {
            // offset: [left as f32, top as f32],
            // extent: [width as f32, height as f32],
            x: left,
            y: top,
            size: [width, height],
        };
    }


    fn render_frame_to_screen(&mut self, props: egui_wgpu::BackgroundRenderProps) -> Result<()> {
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




        // // Make render context
        // let mut render_context = FrameContext {
        //     vulkan_context: self.vulkan_context.clone(),
        //     screen_viewport,
        //     command_builder: &mut command_builder,
        //     globals: Default::default(),
        //     // simulation_elapsed_time_since_last_render: 0.0,
        // };
        // render_context.globals.app_time = self.app_start_time.elapsed().as_secs_f32();
        let encoder = self
            .gpu_context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        let mut frame_context = FrameContext {
            gpu_context: self.gpu_context.clone(),
            command_encoder: encoder,
            globals: Default::default(),
            simulation_elapsed_time_since_last_render: 0.0,
            screen_viewport: self.viewport,
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

    fn render_ui(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let scale_factor = ctx.pixels_per_point();
        let size = ctx.input(|i: &egui::InputState| i.screen_rect()).size();
        self.compute_viewport([size.x as u32, size.y as u32]);
        let pixels_per_point =
            if scale_factor > 1.0 { scale_factor } else { 1.15f32 * scale_factor };
        let bottom_panel_height = self.ui_height;

        ctx.set_pixels_per_point(pixels_per_point);
        self.ui.draw(
            ctx,
            &mut self.content_renderer.app_state,
            bottom_panel_height,
        );
    }
}

struct App {
    inner: Arc<Mutex<Option<AppInner>>>,
    // pub vulkan_context: Arc<WindowContext>,
    // windows: VulkanoWindows,
    // window: Arc<Window>,
    // // surface: Surface<'static>,
    // // surface_config: SurfaceConfiguration,
    // egui_context: egui::Context,
    // egui_wgpu_painter: egui_wgpu::winit::Painter,
    // viewport_id: ViewportId,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(inner) = &mut *inner {
            inner.render_ui(ctx, frame);
        }
    }
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

    fn new(cc: &eframe::CreationContext<'_>, inner: Arc<Mutex<Option<AppInner>>>) -> Result<Box<dyn eframe::App>> {
        let render_state = cc.wgpu_render_state.as_ref().context("No WGPU render state")?;

        let adapter_info = render_state.adapter.get_info();
        info!("WGPU adapter: {:?} on {}", adapter_info.backend, adapter_info.name);

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

        let mut app = Self {
            inner,
        };

        if START_IN_DEMO_MODE {
            // Start demo in fullscreen
            info!("Starting demo.");
            app.toggle_fullscreen();
            // TODO: fugly
            let mut lock = app.inner.lock().unwrap();
            lock.as_mut().unwrap().content_renderer.play();
        }

        Ok(Box::new(app))
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

    // fn handle_window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
    //     // self.ui.handle_window_event(&event);
    //     match event {
    //         WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
    //             let new_size = self.window.inner_size();
    //             if let Some(width) = NonZeroU32::new(new_size.width) {
    //                 if let Some(height) = NonZeroU32::new(new_size.height) {
    //                     self.egui_wgpu_painter.on_window_resized(self.viewport_id, width, height);
    //                 }
    //             }
    //             // if new_size.width > 0 && new_size.height > 0 {
    //             //     // self.surface_config.width = new_size.width;
    //             //     // self.surface_config.height = new_size.height;
    //             //     // self.surface.configure(&self.gpu_context.device, &self.surface_config);
    //             // }
    //         }
    //         WindowEvent::CloseRequested => {
    //             info!("App closed.");
    //             event_loop.exit();
    //         }
    //         WindowEvent::KeyboardInput {
    //             event: key_event, ..
    //         } => {
    //             if key_event.state == winit::event::ElementState::Pressed {
    //                 match key_event.logical_key {
    //                     Key::Named(NamedKey::F11) => {
    //                         self.toggle_fullscreen();
    //                         self.inner.lock().unwrap().demo_mode = false;
    //                     }
    //                     Key::Named(NamedKey::Escape) => {
    //                         // if self.demo_mode {
    //                         //     info!("Exiting on user request.");
    //                         //     event_loop.exit();
    //                         // } else if self.is_fullscreen {
    //                         //     self.toggle_fullscreen();
    //                         //     self.content_renderer.app_state.pause();
    //                         // }
    //                     }
    //                     Key::Named(NamedKey::Space) => {
    //                         self.inner.lock().unwrap().content_renderer.toggle_play();
    //                         // self.content_renderer.toggle_play();
    //                     }
    //                     Key::Named(NamedKey::F1) => {
    //                         // if let Err(err) =
    //                         //     self.content_renderer.reset_simulation(&self.gpu_context)
    //                         // {
    //                         //     error!("Failed to reset simulation: {:?}", err);
    //                         // }
    //                         // // Skip the time spent resetting the simulation
    //                         // self.content_renderer
    //                         //     .set_last_render_time(self.app_start_time.elapsed().as_secs_f32());
    //                     }
    //                     Key::Named(NamedKey::F2) => {
    //                         // self.content_renderer.app_state.is_simulation_enabled =
    //                         //     !self.content_renderer.app_state.is_simulation_enabled;
    //                     }
    //                     _ => (),
    //                 }
    //             }
    //         }
    //         WindowEvent::RedrawRequested => {
    //             if let Err(err) = self.render_frame_to_screen() {
    //                 error!("Failed to render frame: {err}");
    //             } else if self.has_timeline_ended() {
    //                 // if self.demo_mode {
    //                 //     info!("Everything that has a beginning must have an end.");
    //                 //     event_loop.exit();
    //                 // } else if self.is_fullscreen {
    //                 //     self.toggle_fullscreen();
    //                 // }
    //                 // self.content_renderer.stop();
    //             }
    //         }
    //         _ => (),
    //     }
    // }

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
        todo!()
        // let renderer = self.windows.get_primary_renderer_mut().unwrap();
        // if self.window.fullscreen().is_none() {
        //     self.inner.lock().unwrap().is_fullscreen = true;
        //     if BORDERLESS_FULL_SCREEN {
        //         self.window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        //         self.window.set_cursor_visible(false);
        //     } else if let Some(monitor) = self.window.current_monitor() {
        //         let video_mode =
        //             monitor.video_modes().find(|mode| mode.size() == PhysicalSize::new(1920, 1080));
        //         if let Some(video_mode) = video_mode {
        //             self.window.set_fullscreen(Some(Fullscreen::Exclusive(video_mode)));
        //             self.window.set_cursor_visible(false);
        //         } else {
        //             error!("Could not find 1920x1080 video mode");
        //         }
        //     } else {
        //         error!("Could not find current monitor");
        //     }
        // } else {
        //     self.inner.lock().unwrap().is_fullscreen = false;
        //     self.window.set_fullscreen(None);
        //     self.window.set_cursor_visible(true);
        //     self.window.focus_window();
        // }
    }

    // fn get_renderer(&mut self) -> &mut VulkanoWindowRenderer {
    //     self.windows.get_primary_renderer_mut().unwrap()
    // }

    // fn get_window(&self) -> &Window {
    //     self.windows.get_primary_window().unwrap()
    // }

    // fn request_redraw(&self) {
    //     self.window.request_redraw();
    // }
}
