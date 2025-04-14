use crate::document::render_object::RenderObject;
use crate::tool::RenderPassContext;
use anyhow::Result;

pub struct Scene {
    pub _id: String,
    pub objects: Vec<RenderObject>,
}

impl Scene {
    pub fn render(
        &self,
        context: &mut RenderPassContext,
        material_pass_index: usize,
    ) -> Result<()> {
        for object in &self.objects {
            object.render(context, material_pass_index)?;
        }
        Ok(())
    }
}
