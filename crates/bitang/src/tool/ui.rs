use crate::control::controls::{Control, ControlSet, UsedControlsNode};
use crate::control::{ControlId, ControlIdPartType};
use crate::render::vulkan_window::{RenderContext, VulkanContext};
use crate::tool::demo_tool::UiState;
use crate::tool::spline_editor::SplineEditor;
use anyhow::Result;
use egui::Modifiers;
use egui_winit_vulkano::{Gui, GuiConfig};
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
    pub fn new(context: &VulkanContext, event_loop: &EventLoop<()>) -> Result<Ui> {
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
        )?;
        let subpass = Subpass::from(render_pass, 0).unwrap(); // unwrap is okay here

        let gui = Gui::new_with_subpass(
            event_loop,
            context.surface.clone(),
            context.gfx_queue.clone(),
            subpass.clone(),
            GuiConfig {
                preferred_format: Some(vulkano::format::Format::B8G8R8A8_SRGB),
                ..Default::default()
            },
        );
        let spline_editor = SplineEditor::new();

        Ok(Ui {
            gui,
            subpass,
            spline_editor,
        })
    }

    pub fn draw(
        &mut self,
        context: &mut RenderContext,
        bottom_panel_height: f32,
        ui_state: &mut UiState,
    ) {
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
                        Self::draw_control_tree(ui, ui_state);
                        ui.separator();
                        if let Some(controls) = ui_state.get_current_chart_control_set() {
                            if let Some((control, component_index)) =
                                Self::draw_control_sliders(ui, ui_state, &controls)
                            {
                                spline_editor.set_control(control, component_index);
                            }
                        }
                        spline_editor.draw(ui, &mut ui_state.time);
                    });
                });
            Self::handle_hotkeys(ctx, ui_state);
        });
        self.render_to_swapchain(context);
    }

    fn handle_hotkeys(ctx: egui::Context, ui_state: &mut UiState) {
        // Save
        let save_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::S);
        if ctx.input_mut(|i| i.consume_shortcut(&save_shortcut)) {
            if let Some(project) = &ui_state.project {
                if let Err(err) = ui_state
                    .control_repository
                    .borrow()
                    .save_control_files(project)
                {
                    error!("Failed to save controls: {}", err);
                }
            }
        }
    }

    fn draw_control_tree(ui: &mut egui::Ui, ui_state: &mut UiState) {
        let Some(project) = &ui_state.project else {
            return
        };
        let project = project.clone();

        ui.push_id("control_tree", |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_min_width(150.0);
                    ui.label("Charts");
                    Self::draw_control_tree_project_node(ui, ui_state);
                    for chart in &project.charts {
                        Self::draw_control_tree_node(
                            ui,
                            &chart.controls.root_node.borrow(),
                            ui_state,
                        );
                    }
                })
            });
        });
    }

    fn draw_control_tree_project_node(ui: &mut egui::Ui, ui_state: &mut UiState) {
        let id = ui.make_persistent_id("node:project");
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
            .show_header(ui, |ui| {
                let selected = ui_state.selected_control_id.parts.is_empty();
                let mut new_selected = selected;
                ui.toggle_value(&mut new_selected, "📁 Project");
                if new_selected && !selected {
                    if ui_state.project.is_some() {
                        ui_state.selected_control_id = ControlId::default();
                    }
                }
            })
            .body(|_ui| ());
    }

    fn draw_control_tree_node(ui: &mut egui::Ui, node: &UsedControlsNode, ui_state: &mut UiState) {
        let id_str = format!("node:{}", node.id_prefix);
        let id = ui.make_persistent_id(&id_str);
        // Unwrap is safe because we know that the prefix has at least one part
        let control_id_part = &node.id_prefix.parts.last().unwrap();
        let default_open = control_id_part.part_type != ControlIdPartType::Chart;
        egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            id,
            default_open,
        )
        .show_header(ui, |ui| {
            let selected = ui_state.selected_control_id == node.id_prefix;
            let mut new_selected = selected;
            let icon = match control_id_part.part_type {
                ControlIdPartType::Chart => '📈',
                ControlIdPartType::ChartValues => '🌐',
                ControlIdPartType::Pass => '📦',
                ControlIdPartType::Camera => '📷',
                ControlIdPartType::Object => '📝',
                ControlIdPartType::Value => '📊',
                ControlIdPartType::BufferGenerator => '🔮',
            };
            ui.toggle_value(
                &mut new_selected,
                format!("{} {}", icon, control_id_part.name),
            );
            if new_selected && !selected {
                ui_state.selected_control_id = node.id_prefix.clone();
            }
        })
        .body(|ui| {
            for child in &node.children {
                let child = child.borrow();
                if !child.children.is_empty() {
                    Self::draw_control_tree_node(ui, &child, ui_state);
                }
            }
        });
    }

    // Returns the spline that was activated
    fn draw_control_sliders<'a>(
        ui: &mut egui::Ui,
        ui_state: &mut UiState,
        controls: &'a ControlSet,
    ) -> Option<(&'a Rc<Control>, usize)> {
        // An iterator that mutably borrows all used control values
        let trim_parts = ui_state.selected_control_id.parts.len();
        let controls_borrow = controls
            .used_controls
            .iter()
            .enumerate()
            .filter(|(_, c)| c.id.parts.starts_with(&ui_state.selected_control_id.parts))
            .map(|(index, c)| {
                let name = c.id.parts[trim_parts..]
                    .iter()
                    .map(|p| p.name.clone())
                    .collect::<Vec<_>>()
                    .join("/");
                (
                    index,
                    name,
                    c.used_component_count.get(),
                    c.components.borrow_mut(),
                )
            });
        let mut selected = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                for (control_index, control_name, component_count, mut control) in controls_borrow {
                    ui.label(&control_name);
                    let components = control.as_mut();
                    for i in 0..component_count {
                        let component = &mut components[i];
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                            ui.add_sized(
                                [350.0, 0.0],
                                egui::Slider::new(&mut component.value, 0.0..=1.0)
                                    .clamp_to_range(false)
                                    .max_decimals(3),
                            );

                            if ui.button("✏").clicked() {
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
