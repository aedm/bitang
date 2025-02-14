// use crate::render::generate_mip_levels::generate_mip_levels;
use crate::tool::{GpuContext, WindowContext};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use tracing::warn;
use wgpu::util::DeviceExt;
use std::sync::{Arc, RwLock};
// use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage};
// use vulkano::command_buffer::{
//     AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferToImageInfo,
//     PrimaryCommandBufferAbstract,
// };
// use vulkano::image::view::{ImageView, ImageViewCreateInfo};
// use vulkano::image::{
//     max_mip_levels, Image, ImageCreateInfo, ImageLayout, ImageSubresourceRange, ImageTiling,
//     ImageType, ImageUsage, SampleCount,
// };
// use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
// use vulkano::render_pass::{AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp};

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
    // pub fn vulkan_format(&self) -> vulkano::format::Format {
    //     match self {
    //         PixelFormat::Rgba16F => vulkano::format::Format::R16G16B16A16_SFLOAT,
    //         PixelFormat::Rgba32F => vulkano::format::Format::R32G32B32A32_SFLOAT,
    //         PixelFormat::Depth32F => vulkano::format::Format::D32_SFLOAT,
    //         PixelFormat::Rgba8U => vulkano::format::Format::R8G8B8A8_UNORM,
    //         PixelFormat::Rgba8Srgb => vulkano::format::Format::R8G8B8A8_SRGB,
    //         PixelFormat::Bgra8Srgb => vulkano::format::Format::B8G8R8A8_SRGB,
    //     }
    // }

    pub fn wgpu_format(&self) -> wgpu::TextureFormat {
        match self {
            PixelFormat::Rgba16F => wgpu::TextureFormat::Rgba16Float,
            PixelFormat::Rgba32F => wgpu::TextureFormat::Rgba32Float,
            PixelFormat::Depth32F => wgpu::TextureFormat::Depth32Float,
            PixelFormat::Rgba8U => wgpu::TextureFormat::Rgba8Unorm,
            PixelFormat::Rgba8Srgb => wgpu::TextureFormat::Rgba8UnormSrgb,
            PixelFormat::Bgra8Srgb => wgpu::TextureFormat::Bgra8UnormSrgb,
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
    // Immutable image that is loaded from an external source, e.g. a jpg file.
    Immutable(Arc<wgpu::Texture>),

    // Attachment image used both as a render target and a sampler source.
    // TODO: might not need Arc or Rc
    Attachment(RwLock<Option<Arc<wgpu::Texture>>>),

    // The final render target. In windowed mode, this is displayed on the screen
    Swapchain(RwLock<Option<wgpu::TextureView>>),
}

pub struct BitangImage {
    pub id: String,
    pub size_rule: ImageSizeRule,
    // pub vulkan_format: vulkano::format::Format,
    // TODO: use Pixelformat
    pub wgpu_format: wgpu::TextureFormat,
    // pub mip_levels: MipLevels,
    inner: ImageInner,
    size: RwLock<Option<(u32, u32)>>,
    has_mipmaps: bool,
}

impl BitangImage {
    pub fn new_attachment(
        id: &str,
        format: PixelFormat,
        size_rule: ImageSizeRule,
        has_mipmaps: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            id: id.to_owned(),
            inner: ImageInner::Attachment(RwLock::new(None)),
            size_rule,
            size: RwLock::new(None),
            wgpu_format: format.wgpu_format(),
            has_mipmaps,
        })
    }

    pub fn new_swapchain(id: &str, format: PixelFormat) -> Arc<Self> {
        Arc::new(Self {
            id: id.to_owned(),
            inner: ImageInner::Swapchain(RwLock::new(None)),
            size_rule: ImageSizeRule::CanvasRelative(1.0),
            size: RwLock::new(None),
            wgpu_format: format.wgpu_format(),
            has_mipmaps: false,
        })
    }

    pub fn immutable_from_srgb(
        id: &str,
        context: &GpuContext,
        format: PixelFormat,
        dimensions: [u32; 3],
        data: &[u8]) -> Result<Arc<Self>> {

            let texture_descriptor = wgpu::TextureDescriptor {
                label: Some(id),
                size: wgpu::Extent3d {
                    width: dimensions[0],
                    height: dimensions[1],
                    depth_or_array_layers: dimensions[2],
                },
                mip_level_count: 1,
                sample_count: 1,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST,
                format: format.wgpu_format(),
                dimension: wgpu::TextureDimension::D2,
                view_formats: &[PixelFormat::Rgba8Srgb.wgpu_format(), PixelFormat::Rgba8U.wgpu_format()],
            };
            let image = context.device.create_texture_with_data(
                &context.queue,
                &texture_descriptor,
                wgpu::util::TextureDataOrder::default(),
                data,
            );

            warn!("immutable_from_srgb(): no mipmaps");

            Ok(Arc::new(Self {
                id: id.to_owned(),
                wgpu_format: image.format(),
                inner: ImageInner::Immutable(Arc::new(image)),
                size_rule: ImageSizeRule::Fixed(dimensions[0], dimensions[1]),
                size: RwLock::new(Some((dimensions[0], dimensions[1]))),
                has_mipmaps: true,
            }))
        }

    // pub fn immutable_from_iter<I, T>(
    //     id: &str,
    //     context: &Arc<WindowContext>,
    //     format: PixelFormat,
    //     dimensions: [u32; 3],
    //     pixel_data: I,
    // ) -> Result<Arc<Self>>
    // where
    //     T: BufferContents,
    //     I: IntoIterator<Item = T>,
    //     I::IntoIter: ExactSizeIterator,
    // {
    //     let mut cbb = AutoCommandBufferBuilder::primary(
    //         &context.command_buffer_allocator,
    //         context.gfx_queue.queue_family_index(),
    //         CommandBufferUsage::OneTimeSubmit,
    //     )?;

    //     let mip_levels = max_mip_levels(dimensions);

    //     let image = Image::new(
    //         context.memory_allocator.clone(),
    //         vulkano::image::ImageCreateInfo {
    //             image_type: ImageType::Dim2d,
    //             format: format.vulkan_format(),
    //             extent: dimensions,
    //             usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED | ImageUsage::TRANSFER_SRC,
    //             mip_levels,
    //             tiling: ImageTiling::Optimal,
    //             ..Default::default()
    //         },
    //         AllocationCreateInfo::default(),
    //     )?;

    //     let upload_buffer = Buffer::from_iter(
    //         context.memory_allocator.clone(),
    //         BufferCreateInfo {
    //             usage: BufferUsage::TRANSFER_SRC,
    //             ..Default::default()
    //         },
    //         AllocationCreateInfo {
    //             memory_type_filter: MemoryTypeFilter::PREFER_HOST
    //                 | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
    //             ..Default::default()
    //         },
    //         pixel_data,
    //     )?;

    //     cbb.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
    //         upload_buffer,
    //         image.clone(),
    //     ))?;

    //     generate_mip_levels(image.clone(), &mut cbb)?;

    //     let _fut = cbb.build()?.execute(context.gfx_queue.clone())?;

    //     Ok(Arc::new(Self {
    //         id: id.to_owned(),
    //         vulkan_format: image.format(),
    //         inner: ImageInner::Immutable(image),
    //         size_rule: ImageSizeRule::Fixed(dimensions[0], dimensions[1]),
    //         size: RwLock::new(Some((dimensions[0], dimensions[1]))),
    //         has_mipmaps: true,
    //     }))
    // }

    // Returns an image view that only has one mip level.
    pub fn get_view_for_render_target(&self) -> Result<wgpu::TextureView> {
        let view: wgpu::TextureView = match &self.inner {
            ImageInner::Immutable(_) => {
                bail!("Immutable image can't be used as a render target");
            }
            ImageInner::Attachment(image) => {
                let image = image.read().unwrap();
                if let Some(image) = image.as_ref() {
                    image.create_view(&wgpu::TextureViewDescriptor::default())

                    // let i = ImageViewCreateInfo::from_image(&image);
                    // let vci = ImageViewCreateInfo {
                    //     subresource_range: ImageSubresourceRange {
                    //         mip_levels: 0..1,
                    //         ..i.subresource_range
                    //     },
                    //     ..i
                    // };
                    // ImageView::new(image.clone(), vci)?
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

    // Returns an image view for sampling purposes. It includes all mip levels.
    pub fn get_view_for_sampler(&self) -> Result<Arc<ImageView>> {
        let view: Arc<ImageView> = match &self.inner {
            ImageInner::Immutable(image) => ImageView::new_default(image.clone())?,
            ImageInner::Attachment(image) => {
                let image = image.read().unwrap();
                let Some(image) = image.as_ref() else {
                    bail!("Attachment image not initialized");
                };
                ImageView::new_default(image.clone())?
            }
            ImageInner::Swapchain(_) => {
                bail!("Swapchain image can't be used in a sampler");
            }
        };
        Ok(view)
    }

    pub fn get_image(&self) -> Arc<Image> {
        match &self.inner {
            ImageInner::Immutable(image) => Arc::clone(image),
            ImageInner::Attachment(image) => {
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
        context: &Arc<WindowContext>,
        viewport_size: [f32; 2],
    ) -> Result<()> {
        // Only attachments need to be resized.
        let ImageInner::Attachment(attachment) = &self.inner else {
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
        let mip_levels = if self.has_mipmaps { max_mip_levels(extent) } else { 1 };
        let image = Image::new(
            context.memory_allocator.clone(),
            ImageCreateInfo {
                image_type: ImageType::Dim2d,
                usage,
                format: self.vulkan_format,
                extent,
                mip_levels,
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
