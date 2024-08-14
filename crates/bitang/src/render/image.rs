use crate::tool::VulkanContext;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::sync::{Arc, RwLock};
use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, BlitImageInfo, CommandBufferUsage, CopyBufferToImageInfo, ImageBlit,
    PrimaryCommandBufferAbstract,
};
use vulkano::image::sampler::Filter;
use vulkano::image::view::ImageView;
use vulkano::image::{
    max_mip_levels, mip_level_extent, Image, ImageCreateInfo, ImageLayout, ImageSubresourceLayers,
    ImageTiling, ImageType, ImageUsage, SampleCount,
};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano::render_pass::{AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp};

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum PixelFormat {
    Rgba16F,
    Rgba32F,
    Depth32F,
    Rgba8U,
    Rgba8Srgb,

    // Intel only apparently supports this surface format, no RGBA_SRGB.
    Bgra8Srgb,
}

impl PixelFormat {
    pub fn vulkan_format(&self) -> vulkano::format::Format {
        match self {
            PixelFormat::Rgba16F => vulkano::format::Format::R16G16B16A16_SFLOAT,
            PixelFormat::Rgba32F => vulkano::format::Format::R32G32B32A32_SFLOAT,
            PixelFormat::Depth32F => vulkano::format::Format::D32_SFLOAT,
            PixelFormat::Rgba8U => vulkano::format::Format::R8G8B8A8_UNORM,
            PixelFormat::Rgba8Srgb => vulkano::format::Format::R8G8B8A8_SRGB,
            PixelFormat::Bgra8Srgb => vulkano::format::Format::B8G8R8A8_SRGB,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum ImageSizeRule {
    Fixed(u32, u32),
    CanvasRelative(f32),
    At4k(u32, u32),
}

enum ImageInner {
    Immutable(Arc<Image>),
    SingleLevelAttachment(RwLock<Option<Arc<Image>>>),
    Swapchain(RwLock<Option<Arc<ImageView>>>),
}

pub struct BitangImage {
    pub id: String,
    pub size_rule: ImageSizeRule,
    pub vulkan_format: vulkano::format::Format,
    // pub mip_levels: MipLevels,
    inner: ImageInner,
    size: RwLock<Option<(u32, u32)>>,
}

impl BitangImage {
    pub fn new_immutable(id: &str, source: Arc<Image>) -> Arc<Self> {
        let dim = source.extent();
        Arc::new(Self {
            id: id.to_owned(),
            vulkan_format: source.format(),
            inner: ImageInner::Immutable(source),
            size_rule: ImageSizeRule::Fixed(dim[0], dim[1]),
            size: RwLock::new(Some((dim[0], dim[1]))),
        })
    }

    pub fn new_attachment(id: &str, format: PixelFormat, size_rule: ImageSizeRule) -> Arc<Self> {
        Arc::new(Self {
            id: id.to_owned(),
            inner: ImageInner::SingleLevelAttachment(RwLock::new(None)),
            size_rule,
            size: RwLock::new(None),
            vulkan_format: format.vulkan_format(),
        })
    }

    pub fn new_swapchain(id: &str, format: PixelFormat) -> Arc<Self> {
        Arc::new(Self {
            id: id.to_owned(),
            inner: ImageInner::Swapchain(RwLock::new(None)),
            size_rule: ImageSizeRule::CanvasRelative(1.0),
            size: RwLock::new(None),
            vulkan_format: format.vulkan_format(),
        })
    }

    pub fn immutable_from_iter<I, T>(
        id: &str,
        context: &Arc<VulkanContext>,
        format: PixelFormat,
        dimensions: [u32; 3],
        pixel_data: I,
    ) -> Result<Arc<Self>>
    where
        T: BufferContents,
        I: IntoIterator<Item = T>,
        I::IntoIter: ExactSizeIterator,
    {
        let mut cbb = AutoCommandBufferBuilder::primary(
            &context.command_buffer_allocator,
            context.gfx_queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )?;

        let mip_levels = max_mip_levels(dimensions);

        let image = Image::new(
            context.memory_allocator.clone(),
            vulkano::image::ImageCreateInfo {
                image_type: ImageType::Dim2d,
                format: format.vulkan_format(),
                extent: dimensions,
                usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED | ImageUsage::TRANSFER_SRC,
                mip_levels,
                tiling: ImageTiling::Optimal,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
        )?;

        let upload_buffer = Buffer::from_iter(
            context.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_SRC,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            pixel_data,
        )?;

        cbb.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
            upload_buffer,
            image.clone(),
        ))?;

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

        let _fut = cbb.build()?.execute(context.gfx_queue.clone())?;

        Ok(BitangImage::new_immutable(id, image))
    }

    pub fn get_view(&self) -> Result<Arc<ImageView>> {
        let view: Arc<ImageView> = match &self.inner {
            ImageInner::Immutable(image) => ImageView::new_default(image.clone())?,
            ImageInner::SingleLevelAttachment(image) => {
                let image = image.read().unwrap();
                if let Some(image) = image.as_ref() {
                    ImageView::new_default(image.clone())?
                } else {
                    bail!("Attachment image not initialized");
                }
            }
            ImageInner::Swapchain(image) => {
                let image = image.read().unwrap();
                if let Some(image) = image.as_ref() {
                    image.clone()
                } else {
                    bail!("Swapchain image not initialized");
                }
            }
        };
        Ok(view)
    }

    pub fn get_image(&self) -> Arc<Image> {
        match &self.inner {
            ImageInner::Immutable(image) => Arc::clone(image),
            ImageInner::SingleLevelAttachment(image) => {
                let image = image.read().unwrap();
                if let Some(image) = image.as_ref() {
                    Arc::clone(image)
                } else {
                    panic!("Attachment image not initialized");
                }
            }
            ImageInner::Swapchain(_image) => {
                panic!("Swapchain image can't be accessed");
            }
        }
    }

    pub fn make_attachment_description(
        &self,
        layout: ImageLayout,
        load_op: AttachmentLoadOp,
    ) -> AttachmentDescription {
        AttachmentDescription {
            format: self.vulkan_format,
            samples: SampleCount::Sample1,
            load_op,
            store_op: AttachmentStoreOp::Store,
            initial_layout: layout,
            final_layout: layout,
            ..Default::default()
        }
    }

    /// Enforce the size rule.
    pub fn enforce_size_rule(
        &self,
        context: &Arc<VulkanContext>,
        viewport_size: [f32; 2],
    ) -> Result<()> {
        // Only attachments need to be resized.
        let ImageInner::SingleLevelAttachment(attachment) = &self.inner else {
            return Ok(());
        };

        // Calculate the size of the image.
        let extent = match self.size_rule {
            ImageSizeRule::Fixed(w, h) => [w, h, 1],
            ImageSizeRule::CanvasRelative(r) => [
                (viewport_size[0] * r) as u32,
                (viewport_size[1] * r) as u32,
                1,
            ],
            ImageSizeRule::At4k(w, h) => {
                let scale = 4096.0 / viewport_size[0];
                [(w as f32 * scale) as u32, (h as f32 * scale) as u32, 1]
            }
        };

        // Check if the image is already the correct size.
        let mut attachment = attachment.write().unwrap();
        if let Some(attachment) = attachment.as_ref() {
            if attachment.extent() == extent {
                return Ok(());
            }
        }

        let usage = if self.vulkan_format == vulkano::format::Format::D32_SFLOAT {
            ImageUsage::SAMPLED | ImageUsage::DEPTH_STENCIL_ATTACHMENT
        } else {
            ImageUsage::SAMPLED
                | ImageUsage::COLOR_ATTACHMENT
                | ImageUsage::TRANSFER_DST
                | ImageUsage::TRANSFER_SRC
        };

        // Create a new image with the correct size.
        let image = Image::new(
            context.memory_allocator.clone(),
            ImageCreateInfo {
                image_type: ImageType::Dim2d,
                usage,
                format: self.vulkan_format,
                extent,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
        )?;
        *attachment = Some(image);
        *self.size.write().unwrap() = Some((extent[0], extent[1]));
        Ok(())
    }

    pub fn get_size(&self) -> Result<(u32, u32)> {
        let lock = self.size.read().unwrap();
        lock.with_context(|| format!("Image '{}' has no size", self.id))
    }

    pub fn is_swapchain(&self) -> bool {
        matches!(&self.inner, ImageInner::Swapchain(_))
    }

    pub fn set_swapchain_image(&self, image: Arc<ImageView>) {
        match &self.inner {
            ImageInner::Swapchain(swapchain_image) => {
                let size = image.image().extent();
                *self.size.write().unwrap() = Some((size[0], size[1]));
                *swapchain_image.write().unwrap() = Some(image);
            }
            _ => panic!("Not a swapchain image"),
        }
    }
}
