use crate::control::controls::ControlsAndGlobals;
use crate::file::resource_repository::ResourceRepository;
use crate::render;
use crate::render::vulkan_window::VulkanContext;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::array;
use std::collections::HashMap;

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
        resource_repository: &mut ResourceRepository,
        controls: &mut ControlsAndGlobals,
    ) -> Result<render::chart::Chart> {
        // Set uniforms
        for (uniform_name, uniform_value) in &object.uniforms {
            if uniform_value.is_empty() {
                return Err(anyhow!(
                    "Uniform '{}' has no values. Object id '{}'",
                    uniform_name,
                    object.id
                ));
            }
            let _control_id = Self::make_control_id_for_object(&object.id, uniform_name);
            let _value: [f32; 4] = array::from_fn(|i| uniform_value[i % uniform_value.len()]);
            // controls.get_control(&control_id).set_scalar(value);
        }

        let mesh = self.get_mesh(context, &object.mesh_path)?.clone();
        let texture = self.get_texture(context, &object.texture_path)?.clone();
        let solid_step = self.make_material_step(context, controls, &object, &texture)?;
        let material = Material {
            passes: [None, None, Some(solid_step)],
        };

        let render_object = RenderObject {
            mesh,
            material,
            position: Default::default(),
            rotation: Default::default(),
        };
        Ok(render_object)
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

pub fn load_root_chart(
    &mut self,
    context: &VulkanContext,
    controls: &mut ControlsAndGlobals,
) -> Result<RenderObject> {
    let chart = self
        .root_ron_file_cache
        .get_or_load(context, "test-chart/chart.ron")?
        .clone();

    // Set uniforms
    for (uniform_name, uniform_value) in &object.uniforms {
        if uniform_value.is_empty() {
            return Err(anyhow!(
                "Uniform '{}' has no values. Object id '{}'",
                uniform_name,
                object.id
            ));
        }
        let _control_id = Self::make_control_id_for_object(&object.id, uniform_name);
        let _value: [f32; 4] = array::from_fn(|i| uniform_value[i % uniform_value.len()]);
        // controls.get_control(&control_id).set_scalar(value);
    }

    let mesh = self.get_mesh(context, &object.mesh_path)?.clone();
    let texture = self.get_texture(context, &object.texture_path)?.clone();
    let solid_step = self.make_material_step(context, controls, &object, &texture)?;
    let material = Material {
        passes: [None, None, Some(solid_step)],
    };

    let render_object = RenderObject {
        mesh,
        material,
        position: Default::default(),
        rotation: Default::default(),
    };
    Ok(render_object)
}
