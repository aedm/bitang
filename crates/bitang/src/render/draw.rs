use crate::render::camera::Camera;
use crate::render::material::MaterialStepType;
use crate::render::pass::Pass;
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
use crate::render::render_object::RenderObject;

/// Represents a draw step in the chart sequence.
pub struct Draw {
    pub id: String,
    // pub render_targets: Vec<Arc<RenderTarget>>,
    pub passes: Vec<Pass>,
    pub objects: Vec<Arc<RenderObject>>,
    // render_units: Vec<RenderUnit>,
}

impl Draw {
    pub fn new(
        // context: &VulkanContext,
        id: &str,
        passes: Vec<Pass>,
        // render_targets: Vec<Arc<RenderTarget>>,
        objects: Vec<Arc<RenderObject>>,
        // clear_color: Option<[f32; 4]>,
    ) -> Result<Draw> {
        // let solid_pass = passes
        //     .iter()
        //     .find(|pass| pass.id == "solid")
        //     .with_context(|| format!("Draw step '{id}' doesn't have a solid pass. Bitang can only render solid passes at the moment."))?;
        // let render_units = objects
        //     .iter()
        //     .map(|object| RenderUnit::new(context, &solid_pass, object))
        //     .collect::<Result<Vec<_>>>()
        //     .with_context(|| format!("Failed to create render units for pass '{}'", id))?;

        Ok(Draw {
            id: id.to_string(),
            // vulkan_render_pass: render_pass,
            // render_targets,
            passes,
            objects,
            // render_units,
            // clear_color,
        })
    }

    pub fn render(
        &self,
        context: &mut RenderContext,
        // material_step_type: MaterialStepType,
        camera: &Camera,
    ) -> Result<()> {
        if self.passes.is_empty() {
            return Err(anyhow!("Draw '{}' has no passes", self.id));
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
    
    fn render_pass(&self,
                   context: &mut RenderContext,
                   pass_index: usize) -> Result<()> {
        let pass = &self.passes[pass_index];
        pass.set();
        

        todo!("render_pass");

        let render_result = self.render_render_units(context, material_step_type);

        context.command_builder.end_render_pass()?;

        render_result?;
        Ok(())

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
