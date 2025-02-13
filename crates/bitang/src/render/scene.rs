use crate::render::render_object::RenderObject;
use crate::tool::FrameContext;
use anyhow::Result;

pub struct Scene {
    pub _id: String,
    pub objects: Vec<RenderObject>,
}

impl Scene {
    pub fn render(&self, context: &mut FrameContext, material_pass_index: usize) -> Result<()> {
        for object in &self.objects {
            object.render(context, material_pass_index)?;
        }
        Ok(())
    }
}
