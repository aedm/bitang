use crate::control::controls::{ControlRepository, ControlSet};
use crate::control::{ControlId, ControlIdPartType};
use crate::render::chart::Chart;
use crate::render::project::Project;
use std::sync::Arc;

pub struct AppState {
    pub project: Option<Arc<Project>>,
    pub selected_control_id: ControlId,
    pub time: f32,
    pub is_playing: bool,
    pub control_repository: Arc<ControlRepository>,
}

impl AppState {
    pub fn new(
        project: Option<Arc<Project>>,
        control_repository: Arc<ControlRepository>,
    ) -> AppState {
        AppState {
            project,
            selected_control_id: ControlId::default(),
            time: 0.0,
            is_playing: false,
            control_repository,
        }
    }

    pub fn get_chart(&self) -> Option<Arc<Chart>> {
        let id_first = self.selected_control_id.parts.first();
        if let Some(project) = &self.project {
            if let Some(id_first) = id_first {
                if id_first.part_type == ControlIdPartType::Chart {
                    return project.charts_by_id.get(&id_first.name).cloned();
                }
            }
        }
        None
    }

    pub fn get_current_chart_control_set(&self) -> Option<Arc<ControlSet>> {
        self.get_chart().map(|chart| chart.controls.clone())
    }

    pub fn get_time(&self) -> f32 {
        if let Some(part) = self.selected_control_id.parts.first() {
            if let Some(project) = &self.project {
                if part.part_type == ControlIdPartType::Chart {
                    if let Some(time) = project
                        .cuts
                        .iter()
                        .find(|cut| cut.chart.id == part.name)
                        .map(|cut| cut.start_time)
                    {
                        return time + self.time;
                    }
                }
            }
        }
        self.time
    }
}
