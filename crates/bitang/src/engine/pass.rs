use crate::engine::render::image::BitangImage;
use crate::tool::{FrameContext, RenderPassContext, Viewport};
use anyhow::{bail, ensure, Result};
use smallvec::SmallVec;
use std::sync::Arc;

use crate::engine::render::image::PixelFormat;
use crate::engine::render::Size2D;

// TODO: this might not be needed at all
#[derive(Clone, Debug)]
pub struct FramebufferInfo {
    pub color_buffer_formats: Vec<PixelFormat>,
    pub depth_buffer_format: Option<PixelFormat>,
}

pub struct Pass {
    pub id: String,
    pub color_buffers: Vec<Arc<BitangImage>>,
    pub depth_buffer: Option<Arc<BitangImage>>,
    pub clear_color: Option<[f32; 4]>,
    pub framebuffer_info: FramebufferInfo,
}

impl Pass {
    pub fn new(
        id: &str,
        color_buffers: Vec<Arc<BitangImage>>,
        depth_buffer: Option<Arc<BitangImage>>,
        clear_color: Option<[f32; 4]>,
    ) -> Result<Self> {
        let color_buffer_formats =
            color_buffers.iter().map(|image| image.pixel_format).collect::<Vec<_>>();
        let depth_buffer_format = depth_buffer.as_ref().map(|image| image.pixel_format);
        let framebuffer_info = FramebufferInfo {
            color_buffer_formats,
            depth_buffer_format,
        };

        Ok(Pass {
            id: id.to_string(),
            depth_buffer,
            color_buffers,
            clear_color,
            framebuffer_info,
        })
    }

    pub fn make_render_pass_context<'pass, 'frame>(
        &'pass self,
        frame_context: &'pass mut FrameContext,
    ) -> Result<RenderPassContext<'pass>> {
        // Collect attachment texture views
        let color_attachment_views: SmallVec<[_; 64]> = self
            .color_buffers
            .iter()
            .map(|image| image.view_as_render_target())
            .collect::<Result<_>>()?;

        let depth_buffer_view = self
            .depth_buffer
            .as_ref()
            .map(|depth_image| depth_image.view_as_render_target())
            .transpose()?;

        // Collect attachments
        let mut color_attachments = SmallVec::<[_; 64]>::new();
        for i in 0..color_attachment_views.len() {
            color_attachments.push(Some(self.make_color_attachment(&color_attachment_views[i])));
        }
        let depth_stencil_attachment =
            depth_buffer_view.as_ref().map(|view| self.make_depth_attachment(view));

        let pass = frame_context.command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
            globals: &mut frame_context.globals,
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

    pub fn get_viewport_and_canvas_size(
        &self,
        context: &mut FrameContext,
    ) -> Result<(Viewport, Size2D)> {
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
            Viewport { x: 0, y: 0, size }
        };
        Ok((viewport, size))
    }
}
