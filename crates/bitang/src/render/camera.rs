use crate::control::controls::{Control, ControlSetBuilder, Globals};
use crate::control::{ControlId, ControlIdPartType};
use glam::{Mat4, Vec2, Vec3};
use std::f32::consts::PI;
use std::rc::Rc;
use std::sync::Arc;

pub struct Camera {
    target: Arc<Control>,
    orientation: Arc<Control>,
    distance: Arc<Control>,
    field_of_view: Arc<Control>,
    shake: Arc<Control>,
    speed: Arc<Control>,
    time_adjustment: Arc<Control>,
}

impl Camera {
    pub fn new(control_set_builder: &ControlSetBuilder, control_id: &ControlId) -> Self {
        let target_id = control_id.add(ControlIdPartType::Value, "target");
        let orientation_id = control_id.add(ControlIdPartType::Value, "orientation");
        let distance_id = control_id.add(ControlIdPartType::Value, "distance");
        let field_of_view_id = control_id.add(ControlIdPartType::Value, "field_of_view");
        let shake_id = control_id.add(ControlIdPartType::Value, "shake");
        let speed_id = control_id.add(ControlIdPartType::Value, "speed");
        let time_adjustment_id = control_id.add(ControlIdPartType::Value, "time_adjustment");
        Camera {
            target: control_set_builder.get_vec3_with_default(&target_id, &[0.0, 0.0, 0.0]),
            orientation: control_set_builder
                .get_vec3_with_default(&orientation_id, &[0.0, 0.0, 0.0]),
            distance: control_set_builder.get_float_with_default(&distance_id, 5.),
            field_of_view: control_set_builder.get_float_with_default(&field_of_view_id, PI / 2.0),
            shake: control_set_builder.get_vec4(&shake_id),
            speed: control_set_builder.get_float_with_default(&speed_id, 1.),
            time_adjustment: control_set_builder.get_float_with_default(&time_adjustment_id, 0.),
        }
    }

    pub fn set(&self, globals: &mut Globals, viewport_size: [f32; 2]) {
        globals.pixel_size = Vec2::new(1.0 / viewport_size[0], 1.0 / viewport_size[1]);
        globals.aspect_ratio = viewport_size[0] / viewport_size[1];
        globals.field_of_view = self.field_of_view.as_float();
        globals.z_near = 0.1;

        // Vulkan uses a [0,1] depth range, ideal for infinite far plane
        globals.projection_from_camera = Mat4::perspective_infinite_lh(
            globals.field_of_view,
            globals.aspect_ratio,
            globals.z_near,
        );

        // We use a left-handed, y-up coordinate system. But Vulkan screen space is right-handed, y-down.
        // So we need to flip the y-axis in the projection matrix.
        globals.projection_from_camera.y_axis *= -1.;

        // Shake
        let shake = {
            let s = self.shake.as_vec4();
            let time =
                globals.app_time * self.speed.as_float() * 10.0 + self.time_adjustment.as_float();
            let shc = (1.0, 2.423, 1.834634);
            let t = (time, time * 1.257443, time * 1.1123658);
            let sens = 0.004 * s.w;
            let shake_pitch =
                ((t.0 * shc.0).sin() * (t.0 * shc.1).sin() * (t.0 * shc.2).sin()) * s.x * sens;
            let shake_yaw =
                ((t.1 * shc.0).sin() * (t.1 * shc.1).sin() * (t.1 * shc.2).sin()) * s.y * sens;
            let shake_roll =
                ((t.2 * shc.0).sin() * (t.2 * shc.1).sin() * (t.2 * shc.2).sin()) * s.z * sens;
            let roll = Mat4::from_rotation_z(shake_roll);
            let pitch = Mat4::from_rotation_x(shake_pitch);
            let yaw = Mat4::from_rotation_y(shake_yaw);
            roll * pitch * yaw
        };

        // Camera transformation in world space
        let Vec3 { x, y, z } = self.orientation.as_vec3();
        let roll = Mat4::from_rotation_z(z);
        let pitch = Mat4::from_rotation_x(x);
        let yaw = Mat4::from_rotation_y(y);
        let target = Mat4::from_translation(-self.target.as_vec3());
        let distance = Mat4::from_translation(Vec3::new(0.0, 0.0, self.distance.as_float()));
        globals.camera_from_world = shake * distance * roll * pitch * yaw * target;

        // Render objects should take care of their model-to-world transformation
        globals.world_from_model = Mat4::IDENTITY;

        globals.update_compound_matrices();
    }
}
