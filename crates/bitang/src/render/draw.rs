use crate::control::controls::{Control, Globals};
use crate::render::camera::Camera;
use crate::render::pass::Pass;
use crate::render::render_object::RenderObject;
use crate::tool::{FrameContext, RenderPassContext, Viewport};
use anyhow::{ensure, Result};
use glam::{Mat4, Vec2, Vec3};
use std::rc::Rc;
use tracing::warn;

use crate::render::scene::Scene;

pub(crate) enum DrawItem {
    Object(Rc<RenderObject>),
    Scene(Rc<Scene>),
}

/// Represents a draw step in the chart sequence.
pub struct Draw {
    pub id: String,
    pub passes: Vec<Pass>,
    pub items: Vec<DrawItem>,
    pub light_dir_worldspace: Rc<Control>,
    pub shadow_map_size: Rc<Control>,
}

impl Draw {
    pub fn new(
        id: &str,
        passes: Vec<Pass>,
        items: Vec<DrawItem>,
        light_dir_worldspace: Rc<Control>,
        shadow_map_size: Rc<Control>,
    ) -> Result<Draw> {
        Ok(Draw {
            id: id.to_string(),
            passes,
            items,
            light_dir_worldspace,
            shadow_map_size,
        })
    }

    fn render_items(&self, context: &mut RenderPassContext, pass_index: usize) -> Result<()> {
        for object in &self.items {
            match object {
                DrawItem::Object(object) => object.render(context, pass_index)?,
                DrawItem::Scene(scene) => scene.render(context, pass_index)?,
            }
        }
        Ok(())
    }

    fn set_common_globals(&self, globals: &mut Globals) {
        let light_dir_worldspace_norm = self.light_dir_worldspace.as_vec3().normalize();
        globals.light_dir_worldspace_norm = light_dir_worldspace_norm;
    }

    fn set_globals_for_shadow_map_rendering(&self, globals: &mut Globals) {
        let shadow_map_size = self.shadow_map_size.as_float();

        globals.pixel_size = Vec2::new(1.0 / shadow_map_size, 1.0 / shadow_map_size);
        globals.aspect_ratio = 1.0;
        globals.field_of_view = 0.0;
        globals.z_near = -shadow_map_size;
        globals.shadow_map_size = shadow_map_size;

        globals.projection_from_camera = Mat4::orthographic_lh(
            -shadow_map_size,
            shadow_map_size,
            -shadow_map_size,
            shadow_map_size,
            -shadow_map_size,
            shadow_map_size,
        );

        // TODO: position shadow center to camera target, fix artifacts
        globals.camera_from_world =
            Mat4::look_to_lh(Vec3::ZERO, -globals.light_dir_worldspace_norm, Vec3::Y);

        // When camera space is the light source space, the direction of light is always forward
        globals.light_dir_camspace_norm = Vec3::Z;

        // Render objects should take care of their model-to-world transformation
        globals.world_from_model = Mat4::IDENTITY;
        globals.light_projection_from_world =
            globals.projection_from_camera * globals.camera_from_world;

        globals.update_compound_matrices();
    }

    pub fn render(&self, frame_context: &mut FrameContext, camera: &Camera) -> Result<()> {
        ensure!(!self.passes.is_empty(), "Draw '{}' has no passes", self.id);

        // Render each pass
        for (pass_index, pass) in self.passes.iter().enumerate() {
            let (viewport, canvas_size) = pass.get_viewport_and_canvas_size(frame_context)?;

            // Set globals unspecific to pass
            self.set_common_globals(&mut frame_context.globals);

            // Set pass-specific globals
            if pass.id == "shadow" {
                self.set_globals_for_shadow_map_rendering(&mut frame_context.globals);
            } else {
                camera.set_globals(&mut frame_context.globals, canvas_size);
            }

            let mut render_pass_context = pass.make_render_pass_context(frame_context)?;
            let Viewport { x, y, size } = viewport;
            render_pass_context.pass.set_viewport(x as f32, y as f32, size[0] as f32, size[1] as f32, 0.0, 1.0);

            self.render_items(&mut render_pass_context, pass_index)?;
        }

        Ok(())
    }
}
