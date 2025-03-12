use crate::render::image::BitangImage;
use crate::tool::FrameContext;
use crate::tool::GpuContext;
use std::sync::Arc;
use std::sync::OnceLock;

pub struct GenerateMipLevels {
    pub _id: String,
    pub image: Arc<BitangImage>,
}

impl GenerateMipLevels {
    pub fn execute(&self, context: &mut FrameContext) -> anyhow::Result<()> {
        self.image.generate_mipmaps(context)
    }
}

