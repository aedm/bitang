use crate::render::material::{MaterialStep, MATERIAL_STEP_COUNT};
use crate::render::mesh::VertexBuffer;
use crate::render::shader::DescriptorSetIds;
use crate::render::shader_context::ContextUniforms;
use crate::render::vulkan_window::VulkanContext;
use crate::render::{RenderItem, Vertex3};
use std::sync::Arc;
use vulkano::buffer::cpu_pool::CpuBufferPoolSubbuffer;
use vulkano::buffer::{BufferUsage, CpuBufferPool, TypedBufferAccess};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::memory::allocator::MemoryUsage;
use vulkano::pipeline::graphics::depth_stencil::{CompareOp, DepthState, DepthStencilState};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::{GraphicsPipeline, Pipeline, PipelineBindPoint, PipelineLayout, StateMode};
use vulkano::render_pass::Subpass;

struct RenderUnit {
    render_item: Arc<RenderItem>,
    // TODO: use render pass instead
    subpass: Subpass,
    steps: [Option<RenderUnitStep>; MATERIAL_STEP_COUNT],
    uniform_values: ContextUniforms,
}

struct RenderUnitStep {
    pipeline: Arc<GraphicsPipeline>,
    vertex_uniforms_storage: ShaderUniformStorage,
    fragment_uniforms_storage: ShaderUniformStorage,
}

struct ShaderUniformStorage {
    uniform_buffer_pool: CpuBufferPool<ContextUniforms>,
    persistent_descriptor_set: Option<Arc<PersistentDescriptorSet>>,
}

impl RenderUnit {
    pub fn new(
        context: &VulkanContext,
        subpass: Subpass,
        render_item: Arc<RenderItem>,
    ) -> RenderUnit {
        let steps = render_item
            .material
            .passes
            .iter()
            .map(|material_pass| {
                if let Some(material_pass) = material_pass {
                    Some(RenderUnitStep::new(context, &subpass, material_pass))
                } else {
                    None
                }
            })
            .collect();

        let uniform_values = ContextUniforms {
            model_to_projection: Default::default(),
            model_to_camera: Default::default(),
        };

        RenderUnit {
            render_item,
            subpass,
            steps,
            uniform_values,
        }
    }

    pub fn build_uniform_buffers(&mut self) {
        self.vertex_uniforms
            .build_uniform_buffers(&self.uniform_values);
        self.fragment_uniforms
            .build_uniform_buffers(&self.uniform_values);
    }

    pub fn render(
        &self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        material_pass_type: MaterialPassType,
    ) {
        let index = material_pass_type as usize;
        match (&self.steps[index], &self.render_item.material.passes[index]) {
            (Some(component), Some(material_pass)) => {
                component.render(builder, material_pass, &self.render_item.mesh.vertex_buffer);
            }
            (None, None) => {}
            _ => panic!("RenderUnitStep and MaterialStep mismatch"),
        }
    }
}

impl RenderUnitStep {
    pub fn new(
        context: &VulkanContext,
        subpass: &Subpass,
        material_step: &MaterialStep,
    ) -> RenderUnitStep {
        let vertex_uniforms_storage = ShaderUniformStorage::new(context);
        let fragment_uniforms_storage = ShaderUniformStorage::new(context);

        let depth = if material_step.depth_test || material_step.depth_write {
            let compare_op = if material_step.depth_test {
                CompareOp::Less
            } else {
                CompareOp::Always
            };
            Some(DepthState {
                enable_dynamic: false,
                compare_op: StateMode::Fixed(compare_op),
                write_enable: StateMode::Fixed(material_step.depth_write),
            })
        } else {
            None
        };

        let depth_stencil_state = DepthStencilState {
            depth,
            depth_bounds: Default::default(),
            stencil: Default::default(),
        };

        let pipeline = GraphicsPipeline::start()
            .vertex_input_state(BuffersDefinition::new().vertex::<Vertex3>())
            .vertex_shader(
                material_step
                    .vertex_shader
                    .shader_module
                    .entry_point("main")
                    .unwrap(),
                (),
            )
            .input_assembly_state(InputAssemblyState::new())
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .fragment_shader(
                material_step
                    .fragment_shader
                    .shader_module
                    .entry_point("main")
                    .unwrap(),
                (),
            )
            .depth_stencil_state(depth_stencil_state)
            .render_pass(subpass.clone())
            .build(context.context.device().clone())
            .unwrap();

        RenderUnitStep {
            pipeline,
            vertex_uniforms_storage,
            fragment_uniforms_storage,
        }
    }

    pub fn render(
        &self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        material_pass: &MaterialStep,
        vertex_buffer: &VertexBuffer,
    ) {
        let descriptor_set_layouts = self.pipeline.layout().set_layouts();
        let vertex_descriptor_set = self.vertex_uniforms_storage.make_descriptor_set(
            context,
            uniform_values,
            material_pass,
            descriptor_set_layouts
                .get(DescriptorSetIds::Vertex)
                .unwrap(),
        );
        let fragment_descriptor_set = self.fragment_uniforms_storage.make_descriptor_set(
            context,
            uniform_values,
            material_pass,
            descriptor_set_layouts
                .get(DescriptorSetIds::Fragment)
                .unwrap(),
        );

        builder
            .bind_pipeline_graphics(drawable.pipeline.clone())
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline.layout().clone(),
                0,
                vertex_descriptor_set,
            )
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline.layout().clone(),
                1,
                fragment_descriptor_set,
            )
            .bind_vertex_buffers(0, vertex_buffer.clone())
            .draw(vertex_buffer.len().try_into().unwrap(), 1, 0, 0)
            .unwrap();
    }
}

impl ShaderUniformStorage {
    pub fn new(context: &VulkanContext) -> ShaderUniformStorage {
        let uniform_buffer_pool = UniformBufferPool::new(
            context.context.memory_allocator().clone(),
            BufferUsage {
                uniform_buffer: true,
                ..BufferUsage::empty()
            },
            MemoryUsage::Upload,
        );
        ShaderUniformStorage {
            uniform_buffer_pool,
            persistent_descriptor_set: None,
        }
    }

    fn make_descriptor_set(
        &self,
        context: &VulkanContext,
        uniform_values: &ContextUniforms,
        material_pass: &MaterialPass,
        layout: &Arc<DescriptorSetLayout>,
    ) -> Arc<PersistentDescriptorSet> {
        // TODO: uniform mapping
        let uniform_buffer_subbuffer = self.uniform_buffer_pool.from_data(*uniform_values).unwrap();

        let mut descriptors = vec![WriteDescriptorSet::buffer(0, uniform_buffer_subbuffer)];
        descriptors.extend(material_pass.texture_bindings.iter().enumerate().map(
            |(i, texture_binding)| {
                WriteDescriptorSet::image_view_sampler(
                    i as u32 + 1,
                    texture_binding.texture.clone(),
                    texture_binding.sampler.clone(),
                )
            },
        ));

        let persistent_descriptor_set = PersistentDescriptorSet::new(
            &context.descriptor_set_allocator,
            layout.clone(),
            descriptors,
        )
        .unwrap();

        persistent_descriptor_set
    }
}
