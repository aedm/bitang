use crate::control::{ControlId, ControlIdPartType};
use crate::file::chart_file::ChartContext;
use crate::loader::async_cache::LoadFuture;
use crate::render::image::BitangImage;
use crate::render::shader;
use crate::render::shader::{
    DescriptorResource, DescriptorSource, ImageDescriptor, LocalUniformMapping, Shader, ShaderKind,
};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use tracing::instrument;

#[derive(Debug, Deserialize)]
pub enum BufferSource {
    BufferGenerator(String),
    Current(String),
    Next(String),
}

#[derive(Debug, Deserialize, Clone)]
pub struct Sampler {
    bind: SamplerSource,
    pub mode: SamplerMode,
}

#[derive(Debug, Deserialize, Clone)]
pub enum SamplerSource {
    Image(String),
    File(String),
}

#[derive(Debug, Deserialize, Clone)]
pub enum SamplerMode {
    Repeat,
    ClampToEdge,
    MirroredRepeat,
    Envmap,
    Shadow,
}

impl SamplerMode {
    pub fn load(&self) -> shader::SamplerMode {
        match self {
            SamplerMode::Repeat => shader::SamplerMode::Repeat,
            SamplerMode::MirroredRepeat => shader::SamplerMode::MirroredRepeat,
            SamplerMode::ClampToEdge => shader::SamplerMode::ClampToEdge,
            SamplerMode::Envmap => shader::SamplerMode::Envmap,
            SamplerMode::Shadow => shader::SamplerMode::Shadow,
        }
    }
}

/// All the context needed to build a shader
pub struct ShaderContext {
    control_map: HashMap<String, String>,
    control_id: ControlId,
    sampler_futures: HashMap<String, (LoadFuture<BitangImage>, Sampler)>,
    buffers_by_binding: HashMap<String, DescriptorSource>,
}

impl ShaderContext {
    pub fn new(
        chart_context: &ChartContext,
        control_map: &HashMap<String, String>,
        control_id: &ControlId,
        samplers: &HashMap<String, Sampler>,
        buffers: &HashMap<String, BufferSource>,
    ) -> Result<Self> {
        let sampler_futures = samplers
            .iter()
            .map(|(name, sampler)| {
                let resource_repository = chart_context.resource_repository.clone();
                let image: LoadFuture<BitangImage> = {
                    match &sampler.bind {
                        SamplerSource::File(texture_path) => resource_repository.get_texture(
                            &chart_context.vulkan_context,
                            &chart_context.path.relative_path(texture_path)?,
                        ),
                        SamplerSource::Image(id) => chart_context
                            .image_futures_by_id
                            .get(id)
                            .with_context(|| anyhow!("Render target '{id}' not found"))?
                            .clone(),
                    }
                };
                let value = (image, sampler.clone());
                Ok((name.clone(), value))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        let buffers_by_binding = buffers
            .iter()
            .map(|(name, buffer)| {
                let buffer_generator = match buffer {
                    BufferSource::BufferGenerator(id) => {
                        let generator = chart_context
                            .buffer_generators_by_id
                            .get(id)
                            .with_context(|| anyhow!("Buffer generator '{id}' not found"))?
                            .clone();
                        DescriptorSource::BufferGenerator(generator)
                    }
                    BufferSource::Current(id) => {
                        let buffer = chart_context
                            .buffers_by_id
                            .get(id)
                            .with_context(|| anyhow!("Buffer '{id}' not found"))?
                            .clone();
                        DescriptorSource::BufferCurrent(buffer)
                    }
                    BufferSource::Next(id) => {
                        let buffer = chart_context
                            .buffers_by_id
                            .get(id)
                            .with_context(|| anyhow!("Buffer '{id}' not found"))?
                            .clone();
                        DescriptorSource::BufferNext(buffer)
                    }
                };
                Ok((name.clone(), buffer_generator))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        Ok(ShaderContext {
            control_map: control_map.clone(),
            control_id: control_id.clone(),
            sampler_futures,
            buffers_by_binding,
        })
    }

    #[instrument(skip(self, chart_context))]
    pub async fn make_shader(
        &self,
        chart_context: &ChartContext,
        kind: ShaderKind,
        source_path: &str,
    ) -> Result<Shader> {
        let mut macros = vec![];

        // Add sampler macros
        for (sampler_name, _) in &self.sampler_futures {
            macros.push((
                format!("IMAGE_BOUND_TO_SAMPLER_{}", sampler_name.to_uppercase()),
                "1".to_string(),
            ));
        }

        let shader_artifact = chart_context
            .resource_repository
            .shader_cache
            .get(
                chart_context.vulkan_context.clone(),
                chart_context.path.relative_path(source_path)?,
                kind,
                macros,
            )
            .await?;

        let local_uniform_bindings = shader_artifact
            .local_uniform_bindings
            .iter()
            .map(|binding| {
                let control_id = if let Some(mapped_name) = self.control_map.get(&binding.name) {
                    chart_context
                        .values_control_id
                        .add(ControlIdPartType::Value, mapped_name)
                } else {
                    self.control_id.add(ControlIdPartType::Value, &binding.name)
                };
                let control = chart_context
                    .control_set_builder
                    .get_vec(&control_id, binding.f32_count);
                LocalUniformMapping {
                    control,
                    f32_count: binding.f32_count,
                    f32_offset: binding.f32_offset,
                }
            })
            .collect::<Vec<_>>();

        let mut descriptor_resources = vec![];

        // Collect buffer generator bindings
        for buffer in &shader_artifact.buffers {
            let descriptor_source =
                self.buffers_by_binding.get(&buffer.name).with_context(|| {
                    anyhow!(
                        "Buffer generator definition for '{}' not found",
                        buffer.name
                    )
                })?;
            let buffer_descriptor = DescriptorResource {
                id: buffer.name.clone(),
                binding: buffer.binding,
                source: descriptor_source.clone(),
            };
            descriptor_resources.push(buffer_descriptor);
        }

        // Collect sampler bindings
        for sampler in &shader_artifact.samplers {
            let source = self
                .sampler_futures
                .get(&sampler.name)
                .with_context(|| anyhow!("Sampler definition for '{}' not found", sampler.name))?;
            // Wait for the image to load
            let image = source.0.get().await?;
            let sampler_descriptor = DescriptorResource {
                id: sampler.name.clone(),
                binding: sampler.binding,
                source: DescriptorSource::Image(ImageDescriptor {
                    mode: source.1.mode.load(),
                    image,
                }),
            };
            descriptor_resources.push(sampler_descriptor);
        }

        let shader = Shader::new(
            &chart_context.vulkan_context,
            shader_artifact.module.clone(),
            kind,
            shader_artifact.global_uniform_bindings.clone(),
            local_uniform_bindings,
            shader_artifact.uniform_buffer_size,
            descriptor_resources,
        );

        Ok(shader)
    }
}
