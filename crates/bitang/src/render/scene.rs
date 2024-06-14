use crate::render::render_object::RenderObject;
use crate::tool::RenderContext;
use anyhow::Result;

pub struct Scene {
    pub id: String,
    pub objects: Vec<RenderObject>,
}

impl Scene {
    pub fn render(&self, context: &mut RenderContext, material_pass_index: usize) -> Result<()> {
        for object in &self.objects {
            object.render(context, material_pass_index)?;
        }
        Ok(())
    }
}
