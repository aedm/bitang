// use bytemuck::{Pod, Zeroable};
// use std::sync::Arc;
// use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer};
// use vulkano::command_buffer::{
//     AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, SubpassContents,
// };
// use vulkano::device::{Device, Queue};
// use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
// use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
// use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
// use vulkano::pipeline::GraphicsPipeline;
// use vulkano::render_pass::{Framebuffer, RenderPass, Subpass};
// use vulkano::swapchain;
// use vulkano::swapchain::Swapchain;
// use winit::window::Window;

use std::cmp::max;
// Copied from https://github.com/vulkano-rs/vulkano/blob/master/examples/src/bin/triangle.rs
/// Differences:
/// * Set the correct color format for the swapchain
/// * Second renderpass to draw the gui
use std::collections::VecDeque;
use std::convert::TryInto;
use std::sync::Arc;
use std::time::Instant;

use crate::Object;
use bytemuck::{Pod, Zeroable};
use cgmath::{Matrix3, Matrix4, Point3, Rad, Vector3};
use egui::plot::{HLine, Line, Plot, Value, Values};
use egui::{Color32, ColorImage, Ui};
use egui_vulkano::UpdateTexturesResult;
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer, CpuBufferPool, TypedBufferAccess};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer, SubpassContents,
};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::{Device, DeviceCreateInfo, DeviceExtensions, Queue, QueueCreateInfo};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{ImageAccess, ImageUsage, SwapchainImage};
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::Pipeline;
use vulkano::pipeline::{GraphicsPipeline, PipelineBindPoint};
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass};
use vulkano::swapchain::{AcquireError, Swapchain, SwapchainCreateInfo, SwapchainCreationError};
use vulkano::sync::{FenceSignalFuture, FlushError, GpuFuture};
use vulkano::{swapchain, sync};
use vulkano_win::VkSurfaceBuild;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowBuilder};

#[derive(Default, Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Vertex {
    position: [f32; 3],
}

vulkano::impl_vertex!(Vertex, position);

pub struct VulkanRenderer {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub current_frame: usize,
    pub framebuffers: Vec<Arc<Framebuffer>>,
    pub surface: Arc<swapchain::Surface<Window>>,
    pub render_pass: Arc<RenderPass>,
    pub swapchain: Arc<Swapchain<Window>>,
}

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        types_meta: {
            use bytemuck::{Pod, Zeroable};
            #[derive(Clone, Copy, Zeroable, Pod)]
        },
        src: "
				#version 450

				layout(location = 0) in vec3 position;
                layout(set = 0, binding = 0) uniform Data { vec4 e; vec2 viewport_adjust; } uniforms;

				void main() {
                    float sn = sin(uniforms.e.x);
                    float cs = cos(uniforms.e.x);
                    vec2 xy = position.xy;
					gl_Position = vec4(vec2(xy.x *sn + xy.y * cs, -xy.x * cs + xy.y * sn) * uniforms.viewport_adjust, position.z, 1.0);
				}
			"
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: "
				#version 450

				layout(location = 0) out vec4 f_color;

				void main() {
					f_color = vec4(1.0, 0.0, 0.0, 1.0);
				}
			"
    }
}

pub struct Drawable {
    pub pipeline: Arc<GraphicsPipeline>,
    pub vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex]>>,
    pub uniform_buffer: CpuBufferPool<vs::ty::Data>,
}

pub struct DemoApp {
    // vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex]>>,
    // pipeline: Arc<GraphicsPipeline>,
    start_time: Instant,
    // uniform_buffer: CpuBufferPool<vs::ty::Data>,
    drawable: Option<Box<Drawable>>,
}

impl DemoApp {
    pub fn new(renderer: &VulkanRenderer) -> Self {
        DemoApp {
            start_time: Instant::now(),
            drawable: None,
        }
    }

    pub fn load_model(&mut self, renderer: &VulkanRenderer, object: Object) {
        let vertices = object
            .mesh
            .faces
            .iter()
            .flatten()
            .map(|v| Vertex {
                position: [v.0[0], v.0[1], v.0[2]],
            })
            .collect::<Vec<Vertex>>();

        let vertex_buffer = {
            CpuAccessibleBuffer::from_iter(
                renderer.device.clone(),
                BufferUsage::all(),
                false,
                vertices, // [
                          //     Vertex {
                          //         position: [-0.5, -0.25],
                          //     },
                          //     Vertex {
                          //         position: [0.0, 0.5],
                          //     },
                          //     Vertex {
                          //         position: [0.25, -0.1],
                          //     },
                          // ]
                          // .iter()
                          // .cloned(),
            )
            .unwrap()
        };

        let vs = vs::load(renderer.device.clone()).unwrap();
        let fs = fs::load(renderer.device.clone()).unwrap();

        let uniform_buffer = CpuBufferPool::<vs::ty::Data>::new(
            renderer.device.clone(),
            BufferUsage {
                uniform_buffer: true,
                ..BufferUsage::none()
            },
        );

        let pipeline = GraphicsPipeline::start()
            .vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
            .vertex_shader(vs.entry_point("main").unwrap(), ())
            .input_assembly_state(InputAssemblyState::new())
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .fragment_shader(fs.entry_point("main").unwrap(), ())
            .render_pass(Subpass::from(renderer.render_pass.clone().into(), 0).unwrap())
            .build(renderer.device.clone())
            .unwrap();

        self.drawable = Some(Box::new(Drawable {
            pipeline,
            vertex_buffer,
            uniform_buffer,
        }));
    }

    pub fn draw(
        self: &mut Self,
        renderer: &VulkanRenderer,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        framebuffer: Arc<Framebuffer>,
        viewport: Viewport,
    ) {
        let clear_values = vec![[0.0, 0.0, 1.0, 1.0].into()];
        let elapsed = self.start_time.elapsed().as_secs_f32();

        builder
            .begin_render_pass(framebuffer, SubpassContents::Inline, clear_values)
            .unwrap()
            .set_viewport(0, [viewport.clone()]);

        if let Some(drawable) = &self.drawable {
            let uniform_buffer_subbuffer = {
                let uniform_data = vs::ty::Data {
                    e: [elapsed, 0.0, 0.0, 0.0].into(),
                    viewport_adjust: [1.0, viewport.dimensions[0] / viewport.dimensions[1]].into(),
                };
                drawable.uniform_buffer.next(uniform_data).unwrap()
            };

            let layout = drawable.pipeline.layout().set_layouts().get(0).unwrap();
            let set = PersistentDescriptorSet::new(
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
                .bind_vertex_buffers(0, drawable.vertex_buffer.clone())
                .draw(drawable.vertex_buffer.len().try_into().unwrap(), 1, 0, 0)
                .unwrap(); // Don't end the render pass yet
        }
    }
}
