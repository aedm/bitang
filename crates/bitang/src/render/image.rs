// use crate::render::generate_mip_levels::generate_mip_levels;
use crate::tool::{GpuContext, Viewport};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::sync::{Arc, RwLock};
use tracing::warn;
use wgpu::{util::DeviceExt, Extent3d};

use super::Size2D;

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

// TODO: add Size2D type.
#[derive(Debug, Deserialize, Clone, Copy)]
pub enum ImageSizeRule {
    Fixed(u32, u32),
    CanvasRelative(f32),
    At4k(u32, u32),
}

struct AttachmentImage {
    pub size_rule: ImageSizeRule,

    // The underlying texture..
    texture: Option<wgpu::Texture>,

    // As a render target.
    render_target_view: Option<wgpu::TextureView>,
}

enum ImageInner {
    // Immutable image that is loaded from an external source, e.g. a jpg file.
    Immutable(wgpu::Texture),

    // Attachment image used both as a render target and a sampler source.
    Attachment(RwLock<AttachmentImage>),

    // The final render target. In windowed mode, this is displayed on the screen
    Swapchain(RwLock<Option<wgpu::TextureView>>),
}

pub struct BitangImage {
    pub id: String,
    // pub vulkan_format: vulkano::format::Format,
    // TODO: use Pixelformat
    // pub wgpu_format: wgpu::TextureFormat,
    pub pixel_format: PixelFormat,
    // pub mip_levels: MipLevels,
    inner: ImageInner,
    has_mipmaps: bool,
}

impl BitangImage {
    pub fn new_attachment(
        id: &str,
        pixel_format: PixelFormat,
        size_rule: ImageSizeRule,
        has_mipmaps: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            id: id.to_owned(),
            inner: ImageInner::Attachment(RwLock::new(AttachmentImage {
                size_rule,
                texture: None,
                render_target_view: None,
            })),
            pixel_format,
            has_mipmaps,
        })
    }

    pub fn new_swapchain(id: &str, pixel_format: PixelFormat) -> Arc<Self> {
        Arc::new(Self {
            id: id.to_owned(),
            inner: ImageInner::Swapchain(RwLock::new(None)),
            pixel_format,
            has_mipmaps: false,
        })
    }

    pub fn immutable_from_pixel_data(
        id: &str,
        context: &GpuContext,
        pixel_format: PixelFormat,
        size: Size2D,
        data: &[u8],
    ) -> Result<Arc<Self>> {
        let texture_descriptor = wgpu::TextureDescriptor {
            label: Some(id),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            format: pixel_format.wgpu_format(),
            dimension: wgpu::TextureDimension::D2,
            view_formats: &[],
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
            pixel_format,
            inner: ImageInner::Immutable(image),
            has_mipmaps: false,
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
            ImageInner::Attachment(attachment) => {
                let attachment = attachment.read().unwrap();
                let Some(texture) = &attachment.texture else {
                    bail!("Attachment image not initialized");
                };
                texture.create_view(&wgpu::TextureViewDescriptor::default())
            }
            ImageInner::Swapchain(texture_view) => {
                let texture_view = texture_view.read().unwrap();
                let Some(texture_view) = &*texture_view else {
                    bail!("Swapchain image not initialized");
                };
                texture_view.clone()

            }
        };
        Ok(view)
    }

    // Returns an image view for sampling purposes. It includes all mip levels.
    pub fn make_texture_view_for_sampler(&self) -> Result<wgpu::TextureView> {
        let view: wgpu::TextureView = match &self.inner {
            ImageInner::Immutable(texture) => {
                texture.create_view(&wgpu::TextureViewDescriptor::default())
            }
            ImageInner::Attachment(attachment) => {
                let attachment = attachment.read().unwrap();
                let Some(texture) = &attachment.texture else {
                    bail!("Attachment image not initialized");
                };
                texture.create_view(&wgpu::TextureViewDescriptor::default())
            }
            ImageInner::Swapchain(_) => {
                bail!("Swapchain image can't be used in a sampler");
            }
        };
        Ok(view)
    }

    // pub fn get_image(&self) -> Arc<Image> {
    //     match &self.inner {
    //         ImageInner::Immutable(image) => Arc::clone(image),
    //         ImageInner::Attachment(image) => {
    //             let image = image.read().unwrap();
    //             if let Some(image) = image.as_ref() {
    //                 Arc::clone(image)
    //             } else {
    //                 panic!("Attachment image not initialized");
    //             }
    //         }
    //         ImageInner::Swapchain(_image) => {
    //             panic!("Swapchain image can't be accessed");
    //         }
    //     }
    // }

    // pub fn make_attachment_description(
    //     &self,
    //     layout: ImageLayout,
    //     load_op: AttachmentLoadOp,
    // ) -> AttachmentDescription {
    //     AttachmentDescription {
    //         format: self.vulkan_format,
    //         samples: SampleCount::Sample1,
    //         load_op,
    //         store_op: AttachmentStoreOp::Store,
    //         initial_layout: layout,
    //         final_layout: layout,
    //         ..Default::default()
    //     }
    // }

    /// Enforce the size rule.
    pub fn enforce_size_rule(&self, context: &GpuContext, viewport: &Viewport) -> Result<()> {
        // Only attachments need to be resized.
        let ImageInner::Attachment(attachment) = &self.inner else {
            return Ok(());
        };
        let mut attachment = attachment.write().unwrap();

        // Calculate the size of the image.
        let size = match attachment.size_rule {
            ImageSizeRule::Fixed(w, h) => [w, h],
            ImageSizeRule::CanvasRelative(r) => [
                (viewport.size[0] as f32 * r) as u32,
                (viewport.size[1] as f32 * r) as u32,
            ],
            ImageSizeRule::At4k(w, h) => {
                // TODO: 4k is not 4096.
                let scale = 4096.0 / viewport.size[0] as f32;
                [(w as f32 * scale) as u32, (h as f32 * scale) as u32]
            }
        };

        let extent = Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 };
        // Check if the image is already the correct size.
        if let Some(texture) = &attachment.texture {
            if texture.size() == extent {
                return Ok(());
            }
        };

        // let usage = if self.vulkan_format == vulkano::format::Format::D32_SFLOAT {
        //     ImageUsage::SAMPLED | ImageUsage::DEPTH_STENCIL_ATTACHMENT
        // } else {
        //     ImageUsage::SAMPLED
        //         | ImageUsage::COLOR_ATTACHMENT
        //         | ImageUsage::TRANSFER_DST
        //         | ImageUsage::TRANSFER_SRC
        // };

        // Create a new image with the correct size.
        let mip_levels =
            if self.has_mipmaps { extent.max_mips(wgpu::TextureDimension::D2) } else { 1 };
        // let image = Image::new(
        //     context.memory_allocator.clone(),
        //     ImageCreateInfo {
        //         image_type: ImageType::Dim2d,
        //         usage,
        //         format: self.vulkan_format,
        //         extent: size,
        //         mip_levels,
        //         ..Default::default()
        //     },
        //     AllocationCreateInfo::default(),
        // )?;

        let usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT;

        let image = context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&self.id),
            size: extent,
            mip_level_count: mip_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.pixel_format.wgpu_format(),
            usage,
            view_formats: &[
            ],
        });

        attachment.texture = Some(image);
        Ok(())
    }

    pub fn get_size(&self) -> Result<(u32, u32)> {
        match &self.inner {
            ImageInner::Attachment(attachment) => {
                let attachment = attachment.read().unwrap();
                let Some(texture) = &attachment.texture else {
                    bail!("Image '{}' has no texture", self.id);
                };
                let size = texture.size();
                Ok((size.width, size.height))
            }
            ImageInner::Immutable(texture) => {
                let size = texture.size();
                Ok((size.width, size.height))
            }
            ImageInner::Swapchain(_) => {
                bail!("Swapchain image size can't be retrieved");
            }
        }
    }

    pub fn is_swapchain(&self) -> bool {
        matches!(&self.inner, ImageInner::Swapchain(_))
    }

    pub fn set_swapchain_image_view(&self, view: wgpu::TextureView) {
        match &self.inner {
            ImageInner::Swapchain(rw_lock) => {
                let mut swapchain_view = rw_lock.write().unwrap();
                *swapchain_view = Some(view);
            }
            _ => panic!("Not a swapchain image"),
        }
    }
}
