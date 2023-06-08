use crate::control::controls::{ControlSet, ControlSetBuilder};
use crate::control::{ControlId, ControlIdPartType};
use crate::render::buffer_generator::BufferGenerator;
use crate::render::camera::Camera;
use crate::render::draw::Draw;
use crate::render::material::MaterialStepType;
use crate::render::render_target::RenderTarget;
use crate::render::vulkan_window::RenderContext;
use anyhow::Result;
use std::rc::Rc;
use std::sync::Arc;

pub struct Chart {
    pub id: String,
    pub controls: Rc<ControlSet>,
    camera: Camera,
    render_targets: Vec<Arc<RenderTarget>>,
    buffer_generators: Vec<Arc<BufferGenerator>>,
    pub steps: Vec<Draw>,
}

impl Chart {
    pub fn new(
        id: &str,
        control_id: &ControlId,
        mut control_set_builder: ControlSetBuilder,
        render_targets: Vec<Arc<RenderTarget>>,
        buffer_generators: Vec<Arc<BufferGenerator>>,
        passes: Vec<Draw>,
    ) -> Self {
        let _camera = Camera::new(
            &mut control_set_builder,
            &control_id.add(ControlIdPartType::Camera, "camera"),
        );
        let controls = Rc::new(control_set_builder.into_control_set());
        Chart {
            id: id.to_string(),
            camera: _camera,
            render_targets,
            buffer_generators,
            steps: passes,
            controls,
        }
    }

    pub fn render(&self, context: &mut RenderContext) -> Result<()> {
        for render_target in &self.render_targets {
            render_target.ensure_buffer(context)?;
        }
        for buffer_generator in &self.buffer_generators {
            buffer_generator.generate()?;
        }
        for pass in &self.steps {
            pass.render(context, MaterialStepType::Solid, &self.camera)?;
        }
        Ok(())
    }
}
