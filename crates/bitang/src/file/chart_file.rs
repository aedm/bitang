use crate::control::controls::ControlsAndGlobals;
use crate::file::resource_repository::ResourceRepository;
use crate::file::shader_loader::ShaderCompilationResult;
use crate::render;
use crate::render::material::{LocalUniformMapping, SamplerBinding, SamplerSource, Shader};
use crate::render::vulkan_window::VulkanContext;
use anyhow::{anyhow, Context, Result};
use egui::plot::Text;
use serde::{Deserialize, Serialize};
use std::array;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct Chart {
    pub render_targets: Vec<RenderTarget>,
    pub passes: Vec<Pass>,
}

#[derive(Debug, Deserialize)]
pub struct RenderTarget {
    pub id: String,
    pub format: String,
    pub size: RenderTargetSize,
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
        controls: &mut ControlsAndGlobals,
    ) -> Result<render::chart::Chart> {
        let render_targets = self
            .render_targets
            .iter()
            .map(|render_target| {
                let render_target = render_target.load(context)?;
                Ok((render_target.id.clone(), render_target))
            })
            .collect::<Result<HashMap<String, Arc<render::render_target::RenderTarget>>>>()?;

        let passes = self
            .passes
            .iter()
            .map(|pass| pass.load(context, resource_repository, &render_targets, controls))
            .collect::<Result<Vec<_>>>()?;

        let render_targets = render_targets.into_values().collect::<Vec<_>>();

        render::chart::Chart::new(id, controls, render_targets, passes);

        // // Set uniforms
        // for (uniform_name, uniform_value) in &object.uniforms {
        //     if uniform_value.is_empty() {
        //         return Err(anyhow!(
        //             "Uniform '{}' has no values. Object id '{}'",
        //             uniform_name,
        //             object.id
        //         ));
        //     }
        //     let _control_id = Self::make_control_id_for_object(&object.id, uniform_name);
        //     let _value: [f32; 4] = array::from_fn(|i| uniform_value[i % uniform_value.len()]);
        //     // controls.get_control(&control_id).set_scalar(value);
        // }
        //
        // let mesh = self.get_mesh(context, &object.mesh_path)?.clone();
        // let texture = self.get_texture(context, &object.texture_path)?.clone();
        // let solid_step = self.make_material_step(context, controls, &object, &texture)?;
        // let material = Material {
        //     passes: [None, None, Some(solid_step)],
        // };
        //
        // let render_object = RenderObject {
        //     mesh,
        //     material,
        //     position: Default::default(),
        //     rotation: Default::default(),
        // };
        // Ok(render_object)
    }
}

impl Pass {
    pub fn load(
        &self,
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
        render_targets: &HashMap<String, Arc<render::render_target::RenderTarget>>,
        controls: &mut ControlsAndGlobals,
    ) -> Result<render::render_target::Pass> {
        let render_targets = self
            .render_targets
            .iter()
            .map(|render_target_id| {
                render_targets
                    .get(render_target_id)
                    .and_then(|render_target| Some(render_target.clone()))
                    .with_context(|| anyhow!("Render target '{}' not found", render_target_id))
            })
            .collect::<Result<Vec<_>>>()?;

        let objects = self
            .objects
            .iter()
            .map(|object| object.load(context, resource_repository, controls))
            .collect::<Result<Vec<_>>>()?;

        let pass = render::render_target::Pass::new(context, render_targets, objects)?;
        Ok(pass)
    }
}

impl RenderTarget {
    pub fn load(&self, context: &VulkanContext) -> Result<render::render_target::RenderTarget> {
        let size_constraint = self.size.load();
        let role = self.role.load();
        let render_target =
            render::render_target::RenderTarget::new(&self.id, role, size_constraint)?;
        Ok(render_target)
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
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
        controls: &mut ControlsAndGlobals,
        render_targets: &HashMap<String, Arc<render::render_target::RenderTarget>>,
    ) -> Result<render::RenderObject> {
        let mesh = resource_repository
            .get_mesh(context, &self.mesh_path)?
            .clone();
        let vertex_shader = resource_repository.get_shader(context, &self.vertex_shader)?;
        let fragment_shader = resource_repository.get_shader(context, &self.fragment_shader)?;
        let textures = self
            .textures
            .iter()
            .map(|(name, texture_mapping)| {
                let texture_bindig: SamplerSource = match texture_mapping {
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
                Ok((name.clone(), texture_bindig))
            })
            .collect::<Result<HashMap<String, Arc<render::texture::Texture>>>>()?;
        let depth_test = self.depth_test;
        let depth_write = self.depth_write;
        let object = render::RenderObject {
            mesh,
            vertex_shader,
            fragment_shader,
            textures,
            depth_test,
            depth_write,
        };
        Ok(object)
    }
}

pub fn make_material_step(
    context: &VulkanContext,
    controls: &mut ControlsAndGlobals,
    object: &Arc<chart_file::Object>,
    texture: &Arc<Texture>,
) -> Result<MaterialStep> {
    let shaders =
        self.shader_cache
            .get_or_load(context, &object.vertex_shader, &object.fragment_shader)?;

    let vertex_shader = Self::make_shader(controls, &object, &shaders.vertex_shader, &texture);
    let fragment_shader = Self::make_shader(controls, &object, &shaders.fragment_shader, &texture);

    let material_step = MaterialStep {
        vertex_shader,
        fragment_shader,
        depth_test: object.depth_test,
        depth_write: object.depth_write,
    };
    Ok(material_step)
}

#[instrument(skip_all)]
fn make_shader(
    controls: &mut ControlsAndGlobals,
    control_prefix: &str,
    compilation_result: &ShaderCompilationResult,
    sampler_sources: &HashMap<String, SamplerSource>,
) -> Shader {
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
                .context(format!("Sampler binding '{}' not found", sampler.name))?;
            SamplerBinding {
                sampler_source,
                descriptor_set_binding: sampler.binding,
            }
        })
        .collect::<Vec<_>>();

    Shader {
        shader_module: compilation_result.module.clone(),
        sampler_bindings,
        local_uniform_bindings: local_mapping,
        global_uniform_bindings: compilation_result.global_uniform_bindings.clone(),
        uniform_buffer_size: compilation_result.uniform_buffer_size,
    }
}

// pub fn load_root_chart(
//     &mut self,
//     context: &VulkanContext,
//     controls: &mut ControlsAndGlobals,
// ) -> Result<RenderObject> {
//     let chart = self
//         .root_ron_file_cache
//         .get_or_load(context, "test-chart/chart.ron")?
//         .clone();
//
//     // Set uniforms
//     for (uniform_name, uniform_value) in &object.uniforms {
//         if uniform_value.is_empty() {
//             return Err(anyhow!(
//                 "Uniform '{}' has no values. Object id '{}'",
//                 uniform_name,
//                 object.id
//             ));
//         }
//         let _control_id = Self::make_control_id_for_object(&object.id, uniform_name);
//         let _value: [f32; 4] = array::from_fn(|i| uniform_value[i % uniform_value.len()]);
//         // controls.get_control(&control_id).set_scalar(value);
//     }
//
//     let mesh = self.get_mesh(context, &object.mesh_path)?.clone();
//     let texture = self.get_texture(context, &object.texture_path)?.clone();
//     let solid_step = self.make_material_step(context, controls, &object, &texture)?;
//     let material = Material {
//         passes: [None, None, Some(solid_step)],
//     };
//
//     let render_object = RenderObject {
//         mesh,
//         material,
//         position: Default::default(),
//         rotation: Default::default(),
//     };
//     Ok(render_object)
// }
