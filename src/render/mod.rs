mod shader_context;
pub mod vulkan_window;

use anyhow::Result;
use std::cmp::max;
use std::collections::VecDeque;
use std::convert::TryInto;
use std::f32::consts::PI;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use std::time::Instant;

use crate::render::shader_context::ContextUniforms;
use crate::Object;
use bytemuck::{Pod, Zeroable};
use cgmath::{Matrix3, Matrix4, Point3, Rad, Vector3};
// use egui::plot::{HLine, Line, Plot, Value, Values};
use egui::{Color32, ColorImage, Ui};
// use egui_vulkano::UpdateTexturesResult;
use crate::render::vulkan_window::{AppContext, VulkanContext};
use glam::{Mat4, Vec3};
use image::io::Reader as ImageReader;
use image::RgbaImage;
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer, CpuBufferPool, TypedBufferAccess};
use vulkano::command_buffer::PrimaryCommandBufferAbstract;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer, RenderPassBeginInfo,
    SubpassContents,
};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::{Device, DeviceCreateInfo, DeviceExtensions, Queue, QueueCreateInfo};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{
    ImageAccess, ImageDimensions, ImageUsage, ImmutableImage, MipmapsCount, SwapchainImage,
};
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::memory::allocator::MemoryUsage;
use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::{
    BuffersDefinition, VertexMemberInfo, VertexMemberTy,
};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::Pipeline;
use vulkano::pipeline::{GraphicsPipeline, PipelineBindPoint};
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass};
use vulkano::sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo};
use vulkano::shader::ShaderModule;
use vulkano::swapchain::{AcquireError, Swapchain, SwapchainCreateInfo, SwapchainCreationError};
use vulkano::sync::{FenceSignalFuture, FlushError, GpuFuture};
use vulkano::{swapchain, sync};
use vulkano_util::context::VulkanoContext;
use vulkano_util::renderer::{DeviceImageView, SwapchainImageView};
use vulkano_win::VkSurfaceBuild;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowBuilder};

#[derive(Default, Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Vertex3 {
    a_position: [f32; 3],
    a_normal: [f32; 3],
    a_tangent: [f32; 3],
    a_uv: [f32; 2],
    a_padding: f32,
}

vulkano::impl_vertex!(Vertex3, a_position, a_normal, a_tangent, a_uv, a_padding);

// pub struct VulkanRenderer {
//     pub device: Arc<Device>,
//     pub queue: Arc<Queue>,
//     pub current_frame: usize,
//     pub framebuffers: Vec<Arc<Framebuffer>>,
//     pub surface: Arc<swapchain::Surface>,
//     pub render_pass: Arc<RenderPass>,
//     pub swapchain: Arc<Swapchain>,
// }
//
pub struct Drawable {
    pub pipeline: Arc<GraphicsPipeline>,
    pub vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex3]>>,
    pub uniform_buffer: CpuBufferPool<ContextUniforms>,
    pub texture: Arc<ImageView<ImmutableImage>>,
    pub descriptor_set: Arc<PersistentDescriptorSet>,
}

pub struct DemoApp {
    start_time: Instant,
    drawable: Option<Box<Drawable>>,
}

impl DemoApp {
    pub fn new() -> Self {
        use vulkano::pipeline::graphics::vertex_input::Vertex;
        let pos = Vertex3::member("a_position").unwrap();
        println!("sdfgljh {} {:?} {}", pos.array_size, pos.ty, pos.offset);
        DemoApp {
            start_time: Instant::now(),
            drawable: None,
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
        let combined = format!("{}{}", header, source);

        let mut compiler = shaderc::Compiler::new().unwrap();

        let spirv = compiler.compile_into_spirv(&combined, kind, file_name, "main", None)?;
        let spirv_binary = spirv.as_binary_u8();

        let reflect = spirv_reflect::ShaderModule::load_u8_data(spirv_binary).unwrap();
        let ep = &reflect.enumerate_entry_points().unwrap()[0];
        // println!("SPIRV Metadata: {:#?}", ep);

        Ok(unsafe { ShaderModule::from_bytes(context.context.device().clone(), spirv_binary) }?)
    }

    pub fn load_model(
        &mut self,
        context: &VulkanContext,
        app_context: &AppContext,
        object: Object,
    ) -> Result<()> {
        let texture = Self::load_texture(context).unwrap();

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

        let vertex_buffer = CpuAccessibleBuffer::from_iter(
            context.context.memory_allocator(),
            BufferUsage {
                vertex_buffer: true,
                ..BufferUsage::empty()
            },
            false,
            vertices,
        )?;

        let vs = DemoApp::load_shader("app/vs.glsl", context, shaderc::ShaderKind::Vertex)?;
        let fs = DemoApp::load_shader("app/fs.glsl", context, shaderc::ShaderKind::Fragment)?;

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
                sampler.clone(),
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
        self: &mut Self,
        context: &VulkanContext,
        app_context: &AppContext,
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
                    ..RenderPassBeginInfo::framebuffer(framebuffer.clone())
                },
                SubpassContents::Inline,
            )
            .unwrap()
            .set_viewport(0, [viewport.clone()]);

        if let Some(drawable) = &self.drawable {
            let model_to_camera =
                Mat4::from_translation(Vec3::new(0.0, 0.0, 3.0)) * Mat4::from_rotation_y(elapsed);
            let model_to_projection = Mat4::perspective_infinite_lh(
                PI / 2.0,
                viewport.dimensions[0] / viewport.dimensions[1],
                0.1,
            ) * Mat4::from_translation(Vec3::new(0.0, 0.0, 3.0))
                * Mat4::from_rotation_y(elapsed);

            let uniform_buffer_subbuffer = {
                let uniform_data = ContextUniforms {
                    model_to_projection, //: model_to_projection.to_cols_array_2d(),
                    model_to_camera,     //: model_to_camera.to_cols_array_2d(),
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
                .draw(drawable.vertex_buffer.len().try_into().unwrap(), 20, 0, 0)
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
