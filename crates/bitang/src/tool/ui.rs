use crate::control::controls::{Control, Controls};
use crate::file::save_controls;
use crate::render::vulkan_window::{RenderContext, VulkanContext};
use crate::tool::spline_editor::SplineEditor;
use egui_winit_vulkano::Gui;
use std::rc::Rc;
use tracing::error;
use vulkano::command_buffer::{RenderPassBeginInfo, SubpassContents};
use vulkano::image::ImageViewAbstract;
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, Subpass};
use winit::{event::WindowEvent, event_loop::EventLoop};

pub struct Ui {
    pub gui: Gui,
    pub subpass: Subpass,
    spline_editor: SplineEditor,
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
        let spline_editor = SplineEditor::new();

        Ui {
            gui,
            subpass,
            spline_editor,
        }
    }

    pub fn render(
        &mut self,
        context: &mut RenderContext,
        bottom_panel_height: f32,
        controls: &mut Controls,
        time: &mut f32,
    ) {
        // ) -> Box<dyn GpuFuture> {
        let pixels_per_point = 1.15f32;
        let bottom_panel_height = bottom_panel_height / pixels_per_point;
        let spline_editor = &mut self.spline_editor;
        self.gui.immediate_ui(|gui| {
            let ctx = gui.context();
            ctx.set_pixels_per_point(pixels_per_point);
            egui::TopBottomPanel::bottom("ui_root")
                .height_range(bottom_panel_height..=bottom_panel_height)
                .show(&ctx, |ui| {
                    ui.add_space(5.0);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        if let Some((control, component_index)) =
                            Self::draw_control_value_sliders(ui, controls)
                        {
                            spline_editor.set_control(control, component_index);
                        }
                        spline_editor.draw(ui, time);
                    });
                });
            Self::handle_hotkeys(ctx, controls);
        });
        self.render_to_swapchain(context);
    }

    fn handle_hotkeys(ctx: egui::Context, controls: &mut Controls) {
        // Save
        if ctx
            .input_mut()
            .consume_key(egui::Modifiers::CTRL, egui::Key::S)
        {
            println!("Saving");
            if let Err(err) = save_controls(&controls) {
                error!("Failed to save controls: {}", err);
            }
        }
    }

    // Returns the spline that was activated
    fn draw_control_value_sliders<'a>(
        ui: &mut egui::Ui,
        controls: &'a mut Controls,
    ) -> Option<(&'a Rc<Control>, usize)> {
        // An iterator that mutably borrows all used control values
        let controls_borrow = controls
            .used_controls
            .iter_mut()
            .map(|c| (c.id.as_str(), c.components.borrow_mut()));
        let mut selected = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                for (control_index, mut control) in controls_borrow.enumerate() {
                    ui.label(control.0);
                    let components = control.1.as_mut();
                    for i in 0..4 {
                        let component = &mut components[i];
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                            ui.add_sized(
                                [330.0, 0.0],
                                egui::Slider::new(&mut component.value, 0.0..=1.0),
                            );

                            if ui.button("~").clicked() {
                                selected = Some((control_index, i));
                            }
                            ui.checkbox(&mut component.use_spline, "")
                        });
                    }
                }
            })
        });

        selected.and_then(|(control_index, component_index)| {
            Some((&controls.used_controls[control_index], component_index))
        })
    }

    fn render_to_swapchain(&mut self, context: &mut RenderContext) {
        let target_image = context.screen_buffer.clone();
        let dimensions = target_image.dimensions().width_height();
        let framebuffer = Framebuffer::new(
            self.subpass.render_pass().clone(),
            FramebufferCreateInfo {
                attachments: vec![target_image],
                ..Default::default()
            },
        )
        .unwrap();

        context
            .command_builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![None],
                    ..RenderPassBeginInfo::framebuffer(framebuffer)
                },
                SubpassContents::SecondaryCommandBuffers,
            )
            .unwrap();

        let gui_commands = self.gui.draw_on_subpass_image(dimensions);
        context
            .command_builder
            .execute_commands(gui_commands)
            .unwrap();

        context.command_builder.end_render_pass().unwrap();
    }

    pub fn handle_window_event(&mut self, event: &WindowEvent) {
        let _pass_events_to_game = !self.gui.update(event);
    }
}
