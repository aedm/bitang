mod shader_context;
pub mod vulkan_renderer;

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
use glam::{Mat4, Vec3};
use image::io::Reader as ImageReader;
use image::RgbaImage;
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer, CpuBufferPool, TypedBufferAccess};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer, SubpassContents,
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
use vulkano_win::VkSurfaceBuild;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowBuilder};

// #[derive(Default, Debug, Clone, Copy, Pod, Zeroable)]
// #[repr(C)]
// pub struct Vertex3 {
//     a_position: [f32; 3],
//     a_normal: [f32; 3],
//     a_tangent: [f32; 3],
//     a_uv: [f32; 2],
//     a_padding: f32,
// }
//
// vulkano::impl_vertex!(Vertex3, a_position, a_normal, a_tangent, a_uv, a_padding);
//
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
// pub struct Drawable {
//     pub pipeline: Arc<GraphicsPipeline>,
//     pub vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex3]>>,
//     pub uniform_buffer: CpuBufferPool<ContextUniforms>,
//     pub texture: Arc<ImageView<ImmutableImage>>,
//     pub descriptor_set: Arc<PersistentDescriptorSet>,
// }
//
// pub struct DemoApp {
//     start_time: Instant,
//     drawable: Option<Box<Drawable>>,
// }
//
// impl DemoApp {
//     pub fn new(renderer: &VulkanRenderer) -> Self {
//         use vulkano::pipeline::graphics::vertex_input::Vertex;
//         let pos = Vertex3::member("a_position").unwrap();
//         println!("sdfgljh {} {:?} {}", pos.array_size, pos.ty, pos.offset);
//         DemoApp {
//             start_time: Instant::now(),
//             drawable: None,
//         }
//     }
//
//     fn load_texture(renderer: &VulkanRenderer) -> Result<Arc<ImageView<ImmutableImage>>> {
//         let rgba = ImageReader::open("app/naty/Albedo.png")?
//             .decode()?
//             .to_rgba8();
//         let dimensions = ImageDimensions::Dim2d {
//             width: rgba.dimensions().0,
//             height: rgba.dimensions().0,
//             array_layers: 1,
//         };
//         let (image, _future) = ImmutableImage::from_iter(
//             rgba.into_raw(),
//             dimensions,
//             MipmapsCount::One,
//             Format::R8G8B8A8_SRGB,
//             renderer.queue.clone(),
//         )?;
//         Ok(ImageView::new_default(image)?)
//     }
//
//     fn load_shader(
//         file_name: &str,
//         renderer: &VulkanRenderer,
//         kind: shaderc::ShaderKind,
//     ) -> Result<Arc<ShaderModule>> {
//         let source = std::fs::read_to_string(file_name)?;
//         let header = std::fs::read_to_string("app/header.glsl")?;
//         let combined = format!("{}{}", header, source);
//
//         let mut compiler = shaderc::Compiler::new().unwrap();
//
//         let spirv = compiler.compile_into_spirv(&combined, kind, file_name, "main", None)?;
//         let spirv_binary = spirv.as_binary_u8();
//
//         let reflect = spirv_reflect::ShaderModule::load_u8_data(spirv_binary).unwrap();
//         let ep = &reflect.enumerate_entry_points().unwrap()[0];
//         println!("SPIRV Metadata: {:#?}", ep);
//
//         Ok(unsafe { ShaderModule::from_bytes(renderer.device.clone(), spirv_binary) }?)
//     }
//
//     pub fn load_model(&mut self, renderer: &VulkanRenderer, object: Object) -> Result<()> {
//         let texture = Self::load_texture(renderer).unwrap();
//
//         let vertices = object
//             .mesh
//             .faces
//             .iter()
//             .flatten()
//             .map(|v| Vertex3 {
//                 a_position: [v.0[0], v.0[1], v.0[2]],
//                 a_normal: [v.1[0], v.1[1], v.1[2]],
//                 a_tangent: [0.0, 0.0, 0.0],
//                 a_uv: [v.2[0], v.2[1]],
//                 a_padding: 0.0,
//             })
//             .collect::<Vec<Vertex3>>();
//
//         let vertex_buffer = CpuAccessibleBuffer::from_iter(
//             renderer.device.clone(),
//             BufferUsage::all(),
//             false,
//             vertices,
//         )?;
//
//         let vs = DemoApp::load_shader("app/vs.glsl", renderer, shaderc::ShaderKind::Vertex)?;
//         let fs = DemoApp::load_shader("app/fs.glsl", renderer, shaderc::ShaderKind::Fragment)?;
//
//         let uniform_buffer = CpuBufferPool::<ContextUniforms>::new(
//             renderer.device.clone(),
//             BufferUsage {
//                 uniform_buffer: true,
//                 ..BufferUsage::none()
//             },
//         );
//
//         let pipeline = GraphicsPipeline::start()
//             .vertex_input_state(BuffersDefinition::new().vertex::<Vertex3>())
//             .vertex_shader(vs.entry_point("main").unwrap(), ())
//             .input_assembly_state(InputAssemblyState::new())
//             .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
//             .fragment_shader(fs.entry_point("main").unwrap(), ())
//             .depth_stencil_state(DepthStencilState::simple_depth_test())
//             .render_pass(Subpass::from(renderer.render_pass.clone().into(), 0).unwrap())
//             .build(renderer.device.clone())
//             .unwrap();
//
//         let sampler = Sampler::new(
//             renderer.device.clone(),
//             SamplerCreateInfo {
//                 mag_filter: Filter::Linear,
//                 min_filter: Filter::Linear,
//                 address_mode: [SamplerAddressMode::Repeat; 3],
//                 ..Default::default()
//             },
//         )
//         .unwrap();
//
//         let layout = pipeline.layout().set_layouts().get(1).unwrap();
//         let set = PersistentDescriptorSet::new(
//             layout.clone(),
//             [WriteDescriptorSet::image_view_sampler(
//                 0,
//                 texture.clone(),
//                 sampler.clone(),
//             )],
//         )
//         .unwrap();
//
//         self.drawable = Some(Box::new(Drawable {
//             pipeline,
//             vertex_buffer,
//             uniform_buffer,
//             texture,
//             descriptor_set: set,
//         }));
//
//         Ok(())
//     }
//
//     pub fn draw(
//         self: &mut Self,
//         renderer: &VulkanRenderer,
//         builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
//         framebuffer: Arc<Framebuffer>,
//         viewport: Viewport,
//     ) {
//         let clear_values = vec![[0.03, 0.03, 0.03, 1.0].into(), 1f32.into()];
//         let elapsed = self.start_time.elapsed().as_secs_f32();
//
//         builder
//             .begin_render_pass(framebuffer, SubpassContents::Inline, clear_values)
//             .unwrap()
//             .set_viewport(0, [viewport.clone()]);
//
//         if let Some(drawable) = &self.drawable {
//             let model_to_camera =
//                 Mat4::from_translation(Vec3::new(0.0, 0.0, 3.0)) * Mat4::from_rotation_y(elapsed);
//             let model_to_projection = Mat4::perspective_infinite_lh(
//                 PI / 2.0,
//                 viewport.dimensions[0] / viewport.dimensions[1],
//                 0.1,
//             ) * Mat4::from_translation(Vec3::new(0.0, 0.0, 3.0))
//                 * Mat4::from_rotation_y(elapsed);
//
//             let uniform_buffer_subbuffer = {
//                 let uniform_data = ContextUniforms {
//                     model_to_projection, //: model_to_projection.to_cols_array_2d(),
//                     model_to_camera,     //: model_to_camera.to_cols_array_2d(),
//                 };
//                 drawable.uniform_buffer.next(uniform_data).unwrap()
//             };
//
//             let layout = drawable.pipeline.layout().set_layouts().get(0).unwrap();
//             let set = PersistentDescriptorSet::new(
//                 layout.clone(),
//                 [WriteDescriptorSet::buffer(0, uniform_buffer_subbuffer)],
//             )
//             .unwrap();
//
//             builder
//                 .bind_pipeline_graphics(drawable.pipeline.clone())
//                 .bind_descriptor_sets(
//                     PipelineBindPoint::Graphics,
//                     drawable.pipeline.layout().clone(),
//                     0,
//                     set,
//                 )
//                 .bind_descriptor_sets(
//                     PipelineBindPoint::Graphics,
//                     drawable.pipeline.layout().clone(),
//                     1,
//                     self.drawable.as_ref().unwrap().descriptor_set.clone(),
//                 )
//                 .bind_vertex_buffers(0, drawable.vertex_buffer.clone())
//                 .draw(drawable.vertex_buffer.len().try_into().unwrap(), 20, 0, 0)
//                 .unwrap(); // Don't end the render pass yet
//         }
//     }
// }
