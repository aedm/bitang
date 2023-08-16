use crate::render::camera::Camera;
use crate::render::image::Image;
use crate::render::vulkan_window::{RenderContext, VulkanContext};
use anyhow::{anyhow, Context, Result};
use std::sync::Arc;
use vulkano::command_buffer::{RenderPassBeginInfo, SubpassContents};
use vulkano::image::view::ImageView;
use vulkano::image::ImageViewAbstract;
use vulkano::image::{ImageLayout, SampleCount};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::render_pass::{
    AttachmentDescription, AttachmentReference, Framebuffer, FramebufferCreateInfo, LoadOp,
    RenderPassCreateInfo, StoreOp, SubpassDescription,
};

pub enum ImageSelector {
    Image(Arc<Image>),
}

impl ImageSelector {
    pub fn get_image(&self) -> &Arc<Image> {
        match self {
            ImageSelector::Image(image) => image,
        }
    }

    pub fn get_image_view(&self) -> Result<Arc<dyn ImageViewAbstract>> {
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
        context: &VulkanContext,
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
        context: &VulkanContext,
        color_buffers: &Vec<ImageSelector>,
        depth_buffer: &Option<ImageSelector>,
        clear_buffers: bool,
    ) -> Result<Arc<vulkano::render_pass::RenderPass>> {
        let mut attachments = vec![];
        let load_op = if clear_buffers { LoadOp::Clear } else { LoadOp::Load };

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

        // for (index, selector) in color_buffers.iter().enumerate() {
        //     // let layout = match selector.role {
        //     //     RenderTargetRole::Color => ImageLayout::ColorAttachmentOptimal,
        //     //     RenderTargetRole::Depth => ImageLayout::DepthStencilAttachmentOptimal,
        //     // };
        //     let attachment_reference = Some(AttachmentReference {
        //         attachment: index as u32,
        //         layout: ImageLayout::ColorAttachmentOptimal,
        //         ..Default::default()
        //     });
        //     attachments.push(AttachmentDescription {
        //         format: Some(selector.format),
        //         samples: SampleCount::Sample1, // TODO
        //         load_op,
        //         store_op: StoreOp::Store,
        //         initial_layout: ImageLayout::ColorAttachmentOptimal,
        //         final_layout: ImageLayout::ColorAttachmentOptimal,
        //         ..Default::default()
        //     });
        //     match selector.role {
        //         RenderTargetRole::Color => {
        //             color_attachments.push(attachment_reference);
        //         }
        //         RenderTargetRole::Depth => {
        //             depth_stencil_attachment = attachment_reference;
        //         }
        //     }
        // }
        // let depth_stencil_attachment = depth_buffer.map(|selector| AttachmentReference {
        //     attachment: color_attachments.len() as u32,
        //     layout: ImageLayout::DepthStencilAttachmentOptimal,
        //     ..Default::default()
        // });

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
            vulkano::render_pass::RenderPass::new(context.context.device().clone(), create_info)?;
        Ok(render_pass)
    }

    fn make_attachment_reference(
        selector: &ImageSelector,
        attachments: &mut Vec<AttachmentDescription>,
        layout: ImageLayout,
        load_op: LoadOp,
    ) -> AttachmentReference {
        let reference = AttachmentReference {
            attachment: attachments.len() as u32,
            layout,
            ..Default::default()
        };

        let attachment_description = match selector {
            ImageSelector::Image(image) => AttachmentDescription {
                format: Some(image.vulkan_format),
                samples: SampleCount::Sample1, // TODO
                load_op,
                store_op: StoreOp::Store,
                initial_layout: layout,
                final_layout: layout,
                ..Default::default()
            },
        };
        attachments.push(attachment_description);
        reference
    }

    // fn validate(&self) -> Result<()> {
    //     if self.color_buffers.is_empty() && self.depth_buffer.is_none() {
    //         return Err(anyhow!("Pass {} has no color or depth buffers", self.id));
    //     }
    //
    //     let mut size = self.depth_buffer.map(|selector| match selector {
    //         ImageSelector::Image(render_target) => render_target.size,
    //     });
    //     Ok(())
    // }

    pub fn set(&self, context: &mut RenderContext, camera: &Camera) -> Result<()> {
        let first_image = if let Some(img) = self.color_buffers.first() {
            img.get_image()
        } else if let Some(img) = &self.depth_buffer {
            img.get_image()
        } else {
            return Err(anyhow!("Pass {} has no color or depth buffers", self.id));
        };

        // Check that all render targets have the same size
        let size = first_image.get_size()?;
        for img_selector in &self.color_buffers {
            let image = img_selector.get_image();
            if image.get_size()? != size {
                return Err(anyhow!(
                    "Image '{}' in Pass '{}' has different size than other images",
                    image.id,
                    self.id
                ));
            }
        }

        let viewport = if first_image.is_swapchain() {
            context.screen_viewport.clone()
        } else {
            Viewport {
                origin: [0.0, 0.0],
                dimensions: [size.0 as f32, size.1 as f32],
                depth_range: 0.0..1.0,
            }
        };
        camera.set(&mut context.globals, viewport.dimensions);

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

        // let clear_values = self
        //     .render_targets
        //     .iter()
        //     .map(|target| match target.role {
        //         RenderTargetRole::Color => self.clear_color.map(|c| c.into()),
        //         RenderTargetRole::Depth => self.clear_color.map(|_| 1f32.into()),
        //     })
        //     .collect::<Vec<_>>();

        context
            .command_builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values,
                    ..RenderPassBeginInfo::framebuffer(framebuffer)
                },
                SubpassContents::Inline,
            )?
            .set_viewport(0, [viewport]);

        Ok(())
    }
}
