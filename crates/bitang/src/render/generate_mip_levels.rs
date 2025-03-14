use crate::render::image::BitangImage;
use crate::tool::FrameContext;
use std::sync::Arc;

pub struct GenerateMipLevels {
    pub _id: String,
    pub image: Arc<BitangImage>,
}

impl GenerateMipLevels {
    pub fn execute(&self, context: &mut FrameContext) -> anyhow::Result<()> {
        self.image.generate_mipmaps(context)
    }
}
