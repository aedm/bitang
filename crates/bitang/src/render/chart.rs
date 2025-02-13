use crate::control::controls::{ControlSet, ControlSetBuilder};
use crate::control::{ControlId, ControlIdPartType};
use crate::render::buffer_generator::BufferGenerator;
use crate::render::camera::Camera;
use crate::render::compute::{Compute, Run};
use crate::render::draw::Draw;
use crate::render::generate_mip_levels::GenerateMipLevels;
use crate::render::image::BitangImage;
use crate::render::SIMULATION_STEP_SECONDS;
use crate::tool::FrameContext;
use anyhow::Result;
use std::cell::Cell;
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
    buffer_generators: Vec<Rc<BufferGenerator>>,
    pub steps: Vec<ChartStep>,

    /// The time representing the `Next` step
    simulation_next_buffer_time: Cell<f32>,

    /// The elapsed time, normally between `Current` and `Next` steps of the simulation
    simulation_elapsed_time: Cell<f32>,

    /// Simulation should be running this long during precalculation
    simulation_precalculation_time: f32,
}

impl Chart {
    pub fn new(
        id: &str,
        control_id: &ControlId,
        control_set_builder: ControlSetBuilder,
        images: Vec<Arc<BitangImage>>,
        buffer_generators: Vec<Rc<BufferGenerator>>,
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
            buffer_generators,
            steps,
            controls,
            simulation_next_buffer_time: Cell::new(0.0),
            simulation_elapsed_time: Cell::new(0.0),
            simulation_precalculation_time,
        }
    }

    /// Returns true if the simulation is done, false if the simulation needs more iteration to
    /// catch up with the current time.
    pub fn reset_simulation(
        &self,
        context: &mut FrameContext,
        run_init: bool,
        run_precalc: bool,
    ) -> Result<bool> {
        if run_init {
            self.initialize(context)?;

            // Chart is started at negative time during precalculation
            context.simulation_elapsed_time_since_last_render = self.simulation_precalculation_time;
            self.simulation_elapsed_time
                .set(-self.simulation_precalculation_time);
            self.simulation_next_buffer_time
                .set(-self.simulation_precalculation_time);
        }
        if run_precalc {
            return self.simulate(context, true);
        }
        Ok(true)
    }

    fn initialize(&self, context: &mut FrameContext) -> Result<()> {
        self.simulation_next_buffer_time.set(0.0);
        self.simulation_elapsed_time.set(0.0);
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
    /// Returns true if the simulation is done, false if the simulation needs more iteration to
    /// catch up with the current time.
    fn simulate(&self, context: &mut FrameContext, is_precalculation: bool) -> Result<bool> {
        // Save the app time and restore it after the simulation step.
        // The simulation sees the simulation time as the current time.
        let app_time = context.globals.app_time;
        let chart_time = context.globals.chart_time;

        let time =
            self.simulation_elapsed_time.get() + context.simulation_elapsed_time_since_last_render;
        let mut simulation_next_buffer_time = self.simulation_next_buffer_time.get();

        // Failsafe: limit the number steps per frame to avoid overloading the GPU.
        let maximum_steps = if is_precalculation { 10 } else { 3 };
        for _ in 0..maximum_steps {
            if simulation_next_buffer_time > time {
                break;
            }

            // Calculate chart time
            if is_precalculation {
                context.globals.app_time = simulation_next_buffer_time;
                context.globals.chart_time = simulation_next_buffer_time;
                self.evaluate_splines(simulation_next_buffer_time);
            }

            for step in &self.steps {
                if let ChartStep::Compute(compute) = step {
                    if let Run::Simulate(_) = compute.run {
                        compute.execute(context)?;
                    }
                }
            }
            simulation_next_buffer_time += SIMULATION_STEP_SECONDS;
        }

        self.simulation_next_buffer_time
            .set(simulation_next_buffer_time);
        self.simulation_elapsed_time.set(time);

        let ratio = 1.0 - (simulation_next_buffer_time - time) / SIMULATION_STEP_SECONDS;
        context.globals.simulation_frame_ratio = ratio.min(1.0).max(0.0);

        // Restore globals
        context.globals.app_time = app_time;
        context.globals.chart_time = chart_time;

        // Simulation is done if next time is greater than current time
        Ok(simulation_next_buffer_time >= time)
    }

    pub fn render(&self, context: &mut FrameContext) -> Result<()> {
        // Simulation step
        self.simulate(context, false)?;

        // Render step
        self.evaluate_splines(context.globals.chart_time);
        for image in &self.images {
            image.enforce_size_rule(&context.vulkan_context, context.screen_viewport.extent)?;
        }
        for buffer_generator in &self.buffer_generators {
            buffer_generator.generate()?;
        }
        for step in &self.steps {
            match step {
                ChartStep::Draw(draw) => {
                    draw.render(context, &self.camera)?;
                }
                ChartStep::Compute(_) => {
                    // Compute only runs simulation or init, ignore for now
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
