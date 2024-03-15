use crate::tool::VulkanContext;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::sync::{Arc, RwLock};
use vulkano::image::view::ImageView;
use vulkano::image::{Image, ImageCreateInfo, ImageLayout, ImageType, ImageUsage, SampleCount};
use vulkano::memory::allocator::AllocationCreateInfo;
use vulkano::render_pass::{AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp};

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum ImageFormat {
    Rgba16F,
    Depth32F,
    Rgba8U,

    /// Only for swapchain.
    Rgba8Srgb,
}

impl ImageFormat {
    pub fn vulkan_format(&self) -> vulkano::format::Format {
        match self {
            ImageFormat::Rgba16F => vulkano::format::Format::R16G16B16A16_SFLOAT,
            ImageFormat::Depth32F => vulkano::format::Format::D32_SFLOAT,
            ImageFormat::Rgba8U => vulkano::format::Format::B8G8R8A8_UNORM,
            ImageFormat::Rgba8Srgb => vulkano::format::Format::B8G8R8A8_SRGB,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum ImageSizeRule {
    Fixed(u32, u32),
    CanvasRelative(f32),
    At4k(u32, u32),
}

// #[derive(Debug, Deserialize)]
// pub enum MipLevels {
//     Fixed(u32),
//     MinSize(u32),
// }

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

    pub fn new_attachment(id: &str, format: ImageFormat, size_rule: ImageSizeRule) -> Arc<Self> {
        Arc::new(Self {
            id: id.to_owned(),
            inner: ImageInner::SingleLevelAttachment(RwLock::new(None)),
            size_rule,
            size: RwLock::new(None),
            vulkan_format: format.vulkan_format(),
        })
    }

    pub fn new_swapchain(id: &str, format: ImageFormat) -> Arc<Self> {
        Arc::new(Self {
            id: id.to_owned(),
            inner: ImageInner::Swapchain(RwLock::new(None)),
            size_rule: ImageSizeRule::CanvasRelative(1.0),
            size: RwLock::new(None),
            vulkan_format: format.vulkan_format(),
        })
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
