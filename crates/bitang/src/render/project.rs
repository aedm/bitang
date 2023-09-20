use crate::render::chart::Chart;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

pub struct Project {
    pub charts_by_id: HashMap<String, Arc<Chart>>,
    pub charts: Vec<Arc<Chart>>,
    pub cuts: Vec<Cut>,
    pub length: f32,
}

pub struct Cut {
    pub chart: Arc<Chart>,
    pub start_time: f32,
    pub end_time: f32,
    pub offset: f32,
}

impl Project {
    pub fn new(charts_by_id: HashMap<String, Arc<Chart>>, cuts: Vec<Cut>) -> Self {
        let mut charts = vec![];
        let mut charts_inserted = HashSet::new();
        for cut in &cuts {
            if charts_inserted.insert(&cut.chart.id) {
                // Unwrap is safe because we just inserted the key.
                charts.push(cut.chart.clone());
            }
        }
        let length = cuts
            .iter()
            .map(|cut| cut.end_time)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(1.0);

        Self {
            charts_by_id,
            charts,
            cuts,
            length,
        }
    }
}
