use crate::loader::project_loader::ProjectLoader;
use crate::render::chart::Chart;
use crate::render::SIMULATION_STEP_SECONDS;
use crate::tool::app_state::AppState;
use crate::tool::music_player::MusicPlayer;
use crate::tool::{RenderContext, VulkanContext};
use anyhow::{bail, Result};
use std::sync::Arc;
use tracing::error;

pub struct ContentRenderer {
    pub app_state: AppState,
    project_loader: ProjectLoader,
    has_render_failure: bool,
    music_player: MusicPlayer,
    last_render_time: Option<f32>,
    must_reset_simulation: bool,
}

impl ContentRenderer {
    pub fn new(context: &Arc<VulkanContext>) -> Result<ContentRenderer> {
        let music_player = MusicPlayer::new();

        let mut project_loader = ProjectLoader::try_new()?;
        let project = project_loader.get_or_load_project(context);
        let has_render_failure = project.is_none();

        let app_state = AppState::new(
            project,
            project_loader
                .resource_repository
                .control_repository
                .clone(),
        );

        Ok(Self {
            project_loader,
            app_state,
            has_render_failure,
            music_player,
            last_render_time: None,
            must_reset_simulation: true,
        })
    }

    pub fn draw(&mut self, context: &mut RenderContext) {
        self.app_state.tick();

        // TODO: This value should be set to Globals at its initialization
        context.globals.simulation_step_seconds = SIMULATION_STEP_SECONDS;

        context.simulation_elapsed_time_since_last_render = match self.last_render_time {
            Some(last_render_time) => context.globals.app_time - last_render_time,
            None => 0.0,
        };

        let draw_result = match self.app_state.get_chart() {
            Some(chart) => self.draw_chart(&chart, context),
            None => self.draw_project(context),
        };

        self.last_render_time = Some(context.globals.app_time);

        if let Err(err) = draw_result {
            if !self.has_render_failure {
                error!("Render failed: {:?}", err);
                self.has_render_failure = true;
            }
        } else {
            self.has_render_failure = false;
        }
    }

    fn draw_chart(&mut self, chart: &Chart, context: &mut RenderContext) -> Result<()> {
        if self.must_reset_simulation {
            chart.reset_simulation();
            self.must_reset_simulation = false;
        }

        // Evaluate control splines
        let cursor_time = self.app_state.cursor_time;
        let should_evaluate = true;
        if should_evaluate {
            if let Some(control_set) = self.app_state.get_current_chart_control_set() {
                for control in &control_set.used_controls {
                    control.evaluate_splines(cursor_time);
                }
            }
        }
        context.globals.chart_time = cursor_time;
        chart.render(context)
    }

    fn draw_project(&mut self, context: &mut RenderContext) -> Result<()> {
        let cursor_time = self.app_state.cursor_time;
        let Some(project) = &self.app_state.project else {
            bail!("No project loaded");
        };

        // Evaluate control splines and draw charts
        for cut in &project.cuts {
            if cut.start_time <= cursor_time && cursor_time <= cut.end_time {
                if self.must_reset_simulation {
                    cut.chart.reset_simulation();
                }

                let chart_time = cursor_time - cut.start_time + cut.offset;
                for control in &cut.chart.controls.used_controls {
                    control.evaluate_splines(chart_time);
                }
                context.globals.chart_time = chart_time;
                cut.chart.render(context)?
            }
        }
        self.must_reset_simulation = false;
        Ok(())
    }

    pub fn reload_project(&mut self, vulkan_context: &Arc<VulkanContext>) {
        let project = self.project_loader.get_or_load_project(vulkan_context);

        // Compare references to see if it's the same cached value that we tried rendering last time
        if project.as_ref().map(Arc::as_ptr) != self.app_state.project.as_ref().map(Arc::as_ptr) {
            self.app_state.project = project;
            self.has_render_failure = false;
        }
    }

    pub fn toggle_play(&mut self) {
        if self.app_state.is_playing() {
            self.stop();
        } else {
            self.play();
        }
    }

    pub fn play(&mut self) {
        self.music_player
            .play_from(self.app_state.get_project_relative_time());
        self.app_state.start();
    }

    pub fn stop(&mut self) {
        self.music_player.stop();
        self.app_state.pause();
    }

    pub fn reset_simulation(&mut self) {
        self.must_reset_simulation = true;
    }
}
