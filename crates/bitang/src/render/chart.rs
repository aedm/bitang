use crate::control::controls::{Control, ControlSet, ControlSetBuilder};
use crate::control::{ControlId, ControlIdPartType};
use crate::render::material::MaterialStepType;
use crate::render::render_target::{Pass, RenderTarget};
use crate::render::vulkan_window::RenderContext;
use anyhow::Result;
use std::rc::Rc;
use std::sync::Arc;

pub struct Chart {
    pub id: String,
    pub controls: Rc<ControlSet>,
    _camera: Camera,
    render_targets: Vec<Arc<RenderTarget>>,
    pub passes: Vec<Pass>,
}

impl Chart {
    pub fn new(
        id: &str,
        control_prefix: &ControlId,
        mut control_set_builder: ControlSetBuilder,
        render_targets: Vec<Arc<RenderTarget>>,
        passes: Vec<Pass>,
    ) -> Self {
        let _camera = Camera::new(
            &mut control_set_builder,
            &control_prefix.add(ControlIdPartType::Camera, "camera"),
        );
        let controls = Rc::new(control_set_builder.into_control_set());
        Chart {
            id: id.to_string(),
            _camera,
            render_targets,
            passes,
            controls,
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
    fn new(control_set_builder: &mut ControlSetBuilder, control_prefix: &ControlId) -> Self {
        Camera {
            _position: control_set_builder
                .get_control(&control_prefix.add(ControlIdPartType::Value, "position")),
            _target: control_set_builder
                .get_control(&control_prefix.add(ControlIdPartType::Value, "target")),
            _up: control_set_builder
                .get_control(&control_prefix.add(ControlIdPartType::Value, "up")),
        }
    }
}
