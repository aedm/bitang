use crate::file::resource_repository::ResourceRepository;
use crate::render::material::{Material, MaterialStep, MaterialStepType};
use crate::render::mesh::Mesh;
use crate::render::render_target::RenderTarget;
use crate::render::render_unit::RenderUnit;
use crate::render::shader::Shader;
use crate::render::shader_context::ContextUniforms;
use crate::render::vulkan_window::{VulkanApp, VulkanContext};
use crate::render::{RenderObject, Texture, Vertex3};
use crate::tool::ui::Ui;
use crate::types::Object;
use anyhow::Result;
use glam::{Mat4, Vec3};
use image::io::Reader as ImageReader;
use std::cmp::max;
use std::f32::consts::PI;
use std::sync::Arc;
use std::time::Instant;
use vulkano::command_buffer::PrimaryCommandBufferAbstract;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassContents,
};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::ImageViewAbstract;
use vulkano::image::{ImageDimensions, ImmutableImage, MipmapsCount};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo};
use vulkano::shader::ShaderModule;
use vulkano::sync::GpuFuture;
use vulkano_util::renderer::{DeviceImageView, SwapchainImageView, VulkanoWindowRenderer};
use winit::event::WindowEvent;
use winit::event_loop::EventLoop;

pub struct DemoTool {
    render_target: Arc<RenderTarget>,
    ui: Ui,
    start_time: Instant,
    // resource_cache: ResourceCache,
    resource_repository: ResourceRepository,
    render_unit: Option<RenderUnit>,
    render_object: Option<Arc<RenderObject>>,
}

impl DemoTool {
    pub fn new(context: &VulkanContext, event_loop: &EventLoop<()>) -> Result<DemoTool> {
        // let mut resource_cache = ResourceCache::new();
        let mut resource_repository = ResourceRepository::try_new()?;
        let render_target = Arc::new(RenderTarget::from_framebuffer(&context));

        // let render_object = load_render_object(&mut resource_cache, context)?;
        let render_object = resource_repository.load_root_document(context)?;
        let render_unit = RenderUnit::new(context, &render_target, render_object.clone());

        let ui = Ui::new(context, event_loop);

        let demo_tool = DemoTool {
            render_target,
            ui,
            start_time: Instant::now(),
            // resource_cache,
            resource_repository,
            render_unit: Some(render_unit),
            render_object: Some(render_object),
        };
        Ok(demo_tool)
    }

    pub fn draw(
        &mut self,
        context: &VulkanContext,
        target_image: SwapchainImageView,
        depth_image: DeviceImageView,
        viewport: Viewport,
        before_future: Box<dyn GpuFuture>,
    ) -> Box<dyn GpuFuture> {
        // self.update_render_unit(context).unwrap();
        let render_object = self.resource_repository.load_root_document(context);
        if let Ok(render_object) = render_object {
            let mut changed = true;

            if let Some(old_object) = &self.render_object {
                changed = !Arc::ptr_eq(&render_object, old_object);
            }
            if changed {
                self.render_object = Some(render_object.clone());
                self.render_unit =
                    Some(RenderUnit::new(context, &self.render_target, render_object));
            }
        }

        let elapsed = self.start_time.elapsed().as_secs_f32();

        // let dimensions = target_image.dimensions().width_height();
        let framebuffer = Framebuffer::new(
            self.render_target.render_pass.clone(),
            FramebufferCreateInfo {
                attachments: vec![target_image, depth_image],
                ..Default::default()
            },
        )
        .unwrap();

        let mut builder = AutoCommandBufferBuilder::primary(
            &context.command_buffer_allocator,
            context.context.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let clear_values = vec![Some([0.03, 0.03, 0.03, 1.0].into()), Some(1f32.into())];
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

        if let Some(render_unit) = &mut self.render_unit {
            render_unit.uniform_values = {
                let model_to_camera = Mat4::from_translation(Vec3::new(0.0, 0.0, 3.0))
                    * Mat4::from_rotation_y(elapsed);
                let camera_to_projection = Mat4::perspective_infinite_lh(
                    PI / 2.0,
                    viewport.dimensions[0] / viewport.dimensions[1],
                    0.1,
                );
                let model_to_projection = camera_to_projection * model_to_camera;

                ContextUniforms {
                    model_to_projection,
                    model_to_camera,
                }
            };
            render_unit.render(context, &mut builder, MaterialStepType::Solid);
        }

        builder.end_render_pass().unwrap();
        let command_buffer = builder.build().unwrap();

        let after_future = before_future
            .then_execute(context.context.graphics_queue().clone(), command_buffer)
            .unwrap()
            .boxed();

        after_future
    }
}

impl VulkanApp for DemoTool {
    fn paint(&mut self, context: &VulkanContext, renderer: &mut VulkanoWindowRenderer) {
        let before_future = renderer.acquire().unwrap();
        let target_image = renderer.swapchain_image_view();
        let depth_image = renderer.get_additional_image_view(1);
        let scale_factor = renderer.window().scale_factor() as f32;

        let size = target_image.dimensions();
        let movie_height = (size.width() * 9 / 16) as i32;
        let ui_height = max(size.height() as i32 - movie_height, 0) as f32 / scale_factor;

        let render_viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [size.width() as f32, movie_height as f32],
            depth_range: 0.0..1.0,
        };

        // Render app
        let app_finished_future = self.draw(
            context,
            target_image.clone(),
            depth_image,
            render_viewport,
            before_future,
        );

        // Render UI
        let gui_finished_future =
            self.ui
                .render(context, app_finished_future, target_image, ui_height);

        renderer.present(gui_finished_future, true);
    }

    fn handle_window_event(&mut self, event: &WindowEvent) {
        self.ui.handle_window_event(event);
    }
}
