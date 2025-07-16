use std::sync::Arc;

use super::{BitangImage, FrameContext, GpuContext, MipmapGenerator};

pub struct GenerateMipLevels {
    pub _id: String,
    generator: MipmapGenerator,
}

impl GenerateMipLevels {
    pub fn new(context: &GpuContext, id: &str, image: &Arc<BitangImage>) -> Self {
        Self {
            _id: id.to_owned(),
            generator: MipmapGenerator::new(&context.device, Arc::clone(image)),
        }
    }

    pub fn execute(&self, context: &mut FrameContext) -> anyhow::Result<()> {
        self.generator.generate(&mut context.command_encoder, &context.gpu_context.device)
    }
}
