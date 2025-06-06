use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use futures::future::join_all;
use serde::Deserialize;
use tracing::debug;

use crate::engine;
use crate::engine::GpuContext;
use crate::loader::resource_repository::ResourceRepository;

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
    pub async fn load(
        &self,
        context: &Arc<GpuContext>,
        resource_repository: &Rc<ResourceRepository>,
    ) -> Result<engine::Project> {
        let chart_ids: HashSet<_> = self.cuts.iter().map(|cut| &cut.chart).collect();
        let chart_futures_by_id = chart_ids.iter().map(|&chart_name| async move {
            let now = Instant::now();
            let chart = resource_repository.load_chart(chart_name, context).await?;
            debug!("Loaded chart {} in {:?}", chart_name, now.elapsed());
            Ok((chart_name.clone(), chart))
        });

        // Load all charts in parallel.
        let charts_by_id: HashMap<_, _> =
            join_all(chart_futures_by_id).await.into_iter().collect::<Result<_>>()?;

        let cuts = self
            .cuts
            .iter()
            .map(|cut| engine::Cut {
                chart: charts_by_id[&cut.chart].clone(),
                start_time: cut.start_time,
                end_time: cut.end_time,
                offset: cut.offset,
            })
            .collect();
        Ok(engine::Project::new(
            &resource_repository.root_path,
            charts_by_id,
            cuts,
        ))
    }
}
