use crate::control::controls::{ControlSet, ControlSetBuilder};
use crate::control::{ControlId, ControlIdPartType};
use crate::render::camera::Camera;
use crate::render::compute::{Compute, Run};
use crate::render::draw::Draw;
use crate::render::generate_mip_levels::GenerateMipLevels;
use crate::render::image::BitangImage;
use crate::render::SIMULATION_STEP_SECONDS;
use crate::tool::{ComputePassContext, FrameContext};
use anyhow::{bail, ensure, Result};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

pub enum ChartStep {
    Draw(Draw),
    Compute(Compute),
    GenerateMipLevels(GenerateMipLevels),
}

pub struct Chart {
    pub id: String,
    pub controls: Rc<ControlSet>,
    camera: Camera,
    images: Vec<Arc<BitangImage>>,
    pub steps: Vec<ChartStep>,

    simulation_cursor: RefCell<SimulationCursor>,
}

impl Chart {
    pub fn new(
        id: &str,
        control_id: &ControlId,
        control_set_builder: ControlSetBuilder,
        images: Vec<Arc<BitangImage>>,
        steps: Vec<ChartStep>,
        simulation_precalculation_time: f32,
    ) -> Self {
        let _camera = Camera::new(
            &control_set_builder,
            &control_id.add(ControlIdPartType::Camera, "camera"),
        );
        let chart_step_ids = steps
            .iter()
            .map(|step| match step {
                ChartStep::Draw(draw) => draw.id.clone(),
                ChartStep::Compute(compute) => compute.id.clone(),
                ChartStep::GenerateMipLevels(genmips) => genmips._id.clone(),
            })
            .collect::<Vec<String>>();
        let controls = Rc::new(control_set_builder.into_control_set(&chart_step_ids));
        Chart {
            id: id.to_string(),
            camera: _camera,
            images,
            steps,
            controls,
            simulation_cursor: RefCell::new(SimulationCursor::new(simulation_precalculation_time)),
        }
    }

    /// Reruns the initialization step and runs the simulation for the precalculation time.
    pub fn reset_simulation(&self, context: &mut ComputePassContext) -> Result<()> {
        self.initialize(context)?;
        self.simulate(context, true)?;
        Ok(())
    }

    fn initialize(&self, context: &mut ComputePassContext) -> Result<()> {
        let mut simulation_cursor = self.simulation_cursor.borrow_mut();
        simulation_cursor.reset();
        let Some(sim_time) = simulation_cursor.step() else {
            unreachable!("Simulation cursor should always have a first step");
        };
        let globals_time = context.globals.chart_time;
        context.globals.chart_time = sim_time;
        self.evaluate_splines(sim_time);

        for step in &self.steps {
            if let ChartStep::Compute(compute) = step {
                if let Run::Init(_) = compute.run {
                    compute.execute(context)?;
                }
            }
        }
        Ok(())
    }

    /// Runs the simulation shaders.
    ///
    /// Also sets the ratio between two sim steps (`simulation_frame_ratio`) that should be used
    /// to blend buffer values during rendering.
    ///
    /// Returns true if the simulation is done, false if the simulation needs more iteration.
    fn simulate(&self, context: &mut ComputePassContext, is_precalculation: bool) -> Result<()> {
        // Save the app time and restore it after the simulation step.
        // The simulation sees the simulation time as the current time.
        let chart_time = context.globals.chart_time;

        let mut simulation_cursor = self.simulation_cursor.borrow_mut();
        simulation_cursor.advance_cursor(context.globals.simulation_elapsed_time_since_last_render);
        
        // let time = self.simulation_elapsed_time.get()
        //     + context.globals.simulation_elapsed_time_since_last_render;
        // let mut simulation_next_buffer_time = self.simulation_next_buffer_time.get();

        for step_count in 0.. {
            if !is_precalculation && step_count >= 3 {
                // Failsafe: limit the number steps per frame to avoid overloading the GPU.
                break;
            }
            let Some(sim_time) = simulation_cursor.step() else {
                // Simulation is up-to-date
                break;
            };

            context.globals.chart_time = sim_time;
            self.evaluate_splines(sim_time);

            for step in &self.steps {
                if let ChartStep::Compute(compute) = step {
                    if let Run::Simulate(_) = compute.run {
                        compute.execute(context)?;
                    }
                }
            }
        }

        context.globals.simulation_frame_ratio = simulation_cursor.ratio()?;

        // Restore globals
        context.globals.chart_time = chart_time;

        Ok(())
    }

    pub fn render(&self, context: &mut FrameContext) -> Result<()> {
        {
            // Simulation step
            // TODO: check if it's necessary to create a compute pass, if simulation needs to run
            let compute_pass =
                context.command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            let mut compute_pass_context = ComputePassContext {
                gpu_context: &context.gpu_context,
                pass: compute_pass,
                globals: &mut context.globals,
            };
            self.simulate(&mut compute_pass_context, false)?;
        }

        // Render step
        self.evaluate_splines(context.globals.chart_time);
        for image in &self.images {
            image.enforce_size_rule(&context.gpu_context, &context.screen_viewport.size)?;
        }
        for step in &self.steps {
            match step {
                ChartStep::Draw(draw) => {
                    draw.render(context, &self.camera)?;
                }
                ChartStep::Compute(_) => {
                    // Compute only runs simulation or init, no need to do anything during render.
                }
                ChartStep::GenerateMipLevels(genmips) => {
                    genmips.execute(context)?;
                }
            }
        }
        Ok(())
    }

    fn evaluate_splines(&self, time: f32) {
        for control in &self.controls.used_controls {
            control.evaluate_splines(time);
        }
    }
}

struct SimulationCursor {
    /// Current chart cursor.
    /// (time-SIMULATION_STEP_SECONDS) < cursor <= simulation_time
    cursor: f32,

    /// Time of the most recent simulated step
    simulation_time: Option<f32>,

    precalculation_time: f32,
}

impl SimulationCursor {
    pub fn new(precalculation_time: f32) -> SimulationCursor {
        SimulationCursor {
            cursor: 0.0,
            simulation_time: None,
            precalculation_time,
        }
    }

    /// Returns the time at which the init computation needs to run.
    /// Resetting the simulation does not reset the cursor to 0, the simulation
    /// can continue from the current point. It's useful for resetting the simulation
    /// during editing.
    pub fn reset(&mut self) {
        self.simulation_time = None;
    }

    /// Seeks the simulation to a specific time.
    /// Simulation needs to run at that point
    pub fn seek(&mut self, cursor: f32) {
        self.cursor = cursor;
        self.simulation_time = None;
    }

    pub fn advance_cursor(&mut self, elapsed: f32) {
        self.cursor += elapsed;
    }

    /// Returns a time at which the simulation needs to run.
    /// Returns None if the simulation is up-to-date.
    pub fn step(&mut self) -> Option<f32> {
        let sim_time = match self.simulation_time {
            Some(sim_time) if sim_time > self.cursor => return None,
            Some(sim_time) => sim_time + SIMULATION_STEP_SECONDS,
            None => self.cursor - self.precalculation_time,
        };
        self.simulation_time = Some(sim_time);
        Some(sim_time)
    }

    pub fn ratio(&self) -> Result<f32> {
        let Some(sim_time) = self.simulation_time else {
            bail!("Simulation did not run");
        };
        
        let ratio = 1.0 - (sim_time - self.cursor) / SIMULATION_STEP_SECONDS;
        Ok(ratio.min(1.0).max(0.0))
    }
}
