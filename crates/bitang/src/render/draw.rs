use crate::render::camera::Camera;
use crate::render::pass::Pass;
use crate::render::render_object::RenderObject;
use crate::render::vulkan_window::RenderContext;
use anyhow::{anyhow, Result};
use std::sync::Arc;
use vulkano::command_buffer::SubpassContents;

/// Represents a draw step in the chart sequence.
pub struct Draw {
    pub id: String,
    pub passes: Vec<Pass>,
    pub objects: Vec<Arc<RenderObject>>,
}

impl Draw {
    pub fn new(id: &str, passes: Vec<Pass>, objects: Vec<Arc<RenderObject>>) -> Result<Draw> {
        Ok(Draw {
            id: id.to_string(),
            passes,
            objects,
        })
    }

    fn render_objects(&self, context: &mut RenderContext) -> Result<()> {
        for object in &self.objects {
            object.render(context, 0)?;
        }
        Ok(())
    }

    pub fn render(&self, context: &mut RenderContext, camera: &Camera) -> Result<()> {
        if self.passes.is_empty() {
            return Err(anyhow!("Draw '{}' has no passes", self.id));
        }

        for (_pass_index, pass) in self.passes.iter().enumerate() {
            let viewport = pass.get_viewport(context)?;
            camera.set(&mut context.globals, viewport.dimensions);

            let render_pass_begin_info = pass.make_render_pass_begin_info(context)?;
            context
                .command_builder
                .begin_render_pass(render_pass_begin_info, SubpassContents::Inline)?
                .set_viewport(0, [viewport]);

            // Don't fail early, we must end the render pass
            let result = self.render_objects(context);
            context.command_builder.end_render_pass()?;
            result?;
        }

        Ok(())
    }
}
