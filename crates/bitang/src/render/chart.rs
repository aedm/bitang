use crate::control::controls::{Control, Controls};
use crate::control::{ControlId, ControlIdPartType};
use crate::render::material::MaterialStepType;
use crate::render::render_target::{Pass, RenderTarget};
use crate::render::vulkan_window::RenderContext;
use anyhow::Result;
use std::rc::Rc;
use std::sync::Arc;

pub struct Chart {
    pub id: String,
    _camera: Camera,
    render_targets: Vec<Arc<RenderTarget>>,
    pub passes: Vec<Pass>,
}

impl Chart {
    pub fn new(
        id: &str,
        control_prefix: &ControlId,
        controls: &mut Controls,
        render_targets: Vec<Arc<RenderTarget>>,
        passes: Vec<Pass>,
    ) -> Self {
        Chart {
            id: id.to_string(),
            _camera: Camera::new(
                controls,
                &control_prefix.add(ControlIdPartType::Camera, "camera"),
            ),
            render_targets,
            passes,
        }
    }

    pub fn render(&self, context: &mut RenderContext) -> Result<()> {
        for render_target in &self.render_targets {
            render_target.ensure_buffer(context)?;
        }
        for pass in &self.passes {
            pass.render(context, MaterialStepType::Solid)?;
        }
        Ok(())
    }
}

struct Camera {
    _position: Rc<Control>,
    _target: Rc<Control>,
    _up: Rc<Control>,
}

impl Camera {
    fn new(controls: &mut Controls, control_prefix: &ControlId) -> Self {
        Camera {
            _position: controls
                .get_control(&control_prefix.add(ControlIdPartType::Value, "position")),
            _target: controls.get_control(&control_prefix.add(ControlIdPartType::Value, "target")),
            _up: controls.get_control(&control_prefix.add(ControlIdPartType::Value, "up")),
        }
    }
}
