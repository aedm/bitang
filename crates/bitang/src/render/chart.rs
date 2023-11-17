use crate::control::controls::{ControlSet, ControlSetBuilder};
use crate::control::{ControlId, ControlIdPartType};
use crate::render::buffer_generator::BufferGenerator;
use crate::render::camera::Camera;
use crate::render::compute::{Compute, Run};
use crate::render::draw::Draw;
use crate::render::image::Image;
use crate::render::SIMULATION_STEP_SECONDS;
use crate::tool::RenderContext;
use anyhow::Result;
use std::cell::Cell;
use std::sync::Arc;

pub enum ChartStep {
    Draw(Draw),
    Compute(Compute),
}

pub struct Chart {
    pub id: String,
    pub controls: Arc<ControlSet>,
    camera: Camera,
    images: Vec<Arc<Image>>,
    buffer_generators: Vec<Arc<BufferGenerator>>,
    pub steps: Vec<ChartStep>,

    is_initialized: Cell<bool>,

    /// The time representing the `Next` step
    simulation_next_buffer_time: Cell<f32>,

    /// The elapsed time, normally between `Current` and `Next` steps of the simulation
    simulation_elapsed_time: Cell<f32>,
}

impl Chart {
    pub fn new(
        id: &str,
        control_id: &ControlId,
        control_set_builder: ControlSetBuilder,
        images: Vec<Arc<Image>>,
        buffer_generators: Vec<Arc<BufferGenerator>>,
        steps: Vec<ChartStep>,
    ) -> Self {
        let _camera = Camera::new(
            &control_set_builder,
            &control_id.add(ControlIdPartType::Camera, "camera"),
        );
        let controls = Arc::new(control_set_builder.into_control_set());
        Chart {
            id: id.to_string(),
            camera: _camera,
            images,
            buffer_generators,
            steps,
            controls,
            is_initialized: Cell::new(false),
            simulation_next_buffer_time: Cell::new(0.0),
            simulation_elapsed_time: Cell::new(0.0),
        }
    }

    pub fn reset(&self) {
        self.is_initialized.set(false);
        self.simulation_next_buffer_time.set(0.0);
    }

    pub fn initialize(&self, context: &mut RenderContext) -> Result<()> {
        if self.is_initialized.get() {
            return Ok(());
        }
        for step in &self.steps {
            if let ChartStep::Compute(compute) = step {
                if let Run::Init(_) = compute.run {
                    compute.execute(context)?;
                }
            }
        }
        self.is_initialized.set(true);
        Ok(())
    }

    /// Runs the simulation shaders. Returns the ratio between two sim steps that should be used
    /// to blend buffer values during rendering.
    pub fn run_simulation(&self, context: &mut RenderContext) -> Result<f32> {
        let time =
            self.simulation_elapsed_time.get() + context.simulation_elapsed_time_since_last_render;
        let mut simulation_next_buffer_time = self.simulation_next_buffer_time.get();
        // Failsafe: limit the number steps per frame to avoid overloading the GPU.
        for _ in 0..2 {
            if simulation_next_buffer_time > time {
                break;
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
        Ok(ratio.min(1.0).max(0.0))
    }

    pub fn render(&self, context: &mut RenderContext) -> Result<()> {
        for image in &self.images {
            image.enforce_size_rule(&context.vulkan_context, context.screen_viewport.dimensions)?;
        }
        for buffer_generator in &self.buffer_generators {
            buffer_generator.generate()?;
        }
        self.initialize(context)?;
        self.run_simulation(context)?;

        for step in &self.steps {
            if let ChartStep::Draw(draw) = step {
                draw.render(context, &self.camera)?;
            }
        }
        Ok(())
    }
}
