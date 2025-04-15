use crate::engine::render::image::BitangImage;
use crate::tool::{FrameContext, GpuContext};
use std::sync::Arc;

use crate::engine::render::mipmap_generator::MipmapGenerator;

pub struct GenerateMipLevels {
    pub _id: String,
    pub image: Arc<BitangImage>,
    generator: MipmapGenerator,
}

impl GenerateMipLevels {
    pub fn new(context: &GpuContext, id: &str, image: Arc<BitangImage>) -> Self {
        Self {
            _id: id.to_owned(),
            generator: MipmapGenerator::new(&context.device, image.clone()),
            image,
        }
    }

    pub fn execute(&self, context: &mut FrameContext) -> anyhow::Result<()> {
        self.generator.generate(&mut context.command_encoder, &context.gpu_context.device)
    }
}
