use crate::control::controls::{Control, GlobalType};
use crate::render::buffer_generator::BufferGenerator;
use crate::render::image::Image;
use crate::render::vulkan_window::RenderContext;
use anyhow::{Context, Result};
use std::mem::size_of;
use std::rc::Rc;
use std::sync::Arc;
use vulkano::buffer::allocator::SubbufferAllocator;
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::pipeline::{PipelineBindPoint, PipelineLayout};
use vulkano::sampler::SamplerAddressMode;
use vulkano::shader::ShaderModule;

const MAX_UNIFORMS_F32_COUNT: usize = 1024;

#[derive(Clone)]
pub enum ShaderKind {
    Vertex = 0,
    Fragment = 1,
}

impl ShaderKind {
    /// Returns the descriptor set index for this shader stage.
    /// 
    /// Descriptor sets are shared between all shader stages in a pipeline. To distinguish,
    /// Bitang uses unique descriptor set for each shader stage:
    /// - Vertex shader: `set = 0`
    /// - Fragment shader: `set = 1`
    ///
    /// E.g. every resource in a vertex shader must use `layout(set = 0, binding = ...)`,
    pub fn get_descriptor_set_index(&self) -> u32 {
        self as u32
    }
}

/// Shader stage, either vertex or fragment.
#[derive(Clone)]
pub struct Shader {
    /// The compiled shader module.
    pub shader_module: Arc<ShaderModule>,

    /// The kind of shader, either vertex or fragment.
    pub kind: ShaderKind,

    /// The value global uniforms are taken from a `Globals` struct.
    pub global_uniform_bindings: Vec<GlobalUniformMapping>,

    /// The value local uniforms are taken from a `Control` struct.
    pub local_uniform_bindings: Vec<LocalUniformMapping>,

    /// The size of the uniform buffer in bytes.
    pub uniform_buffer_size: usize,

    /// Image bindings
    pub samplers: Vec<SamplerBinding>,

    /// Buffer bindings
    pub buffers: Vec<BufferBinding>,

    /// Storage for the uniform buffer values    
    uniform_buffer_pool: SubbufferAllocator,

    layout: Option<Arc<DescriptorSetLayout>>,
}

impl Shader {
    pub fn bind(
        &self,
        context: &RenderContext,
        pipeline_layout: Arc<PipelineLayout>,
    ) -> Result<()> {
        let Some(layout) = &self.layout else {return Ok(None);};

        // TODO: avoid memory allocation, maybe use tinyvec
        let mut descriptors = vec![];

        if self.uniform_buffer_size > 0 {
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
                DescriptorSource::Image(render_target) => {
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

        // Ok(Some(persistent_descriptor_set))
        context.command_builder.bind_descriptor_sets(
            PipelineBindPoint::Graphics,
            pipeline_layout,
            self.kind.get_descriptor_set_index(),
            persistent_descriptor_set,
        );

        Ok(())
    }
}

/// The binding point of a descriptor.
/// This is value of the `layout(binding = ...)` attribute in the shader.
pub type DescriptorBinding = u32;

#[derive(Clone)]
pub struct SamplerBinding {
    pub id: String,
    pub binding: DescriptorBinding,
    pub image: Arc<Image>,
    pub address_mode: SamplerAddressMode,
}

#[derive(Clone)]
pub struct BufferBinding {
    pub id: String,
    pub binding: DescriptorBinding,
    pub buffer_generator: Arc<BufferGenerator>,
}

#[derive(Clone)]
pub struct LocalUniformMapping {
    pub control: Rc<Control>,
    pub f32_count: usize,
    pub f32_offset: usize,
}

#[derive(Copy, Clone, Debug)]
pub struct GlobalUniformMapping {
    pub global_type: GlobalType,
    pub offset: usize,
}

struct ShaderUniformStorage {}
