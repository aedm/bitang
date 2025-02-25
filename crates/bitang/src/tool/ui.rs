use crate::control::controls::{Control, ControlSet, UsedControlsNode};
use crate::control::{ControlId, ControlIdPartType};
use crate::tool::app_state::AppState;
use crate::tool::spline_editor::SplineEditor;
use crate::tool::{FrameContext, WindowContext};
use anyhow::Result;
use egui::SliderClamping;
// use egui_winit_vulkano::{Gui, GuiConfig};
use std::rc::Rc;
use std::sync::Arc;
use tracing::error;
// use vulkano::command_buffer::{
//     RenderPassBeginInfo, SubpassBeginInfo, SubpassContents, SubpassEndInfo,
// };
// use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, Subpass};
// use vulkano::swapchain::Surface;
use winit::{event::WindowEvent, event_loop::EventLoop};

pub struct Ui {
    // pub gui: Gui,
    // pub subpass: Subpass,
    spline_editor: SplineEditor,
}

impl Ui {
    pub fn new(// context: &Arc<WindowContext>,
        // event_loop: &EventLoop<()>,
        // surface: &Arc<Surface>,
    ) -> Result<Ui> {
        // let render_pass = vulkano::single_pass_renderpass!(
        //     context.device.clone(),
        //     attachments: {
        //         color: {
        //             format: context.swapchain_format,
        //             samples: 1,
        //             load_op: DontCare,
        //             store_op: Store,
        //         }
        //     },
        //     pass:
        //         { color: [color], depth_stencil: {} }
        // )?;
        // let subpass = Subpass::from(render_pass, 0).unwrap(); // unwrap is okay here

        // let gui = Gui::new_with_subpass(
        //     event_loop,
        //     surface.clone(),
        //     context.gfx_queue.clone(),
        //     subpass.clone(),
        //     vulkano::format::Format::B8G8R8A8_SRGB,
        //     // TODO: use UNORM instead of SRGB
        //     GuiConfig {
        //         allow_srgb_render_target: true,
        //         ..Default::default()
        //     },
        // );
        let spline_editor = SplineEditor::new();

        Ok(Ui {
            // gui,
            // subpass,
            spline_editor,
        })
    }

    pub fn draw(
        &mut self,
        ctx: &egui::Context,
        app_state: &mut AppState,
        bottom_panel_height: f32,
    ) {
        let spline_editor = &mut self.spline_editor;
        egui::TopBottomPanel::bottom("ui_root")
            .height_range(bottom_panel_height..=bottom_panel_height)
            .show(&ctx, |ui| {
                ui.add_space(5.0);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                    Self::draw_control_tree(ui, app_state);
                    ui.separator();
                    if let Some(controls) = app_state.get_current_chart_control_set() {
                        if let Some((control, component_index)) =
                            Self::draw_control_sliders(ui, app_state, &controls)
                        {
                            spline_editor.set_control(control, component_index);
                        }
                    }
                    spline_editor.draw(ui, app_state);
                });
            });
    }

    fn draw_control_tree(ui: &mut egui::Ui, ui_state: &mut AppState) {
        let Some(project) = &ui_state.project else {
            return;
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

    fn draw_control_tree_project_node(ui: &mut egui::Ui, ui_state: &mut AppState) {
        let id = ui.make_persistent_id("node:project");
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
            .show_header(ui, |ui| {
                let selected = ui_state.selected_control_id.parts.is_empty();
                let mut new_selected = selected;
                ui.toggle_value(&mut new_selected, "📁 Project");
                if new_selected && !selected && ui_state.project.is_some() {
                    ui_state.selected_control_id = ControlId::default();
                }
            })
            .body(|_ui| ());
    }

    fn draw_control_tree_node(ui: &mut egui::Ui, node: &UsedControlsNode, ui_state: &mut AppState) {
        let id_str = format!("node:{}", node.id_prefix);
        let id = ui.make_persistent_id(id_str);
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
                ControlIdPartType::ChartStep => '📦',
                ControlIdPartType::Camera => '📷',
                ControlIdPartType::Object => '🏠',
                ControlIdPartType::Scene => '🏰',
                ControlIdPartType::Value => '📊',
                ControlIdPartType::BufferGenerator => '🔮',
                ControlIdPartType::Compute => '🧮',
            };
            ui.toggle_value(
                &mut new_selected,
                format!("{icon} {}", control_id_part.name),
            );
            if new_selected && !selected {
                ui_state.selected_control_id = node.id_prefix.clone();
            }
        })
        .body(|ui| {
            for child in &node.children {
                if !child.children.is_empty() {
                    Self::draw_control_tree_node(ui, child, ui_state);
                }
            }
        });
    }

    // Returns the spline that was activated
    fn draw_control_sliders<'a>(
        ui: &mut egui::Ui,
        ui_state: &mut AppState,
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
                    for (i, component) in components.iter_mut().enumerate().take(component_count) {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                            ui.add_sized(
                                [350.0, 0.0],
                                egui::Slider::new(&mut component.value, 0.0..=1.0)
                                    .clamping(SliderClamping::Never)
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

        selected.map(|(control_index, component_index)| {
            (&controls.used_controls[control_index], component_index)
        })
    }

    // fn render_to_swapchain(&mut self, context: &mut FrameContext) {
    //     let target_image = context
    //         .vulkan_context
    //         .final_render_target
    //         .get_view_for_render_target()
    //         .unwrap();
    //     let [width, height, _] = target_image.image().extent();
    //     let framebuffer = Framebuffer::new(
    //         self.subpass.render_pass().clone(),
    //         FramebufferCreateInfo {
    //             attachments: vec![target_image],
    //             ..Default::default()
    //         },
    //     )
    //     .unwrap();

    //     context
    //         .command_builder
    //         .begin_render_pass(
    //             RenderPassBeginInfo {
    //                 clear_values: vec![None],
    //                 ..RenderPassBeginInfo::framebuffer(framebuffer)
    //             },
    //             SubpassBeginInfo {
    //                 contents: SubpassContents::SecondaryCommandBuffers,
    //                 ..Default::default()
    //             },
    //         )
    //         .unwrap();

    //     let gui_commands = self.gui.draw_on_subpass_image([width, height]);
    //     context
    //         .command_builder
    //         .execute_commands(gui_commands)
    //         .unwrap();

    //     context
    //         .command_builder
    //         .end_render_pass(SubpassEndInfo::default())
    //         .unwrap();
    // }

    // pub fn handle_window_event(&mut self, event: &WindowEvent) {
    //     let _pass_events_to_game = !self.gui.update(event);
    // }
}
