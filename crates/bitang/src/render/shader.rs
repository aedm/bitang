use crate::control::controls::{Control, GlobalType};
use crate::render::buffer::Buffer;
use crate::render::buffer_generator::BufferGenerator;
use crate::render::image::BitangImage;
use crate::tool::{RenderContext, VulkanContext};
use anyhow::{Context, Result};
use smallvec::SmallVec;
use std::mem::size_of;
use std::rc::Rc;
use std::sync::Arc;
use tracing::warn;
use vulkano::buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo};
use vulkano::buffer::BufferUsage;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::image::sampler::{BorderColor, Sampler, SamplerCreateInfo};
use vulkano::memory::allocator::MemoryTypeFilter;
use vulkano::pipeline::graphics::depth_stencil::CompareOp;
use vulkano::pipeline::{PipelineBindPoint, PipelineLayout};
use vulkano::shader::ShaderModule;

const MAX_UNIFORMS_F32_COUNT: usize = 1024;

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub enum ShaderKind {
    Vertex = 0,
    Fragment = 1,
    Compute = 2,
}

impl ShaderKind {
    /// Returns the descriptor set index for this shader stage.
    ///
    /// Descriptor sets are shared between all shader stages in a pipeline. To distinguish,
    /// Bitang uses a unique descriptor set for each shader stage:
    /// - Vertex shader: `set = 0`
    /// - Fragment shader: `set = 1`
    /// - Compute shader: `set = 0`
    ///
    /// E.g. every resource in a vertex shader must use `layout(set = 0, binding = ...)`,
    pub fn get_descriptor_set_index(&self) -> u32 {
        match self {
            ShaderKind::Vertex => 0,
            ShaderKind::Fragment => 1,
            ShaderKind::Compute => 0,
        }
    }
}

/// Shader stage, either vertex or fragment.
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

    /// Descriptor bindings, e.g. samplers, buffers, etc.
    pub descriptor_resources: Vec<DescriptorResource>,

    /// Storage for the uniform buffer values    
    uniform_buffer_pool: SubbufferAllocator,
}

impl Shader {
    pub fn new(
        context: &Arc<VulkanContext>,
        shader_module: Arc<ShaderModule>,
        kind: ShaderKind,
        global_uniform_bindings: Vec<GlobalUniformMapping>,
        local_uniform_bindings: Vec<LocalUniformMapping>,
        uniform_buffer_size: usize,
        descriptor_resources: Vec<DescriptorResource>,
    ) -> Shader {
        let uniform_buffer_pool = SubbufferAllocator::new(
            context.memory_allocator.clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::UNIFORM_BUFFER,
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
        );

        Shader {
            shader_module,
            kind,
            global_uniform_bindings,
            local_uniform_bindings,
            uniform_buffer_size,
            descriptor_resources,
            uniform_buffer_pool,
        }
    }

    pub fn bind(
        &self,
        context: &mut RenderContext,
        pipeline_layout: &Arc<PipelineLayout>,
    ) -> Result<()> {
        if self.uniform_buffer_size == 0 && self.descriptor_resources.is_empty() {
            return Ok(());
        }

        let descriptor_set_layout = pipeline_layout
            .set_layouts()
            .get(self.kind.get_descriptor_set_index() as usize)
            .context("Failed to get descriptor set layout")?;

        let mut descriptors = SmallVec::<[_; 64]>::new();

        if self.uniform_buffer_size > 0 {
            // Fill uniform array
            let mut uniform_values = [0.0f32; MAX_UNIFORMS_F32_COUNT];
            for global_mapping in &self.global_uniform_bindings {
                let values = context.globals.get(global_mapping.global_type);
                let start_index = global_mapping.f32_offset;
                for (i, value) in values.iter().enumerate() {
                    uniform_values[start_index + i] = *value;
                }
            }
            for local_mapping in &self.local_uniform_bindings {
                let components = local_mapping.control.components.borrow();
                for i in 0..local_mapping.f32_count {
                    uniform_values[local_mapping.f32_offset + i] = components[i].value;
                }
            }
            let _value_count = self.uniform_buffer_size / size_of::<f32>();
            // Unwrap is okay: we want to panic if we can't allocate
            let uniform_buffer_subbuffer = self.uniform_buffer_pool.allocate_sized().unwrap();
            *uniform_buffer_subbuffer.write().unwrap() = uniform_values;

            // Uniforms are always at binding 0
            descriptors.push(WriteDescriptorSet::buffer(0, uniform_buffer_subbuffer));
        }

        for descriptor_resource in &self.descriptor_resources {
            let write_descriptor_set = match &descriptor_resource.source {
                DescriptorSource::Image(image_descriptor) => {
                    let image_view = image_descriptor.image.get_view_for_sampler()?;
                    WriteDescriptorSet::image_view(descriptor_resource.binding, image_view)
                }
                DescriptorSource::Sampler(sampler_descriptor) => {
                    let sampler = Sampler::new(
                        context.vulkan_context.device.clone(),
                        SamplerCreateInfo {
                            address_mode: sampler_descriptor.mode.to_vulkano_address_mode(),
                            compare: sampler_descriptor.mode.to_vulkano_compare_op(),
                            border_color: BorderColor::FloatOpaqueWhite,
                            ..SamplerCreateInfo::simple_repeat_linear()
                        },
                    )?;

                    WriteDescriptorSet::sampler(descriptor_resource.binding, sampler)
                }
                DescriptorSource::BufferGenerator(buffer_generator) => {
                    let buffer = buffer_generator.get_buffer().with_context(|| {
                        format!(
                            "Failed to get buffer for buffer generator for binding {}",
                            descriptor_resource.id
                        )
                    })?;
                    WriteDescriptorSet::buffer(descriptor_resource.binding, buffer.clone())
                }
                DescriptorSource::BufferCurrent(buffer) => WriteDescriptorSet::buffer(
                    descriptor_resource.binding,
                    buffer.get_current_buffer(),
                ),
                DescriptorSource::BufferNext(buffer) => WriteDescriptorSet::buffer(
                    descriptor_resource.binding,
                    buffer.get_next_buffer(),
                ),
            };
            descriptors.push(write_descriptor_set);
        }

        let persistent_descriptor_set = PersistentDescriptorSet::new(
            &context.vulkan_context.descriptor_set_allocator,
            descriptor_set_layout.clone(),
            descriptors,
            [],
        )?;

        let pipeline_bind_point = match self.kind {
            ShaderKind::Vertex => PipelineBindPoint::Graphics,
            ShaderKind::Fragment => PipelineBindPoint::Graphics,
            ShaderKind::Compute => PipelineBindPoint::Compute,
        };

        context.command_builder.bind_descriptor_sets(
            pipeline_bind_point,
            pipeline_layout.clone(),
            self.kind.get_descriptor_set_index(),
            persistent_descriptor_set,
        )?;

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum SamplerMode {
    Repeat,
    ClampToEdge,
    MirroredRepeat,
    Envmap,
    Shadow,
}

impl SamplerMode {
    pub fn to_vulkano_compare_op(&self) -> Option<CompareOp> {
        match self {
            SamplerMode::Shadow => Some(CompareOp::Less),
            _ => None,
        }
    }

    pub fn to_vulkano_address_mode(&self) -> [vulkano::image::sampler::SamplerAddressMode; 3] {
        match self {
            SamplerMode::Repeat => [
                vulkano::image::sampler::SamplerAddressMode::Repeat,
                vulkano::image::sampler::SamplerAddressMode::Repeat,
                vulkano::image::sampler::SamplerAddressMode::Repeat,
            ],
            SamplerMode::MirroredRepeat => [
                vulkano::image::sampler::SamplerAddressMode::MirroredRepeat,
                vulkano::image::sampler::SamplerAddressMode::MirroredRepeat,
                vulkano::image::sampler::SamplerAddressMode::MirroredRepeat,
            ],
            SamplerMode::ClampToEdge => [
                vulkano::image::sampler::SamplerAddressMode::ClampToEdge,
                vulkano::image::sampler::SamplerAddressMode::ClampToEdge,
                vulkano::image::sampler::SamplerAddressMode::ClampToEdge,
            ],
            SamplerMode::Envmap => [
                vulkano::image::sampler::SamplerAddressMode::Repeat,
                vulkano::image::sampler::SamplerAddressMode::ClampToEdge,
                vulkano::image::sampler::SamplerAddressMode::ClampToEdge,
            ],
            SamplerMode::Shadow => [
                vulkano::image::sampler::SamplerAddressMode::ClampToBorder,
                vulkano::image::sampler::SamplerAddressMode::ClampToBorder,
                vulkano::image::sampler::SamplerAddressMode::ClampToEdge,
            ],
        }
    }
}

#[derive(Clone)]
pub struct ImageDescriptor {
    pub image: Arc<BitangImage>,
}

#[derive(Clone)]
pub struct SamplerDescriptor {
    pub mode: SamplerMode,
}

#[derive(Clone)]
pub enum DescriptorSource {
    Image(ImageDescriptor),
    Sampler(SamplerDescriptor),
    BufferGenerator(Rc<BufferGenerator>),
    BufferCurrent(Rc<Buffer>),
    BufferNext(Rc<Buffer>),
}

pub struct DescriptorResource {
    pub id: String,

    /// This is value of the `layout(binding = ...)` attribute in the shader.
    pub binding: u32,

    pub source: DescriptorSource,
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
    pub f32_offset: usize,
}
