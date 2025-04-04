use crate::control::controls::{Control, GlobalType, Globals};
use crate::render::double_buffer::DoubleBuffer;
use crate::render::image::BitangImage;
use crate::tool::{ComputePassContext, GpuContext, RenderPassContext};
use anyhow::Result;
use smallvec::SmallVec;
use std::mem::size_of;
use std::rc::Rc;
use std::sync::Arc;

use super::image::PixelFormat;

const MAX_UNIFORMS_F32_COUNT: usize = 1024;

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub enum ShaderKind {
    Vertex,
    Fragment,
    Compute,
    ComputeInit,
    ComputeSimulate,
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
            ShaderKind::ComputeInit => 0,
            ShaderKind::ComputeSimulate => 0,
        }
    }

    pub fn entry_point(&self) -> &'static str {
        match self {
            ShaderKind::Vertex => "vs_main",
            ShaderKind::Fragment => "fs_main",
            ShaderKind::Compute => "cs_main",
            ShaderKind::ComputeInit => "cs_main",
            ShaderKind::ComputeSimulate => "cs_main",
        }
    }
}

/// Shader stage, either vertex or fragment.
pub struct Shader {
    /// The compiled shader module.
    pub shader_module: wgpu::ShaderModule,

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

    /// Layout of the single bind group containing all resources
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl Shader {
    pub fn new(
        context: &GpuContext,
        shader_module: wgpu::ShaderModule,
        kind: ShaderKind,
        global_uniform_bindings: Vec<GlobalUniformMapping>,
        local_uniform_bindings: Vec<LocalUniformMapping>,
        uniform_buffer_size: usize,
        descriptor_resources: Vec<DescriptorResource>,
    ) -> Shader {
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
            ShaderKind::ComputeInit => wgpu::ShaderStages::COMPUTE,
            ShaderKind::ComputeSimulate => wgpu::ShaderStages::COMPUTE,
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
                }
                DescriptorSource::BufferCurrent(_) => wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                DescriptorSource::BufferNext(_) => wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            };
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: descriptor_resource.binding,
                visibility,
                ty,
                count: None,
            });
        }

        let bind_group_layout =
            context.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

    pub fn bind_to_render_pass(&self, context: &mut RenderPassContext<'_>) -> Result<()> {
        let bind_group = self.make_bind_group(&context.gpu_context, &context.globals)?;
        context.pass.set_bind_group(self.kind.get_descriptor_set_index(), &bind_group, &[]);
        Ok(())
    }

    pub fn bind_to_compute_pass(&self, context: &mut ComputePassContext<'_>) -> Result<()> {
        let bind_group = self.make_bind_group(&context.gpu_context, &context.globals)?;
        context.pass.set_bind_group(self.kind.get_descriptor_set_index(), &bind_group, &[]);
        Ok(())
    }

    fn make_bind_group(
        &self,
        context: &GpuContext,
        globals: &Globals,
        // TODO: no result
    ) -> Result<wgpu::BindGroup> {
        let mut texture_views = SmallVec::<[_; 64]>::new();
        let mut entries = SmallVec::<[_; 64]>::new();

        if self.uniform_buffer_size > 0 {
            // Fill uniform array
            let mut uniform_values = [0.0f32; MAX_UNIFORMS_F32_COUNT];
            for global_mapping in &self.global_uniform_bindings {
                let values = globals.get(global_mapping.global_type);
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
            let f32_count = self.uniform_buffer_size / size_of::<f32>();
            context.queue.write_buffer(
                &self.uniform_buffer,
                0,
                bytemuck::cast_slice(&uniform_values[..f32_count]),
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
                    // Just store texture view in an array.
                    // This is needed because we create texture views per frame.
                    // TODO: cache texture views
                    let texture_view = image_descriptor.image.view_as_sampler()?;
                    texture_views.push((descriptor_resource.binding, texture_view));
                    continue;
                }
                DescriptorSource::Sampler(sampler_descriptor) => wgpu::BindGroupEntry {
                    binding: descriptor_resource.binding,
                    resource: wgpu::BindingResource::Sampler(&sampler_descriptor.sampler),
                },
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

        for texture_view in &texture_views {
            entries.push(wgpu::BindGroupEntry {
                binding: texture_view.0,
                resource: wgpu::BindingResource::TextureView(&texture_view.1),
            });
        }

        let bind_group = context.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bind Group"),
            layout: &self.bind_group_layout,
            entries: &entries,
        });
        Ok(bind_group)
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
    pub fn to_wgpu_compare_op(&self) -> Option<wgpu::CompareFunction> {
        match self {
            SamplerMode::Shadow => Some(wgpu::CompareFunction::Less),
            _ => None,
        }
    }

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

// TODO: rename TextureResource or TextureShaderResource
#[derive(Clone)]
pub struct ImageDescriptor {
    pub image: Arc<BitangImage>,
}

impl ImageDescriptor {
    pub fn new(image: Arc<BitangImage>) -> Result<Self> {
        Ok(Self { image })
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
}

#[derive(Clone)]
pub enum DescriptorSource {
    Image(ImageDescriptor),
    Sampler(SamplerDescriptor),
    BufferCurrent(Rc<DoubleBuffer>),
    BufferNext(Rc<DoubleBuffer>),
}

pub struct DescriptorResource {
    #[allow(dead_code)]
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
