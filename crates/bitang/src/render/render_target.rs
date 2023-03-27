use crate::control::controls::Globals;
use crate::render::material::MaterialStepType;
use crate::render::render_target::RenderTargetSizeConstraint::Static;
use crate::render::render_unit::RenderUnit;
use crate::render::vulkan_window::VulkanContext;
use crate::render::RenderObject;
use anyhow::{anyhow, Context, Result};
use std::sync::Arc;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, RenderPassBeginInfo, SubpassContents,
};
use vulkano::format::Format;
use vulkano::image::{AttachmentImage, ImageLayout, ImageViewAbstract, SampleCount};
use vulkano::render_pass::{
    AttachmentDescription, AttachmentReference, Framebuffer, FramebufferCreateInfo, LoadOp,
    RenderPassCreateInfo, StoreOp, SubpassDescription,
};

pub struct Pass {
    pub vulkan_render_pass: Arc<vulkano::render_pass::RenderPass>,
    pub render_targets: Vec<Arc<RenderTarget>>,
    pub objects: Vec<Arc<RenderObject>>,
    render_units: Vec<RenderUnit>,
}

impl Pass {
    pub fn new(
        context: &VulkanContext,
        render_targets: Vec<Arc<RenderTarget>>,
        objects: Vec<Arc<RenderObject>>,
    ) -> Result<Pass> {
        // let render_pass = vulkano::single_pass_renderpass!(
        //     context.context.device().clone(),
        //     attachments: {
        //         color: {
        //             load: Clear,
        //             store: Store,
        //             format: context.swapchain_format,
        //             samples: 1,
        //         },
        //         depth: {
        //             load: Clear,
        //             store: DontCare,
        //             format: Format::D16_UNORM,
        //             samples: 1,
        //         }
        //     },
        //     pass:
        //         { color: [color], depth_stencil: {depth} }
        // )
        // .unwrap();

        let render_pass = Self::make_vulkan_render_pass(&render_targets)?;
        let render_units = objects
            .iter()
            .map(|object| RenderUnit::new(context, &render_pass, object))
            .collect::<Result<Vec<_>>>()?;

        Ok(Pass {
            vulkan_render_pass: render_pass,
            render_targets,
            objects,
            render_units,
        })
    }

    pub fn render(
        &self,
        context: &VulkanContext,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        material_step_type: MaterialStepType,
        globals: &Globals,
    ) {
        let attachments = self
            .render_targets
            .iter()
            .map(|target| target.image.clone())
            .collect::<Vec<_>>();
        let framebuffer = Framebuffer::new(
            self.vulkan_render_pass.clone(),
            FramebufferCreateInfo {
                attachments,
                ..Default::default()
            },
        )
        .unwrap();

        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values,
                    ..RenderPassBeginInfo::framebuffer(framebuffer)
                },
                SubpassContents::Inline,
            )
            .unwrap()
            .set_viewport(0, [viewport.clone()]);

        for render_unit in &self.render_units {
            render_unit.render(context, builder, material_step_type, globals);
        }

        builder.end_render_pass().unwrap();
    }

    fn make_vulkan_render_pass(
        render_targets: &[Arc<RenderTarget>],
    ) -> Result<Arc<vulkano::render_pass::RenderPass>> {
        let mut attachments = vec![];
        let mut color_attachments = vec![];
        let mut depth_stencil_attachment = None;
        for (index, render_target) in render_targets.iter().enumerate() {
            attachments.push(AttachmentDescription {
                format: Some(render_target.format),
                samples: SampleCount::Sample1, // TODO
                load_op: LoadOp::Load,
                store_op: StoreOp::Store,
                stencil_load_op: LoadOp::Load,
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
                RenderTargetRole::Other => {}
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

struct RenderTargetImage {
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
    pub image: Option<RenderTargetImage>,
}

impl RenderTarget {
    //! Swapchain render targets acquire their image view later before rendering
    pub fn from_swapchain(role: RenderTargetRole, format: Format) -> Arc<RenderTarget> {
        let id = match role {
            RenderTargetRole::Color => "screen",
            RenderTargetRole::Depth => "screen_depth",
            RenderTargetRole::Other => panic!("Other render target cannot come from swapchain"),
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
            image: None,
        })
    }

    pub fn update_swapchain_image(&mut self, image_view: &Arc<dyn ImageViewAbstract>) {
        // TODO: check if format is the same
        self.image_view = Some(RenderTargetImage {
            image_view: image_view.clone(),
            texture: None,
            texture_size: (
                image_view.dimensions().width,
                image_view.dimensions().height,
            ),
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
            image: None,
        })
    }

    pub fn ensure_buffer(&mut self, context: &VulkanContext, screen_size: (u32, u32)) {
        let texture_size = match self.size {
            RenderTargetSizeConstraint::Static { width, height } => (width, height),
            RenderTargetSizeConstraint::ScreenRelative { width, height } => (
                (screen_size.0 as f32 * width) as u32,
                (screen_size.1 as f32 * height) as u32,
            ),
        };

        //! Skip if texture size is the same
        if let Some(image) = &self.image {
            if image.texture_size == texture_size {
                return;
            }
        }

        let texture =
            AttachmentImage::sampled(context.context.memory_allocator(), [texture_size.0, texture_size.1], self.format)?;
        let image_view = texture.view()?;
        self.image = Some(RenderTargetImage {
            image_view,
            texture: Some(texture),
            texture_size,
        });
    }
}
