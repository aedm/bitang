use crate::render::material::MaterialStepType;
use crate::render::render_unit::RenderUnit;
use crate::render::vulkan_window::{RenderContext, VulkanContext};
use crate::render::RenderObject;
use anyhow::{anyhow, Context, Result};
use std::cell::RefCell;
use std::sync::Arc;
use tracing::error;
use vulkano::command_buffer::{RenderPassBeginInfo, SubpassContents};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{AttachmentImage, ImageLayout, ImageViewAbstract, SampleCount};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::render_pass::{
    AttachmentDescription, AttachmentReference, Framebuffer, FramebufferCreateInfo, LoadOp,
    RenderPassCreateInfo, StoreOp, SubpassDescription,
};

pub struct Pass {
    pub id: String,
    pub vulkan_render_pass: Arc<vulkano::render_pass::RenderPass>,
    pub render_targets: Vec<Arc<RenderTarget>>,
    pub objects: Vec<Arc<RenderObject>>,
    render_units: Vec<RenderUnit>,
}

impl Pass {
    pub fn new(
        context: &VulkanContext,
        id: &str,
        render_targets: Vec<Arc<RenderTarget>>,
        objects: Vec<Arc<RenderObject>>,
    ) -> Result<Pass> {
        let render_pass = Self::make_vulkan_render_pass(context, &render_targets)?;
        let render_units = objects
            .iter()
            .map(|object| RenderUnit::new(context, &render_pass, object))
            .collect::<Result<Vec<_>>>()?;

        Ok(Pass {
            id: id.to_string(),
            vulkan_render_pass: render_pass,
            render_targets,
            objects,
            render_units,
        })
    }

    pub fn render(
        &self,
        context: &mut RenderContext,
        material_step_type: MaterialStepType,
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

        let attachments = self
            .render_targets
            .iter()
            .map(|target| {
                target
                    .image
                    .borrow()
                    .as_ref()
                    .and_then(|image| Some(image.image_view.clone()))
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
                RenderTargetRole::Color => Some([0.03, 0.03, 0.03, 1.0].into()),
                RenderTargetRole::Depth => Some(1f32.into()),
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

        for render_unit in &self.render_units {
            render_unit.render(context, material_step_type)?;
        }

        context.command_builder.end_render_pass()?;
        Ok(())
    }

    fn make_vulkan_render_pass(
        context: &VulkanContext,
        render_targets: &[Arc<RenderTarget>],
    ) -> Result<Arc<vulkano::render_pass::RenderPass>> {
        let mut attachments = vec![];
        let mut color_attachments = vec![];
        let mut depth_stencil_attachment = None;
        for (index, render_target) in render_targets.iter().enumerate() {
            attachments.push(AttachmentDescription {
                format: Some(render_target.format),
                samples: SampleCount::Sample1, // TODO
                load_op: LoadOp::Clear,
                store_op: StoreOp::Store,
                stencil_load_op: LoadOp::Clear,
                stencil_store_op: StoreOp::Store,
                initial_layout: ImageLayout::General,
                final_layout: ImageLayout::General,
                ..Default::default()
            });
            match render_target.role {
                RenderTargetRole::Color => {
                    color_attachments.push(Some(AttachmentReference {
                        attachment: index as u32,
                        layout: ImageLayout::ColorAttachmentOptimal,
                        ..Default::default()
                    }));
                }
                RenderTargetRole::Depth => {
                    depth_stencil_attachment = Some(AttachmentReference {
                        attachment: index as u32,
                        layout: ImageLayout::DepthStencilAttachmentOptimal,
                        ..Default::default()
                    });
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
}

pub enum RenderTargetRole {
    Color,
    Depth,
}

pub enum RenderTargetSizeConstraint {
    Static { width: u32, height: u32 },
    ScreenRelative { width: f32, height: f32 },
}

pub struct RenderTargetImage {
    pub image_view: Arc<dyn ImageViewAbstract>,
    pub texture: Option<Arc<AttachmentImage>>,
    pub texture_size: (u32, u32),
}

pub struct RenderTarget {
    pub is_swapchain: bool,
    pub id: String,
    pub format: Format,
    pub size_constraint: RenderTargetSizeConstraint,
    pub role: RenderTargetRole,
    pub image: RefCell<Option<RenderTargetImage>>,
}

impl RenderTarget {
    //! Swapchain render targets acquire their image view later before rendering
    pub fn from_swapchain(role: RenderTargetRole, format: Format) -> Arc<RenderTarget> {
        let id = match role {
            RenderTargetRole::Color => "screen",
            RenderTargetRole::Depth => "screen_depth",
        };
        Arc::new(RenderTarget {
            is_swapchain: true,
            id: id.to_string(),
            format,
            size_constraint: RenderTargetSizeConstraint::ScreenRelative {
                width: 1.0,
                height: 1.0,
            },
            role,
            image: RefCell::new(None),
        })
    }

    pub fn update_swapchain_image(&self, image_view: Arc<dyn ImageViewAbstract>) {
        // TODO: check if format is the same
        *self.image.borrow_mut() = Some(RenderTargetImage {
            texture: None,
            texture_size: (
                image_view.dimensions().width(),
                image_view.dimensions().height(),
            ),
            image_view,
        });
    }

    pub fn new(
        id: &str,
        role: RenderTargetRole,
        size_constraint: RenderTargetSizeConstraint,
    ) -> Arc<RenderTarget> {
        let format = match role {
            RenderTargetRole::Color => Format::R16G16B16A16_SFLOAT,
            RenderTargetRole::Depth => Format::D32_SFLOAT,
        };
        Arc::new(RenderTarget {
            is_swapchain: false,
            id: id.to_string(),
            format,
            size_constraint,
            role,
            image: RefCell::new(None),
        })
    }

    pub fn ensure_buffer(&self, context: &RenderContext) -> Result<()> {
        let texture_size = match self.size_constraint {
            RenderTargetSizeConstraint::Static { width, height } => (width, height),
            RenderTargetSizeConstraint::ScreenRelative { width, height } => {
                let dimensions = &context.screen_viewport.dimensions;
                (
                    (dimensions[0] * width) as u32,
                    (dimensions[1] * height) as u32,
                )
            }
        };

        // Skip if texture size is the same
        if let Some(image) = self.image.borrow().as_ref() {
            if image.texture_size == texture_size {
                return Ok(());
            }
        }

        let texture = AttachmentImage::sampled(
            context.vulkan_context.context.memory_allocator(),
            [texture_size.0, texture_size.1],
            self.format,
        )?;
        *self.image.borrow_mut() = Some(RenderTargetImage {
            image_view: ImageView::new_default(texture.clone())?,
            texture: Some(texture),
            texture_size,
        });
        Ok(())
    }
}
