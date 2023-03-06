use crate::control::controls::Globals;
use crate::render::material::{
    MaterialStep, MaterialStepType, Shader, ShaderKind, MATERIAL_STEP_COUNT,
};
use crate::render::mesh::Mesh;
use crate::render::render_target::RenderTarget;
use crate::render::vulkan_window::VulkanContext;
use crate::render::{RenderObject, Vertex3};
use std::array;
use std::mem::size_of;
use std::sync::Arc;
use vulkano::buffer::{BufferUsage, CpuBufferPool, TypedBufferAccess};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::memory::allocator::MemoryUsage;
use vulkano::pipeline::graphics::depth_stencil::{CompareOp, DepthState, DepthStencilState};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::{GraphicsPipeline, Pipeline, PipelineBindPoint, StateMode};
use vulkano::render_pass::Subpass;
use vulkano::sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo};

// TODO: use a dynamically sized ring buffer for uniforms instead
const MAX_UNIFORMS_F32_COUNT: usize = 1024;
type UniformBufferPool = CpuBufferPool<[f32; MAX_UNIFORMS_F32_COUNT]>;

pub struct RenderUnit {
    render_object: Arc<RenderObject>,
    steps: [Option<RenderUnitStep>; MATERIAL_STEP_COUNT],
}

struct RenderUnitStep {
    pipeline: Arc<GraphicsPipeline>,
    vertex_uniforms_storage: ShaderUniformStorage,
    fragment_uniforms_storage: ShaderUniformStorage,
}

struct ShaderUniformStorage {
    uniform_buffer_pool: UniformBufferPool,
}

impl RenderUnit {
    pub fn new(
        context: &VulkanContext,
        render_target: &Arc<RenderTarget>,
        render_object: Arc<RenderObject>,
    ) -> RenderUnit {
        let steps = array::from_fn(|index| {
            let material_step = &render_object.material.passes[index];
            if let Some(material_step) = material_step {
                Some(RenderUnitStep::new(context, render_target, material_step))
            } else {
                None
            }
        });

        RenderUnit {
            render_object,
            steps,
        }
    }

    pub fn render(
        &self,
        context: &VulkanContext,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        material_step_type: MaterialStepType,
        globals: &Globals,
    ) {
        let index = material_step_type as usize;
        match (
            &self.steps[index],
            &self.render_object.material.passes[index],
        ) {
            (Some(component), Some(material_step)) => {
                component.render(
                    context,
                    builder,
                    material_step,
                    &self.render_object.mesh,
                    globals,
                );
            }
            (None, None) => {}
            _ => panic!("RenderUnitStep and MaterialStep mismatch"),
        }
    }
}

impl RenderUnitStep {
    pub fn new(
        context: &VulkanContext,
        render_target: &RenderTarget,
        material_step: &MaterialStep,
    ) -> RenderUnitStep {
        let vertex_uniforms_storage = ShaderUniformStorage::new(context);
        let fragment_uniforms_storage = ShaderUniformStorage::new(context);

        let depth = if material_step.depth_test || material_step.depth_write {
            let compare_op =
                if material_step.depth_test { CompareOp::Less } else { CompareOp::Always };
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
            .render_pass(Subpass::from(render_target.render_pass.clone(), 0).unwrap())
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
        context: &VulkanContext,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        material_step: &MaterialStep,
        mesh: &Mesh,
        globals: &Globals,
    ) {
        let descriptor_set_layouts = self.pipeline.layout().set_layouts();
        let vertex_descriptor_set = self.vertex_uniforms_storage.make_descriptor_set(
            context,
            &material_step.vertex_shader,
            descriptor_set_layouts
                .get(ShaderKind::Vertex as usize)
                .unwrap(),
            globals,
        );
        let fragment_descriptor_set = self.fragment_uniforms_storage.make_descriptor_set(
            context,
            &material_step.fragment_shader,
            descriptor_set_layouts
                .get(ShaderKind::Fragment as usize)
                .unwrap(),
            globals,
        );

        builder
            .bind_pipeline_graphics(self.pipeline.clone())
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
            .bind_vertex_buffers(0, mesh.vertex_buffer.clone())
            .draw(mesh.vertex_buffer.len().try_into().unwrap(), 1, 0, 0)
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
        }
    }

    fn make_descriptor_set(
        &self,
        context: &VulkanContext,
        shader: &Shader,
        layout: &Arc<DescriptorSetLayout>,
        globals: &Globals,
    ) -> Arc<PersistentDescriptorSet> {
        // TODO: avoid memory allocation, maybe use tinyvec
        let mut descriptors = vec![];

        if shader.uniform_buffer_size > 0 {
            // Fill uniform array
            let mut uniform_values = [0.0f32; MAX_UNIFORMS_F32_COUNT];
            for global_mapping in &shader.global_uniform_bindings {
                let values = globals.get(global_mapping.global_type);
                // TODO: store f32 offset instead of byte offset
                let offset = global_mapping.offset / size_of::<f32>();
                for (i, value) in values.iter().enumerate() {
                    uniform_values[offset + i] = *value;
                }
            }
            for local_mapping in &shader.local_uniform_bindings {
                for i in 0..local_mapping.f32_count {
                    uniform_values[local_mapping.f32_offset + i] =
                        local_mapping.control.get_value(i, 0.0);
                }
            }
            let _value_count = shader.uniform_buffer_size / size_of::<f32>();
            let uniform_buffer_subbuffer =
                self.uniform_buffer_pool.from_data(uniform_values).unwrap();
            descriptors.push(WriteDescriptorSet::buffer(0, uniform_buffer_subbuffer));
        }

        for texture_binding in &shader.texture_bindings {
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
            descriptors.push(WriteDescriptorSet::image_view_sampler(
                texture_binding.descriptor_set_binding,
                texture_binding.texture.clone(),
                sampler,
            ));
        }

        let persistent_descriptor_set = PersistentDescriptorSet::new(
            &context.descriptor_set_allocator,
            layout.clone(),
            descriptors,
        )
        .unwrap();

        persistent_descriptor_set
    }
}
