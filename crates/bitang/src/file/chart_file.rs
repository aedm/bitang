use crate::control::controls::ControlSetBuilder;
use crate::control::{ControlId, ControlIdPartType};
use crate::file::resource_repository::ResourceRepository;
use crate::file::shader_loader::ShaderCompilationResult;
use crate::file::ResourcePath;
use crate::render;
use crate::render::buffer_generator::BufferGeneratorType;
use crate::render::material::{
    DescriptorBinding, DescriptorSource, LocalUniformMapping, Material, MaterialStep, Shader,
};
use crate::render::vulkan_window::VulkanContext;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::instrument;

const COMMON_SHADER_FILE: &str = "common.glsl";

#[derive(Debug, Deserialize)]
pub struct Chart {
    #[serde(default)]
    pub render_targets: Vec<RenderTarget>,

    #[serde(default)]
    pub buffer_generators: Vec<BufferGenerator>,

    pub steps: Vec<Draw>,
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

#[derive(Debug, Deserialize, Default)]
pub enum BlendMode {
    #[default]
    None,
    Alpha,
    Additive,
}

#[derive(Debug, Deserialize, Default)]
pub enum SamplerAddressMode {
    #[default]
    Repeat,
    ClampToEdge,
    MirroredRepeat,
}

fn default_clear_color() -> Option<[f32; 4]> {
    Some([0.03, 0.03, 0.03, 1.0])
}

/// Represents a draw step in the chart sequence.
#[derive(Debug, Deserialize)]
pub struct Draw {
    pub id: String,
    pub render_targets: Vec<String>,
    pub objects: Vec<Object>,

    #[serde(default = "default_clear_color")]
    pub clear_color: Option<[f32; 4]>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct Object {
    pub id: String,
    pub mesh_file: String,
    pub mesh_name: String,
    pub vertex_shader: String,
    pub fragment_shader: String,

    #[serde(default = "default_true")]
    pub depth_test: bool,

    #[serde(default = "default_true")]
    pub depth_write: bool,

    #[serde(default)]
    pub blend_mode: BlendMode,

    #[serde(default)]
    pub sampler_address_mode: SamplerAddressMode,

    #[serde(default)]
    pub textures: HashMap<String, TextureMapping>,

    #[serde(default)]
    pub buffers: HashMap<String, BufferMapping>,

    #[serde(default)]
    pub control_map: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub enum TextureMapping {
    File(String),
    RenderTargetId(String),
}

#[derive(Debug, Deserialize)]
pub enum BufferMapping {
    BufferGeneratorId(String),
}

#[derive(Debug, Deserialize)]
pub struct BufferGenerator {
    id: String,
    size: u32,
    generator: BufferGeneratorType,
}

impl Chart {
    pub fn load(
        &self,
        id: &str,
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
        path: &ResourcePath,
    ) -> Result<render::chart::Chart> {
        let control_id = ControlId::default().add(ControlIdPartType::Chart, id);
        let mut control_set_builder = ControlSetBuilder::new(
            control_id.clone(),
            resource_repository.control_repository.clone(),
        );

        let render_targets_by_id = self
            .render_targets
            .iter()
            .map(|render_target| {
                let render_target = render_target.load();
                (render_target.id.clone(), render_target)
            })
            .collect::<HashMap<_, _>>();

        let buffer_generators_by_id = self
            .buffer_generators
            .iter()
            .map(|buffer_generator| {
                let generator =
                    buffer_generator.load(context, &control_id, &mut control_set_builder);
                (buffer_generator.id.clone(), Arc::new(generator))
            })
            .collect::<HashMap<_, _>>();

        let passes = self
            .steps
            .iter()
            .map(|pass| {
                pass.load(
                    context,
                    resource_repository,
                    &mut control_set_builder,
                    &render_targets_by_id,
                    &buffer_generators_by_id,
                    &control_id,
                    path,
                )
            })
            .collect::<Result<Vec<_>>>()?;

        let render_targets = render_targets_by_id.into_values().collect::<Vec<_>>();
        let buffer_generators = buffer_generators_by_id.into_values().collect::<Vec<_>>();

        let chart = render::chart::Chart::new(
            id,
            &control_id,
            control_set_builder,
            render_targets,
            buffer_generators,
            passes,
        );
        Ok(chart)
    }
}

impl BufferGenerator {
    pub fn load(
        &self,
        context: &VulkanContext,
        parent_id: &ControlId,
        control_set_builder: &mut ControlSetBuilder,
    ) -> render::buffer_generator::BufferGenerator {
        let control_id = parent_id.add(ControlIdPartType::BufferGenerator, &self.id);

        render::buffer_generator::BufferGenerator::new(
            self.size,
            context,
            &control_id,
            control_set_builder,
            &self.generator,
        )
    }
}

impl Draw {
    #[allow(clippy::too_many_arguments)]
    pub fn load(
        &self,
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
        control_set_builder: &mut ControlSetBuilder,
        render_targets_by_id: &HashMap<String, Arc<render::render_target::RenderTarget>>,
        buffer_generators_by_id: &HashMap<String, Arc<render::buffer_generator::BufferGenerator>>,
        chart_id: &ControlId,
        path: &ResourcePath,
    ) -> Result<render::draw::Draw> {
        let control_prefix = chart_id.add(ControlIdPartType::Pass, &self.id);
        let chart_id = chart_id.add(ControlIdPartType::ChartValues, "Chart Values");
        let render_targets = self
            .render_targets
            .iter()
            .map(|render_target_id| {
                render_targets_by_id
                    .get(render_target_id)
                    .or_else(|| context.swapchain_render_targets_by_id.get(render_target_id))
                    .cloned()
                    .with_context(|| anyhow!("Render target '{}' not found", render_target_id))
            })
            .collect::<Result<Vec<_>>>()?;

        let objects = self
            .objects
            .iter()
            .map(|object| {
                object.load(
                    &control_prefix,
                    &chart_id,
                    context,
                    resource_repository,
                    control_set_builder,
                    render_targets_by_id,
                    buffer_generators_by_id,
                    path,
                )
            })
            .collect::<Result<Vec<_>>>()?;

        let pass =
            render::draw::Draw::new(context, &self.id, render_targets, objects, self.clear_color)?;
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
    #[allow(clippy::too_many_arguments)]
    pub fn load(
        &self,
        parent_id: &ControlId,
        chart_id: &ControlId,
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
        control_set_builder: &mut ControlSetBuilder,
        render_targets_by_id: &HashMap<String, Arc<render::render_target::RenderTarget>>,
        buffer_generators_by_id: &HashMap<String, Arc<render::buffer_generator::BufferGenerator>>,
        path: &ResourcePath,
    ) -> Result<Arc<render::RenderObject>> {
        let control_id = parent_id.add(ControlIdPartType::Object, &self.id);
        let mesh = resource_repository
            .get_mesh(
                context,
                &path.relative_path(&self.mesh_file),
                &self.mesh_name,
            )?
            .clone();

        let sampler_sources_by_id = self
            .textures
            .iter()
            .map(|(name, texture_mapping)| {
                let texture_binding: DescriptorSource = match texture_mapping {
                    TextureMapping::File(texture_path) => {
                        let texture = resource_repository
                            .get_texture(context, &path.relative_path(texture_path))?
                            .clone();
                        DescriptorSource::Texture(texture)
                    }
                    TextureMapping::RenderTargetId(id) => {
                        let render_target = render_targets_by_id
                            .get(id)
                            .with_context(|| anyhow!("Render target '{}' not found", id))?;
                        DescriptorSource::RenderTarget(render_target.clone())
                    }
                };
                Ok((name.clone(), texture_binding))
            })
            .collect::<Result<HashMap<String, DescriptorSource>>>()?;

        let buffer_sources_by_id = self
            .buffers
            .iter()
            .map(|(name, buffer_mapping)| {
                let buffer_binding: DescriptorSource = match buffer_mapping {
                    BufferMapping::BufferGeneratorId(id) => {
                        let buffer_generator = buffer_generators_by_id
                            .get(id)
                            .with_context(|| anyhow!("Buffer generator '{}' not found", id))?;
                        DescriptorSource::BufferGenerator(buffer_generator.clone())
                    }
                };
                Ok((name.clone(), buffer_binding))
            })
            .collect::<Result<HashMap<String, DescriptorSource>>>()?;

        let solid_step = self.make_material_step(
            context,
            resource_repository,
            control_set_builder,
            &control_id,
            chart_id,
            &self.control_map,
            &sampler_sources_by_id,
            &buffer_sources_by_id,
            path,
        )?;
        let material = Material {
            passes: [None, None, Some(solid_step)],
            sampler_address_mode: self.sampler_address_mode.load(),
        };

        let position_id = control_id.add(ControlIdPartType::Value, "position");
        let rotation_id = control_id.add(ControlIdPartType::Value, "rotation");
        let instances_id = control_id.add(ControlIdPartType::Value, "instances");

        let object = render::RenderObject {
            id: self.id.clone(),
            mesh,
            position: control_set_builder.get_vec3(&position_id),
            rotation: control_set_builder.get_vec3(&rotation_id),
            instances: control_set_builder.get_float_with_default(&instances_id, 1.),
            material,
        };
        Ok(Arc::new(object))
    }

    #[allow(clippy::too_many_arguments)]
    fn make_material_step(
        &self,
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
        control_set_builder: &mut ControlSetBuilder,
        parent_id: &ControlId,
        chart_id: &ControlId,
        control_map: &HashMap<String, String>,
        sampler_sources_by_id: &HashMap<String, DescriptorSource>,
        buffer_sources_by_id: &HashMap<String, DescriptorSource>,
        path: &ResourcePath,
    ) -> Result<MaterialStep> {
        let shaders = resource_repository.shader_cache.get_or_load(
            context,
            &path.relative_path(&self.vertex_shader),
            &path.relative_path(&self.fragment_shader),
            &path.relative_path(COMMON_SHADER_FILE),
        )?;

        let vertex_shader = make_shader(
            control_set_builder,
            parent_id,
            chart_id,
            control_map,
            &shaders.vertex_shader,
            sampler_sources_by_id,
            buffer_sources_by_id,
        )?;
        let fragment_shader = make_shader(
            control_set_builder,
            parent_id,
            chart_id,
            control_map,
            &shaders.fragment_shader,
            sampler_sources_by_id,
            buffer_sources_by_id,
        )?;

        let material_step = MaterialStep {
            vertex_shader,
            fragment_shader,
            depth_test: self.depth_test,
            depth_write: self.depth_write,
            blend_mode: self.blend_mode.load(),
            sampler_address_mode: self.sampler_address_mode.load(),
        };
        Ok(material_step)
    }
}

impl BlendMode {
    pub fn load(&self) -> render::material::BlendMode {
        match self {
            BlendMode::None => render::material::BlendMode::None,
            BlendMode::Alpha => render::material::BlendMode::Alpha,
            BlendMode::Additive => render::material::BlendMode::Additive,
        }
    }
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

#[instrument(skip_all)]
fn make_shader(
    control_set_builder: &mut ControlSetBuilder,
    parent_id: &ControlId,
    chart_id: &ControlId,
    control_map: &HashMap<String, String>,
    compilation_result: &ShaderCompilationResult,
    sampler_sources_by_id: &HashMap<String, DescriptorSource>,
    buffer_sources_by_id: &HashMap<String, DescriptorSource>,
) -> Result<Shader> {
    let local_mapping = compilation_result
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

    let mut sampler_bindings = compilation_result
        .samplers
        .iter()
        .map(|sampler| {
            let sampler_source = sampler_sources_by_id
                .get(&sampler.name)
                .cloned()
                .with_context(|| format!("Sampler binding '{}' not found", sampler.name))?;
            Ok(DescriptorBinding {
                descriptor_source: sampler_source,
                descriptor_set_binding: sampler.binding,
            })
        })
        .collect::<Result<Vec<DescriptorBinding>>>()?;

    let mut buffer_bindings = compilation_result
        .buffers
        .iter()
        .map(|buffer| {
            let buffer_source = buffer_sources_by_id
                .get(&buffer.name)
                .cloned()
                .with_context(|| format!("Buffer binding '{}' not found", buffer.name))?;
            Ok(DescriptorBinding {
                descriptor_source: buffer_source,
                descriptor_set_binding: buffer.binding,
            })
        })
        .collect::<Result<Vec<DescriptorBinding>>>()?;

    let mut descriptor_bindings = vec![];
    descriptor_bindings.append(&mut sampler_bindings);
    descriptor_bindings.append(&mut buffer_bindings);

    Ok(Shader {
        shader_module: compilation_result.module.clone(),
        descriptor_bindings,
        local_uniform_bindings: local_mapping,
        global_uniform_bindings: compilation_result.global_uniform_bindings.clone(),
        uniform_buffer_size: compilation_result.uniform_buffer_size,
    })
}
