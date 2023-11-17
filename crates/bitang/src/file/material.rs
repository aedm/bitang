use crate::control::{ControlId, ControlIdPartType};
use crate::file::chart_file::ChartContext;
use crate::file::shader_context::{BufferSource, Sampler, ShaderContext};
use crate::file::{default_true, COMMON_SHADER_FILE};
use crate::loader::async_cache::LoadFuture;
use crate::loader::shader_compiler::ShaderArtifact;
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
        let shader_context = ShaderContext::new(
            chart_context,
            &control_map,
            &object_cid,
            &self.samplers,
            &self.buffers,
            None,
        )?;

        let material_pass_futures = passes.iter().map(|pass| async {
            if let Some(material_pass) = self.passes.get(&pass.id) {
                let pass = material_pass
                    .load(
                        &pass.id,
                        &shader_context,
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
    async fn load(
        &self,
        id: &str,
        shader_context: &ShaderContext,
        chart_context: &ChartContext,
        vulkan_render_pass: Arc<vulkano::render_pass::RenderPass>,
    ) -> Result<render::material::MaterialPass> {
        let vertex_shader_future =
            shader_context.make_shader(chart_context, ShaderKind::Vertex, &self.vertex_shader);

        let fragment_shader_future =
            shader_context.make_shader(chart_context, ShaderKind::Fragment, &self.fragment_shader);

        let [vertex_shader, fragment_shader] =
            join_all(vec![vertex_shader_future, fragment_shader_future])
                .await
                .try_into()
                .map_err(|_| anyhow!("shouldn't happen"))?;

        let material_props = MaterialPassProps {
            id: id.to_string(),
            vertex_shader: vertex_shader?,
            fragment_shader: fragment_shader?,
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
