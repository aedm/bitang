use crate::control::controls::Controls;
use crate::file::resource_repository::ResourceRepository;
use crate::file::shader_loader::ShaderCompilationResult;
use crate::render;
use crate::render::material::{
    LocalUniformMapping, Material, MaterialStep, SamplerBinding, SamplerSource, Shader,
};
use crate::render::vulkan_window::VulkanContext;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::instrument;

#[derive(Debug, Deserialize)]
pub struct Chart {
    pub render_targets: Vec<RenderTarget>,
    pub passes: Vec<Pass>,
}

#[derive(Debug, Deserialize)]
pub struct RenderTarget {
    pub id: String,
    pub size: RenderTargetSize,
    pub role: RenderTargetRole,
}

#[derive(Debug, Deserialize)]
pub enum RenderTargetSize {
    Static { width: u32, height: u32 },
    ScreenRelative { width: f32, height: f32 },
}

#[derive(Debug, Deserialize)]
pub enum RenderTargetRole {
    Color,
    Depth,
}

#[derive(Debug, Deserialize)]
pub struct Pass {
    pub id: String,
    pub render_targets: Vec<String>,
    pub objects: Vec<Object>,
}

#[derive(Debug, Deserialize)]
pub struct Object {
    pub id: String,
    pub mesh_path: String,
    pub vertex_shader: String,
    pub fragment_shader: String,
    pub depth_test: bool,
    pub depth_write: bool,
    pub textures: HashMap<String, TextureMapping>,
}

#[derive(Debug, Deserialize)]
pub enum TextureMapping {
    File(String),
    RenderTargetId(String),
}

impl Chart {
    pub fn load(
        &self,
        context: &VulkanContext,
        id: &str,
        resource_repository: &mut ResourceRepository,
    ) -> Result<render::chart::Chart> {
        let render_targets_by_id = self
            .render_targets
            .iter()
            .map(|render_target| {
                let render_target = render_target.load();
                (render_target.id.clone(), render_target)
            })
            .collect::<HashMap<String, Arc<render::render_target::RenderTarget>>>();

        let passes = self
            .passes
            .iter()
            .map(|pass| pass.load(context, resource_repository, &render_targets_by_id, id))
            .collect::<Result<Vec<_>>>()?;

        let render_targets = render_targets_by_id.into_values().collect::<Vec<_>>();

        let chart = render::chart::Chart::new(
            id,
            &mut resource_repository.controls,
            render_targets,
            passes,
        );
        Ok(chart)
    }
}

impl Pass {
    pub fn load(
        &self,
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
        render_targets_by_id: &HashMap<String, Arc<render::render_target::RenderTarget>>,
        control_prefix: &str,
    ) -> Result<render::render_target::Pass> {
        let render_targets = self
            .render_targets
            .iter()
            .map(|render_target_id| {
                render_targets_by_id
                    .get(render_target_id)
                    .or_else(|| context.swapchain_render_targets_by_id.get(render_target_id))
                    .and_then(|render_target| Some(render_target.clone()))
                    .with_context(|| anyhow!("Render target '{}' not found", render_target_id))
            })
            .collect::<Result<Vec<_>>>()?;

        let objects = self
            .objects
            .iter()
            .map(|object| {
                object.load(
                    control_prefix,
                    context,
                    resource_repository,
                    render_targets_by_id,
                )
            })
            .collect::<Result<Vec<_>>>()?;

        let pass = render::render_target::Pass::new(context, &self.id, render_targets, objects)?;
        Ok(pass)
    }
}

impl RenderTarget {
    pub fn load(&self) -> Arc<render::render_target::RenderTarget> {
        let size_constraint = self.size.load();
        let role = self.role.load();
        render::render_target::RenderTarget::new(&self.id, role, size_constraint)
    }
}

impl RenderTargetSize {
    pub fn load(&self) -> render::render_target::RenderTargetSizeConstraint {
        match self {
            RenderTargetSize::Static { width, height } => {
                render::render_target::RenderTargetSizeConstraint::Static {
                    width: *width,
                    height: *height,
                }
            }
            RenderTargetSize::ScreenRelative { width, height } => {
                render::render_target::RenderTargetSizeConstraint::ScreenRelative {
                    width: *width,
                    height: *height,
                }
            }
        }
    }
}

impl RenderTargetRole {
    pub fn load(&self) -> render::render_target::RenderTargetRole {
        match self {
            RenderTargetRole::Color => render::render_target::RenderTargetRole::Color,
            RenderTargetRole::Depth => render::render_target::RenderTargetRole::Depth,
        }
    }
}

impl Object {
    pub fn load(
        &self,
        control_prefix: &str,
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
        render_targets: &HashMap<String, Arc<render::render_target::RenderTarget>>,
    ) -> Result<Arc<render::RenderObject>> {
        let mesh = resource_repository
            .get_mesh(context, &self.mesh_path)?
            .clone();
        let sampler_sources = self
            .textures
            .iter()
            .map(|(name, texture_mapping)| {
                let texture_binding: SamplerSource = match texture_mapping {
                    TextureMapping::File(path) => {
                        let texture = resource_repository.get_texture(context, path)?.clone();
                        SamplerSource::Texture(texture)
                    }
                    TextureMapping::RenderTargetId(id) => {
                        let render_target = render_targets
                            .get(id)
                            .with_context(|| anyhow!("Render target '{}' not found", id))?;
                        SamplerSource::RenderTarget(render_target.clone())
                    }
                };
                Ok((name.clone(), texture_binding))
            })
            .collect::<Result<HashMap<String, SamplerSource>>>()?;

        let solid_step = self.make_material_step(
            context,
            resource_repository,
            control_prefix,
            &sampler_sources,
        )?;
        let material = Material {
            passes: [None, None, Some(solid_step)],
        };

        let object = render::RenderObject {
            id: self.id.clone(),
            mesh,
            position: Default::default(),
            rotation: Default::default(),
            material,
        };
        Ok(Arc::new(object))
    }

    fn make_material_step(
        &self,
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
        control_prefix: &str,
        sampler_sources: &HashMap<String, SamplerSource>,
    ) -> Result<MaterialStep> {
        let shaders = resource_repository.shader_cache.get_or_load(
            context,
            &self.vertex_shader,
            &self.fragment_shader,
        )?;

        let vertex_shader = make_shader(
            &mut resource_repository.controls,
            control_prefix,
            &shaders.vertex_shader,
            sampler_sources,
        )?;
        let fragment_shader = make_shader(
            &mut resource_repository.controls,
            control_prefix,
            &shaders.fragment_shader,
            sampler_sources,
        )?;

        let material_step = MaterialStep {
            vertex_shader,
            fragment_shader,
            depth_test: self.depth_test,
            depth_write: self.depth_write,
        };
        Ok(material_step)
    }
}

#[instrument(skip_all)]
fn make_shader(
    controls: &mut Controls,
    control_prefix: &str,
    compilation_result: &ShaderCompilationResult,
    sampler_sources: &HashMap<String, SamplerSource>,
) -> Result<Shader> {
    let local_mapping = compilation_result
        .local_uniform_bindings
        .iter()
        .map(|binding| {
            let control_id = format!("{control_prefix}/{}", binding.name);
            let control = controls.get_control(&control_id);
            LocalUniformMapping {
                control,
                f32_count: binding.f32_count,
                f32_offset: binding.f32_offset,
            }
        })
        .collect::<Vec<_>>();

    let sampler_bindings = compilation_result
        .samplers
        .iter()
        .map(|sampler| {
            let sampler_source = sampler_sources
                .get(&sampler.name)
                .and_then(|sampler_source| Some(sampler_source.clone()))
                .with_context(|| format!("Sampler binding '{}' not found", sampler.name))?;
            Ok(SamplerBinding {
                sampler_source,
                descriptor_set_binding: sampler.binding,
            })
        })
        .collect::<Result<Vec<SamplerBinding>>>()?;

    Ok(Shader {
        shader_module: compilation_result.module.clone(),
        sampler_bindings,
        local_uniform_bindings: local_mapping,
        global_uniform_bindings: compilation_result.global_uniform_bindings.clone(),
        uniform_buffer_size: compilation_result.uniform_buffer_size,
    })
}
