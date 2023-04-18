use crate::file::resource_repository::ResourceRepository;
use crate::render;
use crate::render::vulkan_window::VulkanContext;
use anyhow::Result;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

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
                Ok((chart_name.clone(), Rc::new(chart)))
            })
            .collect::<Result<_>>()?;
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
        Ok(render::project::Project {
            charts_by_id: charts_by_name,
            cuts,
        })
    }
}
