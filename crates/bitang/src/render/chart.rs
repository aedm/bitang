use crate::control::controls::{Control, ControlSet, ControlSetBuilder};
use crate::control::{ControlId, ControlIdPartType};
use crate::render::buffer_generator::BufferGenerator;
use crate::render::material::MaterialStepType;
use crate::render::render_target::{Pass, RenderTarget};
use crate::render::vulkan_window::RenderContext;
use anyhow::Result;
use glam::Mat4;
use std::f32::consts::PI;
use std::rc::Rc;
use std::sync::Arc;

pub struct Chart {
    pub id: String,
    pub controls: Rc<ControlSet>,
    camera: Camera,
    render_targets: Vec<Arc<RenderTarget>>,
    buffer_generators: Vec<Arc<BufferGenerator>>,
    pub passes: Vec<Pass>,
}

impl Chart {
    pub fn new(
        id: &str,
        control_id: &ControlId,
        mut control_set_builder: ControlSetBuilder,
        render_targets: Vec<Arc<RenderTarget>>,
        buffer_generators: Vec<Arc<BufferGenerator>>,
        passes: Vec<Pass>,
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
            passes,
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
        for pass in &self.passes {
            pass.render(context, MaterialStepType::Solid, &self.camera)?;
        }
        Ok(())
    }
}

pub struct Camera {
    position: Rc<Control>,
    target: Rc<Control>,
    up: Rc<Control>,
}

impl Camera {
    fn new(control_set_builder: &mut ControlSetBuilder, control_id: &ControlId) -> Self {
        let position_id = control_id.add(ControlIdPartType::Value, "position");
        let target_id = control_id.add(ControlIdPartType::Value, "target");
        let up_id = control_id.add(ControlIdPartType::Value, "up");
        Camera {
            position: control_set_builder.get_vec3_with_default(&position_id, &[0.0, 0.0, -3.0]),
            target: control_set_builder.get_vec3_with_default(&target_id, &[0.0, 0.0, 0.0]),
            up: control_set_builder.get_vec3_with_default(&up_id, &[0.0, -1.0, 0.0]),
        }
    }

    pub fn set(&self, context: &mut RenderContext, render_target_size: [f32; 2]) {
        // Vulkan uses a [0,1] depth range, ideal for infinite far plane
        let aspect_ratio = render_target_size[0] as f32 / render_target_size[1] as f32;
        context.globals.projection_from_camera = Mat4::perspective_infinite_lh(
            PI / 2.0,
            aspect_ratio,
            // viewport.dimensions[0] / viewport.dimensions[1],
            0.1,
        );

        // We use a left-handed, y-up coordinate system.
        // Vulkan uses y-down, so we need to flip it back.
        context.globals.camera_from_world = Mat4::look_at_lh(
            self.position.as_vec3(),
            self.target.as_vec3(),
            self.up.as_vec3(),
        );

        context.globals.world_from_model = Mat4::IDENTITY;
        context.globals.update_compound_matrices();
    }
}
