use crate::render::image::BitangImage;
use crate::tool::RenderContext;
use std::sync::Arc;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, BlitImageInfo, ImageBlit, PrimaryAutoCommandBuffer,
};
use vulkano::image::sampler::Filter;
use vulkano::image::{
    max_mip_levels, mip_level_extent, Image, ImageLayout, ImageSubresourceLayers,
};

pub struct GenerateMipLevels {
    pub _id: String,
    pub image: Arc<BitangImage>,
}

impl GenerateMipLevels {
    pub fn execute(&self, context: &mut RenderContext) -> anyhow::Result<()> {
        generate_mip_levels(self.image.get_image(), context.command_builder)
    }
}

pub fn generate_mip_levels(
    image: Arc<Image>,
    cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
) -> anyhow::Result<()> {
    let dimensions = image.extent();
    let mip_levels = max_mip_levels(dimensions);

    for mip_level in 1..mip_levels {
        cbb.blit_image(BlitImageInfo {
            src_image_layout: ImageLayout::General,
            dst_image_layout: ImageLayout::General,
            regions: [ImageBlit {
                src_subresource: ImageSubresourceLayers {
                    aspects: image.format().aspects(),
                    mip_level: mip_level - 1,
                    array_layers: 0..image.array_layers(),
                },
                dst_subresource: ImageSubresourceLayers {
                    aspects: image.format().aspects(),
                    mip_level: mip_level,
                    array_layers: 0..image.array_layers(),
                },
                src_offsets: [
                    [0, 0, 0],
                    mip_level_extent(dimensions, mip_level - 1).unwrap(),
                ],
                dst_offsets: [[0, 0, 0], mip_level_extent(dimensions, mip_level).unwrap()],
                ..Default::default()
            }]
            .into(),
            filter: Filter::Linear,
            ..BlitImageInfo::images(Arc::clone(&image), Arc::clone(&image))
        })?;
    }
    Ok(())
}
