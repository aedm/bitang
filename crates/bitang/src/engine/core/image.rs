use std::sync::{Arc, RwLock};

use anyhow::{bail, Result};
use serde::Deserialize;
use wgpu::Extent3d;

use super::context::{FrameContext, GpuContext};
use super::mipmap_generator::MipmapGenerator;
use super::Size2D;

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum PixelFormat {
    Rgba16F,
    Rgba32F,
    Depth32F,
    Rgba8U,
    Rgba8Srgb,

    // Intel only apparently supports this surface format, no RGBA_SRGB.
    Bgra8Srgb,
    Bgra8Unorm,
}

impl PixelFormat {
    pub fn wgpu_format(&self) -> wgpu::TextureFormat {
        match self {
            PixelFormat::Rgba16F => wgpu::TextureFormat::Rgba16Float,
            PixelFormat::Rgba32F => wgpu::TextureFormat::Rgba32Float,
            PixelFormat::Depth32F => wgpu::TextureFormat::Depth32Float,
            PixelFormat::Rgba8U => wgpu::TextureFormat::Rgba8Unorm,
            PixelFormat::Rgba8Srgb => wgpu::TextureFormat::Rgba8UnormSrgb,
            PixelFormat::Bgra8Srgb => wgpu::TextureFormat::Bgra8UnormSrgb,
            PixelFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
        }
    }

    pub fn from_wgpu_format(format: wgpu::TextureFormat) -> Result<PixelFormat> {
        match format {
            wgpu::TextureFormat::Rgba16Float => Ok(PixelFormat::Rgba16F),
            wgpu::TextureFormat::Rgba32Float => Ok(PixelFormat::Rgba32F),
            wgpu::TextureFormat::Depth32Float => Ok(PixelFormat::Depth32F),
            wgpu::TextureFormat::Rgba8Unorm => Ok(PixelFormat::Rgba8U),
            wgpu::TextureFormat::Rgba8UnormSrgb => Ok(PixelFormat::Rgba8Srgb),
            wgpu::TextureFormat::Bgra8UnormSrgb => Ok(PixelFormat::Bgra8Srgb),
            wgpu::TextureFormat::Bgra8Unorm => Ok(PixelFormat::Bgra8Unorm),
            _ => bail!("Unsupported format: {:?}", format),
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
}

pub struct SwapchainImage {
    pub texture_view: wgpu::TextureView,
    pub size: Size2D,
}

enum ImageInner {
    // Immutable image that is loaded from an external source, e.g. a jpg file.
    Immutable(wgpu::Texture),

    // Attachment image used both as a render target and a sampler source.
    Attachment(RwLock<AttachmentImage>),

    // The final render target. In windowed mode, this is displayed on the screen
    Swapchain(RwLock<Option<SwapchainImage>>),
}

pub struct BitangImage {
    pub id: String,
    pub pixel_format: PixelFormat,
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
        let extent = wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        };
        let mip_level_count = extent.max_mips(wgpu::TextureDimension::D2);
        let texture_descriptor = wgpu::TextureDescriptor {
            label: Some(id),
            size: extent,
            mip_level_count,
            sample_count: 1,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: pixel_format.wgpu_format(),
            dimension: wgpu::TextureDimension::D2,
            view_formats: &[],
        };
        let texture = context.device.create_texture(&texture_descriptor);

        let bytes_per_pixel = pixel_format.wgpu_format().block_copy_size(None).unwrap_or(4);
        let bytes_per_row = bytes_per_pixel * size[0];

        context.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(size[1]),
            },
            extent,
        );

        let image = Arc::new(Self {
            id: id.to_owned(),
            pixel_format,
            inner: ImageInner::Immutable(texture),
            has_mipmaps: true,
        });

        if mip_level_count > 1 {
            let mut command_encoder =
                context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            MipmapGenerator::new(&context.device, image.clone())
                .generate(&mut command_encoder, &context.device)?;
            context.queue.submit(Some(command_encoder.finish()));
        }

        Ok(image)
    }

    // Returns an image view that only has one mip level.
    pub fn view_as_render_target(&self) -> Result<wgpu::TextureView> {
        let view: wgpu::TextureView = match &self.inner {
            ImageInner::Immutable(_) => {
                bail!("Immutable image can't be used as a render target");
            }
            ImageInner::Attachment(attachment) => {
                let attachment = attachment.read().unwrap();
                let Some(texture) = &attachment.texture else {
                    bail!("Attachment image not initialized");
                };
                texture.create_view(&wgpu::TextureViewDescriptor {
                    usage: Some(wgpu::TextureUsages::RENDER_ATTACHMENT),
                    base_mip_level: 0,
                    mip_level_count: Some(1),
                    ..wgpu::TextureViewDescriptor::default()
                })
            }
            ImageInner::Swapchain(texture_view) => {
                let texture_view = texture_view.read().unwrap();
                let Some(texture_view) = &*texture_view else {
                    bail!("Swapchain image not initialized");
                };
                texture_view.texture_view.clone()
            }
        };
        Ok(view)
    }

    // Returns an image view for sampling purposes. It includes all mip levels.
    pub fn view_as_sampler(&self) -> Result<wgpu::TextureView> {
        let view: wgpu::TextureView = match &self.inner {
            ImageInner::Immutable(texture) => texture.create_view(&wgpu::TextureViewDescriptor {
                usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
                base_mip_level: 0,
                mip_level_count: Some(texture.mip_level_count()),
                ..wgpu::TextureViewDescriptor::default()
            }),
            ImageInner::Attachment(attachment) => {
                let attachment = attachment.read().unwrap();
                let Some(texture) = &attachment.texture else {
                    bail!("Attachment image not initialized");
                };
                // texture.create_view(&wgpu::TextureViewDescriptor::default())
                texture.create_view(&wgpu::TextureViewDescriptor {
                    usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
                    base_mip_level: 0,
                    mip_level_count: Some(texture.mip_level_count()),
                    ..wgpu::TextureViewDescriptor::default()
                })
            }
            ImageInner::Swapchain(_) => {
                bail!("Swapchain image can't be used in a sampler");
            }
        };
        Ok(view)
    }

    pub fn view_mip_level(&self, mip_level: u32) -> Result<wgpu::TextureView> {
        let view_descriptor = wgpu::TextureViewDescriptor {
            label: Some("mip"),
            format: None,
            dimension: None,
            usage: None,
            aspect: wgpu::TextureAspect::All,
            base_mip_level: mip_level,
            mip_level_count: Some(1),
            base_array_layer: 0,
            array_layer_count: None,
        };
        let view = match &self.inner {
            ImageInner::Immutable(texture) => texture.create_view(&view_descriptor),
            ImageInner::Attachment(attachment) => {
                let attachment = attachment.read().unwrap();
                let Some(texture) = &attachment.texture else {
                    bail!("Attachment image not initialized");
                };
                texture.create_view(&view_descriptor)
            }
            ImageInner::Swapchain(_) => {
                bail!("Swapchain image can't be used in a sampler");
            }
        };
        Ok(view)
    }

    /// Enforce the size rule.
    pub fn enforce_size_rule(&self, context: &GpuContext, canvas_size: &Size2D) -> Result<()> {
        // Only attachments need to be resized.
        let ImageInner::Attachment(attachment) = &self.inner else {
            return Ok(());
        };
        let mut attachment = attachment.write().unwrap();

        // Calculate the size of the image.
        let size = match attachment.size_rule {
            ImageSizeRule::Fixed(w, h) => [w, h],
            ImageSizeRule::CanvasRelative(r) => [
                ((canvas_size[0] as f32 * r) as u32).max(1),
                ((canvas_size[1] as f32 * r) as u32).max(1),
            ],
            ImageSizeRule::At4k(w, h) => {
                let scale = 3840.0 / canvas_size[0] as f32;
                [
                    ((w as f32 * scale) as u32).max(1),
                    ((h as f32 * scale) as u32).max(1),
                ]
            }
        };

        let extent = Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        };
        // Check if the image is already the correct size.
        if let Some(texture) = &attachment.texture {
            if texture.size() == extent {
                return Ok(());
            }
        };

        // Create a new image with the correct size.
        let mip_levels =
            if self.has_mipmaps { extent.max_mips(wgpu::TextureDimension::D2) } else { 1 };
        let usage = wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC;
        let image = context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&self.id),
            size: extent,
            mip_level_count: mip_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.pixel_format.wgpu_format(),
            usage,
            view_formats: &[],
        });

        attachment.texture = Some(image);
        Ok(())
    }

    pub fn get_size(&self) -> Result<Size2D> {
        match &self.inner {
            ImageInner::Attachment(attachment) => {
                let attachment = attachment.read().unwrap();
                let Some(texture) = &attachment.texture else {
                    bail!("Image '{}' has no texture", self.id);
                };
                let size = texture.size();
                Ok([size.width, size.height])
            }
            ImageInner::Immutable(texture) => {
                let size = texture.size();
                Ok([size.width, size.height])
            }
            ImageInner::Swapchain(swapchain) => {
                let swapchain = swapchain.read().unwrap();
                let Some(swapchain) = swapchain.as_ref() else {
                    bail!("Swapchain image not initialized");
                };
                Ok(swapchain.size)
            }
        }
    }

    pub fn is_swapchain(&self) -> bool {
        matches!(&self.inner, ImageInner::Swapchain(_))
    }

    pub fn set_swapchain_image_view(&self, view: Option<SwapchainImage>) {
        match &self.inner {
            ImageInner::Swapchain(rw_lock) => {
                let mut swapchain_view = rw_lock.write().unwrap();
                *swapchain_view = view;
            }
            _ => panic!("Not a swapchain image"),
        }
    }

    pub fn mip_levels(&self) -> Result<u32> {
        match &self.inner {
            ImageInner::Attachment(attachment) => {
                let attachment = attachment.read().unwrap();
                let Some(texture) = &attachment.texture else {
                    bail!("Image '{}' has no texture", self.id);
                };
                Ok(texture.mip_level_count())
            }
            ImageInner::Immutable(texture) => Ok(texture.mip_level_count()),
            ImageInner::Swapchain(_) => {
                bail!("Swapchain image has no mip levels");
            }
        }
    }

    pub fn copy_attachment_to_buffer(
        &self,
        context: &mut FrameContext,
        buffer: &wgpu::Buffer,
    ) -> Result<()> {
        let ImageInner::Attachment(attachment) = &self.inner else {
            bail!("Only attachment images can be copied to a buffer, and image '{}' is not an attachment texture.", self.id);
        };
        let attachment = attachment.read().unwrap();
        let Some(texture) = &attachment.texture else {
            bail!("Image '{}' has no texture, cannot copy to buffer.", self.id);
        };
        let extent = texture.size();
        context.command_encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(extent.width * 4),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: extent.width,
                height: extent.height,
                depth_or_array_layers: 1,
            },
        );
        Ok(())
    }
}
