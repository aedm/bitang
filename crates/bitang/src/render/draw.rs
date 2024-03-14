use crate::control::controls::{Control, Globals};
use crate::render::camera::Camera;
use crate::render::pass::Pass;
use crate::render::render_object::RenderObject;
use crate::tool::RenderContext;
use anyhow::{ensure, Result};
use glam::{Mat4, Vec2, Vec3};
use std::rc::Rc;

use vulkano::command_buffer::{SubpassBeginInfo, SubpassContents};

/// Represents a draw step in the chart sequence.
pub struct Draw {
    pub id: String,
    pub passes: Vec<Pass>,
    pub objects: Vec<Rc<RenderObject>>,
    pub light_dir: Rc<Control>,
    pub shadow_map_size: Rc<Control>,
}

impl Draw {
    pub fn new(
        id: &str,
        passes: Vec<Pass>,
        objects: Vec<Rc<RenderObject>>,
        light_dir: Rc<Control>,
        shadow_map_size: Rc<Control>,
    ) -> Result<Draw> {
        Ok(Draw {
            id: id.to_string(),
            passes,
            objects,
            light_dir,
            shadow_map_size,
        })
    }

    fn render_objects(&self, context: &mut RenderContext, pass_index: usize) -> Result<()> {
        for object in &self.objects {
            object.render(context, pass_index)?;
        }
        Ok(())
    }

    fn set_light(&self, globals: &mut Globals) {
        let light_dir = self.light_dir.as_vec3().normalize();
        let shadow_map_size = self.shadow_map_size.as_float();

        globals.pixel_size = Vec2::new(1.0 / shadow_map_size, 1.0 / shadow_map_size);
        globals.aspect_ratio = 1.0;
        globals.field_of_view = 0.0;
        globals.z_near = -shadow_map_size;
        globals.light_dir = light_dir;
        globals.shadow_map_size = shadow_map_size;

        globals.projection_from_camera = Mat4::orthographic_lh(
            -shadow_map_size,
            shadow_map_size,
            -shadow_map_size,
            shadow_map_size,
            -shadow_map_size,
            shadow_map_size,
        );

        globals.camera_from_world = Mat4::look_to_lh(Vec3::ZERO, -light_dir, Vec3::Y);

        // Render objects should take care of their model-to-world transformation
        globals.world_from_model = Mat4::IDENTITY;
        globals.light_projection_from_world =
            globals.projection_from_camera * globals.camera_from_world;

        globals.update_compound_matrices();
    }

    pub fn render(&self, context: &mut RenderContext, camera: &Camera) -> Result<()> {
        ensure!(!self.passes.is_empty(), "Draw '{}' has no passes", self.id);

        for (pass_index, pass) in self.passes.iter().enumerate() {
            let viewport = pass.get_viewport(context)?;
            if pass.id == "shadow" {
                self.set_light(&mut context.globals);
            } else {
                camera.set(&mut context.globals, viewport.extent);
            }

            let render_pass_begin_info = pass.make_render_pass_begin_info(context)?;
            let subpass_begin_info = SubpassBeginInfo {
                contents: SubpassContents::Inline,
                ..Default::default()
            };
            context
                .command_builder
                .begin_render_pass(render_pass_begin_info, subpass_begin_info)?
                .set_viewport(0, [viewport].into_iter().collect())?;

            // Don't fail early, we must end the render pass
            let result = self.render_objects(context, pass_index);
            context
                .command_builder
                .end_render_pass(Default::default())?;
            result?;
        }

        Ok(())
    }
}
