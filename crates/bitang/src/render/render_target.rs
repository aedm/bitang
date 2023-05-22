use crate::render::camera::Camera;
use crate::render::material::MaterialStepType;
use crate::render::render_unit::RenderUnit;
use crate::render::vulkan_window::{RenderContext, VulkanContext};
use crate::render::RenderObject;
use anyhow::{anyhow, Context, Result};
use std::cell::RefCell;
use std::sync::Arc;
use vulkano::command_buffer::{RenderPassBeginInfo, SubpassContents};
use vulkano::format::Format;
use vulkano::image::view::{ImageView, ImageViewCreateInfo};
use vulkano::image::{AttachmentImage, ImageLayout, ImageUsage, ImageViewAbstract, SampleCount};
use vulkano::memory::allocator::StandardMemoryAllocator;
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
    pub clear_color: Option<[f32; 4]>,
    render_units: Vec<RenderUnit>,
}

impl Pass {
    pub fn new(
        context: &VulkanContext,
        id: &str,
        render_targets: Vec<Arc<RenderTarget>>,
        objects: Vec<Arc<RenderObject>>,
        clear_color: Option<[f32; 4]>,
    ) -> Result<Pass> {
        let render_pass =
            Self::make_vulkan_render_pass(context, &render_targets, clear_color.is_some())?;
        let render_units = objects
            .iter()
            .map(|object| RenderUnit::new(context, &render_pass, object))
            .collect::<Result<Vec<_>>>()
            .with_context(|| format!("Failed to create render units for pass '{}'", id))?;

        Ok(Pass {
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
                // RenderTargetRole::Color => Some([0.03, 0.03, 0.03, 1.0].into()),
                RenderTargetRole::Color => self.clear_color.map(|c| c.into()),
                // RenderTargetRole::Depth => Some(1f32.into()),
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
}

#[derive(PartialEq, Eq)]
pub enum RenderTargetRole {
    Color,
    Depth,
}

#[derive(Clone)]
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

    pub fn new_fake_swapchain(
        memory_allocator: &Arc<StandardMemoryAllocator>,
        role: RenderTargetRole,
        texture_size: (u32, u32),
    ) -> Arc<RenderTarget> {
        let id = match role {
            RenderTargetRole::Color => "screen",
            RenderTargetRole::Depth => "screen_depth",
        };
        let format = match role {
            RenderTargetRole::Color => Format::R8G8B8A8_SRGB,
            RenderTargetRole::Depth => Format::D32_SFLOAT,
        };
        let mut usage = ImageUsage::SAMPLED | ImageUsage::TRANSFER_SRC;
        if role == RenderTargetRole::Color {
            usage |= ImageUsage::COLOR_ATTACHMENT;
        } else if role == RenderTargetRole::Depth {
            usage |= ImageUsage::DEPTH_STENCIL_ATTACHMENT;
        }
        let texture = AttachmentImage::with_usage(
            memory_allocator,
            [texture_size.0, texture_size.1],
            format,
            usage,
        )
        .unwrap();
        let create_info = ImageViewCreateInfo {
            usage,
            ..ImageViewCreateInfo::from_image(&texture)
        };
        let image = Some(RenderTargetImage {
            image_view: ImageView::new(texture.clone(), create_info).unwrap(),
            texture: Some(texture),
            texture_size,
        });

        Arc::new(RenderTarget {
            is_swapchain: true,
            id: id.to_string(),
            format,
            size_constraint: RenderTargetSizeConstraint::Static {
                width: texture_size.0,
                height: texture_size.1,
            },
            role,
            image: RefCell::new(image),
        })
    }

    pub fn ensure_buffer(&self, context: &RenderContext) -> Result<()> {
        if self.is_swapchain {
            return Ok(());
        }
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

        let texture = AttachmentImage::with_usage(
            context.vulkan_context.context.memory_allocator(),
            [texture_size.0, texture_size.1],
            self.format,
            ImageUsage::SAMPLED | ImageUsage::TRANSFER_DST,
        )?;
        *self.image.borrow_mut() = Some(RenderTargetImage {
            image_view: ImageView::new_default(texture.clone())?,
            texture: Some(texture),
            texture_size,
        });
        Ok(())
    }
}
