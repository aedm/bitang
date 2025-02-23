use crate::render::image::BitangImage;
use crate::tool::{FrameContext, RenderPassContext, Viewport, WindowContext};
use anyhow::{bail, ensure, Result};
use smallvec::SmallVec;
use std::sync::Arc;

// use vulkano::command_buffer::RenderPassBeginInfo;
// use vulkano::image::ImageLayout;
// use vulkano::pipeline::graphics::viewport::Viewport;
// use vulkano::render_pass::{
//     AttachmentDescription, AttachmentLoadOp, AttachmentReference, Framebuffer,
//     FramebufferCreateInfo, RenderPassCreateInfo, SubpassDescription,
// };

pub struct Pass {
    pub id: String,
    pub color_buffers: Vec<Arc<BitangImage>>,
    pub depth_buffer: Option<Arc<BitangImage>>,
    pub clear_color: Option<[f32; 4]>,
    // pub vulkan_render_pass: Arc<vulkano::render_pass::RenderPass>,
}

impl Pass {
    pub fn new(
        id: &str,
        context: &Arc<WindowContext>,
        color_buffers: Vec<Arc<BitangImage>>,
        depth_buffer: Option<Arc<BitangImage>>,
        clear_color: Option<[f32; 4]>,
    ) -> Result<Self> {
        // let vulkan_render_pass = Self::make_vulkan_render_pass(
        //     context,
        //     &color_buffers,
        //     &depth_buffer,
        //     clear_color.is_some(),
        // )?;
        Ok(Pass {
            id: id.to_string(),
            depth_buffer,
            color_buffers,
            clear_color,
            // vulkan_render_pass,
        })
    }

    // fn make_vulkan_render_pass(
    //     context: &Arc<WindowContext>,
    //     color_buffers: &[Arc<BitangImage>],
    //     depth_buffer: &Option<Arc<BitangImage>>,
    //     clear_buffers: bool,
    // ) -> Result<Arc<vulkano::render_pass::RenderPass>> {
    //     let mut attachments = vec![];
    //     let load_op = if clear_buffers { AttachmentLoadOp::Clear } else { AttachmentLoadOp::Load };

    //     let color_attachments = color_buffers
    //         .iter()
    //         .map(|selector| {
    //             Some(Self::make_attachment_reference(
    //                 selector,
    //                 &mut attachments,
    //                 ImageLayout::ColorAttachmentOptimal,
    //                 load_op,
    //             ))
    //         })
    //         .collect::<Vec<_>>();
    //     let depth_stencil_attachment = depth_buffer.as_ref().map(|selector| {
    //         Self::make_attachment_reference(
    //             selector,
    //             &mut attachments,
    //             ImageLayout::DepthStencilAttachmentOptimal,
    //             load_op,
    //         )
    //     });

    //     let subpasses = vec![SubpassDescription {
    //         color_attachments,
    //         depth_stencil_attachment,
    //         ..Default::default()
    //     }];

    //     let create_info = RenderPassCreateInfo {
    //         attachments,
    //         subpasses,
    //         ..Default::default()
    //     };
    //     let render_pass =
    //         vulkano::render_pass::RenderPass::new(context.device.clone(), create_info)?;
    //     Ok(render_pass)
    // }

    fn make_render_pass_context<'pass, 'frame>(
        &'pass self,
        frame_context: &'pass mut FrameContext,
    ) -> Result<RenderPassContext<'pass>> {
        // Collect attachment texture views
        let color_attachment_views: SmallVec<[_; 64]> = self
            .color_buffers
            .iter()
            .map(|image| image.get_view_for_render_target())
            .collect::<Result<_>>()?;

        let depth_buffer_view = self
            .depth_buffer
            .as_ref()
            .map(|depth_image| depth_image.get_view_for_render_target())
            .transpose()?;

        // Collect attachments
        let mut color_attachments = SmallVec::<[_; 64]>::new();
        for i in 0..color_attachment_views.len() {
            color_attachments.push(Some(self.make_color_attachment(&color_attachment_views[i])));
        }
        let depth_stencil_attachment = depth_buffer_view
            .as_ref()
            .map(|view| self.make_depth_attachment(view));

        let pass =
            frame_context
                .command_encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    // TODO: label
                    label: None,
                    color_attachments: &color_attachments,
                    depth_stencil_attachment,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

        Ok(RenderPassContext {
            gpu_context: &frame_context.gpu_context,
            pass,
            globals: &frame_context.globals,
        })
    }

    fn make_color_attachment<'a>(
        &self,
        texture_view: &'a wgpu::TextureView,
    ) -> wgpu::RenderPassColorAttachment<'a> {
        let load = match &self.clear_color {
            Some(clear_color) => wgpu::LoadOp::Clear(wgpu::Color {
                r: clear_color[0] as f64,
                g: clear_color[1] as f64,
                b: clear_color[2] as f64,
                a: clear_color[3] as f64,
            }),
            None => wgpu::LoadOp::Load,
        };

        wgpu::RenderPassColorAttachment {
            view: texture_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        }
    }

    fn make_depth_attachment<'a>(
        &self,
        texture_view: &'a wgpu::TextureView,
    ) -> wgpu::RenderPassDepthStencilAttachment<'a> {
        let load = match &self.clear_color {
            Some(_) => wgpu::LoadOp::Clear(1.0),
            None => wgpu::LoadOp::Load,
        };

        wgpu::RenderPassDepthStencilAttachment {
            view: texture_view,
            depth_ops: Some(wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }
    }

    // fn make_attachment_reference(
    //     image: &Arc<BitangImage>,
    //     attachments: &mut Vec<AttachmentDescription>,
    //     layout: ImageLayout,
    //     load_op: AttachmentLoadOp,
    // ) -> AttachmentReference {
    //     let reference = AttachmentReference {
    //         attachment: attachments.len() as u32,
    //         layout,
    //         ..Default::default()
    //     };

    //     let attachment_description = image.make_attachment_description(layout, load_op);
    //     // let attachment_description = match selector {
    //     //     ImageSelector::Image(image) => image.make_attachment_description(layout, load_op),
    //     // };

    //     attachments.push(attachment_description);
    //     reference
    // }

    pub fn get_viewport(&self, context: &mut FrameContext) -> Result<Viewport> {
        let first_image = if let Some(img) = self.color_buffers.first() {
            img
        } else if let Some(img) = &self.depth_buffer {
            img
        } else {
            bail!("Pass {} has no color or depth buffers", self.id);
        };

        // Check that all render targets have the same size
        let size = first_image.get_size()?;
        for image in &self.color_buffers {
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
                x: 0,
                y: 0,
                width: size.0,
                height: size.1,
            }
        };
        Ok(viewport)
    }

    // pub fn make_render_pass_descriptor(&self) -> Result<wgpu::RenderPassDescriptor> {
    //     // Collect color attachment images...
    //     let mut attachments = vec![];
    //     let mut clear_values = vec![];
    //     for image in &self.color_buffers {
    //         attachments.push(image.get_view_for_render_target()?);
    //         clear_values.push(self.clear_color.map(|c| c.into()));
    //     }

    //     // ...and the depth attachment image
    //     if let Some(depth) = &self.depth_buffer {
    //         attachments.push(depth.get_view_for_render_target()?);
    //         clear_values.push(self.clear_color.map(|_| 1f32.into()));
    //     }

    //     // Create the framebuffer
    //     let framebuffer = Framebuffer::new(
    //         self.vulkan_render_pass.clone(),
    //         FramebufferCreateInfo {
    //             attachments,
    //             ..Default::default()
    //         },
    //     )?;

    //     Ok(RenderPassBeginInfo {
    //         clear_values,
    //         ..RenderPassBeginInfo::framebuffer(framebuffer)
    //     })
    // }
}
