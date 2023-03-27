use crate::control::controls::{Control, Controls, ControlsAndGlobals};
use crate::render::render_target::{Pass, RenderTarget};
use crate::render::render_unit::RenderUnit;
use crate::render::vulkan_window::VulkanContext;
use crate::render::RenderObject;
use std::rc::Rc;
use std::sync::Arc;

pub struct Chart {
    id: String,
    camera: Camera,
    render_targets: Vec<Arc<RenderTarget>>,
    passes: Vec<Pass>,
    // render_object: Arc<RenderObject>,
    // render_unit: Option<RenderUnit>,
}

impl Chart {
    pub fn new(
        id: &str,
        controls: &mut ControlsAndGlobals,
        render_targets: Vec<Arc<RenderTarget>>,
        passes: Vec<Pass>,
        // render_object: &Arc<RenderObject>,
    ) -> Self {
        Chart {
            id: id.to_string(),
            camera: Camera::new(controls, &format!("{prefix}/camera")),
            render_targets,
            passes,
            // render_object: render_object.clone(),
            // render_unit: None,
        }
    }

    pub fn generate_render_sequence(&mut self, context: &VulkanContext) {}

    pub fn render(&mut self, context: &VulkanContext) {}
}

struct Camera {
    position: Rc<Control>,
    target: Rc<Control>,
    up: Rc<Control>,
}

impl Camera {
    fn new(controls: &mut ControlsAndGlobals, prefix: &str) -> Self {
        Camera {
            position: controls.get_control(&format!("{prefix}/position")),
            target: controls.get_control(&format!("{prefix}/target")),
            up: controls.get_control(&format!("{prefix}/up")),
        }
    }
}
