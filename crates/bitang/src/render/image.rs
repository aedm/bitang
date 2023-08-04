use crate::render::vulkan_window::RenderContext;
use serde::Deserialize;
use std::cell::{Cell, RefCell};
use std::sync::Arc;
use vulkano::image::{AttachmentImage, ImageAccess, ImageUsage, ImmutableImage, StorageImage};
use anyhow::Result;

#[derive(Debug, Deserialize)]
pub enum ImageFormat {
    Rgba16F,
    Depth32F,
    Rgba8U,
}

impl ImageFormat {
    pub fn vulkan_format(&self) -> vulkano::format::Format {
        match self {
            ImageFormat::Rgba16F => vulkano::format::Format::R16G16B16A16_SFLOAT,
            ImageFormat::Depth32F => vulkano::format::Format::D32_SFLOAT,
            ImageFormat::Rgba8U => vulkano::format::Format::B8G8R8A8_UNORM,
        }
    }
}

#[derive(Debug, Deserialize)]
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
    Immutable(Arc<ImmutableImage>),
    SingleLevelAttachment(RefCell<Option<Arc<AttachmentImage>>>),
    Swapchain(RefCell<Option<Arc<dyn ImageAccess>>>),
}

pub struct Image {
    pub id: String,
    pub size_rule: ImageSizeRule,
    // pub mip_levels: MipLevels,
    inner: ImageInner,
    vulkan_format: vulkano::format::Format,
    size: Cell<Option<(u32, u32)>>,
}

impl Image {
    pub fn new_immutable(id: &str, source: Arc<ImmutableImage>) -> Self {
        let dim = source.dimensions().width_height();
        Self {
            id: id.to_owned(),
            inner: ImageInner::Immutable(source),
            size_rule: ImageSizeRule::Fixed(dim[0], dim[1]),
            size: Cell::new(Some((dim[0], dim[1]))),
            vulkan_format: source.format(),
            // mip_levels: MipLevels::Fixed(source.mip_levels()),
        }
    }

    pub fn new_attachment(id: &str, format: ImageFormat, size_rule: ImageSizeRule) -> Self {
        Self {
            id: id.to_owned(),
            inner: ImageInner::SingleLevelAttachment(RefCell::new(None)),
            size_rule,
            size: Cell::new(None),
            vulkan_format: format.vulkan_format(),
        }
    }

    pub fn new_swapchain(id: &str, format: ImageFormat) -> Self {
        Self {
            id: id.to_owned(),
            inner: ImageInner::Swapchain(RefCell::new(None)),
            size_rule: ImageSizeRule::CanvasRelative(1.0),
            size: Cell::new(None),
            vulkan_format: format.vulkan_format(),
        }
    }

    /// Enforce the size rule.
    pub fn enforce_size_rule(&self, context: &RenderContext) -> Result<()> {
        // Only attachments need to be resized.
        let ImageInner::SingleLevelAttachment(attachment) = &self.inner else {
            return Ok(());
        };
        
        // Calculate the size of the image.
        let size = match self.size_rule {
            ImageSizeRule::Fixed(w, h) => [w, h],
            ImageSizeRule::CanvasRelative(r) => {
                let [w, h] = context.screen_viewport.dimensions;
                [(w * r) as u32, (h * r) as u32]
            }
            ImageSizeRule::At4k(w, h) => {
                let [sw, sh] = context.screen_viewport.dimensions;
                let scale = 4096.0 / sw;
                [(w as f32 * scale) as u32, (h as f32 * scale) as u32]
            }
        };
        
        // Check if the image is already the correct size.
        let mut attachment = attachment.borrow_mut();
        if let Some(attachment) = &*attachment {
            if attachment.dimensions().width_height() == size {
                return Ok(());
            }
        }

        // Create a new image with the correct size.
        let image = AttachmentImage::with_usage(
            context.vulkan_context.context.memory_allocator(),
            size,
            self.vulkan_format,
            ImageUsage::SAMPLED | ImageUsage::TRANSFER_DST,
        )?;
        *attachment = Some(image);
        Ok(())
    }

    // pub fn get_view(&self) -> ImageView {
    //     match &self.inner {
    //         ImageInner::Immutable(image) => ImageView::new(image.clone()).unwrap(),
    //         ImageInner::SingleLevelAttachment(image) => {
    //             ImageView::new(image.clone()).unwrap()
    //         }
    //     }
    // }
}

