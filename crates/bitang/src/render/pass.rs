use crate::render::image::BitangImage;
use crate::tool::{RenderContext, VulkanContext};
use anyhow::{bail, ensure, Result};
use std::sync::Arc;
use vulkano::command_buffer::RenderPassBeginInfo;
use vulkano::image::view::ImageView;
use vulkano::image::ImageLayout;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::render_pass::{
    AttachmentDescription, AttachmentLoadOp, AttachmentReference, Framebuffer,
    FramebufferCreateInfo, RenderPassCreateInfo, SubpassDescription,
};

pub enum ImageSelector {
    Image(Arc<BitangImage>),
}

impl ImageSelector {
    pub fn get_image(&self) -> &Arc<BitangImage> {
        match self {
            ImageSelector::Image(image) => image,
        }
    }

    pub fn get_image_view(&self) -> Result<Arc<ImageView>> {
        match self {
            ImageSelector::Image(image) => image.get_view(),
        }
    }
}

pub struct Pass {
    pub id: String,
    pub color_buffers: Vec<ImageSelector>,
    pub depth_buffer: Option<ImageSelector>,
    pub clear_color: Option<[f32; 4]>,
    pub vulkan_render_pass: Arc<vulkano::render_pass::RenderPass>,
}

impl Pass {
    pub fn new(
        id: &str,
        context: &Arc<VulkanContext>,
        color_buffers: Vec<ImageSelector>,
        depth_buffer: Option<ImageSelector>,
        clear_color: Option<[f32; 4]>,
    ) -> Result<Self> {
        let vulkan_render_pass = Self::make_vulkan_render_pass(
            context,
            &color_buffers,
            &depth_buffer,
            clear_color.is_some(),
        )?;
        Ok(Pass {
            id: id.to_string(),
            depth_buffer,
            color_buffers,
            clear_color,
            vulkan_render_pass,
        })
    }

    fn make_vulkan_render_pass(
        context: &Arc<VulkanContext>,
        color_buffers: &[ImageSelector],
        depth_buffer: &Option<ImageSelector>,
        clear_buffers: bool,
    ) -> Result<Arc<vulkano::render_pass::RenderPass>> {
        let mut attachments = vec![];
        let load_op = if clear_buffers { AttachmentLoadOp::Clear } else { AttachmentLoadOp::Load };

        let color_attachments = color_buffers
            .iter()
            .map(|selector| {
                Some(Self::make_attachment_reference(
                    selector,
                    &mut attachments,
                    ImageLayout::ColorAttachmentOptimal,
                    load_op,
                ))
            })
            .collect::<Vec<_>>();
        let depth_stencil_attachment = depth_buffer.as_ref().map(|selector| {
            Self::make_attachment_reference(
                selector,
                &mut attachments,
                ImageLayout::DepthStencilAttachmentOptimal,
                load_op,
            )
        });

        let subpasses = vec![SubpassDescription {
            color_attachments,
            depth_stencil_attachment,
            ..Default::default()
        }];

        let create_info = RenderPassCreateInfo {
            attachments,
            subpasses,
            ..Default::default()
        };
        let render_pass =
            vulkano::render_pass::RenderPass::new(context.device.clone(), create_info)?;
        Ok(render_pass)
    }

    fn make_attachment_reference(
        selector: &ImageSelector,
        attachments: &mut Vec<AttachmentDescription>,
        layout: ImageLayout,
        load_op: AttachmentLoadOp,
    ) -> AttachmentReference {
        let reference = AttachmentReference {
            attachment: attachments.len() as u32,
            layout,
            ..Default::default()
        };

        let attachment_description = match selector {
            ImageSelector::Image(image) => image.make_attachment_description(layout, load_op),
        };

        attachments.push(attachment_description);
        reference
    }

    pub fn get_viewport(&self, context: &mut RenderContext) -> Result<Viewport> {
        let first_image = if let Some(img) = self.color_buffers.first() {
            img.get_image()
        } else if let Some(img) = &self.depth_buffer {
            img.get_image()
        } else {
            bail!("Pass {} has no color or depth buffers", self.id);
        };

        // Check that all render targets have the same size
        let size = first_image.get_size()?;
        for img_selector in &self.color_buffers {
            let image = img_selector.get_image();
            ensure!(
                image.get_size()? == size,
                "Image '{}' in Pass '{}' has different size than other images",
                image.id,
                self.id
            );
        }

        let viewport = if first_image.is_swapchain() {
            context.screen_viewport.clone()
        } else {
            Viewport {
                offset: [0.0, 0.0],
                extent: [size.0 as f32, size.1 as f32],
                depth_range: 0.0..=1.0,
            }
        };
        Ok(viewport)
    }

    pub fn make_render_pass_begin_info(
        &self,
        _context: &mut RenderContext,
    ) -> Result<RenderPassBeginInfo> {
        // Collect color attachment images...
        let mut attachments = vec![];
        let mut clear_values = vec![];
        for img_selector in &self.color_buffers {
            let image = img_selector.get_image();
            attachments.push(image.get_view()?);
            clear_values.push(self.clear_color.map(|c| c.into()));
        }

        // ...and the depth attachment image
        if let Some(depth) = &self.depth_buffer {
            attachments.push(depth.get_image_view()?);
            clear_values.push(self.clear_color.map(|_| 1f32.into()));
        }

        // Create the framebuffer
        let framebuffer = Framebuffer::new(
            self.vulkan_render_pass.clone(),
            FramebufferCreateInfo {
                attachments,
                ..Default::default()
            },
        )?;

        Ok(RenderPassBeginInfo {
            clear_values,
            ..RenderPassBeginInfo::framebuffer(framebuffer)
        })
    }
}
