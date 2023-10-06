use crate::control::controls::{ControlRepository, ControlSet, Globals};
use crate::control::{ControlId, ControlIdPartType};
use crate::loader::project_loader::ProjectLoader;
use crate::render::chart::Chart;
use crate::render::image::ImageSizeRule;
use crate::render::project::Project;
use crate::tool::music_player::MusicPlayer;
use crate::tool::ui::Ui;
use anyhow::{bail, Result};

use crate::tool::{RenderContext, VulkanContext};
use std::cmp::max;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, Instant};
use tracing::{error, info};
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, CopyImageToBufferInfo,
    PrimaryCommandBufferAbstract,
};
use vulkano::image::ImageViewAbstract;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::swapchain::Surface;
use vulkano::sync::GpuFuture;
use vulkano_util::renderer::VulkanoWindowRenderer;
use winit::event::WindowEvent;
use winit::event_loop::EventLoop;

pub struct DemoTool {
    pub app_state: AppState,
    start_time: Instant,
    resource_loader: ProjectLoader,
    has_render_failure: bool,
    play_start_time: Instant,
    last_eval_time: f32,
    music_player: MusicPlayer,
}

pub struct AppState {
    pub project: Option<Arc<Project>>,
    pub selected_control_id: ControlId,
    pub time: f32,
    pub is_playing: bool,
    pub control_repository: Arc<ControlRepository>,
}

impl AppState {
    pub fn get_chart(&self) -> Option<Arc<Chart>> {
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

    pub fn get_current_chart_control_set(&self) -> Option<Arc<ControlSet>> {
        self.get_chart().map(|chart| chart.controls.clone())
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
        self.time
    }
}

impl DemoTool {
    pub fn new(context: &Arc<VulkanContext>) -> Result<DemoTool> {
        let music_player = MusicPlayer::new();

        let mut resource_loader = ProjectLoader::try_new()?;
        let project = resource_loader.get_or_load_project(context);
        let has_render_failure = project.is_none();

        let app_state = AppState {
            time: 0.0,
            is_playing: false,
            project,
            control_repository: resource_loader
                .resource_repository
                .control_repository
                .clone(),
            selected_control_id: ControlId::default(),
        };

        let demo_tool = DemoTool {
            start_time: Instant::now(),
            resource_loader,
            app_state: app_state,
            has_render_failure,
            play_start_time: Instant::now(),
            last_eval_time: -1.0,
            music_player,
        };
        Ok(demo_tool)
    }

    pub fn draw(&mut self, context: &mut RenderContext) -> Result<()> {
        match self.app_state.get_chart() {
            Some(chart) => self.draw_chart(&chart, context),
            None => self.draw_project(context),
        }
    }

    fn draw_chart(&mut self, chart: &Chart, context: &mut RenderContext) -> Result<()> {
        // Evaluate control splines
        let should_evaluate = true;
        if should_evaluate {
            if let Some(control_set) = self.app_state.get_current_chart_control_set() {
                self.last_eval_time = self.app_state.time;
                for control in &control_set.used_controls {
                    control.evaluate_splines(self.app_state.time);
                }
            }
        }
        context.globals.chart_time = self.app_state.time;
        chart.render(context)
    }

    fn draw_project(&mut self, context: &mut RenderContext) -> Result<()> {
        let Some(project) = &self.app_state.project else {
            bail!("No project loaded");
        };

        // Evaluate control splines and draw charts
        let time = self.app_state.time;
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

    fn reload_project(&mut self, vulkan_context: &Arc<VulkanContext>) {
        let project = self.resource_loader.get_or_load_project(&vulkan_context);

        // Compare references to see if it's the same cached value that we tried rendering last time
        if project.as_ref().map(Arc::as_ptr) != self.app_state.project.as_ref().map(Arc::as_ptr) {
            self.app_state.project = project;
            self.has_render_failure = false;
        }
    }

    fn advance_time(&mut self) {
        if self.app_state.is_playing {
            self.app_state.time = self.play_start_time.elapsed().as_secs_f32();
        }
    }

    pub fn issue_render_commands(&mut self, context: &mut RenderContext, frame_dump_mode: bool) {
        if frame_dump_mode {
            context.globals.app_time = self.app_state.time;
        } else {
            self.reload_project(&context.vulkan_context);
            self.advance_time();
            context.globals.app_time = self.start_time.elapsed().as_secs_f32();
        }

        if let Err(err) = self.draw(context) {
            if !self.has_render_failure {
                error!("Render failed: {:?}", err);
                self.has_render_failure = true;
            }
        } else {
            self.has_render_failure = false;
        }
    }

    pub fn toggle_play(&mut self) {
        if self.app_state.is_playing {
            self.stop();
        } else {
            self.play();
        }
    }

    pub fn play(&mut self) {
        self.music_player.play_from(self.app_state.get_time());
        let now = Instant::now();
        // Duration is always positive
        if self.app_state.time >= 0. {
            self.play_start_time = now - Duration::from_secs_f32(self.app_state.time);
        } else {
            self.play_start_time = now + Duration::from_secs_f32(-self.app_state.time);
        }
        self.app_state.is_playing = true;
    }

    pub fn stop(&mut self) {
        self.music_player.stop();
        self.app_state.is_playing = false;
    }
}
