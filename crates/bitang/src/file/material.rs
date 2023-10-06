use crate::control::{ControlId, ControlIdPartType};
use crate::file::chart_file::ChartContext;
use crate::file::default_true;
use crate::loader::async_cache::LoadFuture;
use crate::loader::shader_loader::ShaderCompilationResult;
use crate::render;
use crate::render::image::Image;
use crate::render::material::{BlendMode, MaterialPassProps};
use crate::render::shader::{
    DescriptorResource, DescriptorSource, ImageDescriptor, LocalUniformMapping, Shader, ShaderKind,
};
use anyhow::{anyhow, Context, Result};
use futures::future::join_all;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

const COMMON_SHADER_FILE: &str = "common.glsl";

struct MaterialLoadContext {
    control_map: HashMap<String, String>,
    object_cid: ControlId,
    sampler_futures: HashMap<String, (LoadFuture<Image>, Sampler)>,
    local_buffer_generators_by_id: HashMap<String, Arc<render::buffer_generator::BufferGenerator>>,
}

#[derive(Debug, Deserialize)]
pub struct Material {
    passes: HashMap<String, MaterialPass>,

    #[serde(default)]
    samplers: HashMap<String, Sampler>,

    #[serde(default)]
    buffers: HashMap<String, BufferSource>,
}

impl Material {
    pub async fn load(
        &self,
        chart_context: &ChartContext,
        passes: &[render::pass::Pass],
        control_map: &HashMap<String, String>,
        object_cid: &ControlId,
    ) -> Result<crate::render::material::Material> {
        let sampler_futures = self
            .samplers
            .iter()
            .map(|(name, sampler)| {
                let resource_repository = chart_context.resource_repository.clone();
                let image: LoadFuture<Image> = {
                    match &sampler.bind {
                        SamplerSource::File(texture_path) => resource_repository.get_texture(
                            &chart_context.vulkan_context,
                            &chart_context.path.relative_path(texture_path),
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

        let local_buffer_generators_by_id = self
            .buffers
            .iter()
            .map(|(name, buffer)| {
                let buffer_generator = match buffer {
                    BufferSource::BufferGenerator(id) => chart_context
                        .buffer_generators_by_id
                        .get(id)
                        .with_context(|| anyhow!("Buffer generator '{id}' not found"))?
                        .clone(),
                };
                Ok((name.clone(), buffer_generator.clone()))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        let material_load_context = MaterialLoadContext {
            control_map: control_map.clone(),
            object_cid: object_cid.clone(),
            sampler_futures,
            local_buffer_generators_by_id,
        };

        let material_pass_futures = passes.iter().map(|pass| async {
            if let Some(material_pass) = self.passes.get(&pass.id) {
                let pass = material_pass
                    .load(
                        &pass.id,
                        &material_load_context,
                        chart_context,
                        pass.vulkan_render_pass.clone(),
                    )
                    .await?;
                Ok(Some(pass))
            } else {
                Ok(None)
            }
        });

        let material_passes = join_all(material_pass_futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

        Ok(render::material::Material {
            passes: material_passes,
        })
    }
}

#[derive(Debug, Deserialize)]
struct MaterialPass {
    vertex_shader: String,
    fragment_shader: String,

    #[serde(default = "default_true")]
    depth_test: bool,

    #[serde(default = "default_true")]
    depth_write: bool,

    #[serde(default)]
    blend_mode: BlendMode,
}

impl MaterialPass {
    async fn make_shader(
        &self,
        material_load_context: &MaterialLoadContext,
        chart_context: &ChartContext,
        kind: ShaderKind,
        shader_compilation_result: &ShaderCompilationResult,
    ) -> Result<Shader> {
        let local_uniform_bindings = shader_compilation_result
            .local_uniform_bindings
            .iter()
            .map(|binding| {
                let control_id = if let Some(mapped_name) =
                    material_load_context.control_map.get(&binding.name)
                {
                    chart_context
                        .values_control_id
                        .add(ControlIdPartType::Value, mapped_name)
                } else {
                    material_load_context
                        .object_cid
                        .add(ControlIdPartType::Value, &binding.name)
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
        for buffer in &shader_compilation_result.buffers {
            let buffer_generator = material_load_context
                .local_buffer_generators_by_id
                .get(&buffer.name)
                .with_context(|| {
                    anyhow!(
                        "Buffer generator definition for '{}' not found",
                        buffer.name
                    )
                })?;
            let buffer_descriptor = DescriptorResource {
                id: buffer.name.clone(),
                binding: buffer.binding,
                source: DescriptorSource::BufferGenerator(buffer_generator.clone()),
            };
            descriptor_resources.push(buffer_descriptor);
        }

        // Collect sampler bindings
        for sampler in &shader_compilation_result.samplers {
            let source = material_load_context
                .sampler_futures
                .get(&sampler.name)
                .with_context(|| anyhow!("Sampler definition for '{}' not found", sampler.name))?;
            // Wait for the image to load
            let image = source.0.get().await?;
            let sampler_descriptor = DescriptorResource {
                id: sampler.name.clone(),
                binding: sampler.binding,
                source: DescriptorSource::Image(ImageDescriptor {
                    address_mode: source.1.address_mode.load(),
                    image,
                }),
            };
            descriptor_resources.push(sampler_descriptor);
        }

        let shader = Shader::new(
            &chart_context.vulkan_context,
            shader_compilation_result.module.clone(),
            kind,
            shader_compilation_result.global_uniform_bindings.clone(),
            local_uniform_bindings,
            shader_compilation_result.uniform_buffer_size,
            descriptor_resources,
        );

        Ok(shader)
    }

    async fn load(
        &self,
        id: &str,
        material_load_context: &MaterialLoadContext,
        chart_context: &ChartContext,
        vulkan_render_pass: Arc<vulkano::render_pass::RenderPass>,
    ) -> Result<render::material::MaterialPass> {
        // let shader_cache_value = chart_context
        //     .resource_repository
        //     .shader_cache
        //     .get(
        //         &chart_context.vulkan_context,
        //         &chart_context.path.relative_path(&self.vertex_shader),
        //         &chart_context.path.relative_path(&self.fragment_shader),
        //         &chart_context.path.relative_path(COMMON_SHADER_FILE),
        //     )
        //     .await?;
        //

        let vertex_shader_compile_future = chart_context.resource_repository.shader_cache.get(
            chart_context.vulkan_context.clone(),
            chart_context.path.relative_path(&self.vertex_shader),
            ShaderKind::Vertex,
            chart_context.path.relative_path(COMMON_SHADER_FILE),
        );

        let fragment_shader_compile_future = chart_context.resource_repository.shader_cache.get(
            chart_context.vulkan_context.clone(),
            chart_context.path.relative_path(&self.fragment_shader),
            ShaderKind::Fragment,
            chart_context.path.relative_path(COMMON_SHADER_FILE),
        );

        let compile_results = join_all(vec![
            vertex_shader_compile_future,
            fragment_shader_compile_future,
        ])
        .await
        .into_iter()
        .collect::<Result<Vec<_>>>()?;

        let vertex_shader = self
            .make_shader(
                material_load_context,
                chart_context,
                ShaderKind::Vertex,
                &compile_results[0],
            )
            .await?;

        let fragment_shader = self
            .make_shader(
                material_load_context,
                chart_context,
                ShaderKind::Fragment,
                &compile_results[1],
            )
            .await?;

        let material_props = MaterialPassProps {
            id: id.to_string(),
            vertex_shader,
            fragment_shader,
            depth_test: self.depth_test,
            depth_write: self.depth_write,
            blend_mode: self.blend_mode.clone(),
        };

        render::material::MaterialPass::new(
            &chart_context.vulkan_context,
            material_props,
            vulkan_render_pass,
        )
    }
}

#[derive(Debug, Deserialize, Clone)]
pub enum SamplerSource {
    Image(String),
    File(String),
}

#[derive(Debug, Deserialize)]
pub enum BufferSource {
    BufferGenerator(String),
}

#[derive(Debug, Deserialize, Clone)]
struct Sampler {
    bind: SamplerSource,

    #[serde(default)]
    address_mode: SamplerAddressMode,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub enum SamplerAddressMode {
    #[default]
    Repeat,
    ClampToEdge,
    MirroredRepeat,
}

impl SamplerAddressMode {
    pub fn load(&self) -> vulkano::sampler::SamplerAddressMode {
        match self {
            SamplerAddressMode::Repeat => vulkano::sampler::SamplerAddressMode::Repeat,
            SamplerAddressMode::MirroredRepeat => {
                vulkano::sampler::SamplerAddressMode::MirroredRepeat
            }
            SamplerAddressMode::ClampToEdge => vulkano::sampler::SamplerAddressMode::ClampToEdge,
        }
    }
}
