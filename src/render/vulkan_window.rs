// use vulkano::swapchain::PresentMode;
// use vulkano_util::context::{VulkanoConfig, VulkanoContext};
// use vulkano_util::window::{VulkanoWindows, WindowDescriptor, WindowMode};
// use winit::event::{Event, WindowEvent};
// use winit::event_loop::{ControlFlow, EventLoop};

use egui::{ScrollArea, TextEdit, TextStyle};
use egui_winit_vulkano::Gui;
use std::sync::Arc;
use vulkano::render_pass::RenderPass;
use vulkano_util::{
    context::{VulkanoConfig, VulkanoContext},
    window::{VulkanoWindows, WindowDescriptor},
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

pub struct VulkanRenderer {
    pub context: VulkanoContext,
    pub render_pass: Arc<RenderPass>,
}

pub struct VulkanWindow {
    event_loop: EventLoop<()>,
    renderer: VulkanRenderer,
    windows: VulkanoWindows,
}

impl VulkanWindow {
    pub fn new() -> Self {
        let event_loop = EventLoop::new();

        // let context = VulkanoContext::new(VulkanoConfig {
        //
        //     window: WindowDescriptor {
        //         width: 1280.0,
        //         height: 720.0,
        //         position: None,
        //         resize_constraints: Default::default(),
        //         scale_factor_override: None,
        //         title: "Vulkano Renderer".to_string(),
        //         transparent: false,
        //         resizable: true,
        //         decorations: false,
        //         cursor_visible: false,
        //         cursor_locked: false,
        //         present_mode: PresentMode::Immediate,
        //         mode: WindowMode::Windowed,
        //     },
        //     ..Default::default()
        // })
        // .unwrap();

        let context = VulkanoContext::new(VulkanoConfig::default());

        let mut windows = VulkanoWindows::default();
        windows.create_window(&event_loop, &context, &WindowDescriptor::default(), |ci| {
            ci.image_format = Some(vulkano::format::Format::B8G8R8A8_SRGB)
        });

        let renderer = VulkanRenderer {
            context,
            render_pass: windows
                .get_primary_renderer()
                .unwrap()
                .render_pass()
                .clone(),
        };

        Self {
            windows,
            event_loop,
            renderer,
        }
    }

    pub fn main_loop(mut self) {
        self.event_loop.run(move |event, _, control_flow| {
            let renderer = self.windows.get_primary_renderer_mut().unwrap();
            match event {
                Event::WindowEvent { event, window_id } if window_id == renderer.window().id() => {
                    // Update Egui integration so the UI works!
                    // let _pass_events_to_game = !gui.update(&event);
                    match event {
                        WindowEvent::Resized(_) => {
                            renderer.resize();
                        }
                        WindowEvent::ScaleFactorChanged { .. } => {
                            renderer.resize();
                        }
                        WindowEvent::CloseRequested => {
                            *control_flow = ControlFlow::Exit;
                        }
                        _ => (),
                    }
                }
                Event::RedrawRequested(window_id) if window_id == window_id => {
                    // Set immediate UI in redraw here
                    // gui.immediate_ui(|gui| {
                    //     let ctx = gui.context();
                    //     egui::CentralPanel::default().show(&ctx, |ui| {
                    //         ui.vertical_centered(|ui| {
                    //             ui.add(egui::widgets::Label::new("Hi there!"));
                    //             sized_text(ui, "Rich Text", 32.0);
                    //         });
                    //         ui.separator();
                    //         ui.columns(2, |columns| {
                    //             ScrollArea::vertical().id_source("source").show(
                    //                 &mut columns[0],
                    //                 |ui| {
                    //                     ui.add(
                    //                         TextEdit::multiline(&mut code)
                    //                             .font(TextStyle::Monospace),
                    //                     );
                    //                 },
                    //             );
                    //             ScrollArea::vertical().id_source("rendered").show(
                    //                 &mut columns[1],
                    //                 |ui| {
                    //                     egui_demo_lib::easy_mark::easy_mark(ui, &code);
                    //                 },
                    //             );
                    //         });
                    //     });
                    // });
                    // Render UI
                    // Acquire swapchain future
                    let before_future = renderer.acquire().unwrap();
                    // // Render gui
                    // let after_future =
                    //     gui.draw_on_image(before_future, renderer.swapchain_image_view());
                    // Present swapchain
                    renderer.present(before_future, true);
                }
                Event::MainEventsCleared => {
                    renderer.window().request_redraw();
                }
                _ => (),
            }
        });
    }
}
