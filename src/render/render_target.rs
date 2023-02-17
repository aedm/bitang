use crate::render::vulkan_window::VulkanContext;
use std::sync::Arc;
use vulkano::format::Format;
use vulkano::render_pass::RenderPass;

pub struct RenderTarget {
    pub render_pass: Arc<RenderPass>,
    // attachments
}

impl RenderTarget {
    pub fn from_framebuffer(context: &VulkanContext) -> RenderTarget {
        let render_pass = vulkano::single_pass_renderpass!(
            context.context.device().clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: context.swapchain_format,
                    samples: 1,
                },
                depth: {
                    load: Clear,
                    store: DontCare,
                    format: Format::D16_UNORM,
                    samples: 1,
                }
            },
            pass:
                { color: [color], depth_stencil: {depth} }
        )
        .unwrap();

        RenderTarget { render_pass }
    }
}
