mod ui;

use crate::render::vulkan_window::{AppContext, VulkanApp, VulkanContext};
use crate::render::{Drawable, Vertex3};
use crate::tool::ui::Ui;
use crate::types::Object;
use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use image::io::Reader as ImageReader;
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
use vulkano_util::renderer::{DeviceImageView, SwapchainImageView};
use winit::event_loop::EventLoop;

pub struct DemoTool {
    start_time: Instant,
    drawable: Option<Box<Drawable>>,
    object: Object,
    pub subpass: Option<Subpass>,
    pub ui: Option<Ui>,
}

impl DemoTool {
    pub fn new(object: Object) -> Self {
        DemoTool {
            start_time: Instant::now(),
            drawable: None,
            object,
            subpass: None,
            ui: None,
        }
    }

    fn load_texture(context: &VulkanContext) -> Result<Arc<ImageView<ImmutableImage>>> {
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

    fn load_shader(
        file_name: &str,
        context: &VulkanContext,
        kind: shaderc::ShaderKind,
    ) -> Result<Arc<ShaderModule>> {
        let source = std::fs::read_to_string(file_name)?;
        let header = std::fs::read_to_string("app/header.glsl")?;
        let combined = format!("{header}{source}");

        let compiler = shaderc::Compiler::new().unwrap();

        let spirv = compiler.compile_into_spirv(&combined, kind, file_name, "main", None)?;
        let spirv_binary = spirv.as_binary_u8();

        let reflect = spirv_reflect::ShaderModule::load_u8_data(spirv_binary).unwrap();
        let _ep = &reflect.enumerate_entry_points().unwrap()[0];
        // println!("SPIRV Metadata: {:#?}", ep);

        Ok(unsafe { ShaderModule::from_bytes(context.context.device().clone(), spirv_binary) }?)
    }

    pub fn load_model(&mut self, context: &VulkanContext, app_context: &AppContext) -> Result<()> {
        let texture = Self::load_texture(context).unwrap();

        let vertices = self
            .object
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

        let vertex_buffer = CpuAccessibleBuffer::from_iter(
            context.context.memory_allocator(),
            BufferUsage {
                vertex_buffer: true,
                ..BufferUsage::empty()
            },
            false,
            vertices,
        )?;

        let vs = DemoTool::load_shader("app/vs.glsl", context, shaderc::ShaderKind::Vertex)?;
        let fs = DemoTool::load_shader("app/fs.glsl", context, shaderc::ShaderKind::Fragment)?;

        let uniform_buffer = CpuBufferPool::<ContextUniforms>::new(
            context.context.memory_allocator().clone(),
            BufferUsage {
                uniform_buffer: true,
                ..BufferUsage::empty()
            },
            MemoryUsage::Upload,
        );

        let pipeline = GraphicsPipeline::start()
            .vertex_input_state(BuffersDefinition::new().vertex::<Vertex3>())
            .vertex_shader(vs.entry_point("main").unwrap(), ())
            .input_assembly_state(InputAssemblyState::new())
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .fragment_shader(fs.entry_point("main").unwrap(), ())
            .depth_stencil_state(DepthStencilState::simple_depth_test())
            .render_pass(app_context.subpass.clone())
            .build(context.context.device().clone())
            .unwrap();

        let sampler = Sampler::new(
            context.context.device().clone(),
            SamplerCreateInfo {
                mag_filter: Filter::Linear,
                min_filter: Filter::Linear,
                address_mode: [SamplerAddressMode::Repeat; 3],
                ..Default::default()
            },
        )
        .unwrap();

        let layout = pipeline.layout().set_layouts().get(1).unwrap();
        let set = PersistentDescriptorSet::new(
            &context.descriptor_set_allocator,
            layout.clone(),
            [WriteDescriptorSet::image_view_sampler(
                0,
                texture.clone(),
                sampler,
            )],
        )
        .unwrap();

        self.drawable = Some(Box::new(Drawable {
            pipeline,
            vertex_buffer,
            uniform_buffer,
            texture,
            descriptor_set: set,
        }));

        Ok(())
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
            app_context.subpass.render_pass().clone(),
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

        if let Some(drawable) = &self.drawable {
            let model_to_camera =
                Mat4::from_translation(Vec3::new(0.0, 0.0, 3.0)) * Mat4::from_rotation_y(elapsed);
            let camera_to_projection = Mat4::perspective_infinite_lh(
                PI / 2.0,
                viewport.dimensions[0] / viewport.dimensions[1],
                0.1,
            );

            let model_to_projection = camera_to_projection * model_to_camera;

            let uniform_buffer_subbuffer = {
                let uniform_data = ContextUniforms {
                    model_to_projection,
                    model_to_camera,
                };
                drawable.uniform_buffer.from_data(uniform_data).unwrap()
                // CpuAccessibleBuffer::from_iter(
                //     context.memory_allocator(),
                //     BufferUsage {
                //         uniform_buffer: true,
                //         ..BufferUsage::empty()
                //     },
                //     false,
                //     uniform_data.into_iter(),
                // )
                // .unwrap()
            };

            let layout = drawable.pipeline.layout().set_layouts().get(0).unwrap();
            let set = PersistentDescriptorSet::new(
                &context.descriptor_set_allocator,
                layout.clone(),
                [WriteDescriptorSet::buffer(0, uniform_buffer_subbuffer)],
            )
            .unwrap();

            builder
                .bind_pipeline_graphics(drawable.pipeline.clone())
                .bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    drawable.pipeline.layout().clone(),
                    0,
                    set,
                )
                .bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    drawable.pipeline.layout().clone(),
                    1,
                    self.drawable.as_ref().unwrap().descriptor_set.clone(),
                )
                .bind_vertex_buffers(0, drawable.vertex_buffer.clone())
                .draw(drawable.vertex_buffer.len().try_into().unwrap(), 1, 0, 0)
                .unwrap();
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
    fn init(&mut self, context: &VulkanContext, event_loop: &EventLoop<()>) {
        let render_pass = vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: renderer.swapchain_format(),
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

        let subpass = Subpass::from(render_pass, 0).unwrap();
        self.subpass = Some(subpass);
        self.ui = Some(Ui::new(&context, event_loop));
    }

    fn paint(&mut self, context: &VulkanContext) {
        todo!()
    }
}
