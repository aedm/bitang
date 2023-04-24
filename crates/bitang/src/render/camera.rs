use crate::control::controls::{Control, ControlSetBuilder};
use crate::control::{ControlId, ControlIdPartType};
use crate::render::vulkan_window::RenderContext;
use glam::{Mat4, Vec3};
use std::f32::consts::PI;
use std::rc::Rc;

pub struct Camera {
    target: Rc<Control>,
    orientation: Rc<Control>,
    distance: Rc<Control>,
}

impl Camera {
    pub fn new(control_set_builder: &mut ControlSetBuilder, control_id: &ControlId) -> Self {
        let target_id = control_id.add(ControlIdPartType::Value, "target");
        let orientation_id = control_id.add(ControlIdPartType::Value, "orientation");
        let distance_id = control_id.add(ControlIdPartType::Value, "distance");
        Camera {
            target: control_set_builder.get_vec3_with_default(&target_id, &[0.0, 0.0, 0.0]),
            orientation: control_set_builder
                .get_vec3_with_default(&orientation_id, &[0.0, 0.0, 0.0]),
            distance: control_set_builder.get_float_with_default(&distance_id, 5.),
        }
    }

    pub fn set(&self, context: &mut RenderContext, render_target_size: [f32; 2]) {
        let aspect_ratio = render_target_size[0] as f32 / render_target_size[1] as f32;

        // Vulkan uses a [0,1] depth range, ideal for infinite far plane
        context.globals.projection_from_camera =
            Mat4::perspective_infinite_lh(PI / 2.0, aspect_ratio, 0.1);

        // We use a left-handed, y-up coordinate system. But Vulkan screen space is right-handed, y-down.
        // So we need to flip the y-axis in the projection matrix.
        context.globals.projection_from_camera.y_axis *= -1.;

        // Camera transformation in world space
        let Vec3 { x, y, z } = self.orientation.as_vec3();
        let roll = Mat4::from_rotation_z(z);
        let pitch = Mat4::from_rotation_x(x);
        let yaw = Mat4::from_rotation_y(y);
        let target = Mat4::from_translation(-self.target.as_vec3());
        let distance = Mat4::from_translation(Vec3::new(0.0, 0.0, self.distance.as_float()));
        context.globals.camera_from_world = distance * roll * pitch * yaw * target;

        // Render objects should take care of their model-to-world transformation
        context.globals.world_from_model = Mat4::IDENTITY;

        context.globals.update_compound_matrices();
    }
}
