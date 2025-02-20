use crate::render::image::BitangImage;
use crate::tool::{FrameContext, RenderPassContext, WindowContext};
use anyhow::{bail, ensure, Result};
use std::sync::Arc;

use super::Viewport;
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

    fn make_render_pass_context(
        &self,
        frame_context: &mut FrameContext,
    ) -> Result<RenderPassContext> {
 

        let color_attachments = self
            .color_buffers
            .iter()
            .map(|selector| {
                let texture_view = selector.get_view_for_render_target()?;
                Some(self.make_color_attachment(&texture_view))
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

        let mut render_pass =
            frame_context
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    // TODO: label
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

        Ok(RenderPassContext {
            gpu_context: &frame_context.gpu_context,
            pass: self,
            globals: &frame_context.globals,
        })
    }

    fn make_color_attachment(
        &self,
        texture_view: &wgpu::TextureView,
    ) -> wgpu::RenderPassColorAttachment {
        let load = if let Some(clear_color) = &self.clear_color {
            wgpu::LoadOp::Clear(wgpu::Color {
                r: clear_color[0] as f64,
                g: clear_color[1] as f64,
                b: clear_color[2] as f64,
                a: clear_color[3] as f64,
            })
        } else {
            wgpu::LoadOp::Load
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

    fn make_attachment_reference(
        image: &Arc<BitangImage>,
        attachments: &mut Vec<AttachmentDescription>,
        layout: ImageLayout,
        load_op: AttachmentLoadOp,
    ) -> AttachmentReference {
        let reference = AttachmentReference {
            attachment: attachments.len() as u32,
            layout,
            ..Default::default()
        };

        let attachment_description = image.make_attachment_description(layout, load_op);
        // let attachment_description = match selector {
        //     ImageSelector::Image(image) => image.make_attachment_description(layout, load_op),
        // };

        attachments.push(attachment_description);
        reference
    }

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
                offset: [0.0, 0.0],
                extent: [size.0 as f32, size.1 as f32],
                depth_range: [0.0, 1.0],
            }
        };
        Ok(viewport)
    }

    pub fn make_render_pass_descriptor(&self) -> Result<wgpu::RenderPassDescriptor> {
        // Collect color attachment images...
        let mut attachments = vec![];
        let mut clear_values = vec![];
        for image in &self.color_buffers {
            attachments.push(image.get_view_for_render_target()?);
            clear_values.push(self.clear_color.map(|c| c.into()));
        }

        // ...and the depth attachment image
        if let Some(depth) = &self.depth_buffer {
            attachments.push(depth.get_view_for_render_target()?);
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
