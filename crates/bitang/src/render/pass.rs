use crate::render::render_target::RenderTarget;
use crate::render::vulkan_window::VulkanContext;
use anyhow::{anyhow, Context, Result};
use std::sync::Arc;
use vulkano::image::{ImageLayout, SampleCount};
use vulkano::render_pass::{
    AttachmentDescription, AttachmentReference, LoadOp, RenderPassCreateInfo, StoreOp,
    SubpassDescription,
};

pub enum RenderTargetSelector {
    RenderTargetLevelZero(Arc<RenderTarget>),
}

pub struct Pass {
    pub id: String,
    color_buffers: Vec<RenderTargetSelector>,
    depth_buffer: Option<RenderTargetSelector>,
    clear_color: Option<[f32; 4]>,
    pub vulkan_render_pass: Arc<vulkano::render_pass::RenderPass>,
}

impl Pass {
    pub fn new(
        id: &str,
        context: &VulkanContext,
        color_buffers: Vec<RenderTargetSelector>,
        depth_buffer: Option<RenderTargetSelector>,
        clear_color: Option<[f32; 4]>,
    ) -> Self {
        let vulkan_render_pass = Self::make_vulkan_render_pass(
            context,
            &color_buffers,
            &depth_buffer,
            clear_color.is_some(),
        )?;
        Pass {
            id: id.to_string(),
            depth_buffer,
            color_buffers,
            clear_color,
            vulkan_render_pass,
        }
    }

    fn make_vulkan_render_pass(
        context: &VulkanContext,
        color_buffers: &Vec<RenderTargetSelector>,
        depth_buffer: &Option<RenderTargetSelector>,
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
        selector: &RenderTargetSelector,
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
            RenderTargetSelector::RenderTargetLevelZero(render_target) => AttachmentDescription {
                format: Some(render_target.format),
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
}
