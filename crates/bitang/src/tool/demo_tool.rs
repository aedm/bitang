use crate::control::controls::{ControlRepository, ControlSet};
use crate::control::{ControlId, ControlIdPartType};
use crate::file::resource_repository::ResourceRepository;
use crate::render::chart::Chart;
use crate::render::project::Project;
use crate::render::vulkan_window::{PaintResult, RenderContext, VulkanApp, VulkanContext};
use crate::tool::music_player::MusicPlayer;
use crate::tool::ui::Ui;
use anyhow::{anyhow, Result};
use std::cell::RefCell;
use std::cmp::max;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::error;
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage};
use vulkano::image::ImageViewAbstract;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::sync::GpuFuture;
use vulkano_util::renderer::VulkanoWindowRenderer;
use winit::event::WindowEvent;
use winit::event_loop::EventLoop;

pub struct DemoTool {
    ui: Ui,
    start_time: Instant,
    resource_repository: ResourceRepository,
    has_render_failure: bool,
    ui_state: UiState,
    play_start_time: Instant,
    last_eval_time: f32,
    music_player: MusicPlayer,
}

pub struct UiState {
    pub project: Option<Arc<Project>>,
    pub selected_control_id: ControlId,
    pub time: f32,
    pub is_playing: bool,
    pub control_repository: Rc<RefCell<ControlRepository>>,
}

impl UiState {
    pub fn get_chart(&self) -> Option<Rc<Chart>> {
        let id_first = self.selected_control_id.parts.first();
        if let Some(project) = &self.project {
            if let Some(id_first) = id_first {
                if id_first.part_type == ControlIdPartType::Chart {
                    return project.charts_by_id.get(&id_first.name).cloned();
                }
            }
        }
        None
    }

    pub fn get_current_chart_control_set(&self) -> Option<Rc<ControlSet>> {
        self.get_chart()
            .and_then(|chart| Some(chart.controls.clone()))
    }

    pub fn get_time(&self) -> f32 {
        if let Some(part) = self.selected_control_id.parts.first() {
            if let Some(project) = &self.project {
                if part.part_type == ControlIdPartType::Chart {
                    if let Some(time) = project
                        .cuts
                        .iter()
                        .find(|cut| cut.chart.id == part.name)
                        .map(|cut| cut.start_time)
                    {
                        return time + self.time;
                    }
                }
            }
        }
        return self.time;
    }
}

impl DemoTool {
    pub fn new(context: &VulkanContext, event_loop: &EventLoop<()>) -> Result<DemoTool> {
        let music_player = MusicPlayer::new();

        let mut resource_repository = ResourceRepository::try_new()?;
        let project = resource_repository.get_or_load_project(context);
        let ui = Ui::new(context, event_loop)?;
        let has_render_failure = project.is_none();

        let ui_state = UiState {
            time: 0.0,
            is_playing: false,
            project,
            control_repository: resource_repository.control_repository.clone(),
            selected_control_id: ControlId::default(),
        };

        let demo_tool = DemoTool {
            ui,
            start_time: Instant::now(),
            resource_repository,
            ui_state,
            has_render_failure,
            play_start_time: Instant::now(),
            last_eval_time: -1.0,
            music_player,
        };
        Ok(demo_tool)
    }

    pub fn draw(&mut self, context: &mut RenderContext) -> Result<()> {
        if self.ui_state.is_playing {
            self.ui_state.time = self.play_start_time.elapsed().as_secs_f32();
        }

        match self.ui_state.get_chart() {
            Some(chart) => self.draw_chart(&chart, context),
            None => self.draw_project(context),
        }
    }

    fn draw_chart(&mut self, chart: &Chart, context: &mut RenderContext) -> Result<()> {
        // Evaluate control splines
        let should_evaluate = true;
        if should_evaluate {
            if let Some(control_set) = self.ui_state.get_current_chart_control_set() {
                self.last_eval_time = self.ui_state.time;
                for control in &control_set.used_controls {
                    control.evaluate_splines(self.ui_state.time);
                }
            }
        }
        context.globals.chart_time = self.ui_state.time;
        chart.render(context)
    }

    fn draw_project(&mut self, context: &mut RenderContext) -> Result<()> {
        let Some(project) = &self.ui_state.project else {
            return Err(anyhow!("No project loaded"));
        };

        // Evaluate control splines and draw charts
        let time = self.ui_state.time;
        for cut in &project.cuts {
            if cut.start_time <= time && time <= cut.end_time {
                let chart_time = time - cut.start_time + cut.offset;
                for control in &cut.chart.controls.used_controls {
                    control.evaluate_splines(chart_time);
                }
                context.globals.chart_time = chart_time;
                cut.chart.render(context)?
            }
        }
        Ok(())
    }

    fn toggle_play(&mut self) {
        if self.ui_state.is_playing {
            self.stop();
        } else {
            self.play();
        }
    }
}

impl VulkanApp for DemoTool {
    fn paint(
        &mut self,
        vulkan_context: &VulkanContext,
        renderer: &mut VulkanoWindowRenderer,
    ) -> PaintResult {
        let Some(project) = self
            .resource_repository
            .get_or_load_project(vulkan_context) else {
            return PaintResult::None;
        };

        let before_time = self.ui_state.time;

        if let Some(last_project) = &self.ui_state.project {
            // If the last loaded document is not the same as the current one
            if !Arc::ptr_eq(last_project, &project) {
                self.has_render_failure = false;
                self.ui_state.project = Some(project.clone());
            }
        } else {
            // If there was no last loaded document
            self.has_render_failure = false;
            self.ui_state.project = Some(project.clone());
        }

        let before_future = renderer.acquire().unwrap();
        let target_image = renderer.swapchain_image_view();
        let depth_image = renderer.get_additional_image_view(1);

        let scale_factor = renderer.window().scale_factor() as f32;

        let size = target_image.dimensions();
        let movie_height = (size.width() * 9 / 16) as i32;
        let ui_height = max(size.height() as i32 - movie_height, 0) as f32 / scale_factor;
        let draw_ui = ui_height > 0.0;

        let screen_viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [size.width() as f32, movie_height as f32],
            depth_range: 0.0..1.0,
        };

        let mut command_builder = AutoCommandBufferBuilder::primary(
            &vulkan_context.command_buffer_allocator,
            vulkan_context.context.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        vulkan_context
            .swapchain_render_targets_by_id
            .get("screen")
            .unwrap()
            .update_swapchain_image(target_image.clone());
        vulkan_context
            .swapchain_render_targets_by_id
            .get("screen_depth")
            .unwrap()
            .update_swapchain_image(depth_image.clone());

        {
            let mut context = RenderContext {
                vulkan_context,
                screen_buffer: target_image,
                screen_viewport,
                command_builder: &mut command_builder,
                depth_buffer: depth_image,
                globals: Default::default(),
            };
            context.globals.app_time = self.start_time.elapsed().as_secs_f32();

            // If the last render failed, stop rendering until the user changes the document
            if !self.has_render_failure {
                if let Err(err) = self.draw(&mut context) {
                    error!("Render failed: {}", err);
                    self.has_render_failure = true;
                }
            }

            // Render UI
            if draw_ui {
                self.ui.draw(&mut context, ui_height, &mut self.ui_state);
            }
        }

        let command_buffer = command_builder.build().unwrap();

        let after_future = before_future
            .then_execute(
                vulkan_context.context.graphics_queue().clone(),
                command_buffer,
            )
            .unwrap()
            .boxed();

        renderer.present(after_future, true);

        if before_time < project.length && self.ui_state.time >= project.length {
            return PaintResult::EndReached;
        }

        return PaintResult::None;
    }

    fn handle_window_event(&mut self, event: &WindowEvent) {
        self.ui.handle_window_event(event);
        match event {
            WindowEvent::KeyboardInput { input, .. } => {
                if input.state == winit::event::ElementState::Pressed {
                    match input.virtual_keycode {
                        Some(winit::event::VirtualKeyCode::Space) => {
                            self.toggle_play();
                        }
                        _ => (),
                    }
                }
            }
            _ => {}
        }
    }

    fn play(&mut self) {
        self.music_player.play_from(self.ui_state.get_time());
        let now = Instant::now();
        // Duration is always positive
        if self.ui_state.time >= 0. {
            self.play_start_time = now - Duration::from_secs_f32(self.ui_state.time);
        } else {
            self.play_start_time = now + Duration::from_secs_f32(-self.ui_state.time);
        }
        self.ui_state.is_playing = true;
    }

    fn stop(&mut self) {
        self.music_player.stop();
        self.ui_state.is_playing = false;
    }
}
