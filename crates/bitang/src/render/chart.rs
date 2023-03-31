use crate::control::controls::{Control, Controls};
use crate::render::material::MaterialStepType;
use crate::render::render_target::{Pass, RenderTarget};
use crate::render::render_unit::RenderUnit;
use crate::render::vulkan_window::{RenderContext, VulkanContext};
use crate::render::RenderObject;
use std::rc::Rc;
use std::sync::Arc;

pub struct Chart {
    id: String,
    camera: Camera,
    render_targets: Vec<Arc<RenderTarget>>,
    passes: Vec<Pass>,
}

impl Chart {
    pub fn new(
        id: &str,
        controls: &mut Controls,
        render_targets: Vec<Arc<RenderTarget>>,
        passes: Vec<Pass>,
    ) -> Self {
        Chart {
            id: id.to_string(),
            camera: Camera::new(controls, &format!("{id}/camera")),
            render_targets,
            passes,
        }
    }

    pub fn render(&self, context: &mut RenderContext) {
        for render_target in &self.render_targets {
            render_target.ensure_buffer(context).unwrap();
        }
        for pass in &self.passes {
            pass.render(context, MaterialStepType::Solid);
        }
    }
}

struct Camera {
    position: Rc<Control>,
    target: Rc<Control>,
    up: Rc<Control>,
}

impl Camera {
    fn new(controls: &mut Controls, prefix: &str) -> Self {
        Camera {
            position: controls.get_control(&format!("{prefix}/position")),
            target: controls.get_control(&format!("{prefix}/target")),
            up: controls.get_control(&format!("{prefix}/up")),
        }
    }
}
