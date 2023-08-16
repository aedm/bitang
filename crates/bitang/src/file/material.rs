use crate::control::controls::ControlSetBuilder;
use crate::control::{ControlId, ControlIdPartType};
use crate::file::chart_file::BufferGenerator;
use crate::file::resource_repository::ResourceRepository;
use crate::file::shader_loader::ShaderCompilationResult;
use crate::file::ResourcePath;
use crate::render;
use crate::render::image::{Image, ImageSizeRule};
use crate::render::material::BlendMode;
use crate::render::shader::{
    DescriptorResource, DescriptorSource, ImageDescriptor, LocalUniformMapping, Shader, ShaderKind,
};
use crate::render::vulkan_window::VulkanContext;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

const COMMON_SHADER_FILE: &str = "common.glsl";

#[derive(Debug, Deserialize)]
pub struct Material {
    passes: HashMap<String, MaterialPass>,
    samplers: HashMap<String, Sampler>,
    buffers: HashMap<String, BufferSource>,
}

impl Material {
    pub fn load(
        &self,
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
        images_by_id: &HashMap<String, Arc<render::image::Image>>,
        path: &ResourcePath,
        passes: &Vec<render::pass::Pass>,
        control_set_builder: &mut ControlSetBuilder,
        control_map: &HashMap<String, String>,
        parent_id: &ControlId,
        chart_id: &ControlId,
        buffer_generators_by_id: &HashMap<String, Arc<render::buffer_generator::BufferGenerator>>,
    ) -> Result<crate::render::material::Material> {
        let sampler_images = self
            .samplers
            .iter()
            .map(|(name, sampler)| {
                let image: Arc<Image> = match &sampler.bind {
                    SamplerSource::File(texture_path) => resource_repository
                        .get_texture(context, &path.relative_path(texture_path))?
                        .clone(),
                    SamplerSource::Image(id) => images_by_id
                        .get(id)
                        .with_context(|| anyhow!("Render target '{}' not found", id))?
                        .clone(),
                };
                Ok((name.clone(), (image, sampler)))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        let material_passes = passes
            .iter()
            .map(|pass| {
                if let Some(material_pass) = self.passes.get(&pass.id) {
                    let pass = material_pass.load(
                        &pass.id,
                        context,
                        resource_repository,
                        path,
                        control_set_builder,
                        control_map,
                        parent_id,
                        chart_id,
                        &sampler_images,
                        buffer_generators_by_id,
                        pass.vulkan_render_pass.clone(),
                    )?;
                    Ok(Some(pass))
                } else {
                    Ok(None)
                }
            })
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
    depth_test: bool,
    depth_write: bool,
    blend_mode: BlendMode,
}

impl MaterialPass {
    fn make_shader(
        &self,
        context: &VulkanContext,
        kind: ShaderKind,
        shader_compilation_result: &ShaderCompilationResult,
        control_set_builder: &mut ControlSetBuilder,
        control_map: &HashMap<String, String>,
        parent_id: &ControlId,
        chart_id: &ControlId,
        sampler_images: &HashMap<String, (Arc<Image>, &Sampler)>,
        buffer_generators_by_id: &HashMap<String, Arc<render::buffer_generator::BufferGenerator>>,
    ) -> Result<Shader> {
        let mut descriptor_resources = vec![];

        // Collect sampler bindings
        for sampler in &shader_compilation_result.samplers {
            let source = sampler_images
                .get(&sampler.name)
                .with_context(|| anyhow!("Sampler definition for '{}' not found", sampler.name))?;
            let sampler_descriptor = DescriptorResource {
                id: sampler.name.clone(),
                binding: sampler.binding,
                source: DescriptorSource::Image(ImageDescriptor {
                    address_mode: source.1.address_mode.load(),
                    image: source.0.clone(),
                }),
            };
            descriptor_resources.push(sampler_descriptor);
        }

        // Collect buffer generator bindings
        for buffer in &shader_compilation_result.buffers {
            let buffer_generator =
                buffer_generators_by_id.get(&buffer.name).with_context(|| {
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

        let local_uniform_bindings = shader_compilation_result
            .local_uniform_bindings
            .iter()
            .map(|binding| {
                let control_id = if let Some(mapped_name) = control_map.get(&binding.name) {
                    chart_id.add(ControlIdPartType::Value, mapped_name)
                } else {
                    parent_id.add(ControlIdPartType::Value, &binding.name)
                };
                let control = control_set_builder.get_vec(&control_id, binding.f32_count);
                LocalUniformMapping {
                    control,
                    f32_count: binding.f32_count,
                    f32_offset: binding.f32_offset,
                }
            })
            .collect::<Vec<_>>();

        let shader = Shader::new(
            context,
            shader_compilation_result.module.clone(),
            kind,
            shader_compilation_result.global_uniform_bindings.clone(),
            local_uniform_bindings,
            shader_compilation_result.uniform_buffer_size,
            descriptor_resources,
        );

        Ok(shader)
    }

    fn load(
        &self,
        id: &str,
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
        path: &ResourcePath,
        control_set_builder: &mut ControlSetBuilder,
        control_map: &HashMap<String, String>,
        parent_id: &ControlId,
        chart_id: &ControlId,
        sampler_images: &HashMap<String, (Arc<Image>, &Sampler)>,
        buffer_generators_by_id: &HashMap<String, Arc<render::buffer_generator::BufferGenerator>>,
        vulkan_render_pass: Arc<vulkano::render_pass::RenderPass>,
    ) -> Result<crate::render::material::MaterialPass> {
        let shader_cache_value = resource_repository.shader_cache.get_or_load(
            context,
            &path.relative_path(&self.vertex_shader),
            &path.relative_path(&self.fragment_shader),
            &path.relative_path(COMMON_SHADER_FILE),
        )?;

        let vertex_shader = self.make_shader(
            context,
            ShaderKind::Vertex,
            &shader_cache_value.vertex_shader,
            control_set_builder,
            control_map,
            parent_id,
            chart_id,
            &sampler_images,
            &buffer_generators_by_id,
        )?;

        let fragment_shader = self.make_shader(
            context,
            ShaderKind::Fragment,
            &shader_cache_value.fragment_shader,
            control_set_builder,
            control_map,
            parent_id,
            chart_id,
            &sampler_images,
            &buffer_generators_by_id,
        )?;

        let material_pass = render::material::MaterialPass::new(
            context,
            id.to_string(),
            vertex_shader,
            fragment_shader,
            self.depth_test,
            self.depth_write,
            self.blend_mode.clone(),
            vulkan_render_pass,
        );
        
        material_pass
    }
}

#[derive(Debug, Deserialize)]
pub enum SamplerSource {
    Image(String),
    File(String),
}

#[derive(Debug, Deserialize)]
pub enum BufferSource {
    BufferGenerator(String),
}

#[derive(Debug, Deserialize)]
struct Sampler {
    id: String,
    bind: SamplerSource,
    address_mode: SamplerAddressMode,
}

#[derive(Debug, Deserialize, Default)]
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
