use crate::control::{ControlId, ControlIdPartType};
use crate::file::resource_repository::ResourceRepository;
use crate::file::shader_loader::ShaderCompilationResult;
use crate::render;
use crate::render::material::{
    LocalUniformMapping, Material, MaterialStep, SamplerBinding, SamplerSource, Shader,
};
use crate::render::vulkan_window::VulkanContext;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use tracing::instrument;

#[derive(Debug, Deserialize)]
pub struct Project {
    pub cuts: Vec<Cut>,
}

#[derive(Debug, Deserialize)]
pub struct Cut {
    pub chart: String,
    pub start_time: f32,
    pub end_time: f32,
    pub offset: f32,
}

impl Project {
    pub fn load(
        &self,
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
    ) -> Result<render::project::Project> {
        let chart_names: HashSet<_> = self.cuts.iter().map(|cut| &cut.chart).collect();
        let charts_by_name: HashMap<_, _> = chart_names
            .iter()
            .map(|&chart_name| {
                let chart = resource_repository.load_chart(chart_name, context)?;
                Ok((chart_name, Rc::new(chart)))
            })
            .collect::<Result<_>>()?;
        let charts = charts_by_name.values().cloned().collect();
        let cuts = self
            .cuts
            .iter()
            .map(|cut| render::project::Cut {
                chart: charts_by_name[&cut.chart].clone(),
                start_time: cut.start_time,
                end_time: cut.end_time,
                offset: cut.offset,
            })
            .collect();
        Ok(render::project::Project { charts, cuts })
    }
}
