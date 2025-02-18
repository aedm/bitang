use crate::control::controls::{Control, GlobalType};
use crate::render::buffer::Buffer;
// use crate::render::buffer_generator::BufferGenerator;
use crate::render::image::BitangImage;
use crate::tool::{FrameContext, GpuContext, RenderPassContext, WindowContext};
use anyhow::{Context, Result};
use smallvec::SmallVec;
use std::mem::size_of;
use std::rc::Rc;
use std::sync::Arc;
use tracing::warn;

use super::image::PixelFormat;
// use vulkano::buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo};
// use vulkano::buffer::BufferUsage;
// use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
// use vulkano::image::sampler::{BorderColor, Sampler, SamplerCreateInfo};
// use vulkano::memory::allocator::MemoryTypeFilter;
// use vulkano::pipeline::graphics::depth_stencil::CompareOp;
// use vulkano::pipeline::{PipelineBindPoint, PipelineLayout};
// use vulkano::shader::ShaderModule;

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
    pub shader_module: Arc<wgpu::ShaderModule>,

    /// The kind of shader, either vertex or fragment.
    pub kind: ShaderKind,

    // TODO: merge local and global uniform sources
    /// The value global uniforms are taken from a `Globals` struct.
    pub global_uniform_bindings: Vec<GlobalUniformMapping>,

    /// The value local uniforms are taken from a `Control` struct.
    pub local_uniform_bindings: Vec<LocalUniformMapping>,

    /// The size of the uniform buffer in bytes.
    pub uniform_buffer_size: usize,

    /// Descriptor bindings, e.g. samplers, buffers, etc.
    pub descriptor_resources: Vec<DescriptorResource>,

    /// Storage for the uniform buffer values    
    uniform_buffer: wgpu::Buffer,

    ///
    bind_group_layout: wgpu::BindGroupLayout,
}

impl Shader {
    pub fn new(
        context: &GpuContext,
        shader_module: Arc<wgpu::ShaderModule>,
        kind: ShaderKind,
        global_uniform_bindings: Vec<GlobalUniformMapping>,
        local_uniform_bindings: Vec<LocalUniformMapping>,
        uniform_buffer_size: usize,
        descriptor_resources: Vec<DescriptorResource>,
    ) -> Shader {
        // let uniform_buffer_pool = SubbufferAllocator::new(
        //     context.memory_allocator.clone(),
        //     SubbufferAllocatorCreateInfo {
        //         buffer_usage: BufferUsage::UNIFORM_BUFFER,
        //         memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
        //             | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
        //         ..Default::default()
        //     },
        // );
        let uniform_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: uniform_buffer_size as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut entries = vec![];

        let visibility = match kind {
            ShaderKind::Vertex => wgpu::ShaderStages::VERTEX,
            ShaderKind::Fragment => wgpu::ShaderStages::FRAGMENT,
            ShaderKind::Compute => wgpu::ShaderStages::COMPUTE,
        };

        if uniform_buffer_size > 0 {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
        }   

        for descriptor_resource in &descriptor_resources {
            // TODO: just the resource instead of BindGroupEntry
            let ty = match &descriptor_resource.source {
                DescriptorSource::Image(image_descriptor) => {
                    let sample_type = match image_descriptor.image.pixel_format {
                        PixelFormat::Depth32F => wgpu::TextureSampleType::Depth,
                        _ => wgpu::TextureSampleType::Float { filterable: true },
                    };
                    wgpu::BindingType::Texture {
                        sample_type,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    }
                }
                DescriptorSource::Sampler(sampler_descriptor) => {
                    let filtering = match sampler_descriptor.mode.to_wgpu_compare_op() {
                        None => wgpu::SamplerBindingType::Filtering,
                        _ => wgpu::SamplerBindingType::Comparison,
                    };
                    wgpu::BindingType::Sampler(filtering)
                    // WriteDescriptorSet::sampler(descriptor_resource.binding, sampler)
                }
                DescriptorSource::BufferCurrent(buffer) => wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage {
                        read_only: true,
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                DescriptorSource::BufferNext(buffer) => wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage {
                        read_only: false,
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            };
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility,
                ty,
                count: None,
            });
        }     

        let bind_group_layout = context.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &entries,
        });

        Shader {
            shader_module,
            kind,
            global_uniform_bindings,
            local_uniform_bindings,
            uniform_buffer_size,
            descriptor_resources,
            uniform_buffer,
            bind_group_layout,
        }
    }

    pub fn bind(
        &self,
        context: &mut RenderPassContext<'_>,
        // pipeline_layout: &Arc<PipelineLayout>,
    ) -> Result<()> {
        if self.uniform_buffer_size == 0 && self.descriptor_resources.is_empty() {
            return Ok(());
        }

        let mut entries = SmallVec::<[_; 64]>::new();


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
            let value_count = self.uniform_buffer_size / size_of::<f32>();
            context.gpu_context.queue.write_buffer(
                &self.uniform_buffer,
                0,
                bytemuck::cast_slice(&uniform_values[..value_count]),
            );
            // Uniforms are always at binding 0
            entries.push(wgpu::BindGroupEntry {
                binding: 0,
                resource: self.uniform_buffer.as_entire_binding(),
            });
        }

        for descriptor_resource in &self.descriptor_resources {
            // TODO: just the resource instead of BindGroupEntry
            let write_descriptor_set = match &descriptor_resource.source {
                DescriptorSource::Image(image_descriptor) => {
                    let image_view = image_descriptor.image.get_view_for_sampler()?;
                    wgpu::BindGroupEntry {
                        binding: descriptor_resource.binding,
                        resource: wgpu::BindingResource::TextureView(&image_view),
                    }
                }
                DescriptorSource::Sampler(sampler_descriptor) => {
                    // let sampler = Sampler::new(
                    //     context.vulkan_context.device.clone(),
                    //     SamplerCreateInfo {
                    //         address_mode: sampler_descriptor.mode.to_vulkano_address_mode(),
                    //         compare: sampler_descriptor.mode.to_vulkano_compare_op(),
                    //         border_color: BorderColor::FloatOpaqueWhite,
                    //         ..SamplerCreateInfo::simple_repeat_linear()
                    //     },
                    // )?;
                    wgpu::BindGroupEntry {
                        binding: descriptor_resource.binding,
                        resource: wgpu::BindingResource::Sampler(sampler_descriptor.sampler()),
                    }
                    // WriteDescriptorSet::sampler(descriptor_resource.binding, sampler)
                }
                // DescriptorSource::BufferGenerator(buffer_generator) => {
                //     let buffer = buffer_generator.get_buffer().with_context(|| {
                //         format!(
                //             "Failed to get buffer for buffer generator for binding {}",
                //             descriptor_resource.id
                //         )
                //     })?;
                //     WriteDescriptorSet::buffer(descriptor_resource.binding, buffer.clone())
                // }
                DescriptorSource::BufferCurrent(buffer) => wgpu::BindGroupEntry {
                    binding: descriptor_resource.binding,
                    resource: buffer.get_current_buffer().as_entire_binding(),
                },
                DescriptorSource::BufferNext(buffer) => wgpu::BindGroupEntry {
                    binding: descriptor_resource.binding,
                    resource: buffer.get_next_buffer().as_entire_binding(),
                },
            };
            entries.push(write_descriptor_set);
        }

        let bind_group = context
            .gpu_context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Bind Group"),
                layout: &self.bind_group_layout,
                entries: &entries,
            });


        context.pass.set_bind_group(self.kind.get_descriptor_set_index(), &bind_group, &[]);

        // let pipeline_bind_point = match self.kind {
        //     ShaderKind::Vertex => PipelineBindPoint::Graphics,
        //     ShaderKind::Fragment => PipelineBindPoint::Graphics,
        //     ShaderKind::Compute => PipelineBindPoint::Compute,
        // };

        // context.command_builder.bind_descriptor_sets(
        //     pipeline_bind_point,
        //     pipeline_layout.clone(),
        //     self.kind.get_descriptor_set_index(),
        //     persistent_descriptor_set,
        // )?;

        Ok(())

        // let descriptor_set_layout = pipeline_layout
        //     .set_layouts()
        //     .get(self.kind.get_descriptor_set_index() as usize)
        //     .context("Failed to get descriptor set layout")?;

        // let mut descriptors = SmallVec::<[_; 64]>::new();

        // if self.uniform_buffer_size > 0 {
        //     // Fill uniform array
        //     let mut uniform_values = [0.0f32; MAX_UNIFORMS_F32_COUNT];
        //     for global_mapping in &self.global_uniform_bindings {
        //         let values = context.globals.get(global_mapping.global_type);
        //         let start_index = global_mapping.f32_offset;
        //         for (i, value) in values.iter().enumerate() {
        //             uniform_values[start_index + i] = *value;
        //         }
        //     }
        //     for local_mapping in &self.local_uniform_bindings {
        //         let components = local_mapping.control.components.borrow();
        //         for i in 0..local_mapping.f32_count {
        //             uniform_values[local_mapping.f32_offset + i] = components[i].value;
        //         }
        //     }
        //     let _value_count = self.uniform_buffer_size / size_of::<f32>();
        //     // Unwrap is okay: we want to panic if we can't allocate
        //     let uniform_buffer_subbuffer = self.uniform_buffer_pool.allocate_sized().unwrap();
        //     *uniform_buffer_subbuffer.write().unwrap() = uniform_values;

        //     // Uniforms are always at binding 0
        //     descriptors.push(WriteDescriptorSet::buffer(0, uniform_buffer_subbuffer));
        // }

        // for descriptor_resource in &self.descriptor_resources {
        //     let write_descriptor_set = match &descriptor_resource.source {
        //         DescriptorSource::Image(image_descriptor) => {
        //             let image_view = image_descriptor.image.get_view_for_sampler()?;
        //             WriteDescriptorSet::image_view(descriptor_resource.binding, image_view)
        //         }
        //         DescriptorSource::Sampler(sampler_descriptor) => {
        //             let sampler = Sampler::new(
        //                 context.vulkan_context.device.clone(),
        //                 SamplerCreateInfo {
        //                     address_mode: sampler_descriptor.mode.to_vulkano_address_mode(),
        //                     compare: sampler_descriptor.mode.to_vulkano_compare_op(),
        //                     border_color: BorderColor::FloatOpaqueWhite,
        //                     ..SamplerCreateInfo::simple_repeat_linear()
        //                 },
        //             )?;

        //             WriteDescriptorSet::sampler(descriptor_resource.binding, sampler)
        //         }
        //         DescriptorSource::BufferGenerator(buffer_generator) => {
        //             let buffer = buffer_generator.get_buffer().with_context(|| {
        //                 format!(
        //                     "Failed to get buffer for buffer generator for binding {}",
        //                     descriptor_resource.id
        //                 )
        //             })?;
        //             WriteDescriptorSet::buffer(descriptor_resource.binding, buffer.clone())
        //         }
        //         DescriptorSource::BufferCurrent(buffer) => WriteDescriptorSet::buffer(
        //             descriptor_resource.binding,
        //             buffer.get_current_buffer(),
        //         ),
        //         DescriptorSource::BufferNext(buffer) => WriteDescriptorSet::buffer(
        //             descriptor_resource.binding,
        //             buffer.get_next_buffer(),
        //         ),
        //     };
        //     descriptors.push(write_descriptor_set);
        // }

        // let persistent_descriptor_set = PersistentDescriptorSet::new(
        //     &context.vulkan_context.descriptor_set_allocator,
        //     descriptor_set_layout.clone(),
        //     descriptors,
        //     [],
        // )?;

        // let pipeline_bind_point = match self.kind {
        //     ShaderKind::Vertex => PipelineBindPoint::Graphics,
        //     ShaderKind::Fragment => PipelineBindPoint::Graphics,
        //     ShaderKind::Compute => PipelineBindPoint::Compute,
        // };

        // context.command_builder.bind_descriptor_sets(
        //     pipeline_bind_point,
        //     pipeline_layout.clone(),
        //     self.kind.get_descriptor_set_index(),
        //     persistent_descriptor_set,
        // )?;

        // Ok(())
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
    // pub fn to_vulkano_compare_op(&self) -> Option<CompareOp> {
    //     match self {
    //         SamplerMode::Shadow => Some(CompareOp::Less),
    //         _ => None,
    //     }
    // }

    pub fn to_wgpu_compare_op(&self) -> Option<wgpu::CompareFunction> {
        match self {
            SamplerMode::Shadow => Some(wgpu::CompareFunction::Less),
            _ => None,
        }
    }

    // pub fn to_vulkano_address_mode(&self) -> [vulkano::image::sampler::SamplerAddressMode; 3] {
    //     match self {
    //         SamplerMode::Repeat => [
    //             vulkano::image::sampler::SamplerAddressMode::Repeat,
    //             vulkano::image::sampler::SamplerAddressMode::Repeat,
    //             vulkano::image::sampler::SamplerAddressMode::Repeat,
    //         ],
    //         SamplerMode::MirroredRepeat => [
    //             vulkano::image::sampler::SamplerAddressMode::MirroredRepeat,
    //             vulkano::image::sampler::SamplerAddressMode::MirroredRepeat,
    //             vulkano::image::sampler::SamplerAddressMode::MirroredRepeat,
    //         ],
    //         SamplerMode::ClampToEdge => [
    //             vulkano::image::sampler::SamplerAddressMode::ClampToEdge,
    //             vulkano::image::sampler::SamplerAddressMode::ClampToEdge,
    //             vulkano::image::sampler::SamplerAddressMode::ClampToEdge,
    //         ],
    //         SamplerMode::Envmap => [
    //             vulkano::image::sampler::SamplerAddressMode::Repeat,
    //             vulkano::image::sampler::SamplerAddressMode::ClampToEdge,
    //             vulkano::image::sampler::SamplerAddressMode::ClampToEdge,
    //         ],
    //         SamplerMode::Shadow => [
    //             vulkano::image::sampler::SamplerAddressMode::ClampToBorder,
    //             vulkano::image::sampler::SamplerAddressMode::ClampToBorder,
    //             vulkano::image::sampler::SamplerAddressMode::ClampToEdge,
    //         ],
    //     }
    // }

    pub fn to_wgpu_address_mode(&self) -> [wgpu::AddressMode; 3] {
        match self {
            SamplerMode::Repeat => [
                wgpu::AddressMode::Repeat,
                wgpu::AddressMode::Repeat,
                wgpu::AddressMode::Repeat,
            ],
            SamplerMode::MirroredRepeat => [
                wgpu::AddressMode::MirrorRepeat,
                wgpu::AddressMode::MirrorRepeat,
                wgpu::AddressMode::MirrorRepeat,
            ],
            SamplerMode::ClampToEdge => [
                wgpu::AddressMode::ClampToEdge,
                wgpu::AddressMode::ClampToEdge,
                wgpu::AddressMode::ClampToEdge,
            ],
            SamplerMode::Envmap => [
                wgpu::AddressMode::Repeat,
                wgpu::AddressMode::ClampToEdge,
                wgpu::AddressMode::ClampToEdge,
            ],
            SamplerMode::Shadow => [
                wgpu::AddressMode::ClampToBorder,
                wgpu::AddressMode::ClampToBorder,
                wgpu::AddressMode::ClampToEdge,
            ],
        }
    }
}

#[derive(Clone)]
pub struct ImageDescriptor {
    pub image: Arc<BitangImage>,
    texture_view: wgpu::TextureView,
}

impl ImageDescriptor {
    pub fn new(context: &GpuContext, image: Arc<BitangImage>) -> Self {
        unimplemented!();
        let texture_view = image.get_view_for_sampler();
        Self { image, texture_view }
    }
}


#[derive(Clone)]
pub struct SamplerDescriptor {
    pub mode: SamplerMode,
    sampler: wgpu::Sampler,
}

impl SamplerDescriptor {
    pub fn new(context: &GpuContext, mode: SamplerMode) -> Self {
        let [au, av, aw] = mode.to_wgpu_address_mode();
        let sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: au,
            address_mode_v: av,
            address_mode_w: aw,
            compare: mode.to_wgpu_compare_op(),
            border_color: Some(wgpu::SamplerBorderColor::OpaqueWhite),
            ..wgpu::SamplerDescriptor::default()
        });
        Self { mode, sampler }
    }

    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }
}

#[derive(Clone)]
pub enum DescriptorSource {
    Image(ImageDescriptor),
    Sampler(SamplerDescriptor),
    // BufferGenerator(Rc<BufferGenerator>),
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
