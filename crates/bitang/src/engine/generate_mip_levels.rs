use std::sync::Arc;

use anyhow::bail;

use crate::engine::RenderStage;

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
        let RenderStage::Offscreen(command_encoder) = &mut context.render_stage else {
            bail!("GenerateMipLevels can only be executed in offscreen mode");
        };
        self.generator.generate(command_encoder, &context.gpu_context.device)
    }
}
