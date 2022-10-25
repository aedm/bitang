use crate::render::VulkanRenderer;
use egui::{epaint, ColorImage, FullOutput, TextureHandle};
use std::mem;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::render_pass::Subpass;

pub struct Gui {
    egui_ctx: egui::Context,
    egui_winit: egui_winit::State,
    egui_painter: egui_vulkano::Painter,

    shapes: Vec<epaint::ClippedShape>,

    my_texture: TextureHandle,
}

impl Gui {
    pub fn new(renderer: &VulkanRenderer) -> Gui {
        let egui_ctx = egui::Context::default();
        let mut egui_winit = egui_winit::State::new(4096, renderer.surface.window());

        let mut egui_painter = egui_vulkano::Painter::new(
            renderer.device.clone(),
            renderer.queue.clone(),
            Subpass::from(renderer.render_pass.clone(), 1).unwrap(),
        )
        .unwrap();
        let mut my_texture = egui_ctx.load_texture("my_texture", ColorImage::example());

        Gui {
            egui_ctx,
            egui_winit,
            egui_painter,
            shapes: Vec::new(),
            my_texture,
        }
    }

    pub fn build(
        self: &mut Self,
        renderer: &VulkanRenderer,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        bottom_panel_height: i32,
    ) {
        // Build your gui
        self.egui_ctx
            .begin_frame(self.egui_winit.take_egui_input(renderer.surface.window()));
        // demo_windows.ui(&egui_ctx);

        // egui::Window::new("Color test")
        //     .vscroll(true)
        //     .show(&egui_ctx, |ui| {
        //         egui_test.ui(ui);
        //     });
        //
        // egui::Window::new("Settings").show(&egui_ctx, |ui| {
        //     egui_ctx.settings_ui(ui);
        // });

        // egui::Window::new("Benchmark")
        //     .default_height(600.0)
        //     .show(&egui_ctx, |ui| {
        //         egui_bench.draw(ui);
        //     });
        //
        egui::Window::new("Texture test").show(&self.egui_ctx, |ui| {
            ui.image(self.my_texture.id(), (200.0, 200.0));
            if ui.button("Reload texture").clicked() {
                // previous TextureHandle is dropped, causing egui to free the texture:
                self.my_texture = self
                    .egui_ctx
                    .load_texture("my_texture", ColorImage::example());
            }
        });

        let height = bottom_panel_height as f32;
        egui::TopBottomPanel::bottom("my_panel")
            .height_range(height..=height)
            .show(&self.egui_ctx, |ui| {
                ui.with_layout(
                    egui::Layout::top_down_justified(egui::Align::Center),
                    |ui| {
                        ui.button("I am becoming wider as needed");
                        ui.allocate_space(ui.available_size());
                    },
                );
                // ui.label("Hello World!");
            });

        // Get the shapes from egui
        let egui_output = self.egui_ctx.end_frame();
        let platform_output = egui_output.platform_output;
        self.egui_winit.handle_platform_output(
            renderer.surface.window(),
            &self.egui_ctx,
            platform_output,
        );

        let result = self
            .egui_painter
            .update_textures(egui_output.textures_delta, builder)
            .expect("egui texture error");

        self.shapes = egui_output.shapes;
    }

    pub fn handle_window_event(&mut self, event: &winit::event::WindowEvent) {
        self.egui_winit.on_event(&self.egui_ctx, event);
        // if !egui_consumed_event {
        //     // do your own event handling here
        // };
    }

    pub fn draw(
        self: &mut Self,
        renderer: &VulkanRenderer,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) {
        // Automatically start the next render subpass and draw the gui
        let size = renderer.surface.window().inner_size();
        let sf: f32 = renderer.surface.window().scale_factor() as f32;
        let shapes = mem::take(&mut self.shapes);
        self.egui_painter
            .draw(
                builder,
                [(size.width as f32) / sf, (size.height as f32) / sf],
                &self.egui_ctx,
                shapes,
            )
            .unwrap();

        // egui_bench.push(frame_start.elapsed().as_secs_f64());
    }
}
