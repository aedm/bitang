use crate::file::ResourceCache;
use crate::render::material::{Material, MaterialStep, MaterialStepType};
use crate::render::mesh::Mesh;
use crate::render::render_target::RenderTarget;
use crate::render::render_unit::RenderUnit;
use crate::render::shader::Shader;
use crate::render::shader_context::ContextUniforms;
use crate::render::vulkan_window::{VulkanApp, VulkanContext};
use crate::render::{Drawable, RenderObject, Texture, Vertex3};
use crate::tool::ui::Ui;
use crate::types::Object;
use anyhow::Result;
use glam::{Mat4, Vec3};
use image::io::Reader as ImageReader;
use std::cmp::max;
use std::convert::TryInto;
use std::f32::consts::PI;
use std::sync::Arc;
use std::time::Instant;
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer, CpuBufferPool, TypedBufferAccess};
use vulkano::command_buffer::PrimaryCommandBufferAbstract;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassContents,
};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::ImageViewAbstract;
use vulkano::image::{ImageDimensions, ImmutableImage, MipmapsCount};
use vulkano::memory::allocator::MemoryUsage;
use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::Pipeline;
use vulkano::pipeline::{GraphicsPipeline, PipelineBindPoint};
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, Subpass};
use vulkano::sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo};
use vulkano::shader::ShaderModule;
use vulkano::sync::GpuFuture;
use vulkano_util::renderer::{DeviceImageView, SwapchainImageView, VulkanoWindowRenderer};
use winit::event::WindowEvent;
use winit::event_loop::EventLoop;

pub struct DemoTool {
    render_target: Arc<RenderTarget>,
    ui: Ui,
    start_time: Instant,
    render_unit: RenderUnit,
    resource_cache: ResourceCache,
}

impl DemoTool {
    pub fn new(
        context: &VulkanContext,
        event_loop: &EventLoop<()>,
        object: Object,
    ) -> Result<DemoTool> {
        let mut resource_cache = ResourceCache::new();

        let render_target = Arc::new(RenderTarget::from_framebuffer(&context));
        let texture = Self::load_texture(context)?;
        let mesh = Self::load_mesh(&context, &object);
        let vs = resource_cache.get_vertex_shader(context, "app/vs.glsl")?;
        let fs = resource_cache.get_fragment_shader(context, "app/fs.glsl")?;

        let vertex_shader = Shader {
            shader_module: vs,
            textures: vec![],
        };

        let fragment_shader = Shader {
            shader_module: fs,
            textures: vec![texture],
        };

        let solid_step = MaterialStep {
            vertex_shader,
            fragment_shader,
            depth_test: true,
            depth_write: true,
        };

        let material = Material {
            passes: [None, None, Some(solid_step)],
        };

        let render_item = RenderObject {
            mesh,
            material,
            position: Default::default(),
            rotation: Default::default(),
        };

        let render_unit = RenderUnit::new(context, &render_target, Arc::new(render_item));

        let ui = Ui::new(context, event_loop);

        Ok(DemoTool {
            render_target,
            ui,
            render_unit,
            start_time: Instant::now(),
            resource_cache,
        })
    }

    fn load_texture(context: &VulkanContext) -> Result<Arc<Texture>> {
        let rgba = ImageReader::open("app/naty/Albedo.png")?
            .decode()?
            .to_rgba8();
        let dimensions = ImageDimensions::Dim2d {
            width: rgba.dimensions().0,
            height: rgba.dimensions().0,
            array_layers: 1,
        };

        let mut cbb = AutoCommandBufferBuilder::primary(
            &context.command_buffer_allocator,
            context.context.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )?;

        let image = ImmutableImage::from_iter(
            context.context.memory_allocator(),
            rgba.into_raw(),
            dimensions,
            MipmapsCount::One,
            Format::R8G8B8A8_SRGB,
            &mut cbb,
        )?;
        let _fut = cbb
            .build()
            .unwrap()
            .execute(context.context.graphics_queue().clone())
            .unwrap();

        Ok(ImageView::new_default(image)?)
    }

    pub fn load_mesh(context: &VulkanContext, object: &Object) -> Mesh {
        let vertices = object
            .mesh
            .faces
            .iter()
            .flatten()
            .map(|v| Vertex3 {
                a_position: [v.0[0], v.0[1], v.0[2]],
                a_normal: [v.1[0], v.1[1], v.1[2]],
                a_tangent: [0.0, 0.0, 0.0],
                a_uv: [v.2[0], v.2[1]],
                a_padding: 0.0,
            })
            .collect::<Vec<Vertex3>>();

        Mesh::new(context, vertices)
    }

    pub fn draw(
        &mut self,
        context: &VulkanContext,
        // app_context: &AppContext,
        target_image: SwapchainImageView,
        depth_image: DeviceImageView,
        viewport: Viewport,
        before_future: Box<dyn GpuFuture>,
    ) -> Box<dyn GpuFuture> {
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

        self.render_unit.uniform_values = {
            let model_to_camera =
                Mat4::from_translation(Vec3::new(0.0, 0.0, 3.0)) * Mat4::from_rotation_y(elapsed);
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
        self.render_unit
            .render(context, &mut builder, MaterialStepType::Solid);

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
