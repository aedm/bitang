use crate::types::App;
use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer};
use vulkano::command_buffer::{AutoCommandBufferBuilder, SubpassContents};
use vulkano::device::{Device, Queue};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::{Framebuffer, RenderPass, Subpass};
use vulkano::swapchain;
use vulkano::swapchain::Swapchain;
use winit::window::Window;

#[derive(Default, Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Vertex {
    position: [f32; 2],
}

pub struct VulkanRenderer {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub current_frame: usize,
    pub framebuffers: Vec<Arc<Framebuffer>>,
    pub surface: Arc<swapchain::Surface<Window>>,
    pub render_pass: Arc<RenderPass>,
    pub swapchain: Arc<Swapchain<Window>>,
}

pub struct DemoApp {
    vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex]>>,
    pipeline: Arc<GraphicsPipeline>,
}

impl DemoApp {
    pub fn new(renderer: &VulkanRenderer) -> Self {
        vulkano::impl_vertex!(Vertex, position);

        let vertex_buffer = {
            CpuAccessibleBuffer::from_iter(
                renderer.device.clone(),
                BufferUsage::all(),
                false,
                [
                    Vertex {
                        position: [-0.5, -0.25],
                    },
                    Vertex {
                        position: [0.0, 0.5],
                    },
                    Vertex {
                        position: [0.25, -0.1],
                    },
                ]
                .iter()
                .cloned(),
            )
            .unwrap()
        };

        mod vs {
            vulkano_shaders::shader! {
                ty: "vertex",
                src: "
				#version 450

				layout(location = 0) in vec2 position;

				void main() {
					gl_Position = vec4(position, 0.0, 1.0);
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

        let vs = vs::load(renderer.device.clone()).unwrap();
        let fs = fs::load(renderer.device.clone()).unwrap();

        let pipeline = GraphicsPipeline::start()
            .vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
            .vertex_shader(vs.entry_point("main").unwrap(), ())
            .input_assembly_state(InputAssemblyState::new())
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .fragment_shader(fs.entry_point("main").unwrap(), ())
            .render_pass(Subpass::from(renderer.render_pass.clone().into(), 0).unwrap())
            .build(renderer.device.clone())
            .unwrap();

        DemoApp {
            vertex_buffer,
            pipeline,
        }
    }
}
impl App for DemoApp {
    fn draw(self: &mut Self, renderer: &VulkanRenderer, builder: AutoCommandBufferBuilder<i32>) {
        // Do your usual rendering
        let clear_values = vec![[0.0, 0.0, 1.0, 1.0].into()];

        builder
            .begin_render_pass(
                self.renderer.framebuffers[image_num].clone(),
                SubpassContents::Inline,
                clear_values,
            )
            .unwrap()
            .set_viewport(0, [self.viewport.clone()])
            .bind_pipeline_graphics(self.renderer.pipeline.clone())
            .bind_vertex_buffers(0, self.vertex_buffer.clone())
            .draw(self.vertex_buffer.len().try_into().unwrap(), 1, 0, 0)
            .unwrap(); // Don't end the render pass yet
    }
}
