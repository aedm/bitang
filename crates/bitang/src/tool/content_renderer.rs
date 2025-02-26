use crate::loader::project_loader::ProjectLoader;
use crate::render::chart::Chart;
use crate::render::SIMULATION_STEP_SECONDS;
use crate::tool::app_config::AppConfig;
use crate::tool::app_state::AppState;
use crate::tool::music_player::MusicPlayer;
use crate::tool::{FrameContext};
use anyhow::{bail, Result};
use std::rc::Rc;
use std::sync::Arc;
use tracing::error;

use super::{ComputePassContext, GpuContext, Viewport};
// use vulkano::command_buffer::{
//     AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract,
// };
// use vulkano::pipeline::graphics::viewport::Viewport;
// use vulkano::sync::GpuFuture;

pub struct ContentRenderer {
    pub app_state: AppState,
    project_loader: ProjectLoader,
    has_render_failure: bool,
    music_player: MusicPlayer,
    last_render_time: Option<f32>,
}

impl ContentRenderer {
    pub fn new(context: &Arc<GpuContext>) -> Result<ContentRenderer> {
        let mut music_player = MusicPlayer::new();

        let app_config = AppConfig::load()?;
        music_player.set_root_path(&app_config.root_folder);

        let mut project_loader = ProjectLoader::try_new(&app_config.root_folder)?;
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
        })
    }

    pub fn draw(&mut self, context: &mut FrameContext) {
        self.app_state.tick();

        // TODO: This value should be set to Globals at its initialization
        context.globals.simulation_step_seconds = SIMULATION_STEP_SECONDS;

        if self.app_state.is_simulation_enabled {
            context.simulation_elapsed_time_since_last_render = match self.last_render_time {
                Some(last_render_time) => context.globals.app_time - last_render_time,
                None => 0.0,
            };
        }

        let draw_result = match self.app_state.get_chart() {
            Some(chart) => self.draw_chart(&chart, context),
            None => self.draw_project(context),
        };

        self.last_render_time = Some(context.globals.app_time);

        if let Err(err) = draw_result {
            if !self.has_render_failure {
                error!("{err:?}");
                self.has_render_failure = true;
            }
        } else {
            self.has_render_failure = false;
        }
    }

    fn draw_chart(&mut self, chart: &Chart, context: &mut FrameContext) -> Result<()> {
        context.globals.chart_time = self.app_state.cursor_time;
        chart.render(context)
    }

    fn draw_project(&mut self, context: &mut FrameContext) -> Result<()> {
        let Some(project) = &self.app_state.project else {
            bail!("Can't load project.");
        };

        let cursor_time = self.app_state.cursor_time;
        for cut in &project.cuts {
            if cut.start_time <= cursor_time && cursor_time <= cut.end_time {
                context.globals.chart_time = cursor_time - cut.start_time + cut.offset;
                cut.chart.render(context)?
            }
        }
        Ok(())
    }

    /// Returns true if the project changed.
    pub fn reload_project(&mut self, context: &Arc<GpuContext>) -> bool {
        let project = self.project_loader.get_or_load_project(context);

        // Compare references to see if it's the same cached value that we tried rendering last time
        if project.as_ref().map(Rc::as_ptr) != self.app_state.project.as_ref().map(Rc::as_ptr) {
            self.app_state.project = project;
            self.has_render_failure = false;
            return true;
        }
        false
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

    fn reset_chart_simulation(gpu_context: &Arc<GpuContext>, chart: &Chart) -> Result<()> {
        todo!();
        // let mut first_iteration = true;
        // let mut is_simulation_done = false;
        // while !is_simulation_done {
        //     // Make command buffer
        //     // let mut command_builder = AutoCommandBufferBuilder::primary(
        //     //     &vulkan_context.command_buffer_allocator,
        //     //     vulkan_context.gfx_queue.queue_family_index(),
        //     //     CommandBufferUsage::OneTimeSubmit,
        //     // )?;

        //     // Make render context to run simulation
        //     let compute_pass = 
        //     let mut context = ComputePassContext {
        //         gpu_context: &gpu_context,
        //         pass: todo!(),
        //         globals: Default::default(),
        //     };
        //     // let mut context = FrameContext {
        //     //     screen_viewport: Viewport {
        //     //         x: 0,
        //     //         y: 0,
        //     //         width: 1,
        //     //         height: 1,
        //     //     },
        //     //     globals: Default::default(),
        //     //     simulation_elapsed_time_since_last_render: 0.0,
        //     //     gpu_context: todo!(),
        //     //     final_render_target: todo!(),
        //     //     command_encoder: todo!(),
        //     // };
        //     context.globals.simulation_step_seconds = SIMULATION_STEP_SECONDS;

        //     is_simulation_done = chart.reset_simulation(&mut context, first_iteration, true)?;
        //     first_iteration = false;

        //     // Execute simulation and wait for it to finish
        //     command_builder
        //         .build()?
        //         .execute(vulkan_context.gfx_queue.clone())?
        //         .then_signal_fence_and_flush()?
        //         .wait(None)?;
        // }
        // Ok(())
    }

    pub fn reset_simulation(&mut self, context: &GpuContext) -> Result<()> {
        // TODO: implement
        return Ok(());
        // match self.app_state.get_chart() {
        //     // Reset only the selected chart
        //     Some(chart) => Self::reset_chart_simulation(vulkan_context, &chart)?,

        //     // No chart selected, reset all of them
        //     None => {
        //         if let Some(project) = &self.app_state.project {
        //             for cut in &project.cuts {
        //                 Self::reset_chart_simulation(vulkan_context, &cut.chart)?;
        //             }
        //         }
        //     }
        // };

        // // Update state
        // self.app_state.tick();
        // if self.app_state.is_playing() {
        //     self.app_state.reset();
        //     self.music_player
        //         .play_from(self.app_state.get_project_relative_time());
        // }
        // Ok(())
    }

    pub fn set_last_render_time(&mut self, last_render_time: f32) {
        self.last_render_time = Some(last_render_time);
    }
}
