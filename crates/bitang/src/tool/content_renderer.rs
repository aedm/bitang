use crate::loader::project_loader::ProjectLoader;
use crate::render::chart::Chart;
use crate::render::SIMULATION_STEP_SECONDS;
use crate::tool::app_state::AppState;
use crate::tool::music_player::MusicPlayer;
use crate::tool::timer::Timer;
use crate::tool::{RenderContext, VulkanContext};
use anyhow::{bail, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::error;

pub struct ContentRenderer {
    pub app_state: AppState,
    // start_time: Instant,
    project_loader: ProjectLoader,
    has_render_failure: bool,
    // play_start_time: Instant,
    music_player: MusicPlayer,
    // app_timer: Timer,
    // cursor_timer: Timer,
    // simulation_timer: Timer,
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

        // let mut app_timer = Timer::new();
        // app_timer.start();
        Ok(Self {
            // start_time: Instant::now(),
            project_loader,
            app_state,
            has_render_failure,
            // play_start_time: Instant::now(),
            music_player,
            last_render_time: None,
            must_reset_simulation: true,
            //
            // app_timer,
            // cursor_timer: Timer::new(),
            // simulation_timer: Timer::new(),
        })
    }

    pub fn issue_render_commands(&mut self, context: &mut RenderContext) {
        // if frame_dump_mode {
        //     context.globals.app_time = self.app_state.time;
        // } else {
        //     self.reload_project(&context.vulkan_context);
        //     // self.advance_time();
        //     // context.globals.app_time = self.start_time.elapsed().as_secs_f32();
        //     context.globals.app_time = self.app_timer.elapsed();
        // }
        // if !frame_dump_mode {
        //     self.reload_project(&context.vulkan_context);
        // }
        self.app_state.tick();
        // TODO nem ide
        context.globals.simulation_step_seconds = SIMULATION_STEP_SECONDS;

        context.simulation_elapsed_time_since_last_render = match self.last_render_time {
            Some(last_render_time) => context.globals.app_time - last_render_time,
            None => 0.0,
        };

        // if context.simulation_elapsed_time_since_last_render < 0.0 {
        //     context.simulation_elapsed_time_since_last_render = 0.0;
        // } else {
        //     context.simulation_elapsed_time_since_last_render =
        //         context.globals.app_time - context.simulation_elapsed_time_since_last_render;
        // }

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

    // fn draw(&mut self, context: &mut RenderContext) -> Result<()> {
    //     match self.app_state.get_chart() {
    //         Some(chart) => self.draw_chart(&chart, context),
    //         None => self.draw_project(context),
    //     }
    // }

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

    // fn advance_time(&mut self) {
    //     // if self.app_state.is_playing {
    //     //     self.app_state.time = self.play_start_time.elapsed().as_secs_f32();
    //     // }
    //     self.app_state.time = self.cursor_timer.elapsed();
    // }

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
        // let now = Instant::now();
        // // Duration is always positive
        // if self.app_state.time >= 0. {
        //     self.play_start_time = now - Duration::from_secs_f32(self.app_state.time);
        // } else {
        //     self.play_start_time = now + Duration::from_secs_f32(-self.app_state.time);
        // }
        // self.app_state.is_playing = true;
        // self.cursor_timer.start();
        // self.simulation_timer.start();
        self.app_state.start();
    }

    pub fn stop(&mut self) {
        self.music_player.stop();
        self.app_state.pause();
        // self.app_state.is_playing = false;
        // self.cursor_timer.pause();
        // self.simulation_timer.pause();
    }

    pub fn reset_simulation(&mut self) {
        self.must_reset_simulation = true;
    }
}
