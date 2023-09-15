use crate::render::vulkan_window::VulkanContext;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::cell::{Cell, RefCell};
use std::sync::Arc;
use vulkano::image::view::ImageView;
use vulkano::image::{
    AttachmentImage, ImageAccess, ImageLayout, ImageUsage, ImageViewAbstract, ImmutableImage,
    SampleCount,
};
use vulkano::render_pass::{AttachmentDescription, LoadOp, StoreOp};

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
    Immutable(Arc<ImmutableImage>),
    SingleLevelAttachment(RefCell<Option<Arc<AttachmentImage>>>),
    Swapchain(RefCell<Option<Arc<dyn ImageViewAbstract>>>),
}

pub struct Image {
    pub id: String,
    pub size_rule: ImageSizeRule,
    pub vulkan_format: vulkano::format::Format,
    // pub mip_levels: MipLevels,
    inner: ImageInner,
    size: Cell<Option<(u32, u32)>>,
}

impl Image {
    pub fn new_immutable(id: &str, source: Arc<ImmutableImage>) -> Arc<Self> {
        let dim = source.dimensions().width_height();
        Arc::new(Self {
            id: id.to_owned(),
            vulkan_format: source.format(),
            inner: ImageInner::Immutable(source),
            size_rule: ImageSizeRule::Fixed(dim[0], dim[1]),
            size: Cell::new(Some((dim[0], dim[1]))),
        })
    }

    pub fn new_attachment(id: &str, format: ImageFormat, size_rule: ImageSizeRule) -> Arc<Self> {
        Arc::new(Self {
            id: id.to_owned(),
            inner: ImageInner::SingleLevelAttachment(RefCell::new(None)),
            size_rule,
            size: Cell::new(None),
            vulkan_format: format.vulkan_format(),
        })
    }

    pub fn new_swapchain(id: &str, format: ImageFormat) -> Arc<Self> {
        Arc::new(Self {
            id: id.to_owned(),
            inner: ImageInner::Swapchain(RefCell::new(None)),
            size_rule: ImageSizeRule::CanvasRelative(1.0),
            size: Cell::new(None),
            vulkan_format: format.vulkan_format(),
        })
    }

    pub fn get_view(&self) -> Result<Arc<dyn ImageViewAbstract>> {
        let view: Arc<dyn ImageViewAbstract> = match &self.inner {
            ImageInner::Immutable(image) => ImageView::new_default(image.clone())?,
            ImageInner::SingleLevelAttachment(image) => {
                if let Some(image) = image.borrow().as_ref() {
                    ImageView::new_default(image.clone())?
                } else {
                    bail!("Attachment image not initialized");
                }
            }
            ImageInner::Swapchain(image) => {
                if let Some(image) = image.borrow().as_ref() {
                    image.clone()
                } else {
                    bail!("Swapchain image not initialized");
                }
            }
        };
        Ok(view)
    }

    pub fn get_image_access(&self) -> Arc<dyn ImageAccess> {
        match &self.inner {
            ImageInner::Immutable(image) => image.clone(),
            ImageInner::SingleLevelAttachment(image) => {
                if let Some(image) = image.borrow().as_ref() {
                    image.clone()
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
        load_op: LoadOp,
    ) -> AttachmentDescription {
        AttachmentDescription {
            format: Some(self.vulkan_format),
            samples: SampleCount::Sample1, // TODO
            load_op,
            store_op: StoreOp::Store,
            initial_layout: layout,
            final_layout: layout,
            ..Default::default()
        }
    }

    /// Enforce the size rule.
    pub fn enforce_size_rule(
        &self,
        context: &VulkanContext,
        viewport_size: [f32; 2],
    ) -> Result<()> {
        // Only attachments need to be resized.
        let ImageInner::SingleLevelAttachment(attachment) = &self.inner else {
            return Ok(());
        };

        // Calculate the size of the image.
        let size = match self.size_rule {
            ImageSizeRule::Fixed(w, h) => [w, h],
            ImageSizeRule::CanvasRelative(r) => {
                [(viewport_size[0] * r) as u32, (viewport_size[1] * r) as u32]
            }
            ImageSizeRule::At4k(w, h) => {
                let scale = 4096.0 / viewport_size[0];
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
            context.context.memory_allocator(),
            size,
            self.vulkan_format,
            ImageUsage::SAMPLED | ImageUsage::TRANSFER_DST | ImageUsage::TRANSFER_SRC,
        )?;
        *attachment = Some(image);
        self.size.set(Some((size[0], size[1])));
        Ok(())
    }

    pub fn get_size(&self) -> Result<(u32, u32)> {
        self.size
            .get()
            .with_context(|| format!("Image '{}' has no size", self.id))
    }

    pub fn is_swapchain(&self) -> bool {
        matches!(&self.inner, ImageInner::Swapchain(_))
    }

    pub fn set_swapchain_image(&self, image: Arc<dyn ImageViewAbstract>) {
        match &self.inner {
            ImageInner::Swapchain(swapchain_image) => {
                let size = image.dimensions().width_height();
                self.size.set(Some((size[0], size[1])));
                *swapchain_image.borrow_mut() = Some(image);
            }
            _ => panic!("Not a swapchain image"),
        }
    }
}
