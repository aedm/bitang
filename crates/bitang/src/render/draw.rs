use crate::render::camera::Camera;
use crate::render::material::MaterialStepType;
use crate::render::render_target::{RenderTarget, RenderTargetRole};
use crate::render::render_unit::RenderUnit;
use crate::render::vulkan_window::{RenderContext, VulkanContext};
use crate::render::RenderObject;
use anyhow::{anyhow, Context, Result};
use std::sync::Arc;
use vulkano::command_buffer::{RenderPassBeginInfo, SubpassContents};
use vulkano::image::{ImageLayout, SampleCount};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::render_pass::{
    AttachmentDescription, AttachmentReference, Framebuffer, FramebufferCreateInfo, LoadOp,
    RenderPassCreateInfo, StoreOp, SubpassDescription,
};

/// Represents a draw step in the chart sequence.
pub struct Draw {
    pub id: String,
    pub vulkan_render_pass: Arc<vulkano::render_pass::RenderPass>,
    pub render_targets: Vec<Arc<RenderTarget>>,
    pub objects: Vec<Arc<RenderObject>>,
    pub clear_color: Option<[f32; 4]>,
    render_units: Vec<RenderUnit>,
}

impl Draw {
    pub fn new(
        context: &VulkanContext,
        id: &str,
        render_targets: Vec<Arc<RenderTarget>>,
        objects: Vec<Arc<RenderObject>>,
        clear_color: Option<[f32; 4]>,
    ) -> Result<Draw> {
        let render_pass =
            Self::make_vulkan_render_pass(context, &render_targets, clear_color.is_some())?;
        let render_units = objects
            .iter()
            .map(|object| RenderUnit::new(context, &render_pass, object))
            .collect::<Result<Vec<_>>>()
            .with_context(|| format!("Failed to create render units for pass '{}'", id))?;

        Ok(Draw {
            id: id.to_string(),
            vulkan_render_pass: render_pass,
            render_targets,
            objects,
            render_units,
            clear_color,
        })
    }

    pub fn render(
        &self,
        context: &mut RenderContext,
        material_step_type: MaterialStepType,
        camera: &Camera,
    ) -> Result<()> {
        if self.render_targets.is_empty() {
            return Err(anyhow!("Pass '{}' has no render targets", self.id));
        }

        let size = self.render_targets[0]
            .image
            .borrow()
            .as_ref()
            .with_context(|| format!("Render target '{}' has no image", self.render_targets[0].id))?
            .texture_size;

        // Check that all render targets have the same size
        for render_target in &self.render_targets {
            if render_target
                .image
                .borrow()
                .as_ref()
                .with_context(|| format!("Render target '{}' has no image", render_target.id))?
                .texture_size
                != size
            {
                return Err(anyhow!(
                    "Render targets in pass '{}' have different sizes",
                    self.id
                ));
            }
        }

        let viewport = if self.render_targets[0].is_swapchain {
            context.screen_viewport.clone()
        } else {
            Viewport {
                origin: [0.0, 0.0],
                dimensions: [size.0 as f32, size.1 as f32],
                depth_range: 0.0..1.0,
            }
        };
        camera.set(&mut context.globals, viewport.dimensions);

        let attachments = self
            .render_targets
            .iter()
            .map(|target| {
                target
                    .image
                    .borrow()
                    .as_ref()
                    .map(|image| image.image_view.clone())
                    .with_context(|| {
                        anyhow!("Render target '{}' has no image view", target.id.as_str())
                    })
            })
            .collect::<Result<Vec<_>>>()?;
        let framebuffer = Framebuffer::new(
            self.vulkan_render_pass.clone(),
            FramebufferCreateInfo {
                attachments,
                ..Default::default()
            },
        )?;

        let clear_values = self
            .render_targets
            .iter()
            .map(|target| match target.role {
                RenderTargetRole::Color => self.clear_color.map(|c| c.into()),
                RenderTargetRole::Depth => self.clear_color.map(|_| 1f32.into()),
            })
            .collect::<Vec<_>>();

        context
            .command_builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values,
                    ..RenderPassBeginInfo::framebuffer(framebuffer)
                },
                SubpassContents::Inline,
            )?
            // TODO: generate actual viewport
            .set_viewport(0, [viewport]);

        let render_result = self.render_render_units(context, material_step_type);

        context.command_builder.end_render_pass()?;

        render_result?;
        Ok(())
    }

    fn make_vulkan_render_pass(
        context: &VulkanContext,
        render_targets: &[Arc<RenderTarget>],
        clear_buffers: bool,
    ) -> Result<Arc<vulkano::render_pass::RenderPass>> {
        let mut attachments = vec![];
        let mut color_attachments = vec![];
        let mut depth_stencil_attachment = None;
        let load_op = if clear_buffers { LoadOp::Clear } else { LoadOp::Load };
        for (index, render_target) in render_targets.iter().enumerate() {
            let layout = match render_target.role {
                RenderTargetRole::Color => ImageLayout::ColorAttachmentOptimal,
                RenderTargetRole::Depth => ImageLayout::DepthStencilAttachmentOptimal,
            };
            let attachment_reference = Some(AttachmentReference {
                attachment: index as u32,
                layout,
                ..Default::default()
            });
            attachments.push(AttachmentDescription {
                format: Some(render_target.format),
                samples: SampleCount::Sample1, // TODO
                load_op,
                store_op: StoreOp::Store,
                initial_layout: layout,
                final_layout: layout,
                ..Default::default()
            });
            match render_target.role {
                RenderTargetRole::Color => {
                    color_attachments.push(attachment_reference);
                }
                RenderTargetRole::Depth => {
                    depth_stencil_attachment = attachment_reference;
                }
            }
        }

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

    fn render_render_units(
        &self,
        context: &mut RenderContext,
        material_step_type: MaterialStepType,
    ) -> Result<()> {
        for render_unit in &self.render_units {
            render_unit.render(context, material_step_type)?;
        }
        Ok(())
    }
}
