use crate::render::vulkan_window::VulkanContext;
use egui::plot::{Legend, LinkedAxisGroup, Plot, PlotBounds};
use egui::Align;
use egui_winit_vulkano::Gui;
use std::ops::DerefMut;

use crate::control::controls::ControlValue::Scalars;
use crate::control::controls::Controls;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassContents,
};
use vulkano::image::ImageViewAbstract;
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, Subpass};
use vulkano::sync::GpuFuture;
use vulkano_util::renderer::SwapchainImageView;
use winit::{event::WindowEvent, event_loop::EventLoop};

pub struct Ui {
    pub gui: Gui,
    pub subpass: Subpass,
    spline_axes: LinkedAxisGroup,
}

impl Ui {
    pub fn new(context: &VulkanContext, event_loop: &EventLoop<()>) -> Ui {
        let render_pass = vulkano::single_pass_renderpass!(
            context.context.device().clone(),
            attachments: {
                color: {
                    load: DontCare,
                    store: Store,
                    format: context.swapchain_format,
                    samples: 1,
                }
            },
            pass:
                { color: [color], depth_stencil: {} }
        )
        .unwrap();
        let subpass = Subpass::from(render_pass, 0).unwrap();

        let gui = Gui::new_with_subpass(
            event_loop,
            context.surface.clone(),
            Some(vulkano::format::Format::B8G8R8A8_SRGB),
            context.gfx_queue.clone(),
            subpass.clone(),
        );
        let spline_axes = LinkedAxisGroup::new(true, true);

        Ui {
            gui,
            subpass,
            spline_axes,
        }
    }

    pub fn render(
        &mut self,
        context: &VulkanContext,
        before_future: Box<dyn GpuFuture>,
        target_image: SwapchainImageView,
        bottom_panel_height: f32,
        controls: &mut Controls,
    ) -> Box<dyn GpuFuture> {
        let pixels_per_point = 1.15f32;
        let bottom_panel_height = bottom_panel_height / pixels_per_point;
        let spline_axes = &mut self.spline_axes;
        self.gui.immediate_ui(|gui| {
            let ctx = gui.context();
            ctx.set_pixels_per_point(pixels_per_point);
            egui::TopBottomPanel::bottom("my_panel")
                .height_range(bottom_panel_height..=bottom_panel_height)
                .show(&ctx, |ui| {
                    ui.add_space(5.0);
                    ui.columns(2, |columns| {
                        let [left_ui, right_ui] = columns else { panic!("Column count mismatch") };
                        Self::paint_controls(left_ui, controls);
                        Self::paint_spline_editor(right_ui, spline_axes);
                    });
                });
        });
        self.render_to_swapchain(context, before_future, target_image)
    }

    fn paint_controls(ui: &mut egui::Ui, controls: &mut Controls) {
        // An iterator that mutably borrows all used control values
        let mut controls = controls
            .used_controls
            .iter_mut()
            .map(|c| (c.id.as_str(), c.value.borrow_mut()));

        ui.label("Controls");
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            for mut control in &mut controls {
                ui.label(control.0);
                if let Scalars(scalars) = control.1.deref_mut() {
                    for i in 0..4 {
                        let _ = ui.add(egui::Slider::new(&mut scalars[i], 0.0..=1.0));
                    }
                }
            }
        });
    }

    fn paint_spline_editor(ui: &mut egui::Ui, spline_axes: &mut LinkedAxisGroup) {
        // let mut spline_axes = spline_axes.clone();
        // spline_axes.set(PlotBounds::from_min_max([-1.0, -1.0], [10.0, 10.0]]));
        ui.label("Spline editor");
        let mut plot = Plot::new("lines_demo")
            .legend(Legend::default())
            .include_x(-2.0)
            .include_x(2.0)
            .include_y(-3.0)
            .include_y(5.0)
            .allow_boxed_zoom(false)
            // .auto_bounds_y()
            // .auto_bounds_x()
            .allow_drag(false)
            // .link_axis(spline_axes)
            .allow_zoom(false)
            .allow_scroll(false);
        plot.show(ui, |plot_ui| {
            plot_ui.set_plot_bounds(PlotBounds::from_min_max([2.0, -1.0], [10.0, 10.0]));
        });
    }

    fn render_to_swapchain(
        &mut self,
        context: &VulkanContext,
        before_future: Box<dyn GpuFuture>,
        target_image: SwapchainImageView,
    ) -> Box<dyn GpuFuture> {
        let mut builder = AutoCommandBufferBuilder::primary(
            &context.command_buffer_allocator,
            context.context.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let dimensions = target_image.dimensions().width_height();
        let framebuffer = Framebuffer::new(
            self.subpass.render_pass().clone(),
            FramebufferCreateInfo {
                attachments: vec![target_image],
                ..Default::default()
            },
        )
        .unwrap();

        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![None],
                    ..RenderPassBeginInfo::framebuffer(framebuffer)
                },
                SubpassContents::SecondaryCommandBuffers,
            )
            .unwrap();

        let gui_commands = self.gui.draw_on_subpass_image(dimensions);
        builder.execute_commands(gui_commands).unwrap();

        builder.end_render_pass().unwrap();
        let command_buffer = builder.build().unwrap();

        let after_future = before_future
            .then_execute(context.context.graphics_queue().clone(), command_buffer)
            .unwrap()
            .boxed();

        after_future
    }

    pub fn handle_window_event(&mut self, event: &WindowEvent) {
        let _pass_events_to_game = !self.gui.update(event);
    }
}
