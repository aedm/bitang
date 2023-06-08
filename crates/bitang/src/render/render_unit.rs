use crate::render::material::{
    BlendMode, DescriptorSource, MaterialStep, MaterialStepType, Shader, ShaderKind,
    MATERIAL_STEP_COUNT,
};
use crate::render::mesh::Mesh;
use crate::render::vulkan_window::{RenderContext, VulkanContext};
use crate::render::{RenderObject, Vertex3};
use anyhow::{Context, Result};
use glam::{EulerRot, Mat4};
use std::mem::size_of;
use std::sync::Arc;
use std::{array, mem};
use vulkano::buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo};
use vulkano::buffer::BufferUsage;
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::image::ImageViewAbstract;
use vulkano::pipeline::graphics::color_blend::{
    AttachmentBlend, BlendFactor, BlendOp, ColorBlendState,
};
use vulkano::pipeline::graphics::depth_stencil::{CompareOp, DepthState, DepthStencilState};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::Vertex;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::{GraphicsPipeline, Pipeline, PipelineBindPoint, StateMode};
use vulkano::render_pass::Subpass;
use vulkano::sampler::{Sampler, SamplerAddressMode, SamplerCreateInfo};

// TODO: use a dynamically sized ring buffer for uniforms instead
const MAX_UNIFORMS_F32_COUNT: usize = 1024;

pub struct RenderUnit {
    render_object: Arc<RenderObject>,
    steps: [Option<RenderUnitStep>; MATERIAL_STEP_COUNT],
}

/// Represents a material stage, eg. 'solid' or 'shadow' stage.
///
/// Each stage renders the same mesh with possibly different shaders and different uniforms,
/// maybe even into different render targets.
struct RenderUnitStep {
    pipeline: Arc<GraphicsPipeline>,
    vertex_uniforms_storage: ShaderUniformStorage,
    fragment_uniforms_storage: ShaderUniformStorage,
}

struct ShaderUniformStorage {
    uniform_buffer_pool: SubbufferAllocator,
}

impl RenderUnit {
    pub fn new(
        context: &VulkanContext,
        render_pass: &Arc<vulkano::render_pass::RenderPass>,
        render_object: &Arc<RenderObject>,
    ) -> Result<RenderUnit> {
        let mut steps = render_object
            .material
            .passes
            .iter()
            .map(|material_step| {
                // let material_step = &render_object.material.passes[index];
                if let Some(material_step) = material_step {
                    Ok(Some(
                        RenderUnitStep::new(context, render_pass, material_step).with_context(
                            || {
                                format!(
                                    "Failed to create RenderUnitStep for object {}",
                                    render_object.id
                                )
                            },
                        )?,
                    ))
                } else {
                    Ok(None)
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let steps = array::from_fn(|index| mem::take(&mut steps[index]));

        Ok(RenderUnit {
            render_object: render_object.clone(),
            steps,
        })
    }

    pub fn render(
        &self,
        context: &mut RenderContext,
        material_step_type: MaterialStepType,
    ) -> Result<()> {
        let saved_globals = context.globals;
        self.apply_transformations(context);

        let index = material_step_type as usize;
        let (Some(component), Some(material_step)) = (
            &self.steps[index],
            &self.render_object.material.passes[index],
        ) else {
            panic!("RenderUnitStep and MaterialStep mismatch");
        };
        let instance_count = self.render_object.instances.as_float().round() as u32;
        let result = component.render(
            context,
            material_step,
            &self.render_object.mesh,
            instance_count,
        );
        context.globals = saved_globals;

        result
    }

    fn apply_transformations(&self, context: &mut RenderContext) {
        let rotation = self.render_object.rotation.as_vec3();
        let rotation_matrix = Mat4::from_euler(EulerRot::ZXY, rotation.z, rotation.x, rotation.y);

        let position = self.render_object.position.as_vec3();
        let translation_matrix = Mat4::from_translation(position);

        context.globals.world_from_model = translation_matrix * rotation_matrix;
        context.globals.update_compound_matrices();
    }
}

impl RenderUnitStep {
    pub fn new(
        context: &VulkanContext,
        render_pass: &Arc<vulkano::render_pass::RenderPass>,
        material_step: &MaterialStep,
    ) -> Result<RenderUnitStep> {
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

        let mut color_blend_state = ColorBlendState::new(1);
        match material_step.blend_mode {
            BlendMode::None => {}
            BlendMode::Alpha => {
                color_blend_state = color_blend_state.blend_alpha();
            }
            BlendMode::Additive => {
                color_blend_state = color_blend_state.blend(AttachmentBlend {
                    color_op: BlendOp::Add,
                    color_source: BlendFactor::SrcAlpha,
                    color_destination: BlendFactor::One,
                    alpha_op: BlendOp::Max,
                    alpha_source: BlendFactor::One,
                    alpha_destination: BlendFactor::One,
                });
            }
        };

        let pipeline = GraphicsPipeline::start()
            .vertex_input_state(Vertex3::per_vertex())
            .vertex_shader(
                material_step
                    .vertex_shader
                    .shader_module
                    .entry_point("main")
                    .context("Failed to get vertex shader entry point")?,
                (),
            )
            .input_assembly_state(InputAssemblyState::new())
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .fragment_shader(
                material_step
                    .fragment_shader
                    .shader_module
                    .entry_point("main")
                    .context("Failed to get fragment shader entry point")?,
                (),
            )
            .color_blend_state(color_blend_state)
            .depth_stencil_state(depth_stencil_state)
            // Unwrap is safe: every pass has one subpass
            .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
            .build(context.context.device().clone())?;

        Ok(RenderUnitStep {
            pipeline,
            vertex_uniforms_storage,
            fragment_uniforms_storage,
        })
    }

    pub fn render(
        &self,
        context: &mut RenderContext,
        material_step: &MaterialStep,
        mesh: &Mesh,
        instance_count: u32,
    ) -> Result<()> {
        context.globals.instance_count = instance_count as f32;

        let descriptor_set_layouts = self.pipeline.layout().set_layouts();
        let vertex_descriptor_set = self.vertex_uniforms_storage.make_descriptor_set(
            context,
            &material_step.vertex_shader,
            descriptor_set_layouts.get(ShaderKind::Vertex as usize),
            material_step.sampler_address_mode,
        )?;
        let fragment_descriptor_set = self.fragment_uniforms_storage.make_descriptor_set(
            context,
            &material_step.fragment_shader,
            descriptor_set_layouts.get(ShaderKind::Fragment as usize),
            material_step.sampler_address_mode,
        )?;

        context
            .command_builder
            .bind_pipeline_graphics(self.pipeline.clone())
            .bind_vertex_buffers(0, mesh.vertex_buffer.clone());
        if let Some(vertex_descriptor_set) = vertex_descriptor_set {
            context.command_builder.bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline.layout().clone(),
                0,
                vertex_descriptor_set,
            );
        }
        if let Some(fragment_descriptor_set) = fragment_descriptor_set {
            context.command_builder.bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline.layout().clone(),
                1,
                fragment_descriptor_set,
            );
        }
        context
            .command_builder
            .draw(mesh.vertex_buffer.len() as u32, instance_count, 0, 0)?;
        Ok(())
    }
}

impl ShaderUniformStorage {
    pub fn new(context: &VulkanContext) -> ShaderUniformStorage {
        let uniform_buffer_pool = SubbufferAllocator::new(
            context.context.memory_allocator().clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::UNIFORM_BUFFER,
                ..Default::default()
            },
        );
        ShaderUniformStorage {
            uniform_buffer_pool,
        }
    }

    fn make_descriptor_set(
        &self,
        context: &RenderContext,
        shader: &Shader,
        layout: Option<&Arc<DescriptorSetLayout>>,
        sampler_address_mode: SamplerAddressMode,
    ) -> Result<Option<Arc<PersistentDescriptorSet>>> {
        let Some(layout) = layout else {return Ok(None);};

        // TODO: avoid memory allocation, maybe use tinyvec
        let mut descriptors = vec![];

        if shader.uniform_buffer_size > 0 {
            // Fill uniform array
            let mut uniform_values = [0.0f32; MAX_UNIFORMS_F32_COUNT];
            for global_mapping in &shader.global_uniform_bindings {
                let values = context.globals.get(global_mapping.global_type);
                // TODO: store f32 offset instead of byte offset
                let offset = global_mapping.offset / size_of::<f32>();
                for (i, value) in values.iter().enumerate() {
                    uniform_values[offset + i] = *value;
                }
            }
            for local_mapping in &shader.local_uniform_bindings {
                let components = local_mapping.control.components.borrow();
                for i in 0..local_mapping.f32_count {
                    uniform_values[local_mapping.f32_offset + i] = components[i].value;
                }
            }
            let _value_count = shader.uniform_buffer_size / size_of::<f32>();
            // Unwrap is okay: we want to panic if we can't allocate
            let uniform_buffer_subbuffer = self.uniform_buffer_pool.allocate_sized().unwrap();
            *uniform_buffer_subbuffer.write().unwrap() = uniform_values;
            descriptors.push(WriteDescriptorSet::buffer(0, uniform_buffer_subbuffer));
        }

        for descriptor_binding in &shader.descriptor_bindings {
            let write_descriptor_set = match &descriptor_binding.descriptor_source {
                DescriptorSource::Texture(texture) => Self::make_sampler(
                    context,
                    texture.clone(),
                    descriptor_binding.descriptor_set_binding,
                    sampler_address_mode,
                ),
                DescriptorSource::RenderTarget(render_target) => {
                    let image_borrow = render_target.image.borrow();
                    let render_target_image = image_borrow.as_ref().unwrap();
                    let image_view = render_target_image.image_view.clone();
                    Self::make_sampler(
                        context,
                        image_view,
                        descriptor_binding.descriptor_set_binding,
                        sampler_address_mode,
                    )
                }
                DescriptorSource::BufferGenerator(buffer_generator) => {
                    let buffer = buffer_generator.get_buffer().with_context(|| {
                        format!(
                            "Failed to get buffer for buffer generator at binding {}",
                            descriptor_binding.descriptor_set_binding
                        )
                    })?;
                    Ok(WriteDescriptorSet::buffer(
                        descriptor_binding.descriptor_set_binding,
                        buffer.clone(),
                    ))
                }
            }?;
            descriptors.push(write_descriptor_set);
        }

        if descriptors.is_empty() {
            return Ok(None);
        }

        let persistent_descriptor_set = PersistentDescriptorSet::new(
            &context.vulkan_context.descriptor_set_allocator,
            layout.clone(),
            descriptors,
        )?;

        Ok(Some(persistent_descriptor_set))
    }

    fn make_sampler(
        context: &RenderContext,
        image_view: Arc<dyn ImageViewAbstract>,
        binding: u32,
        address_mode: SamplerAddressMode,
    ) -> Result<WriteDescriptorSet> {
        let sampler = Sampler::new(
            context.vulkan_context.context.device().clone(),
            SamplerCreateInfo {
                address_mode: [address_mode; 3],
                ..SamplerCreateInfo::simple_repeat_linear()
            },
        )?;

        Ok(WriteDescriptorSet::image_view_sampler(
            binding, image_view, sampler,
        ))
    }
}
