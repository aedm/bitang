use crate::control::controls::{ControlRepository, ControlSet};
use crate::control::{ControlId, ControlIdPartType};
use crate::render::chart::Chart;
use crate::render::project::Project;
use crate::tool::timer::Timer;
use std::sync::Arc;

pub struct AppState {
    pub project: Option<Arc<Project>>,
    pub selected_control_id: ControlId,
    pub cursor_time: f32,
    // pub time: f32,
    cursor: Timer,
    // pub is_simulation_playing: bool,
    // pub is_playing: bool,
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
            // time: 0.0,
            // is_playing: false,
            cursor: Timer::new(),
            cursor_time: 0.0,
            control_repository,
        }
    }

    pub fn tick(&mut self) {
        self.cursor_time = self.cursor.now();
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

    /// Returns the cursor position in seconds.
    ///
    /// If a chart is selected, it returns the time relative to the start of the chart
    /// so music can be played properly.
    pub fn get_project_relative_time(&self) -> f32 {
        let cursor_time = self.cursor.now();
        if let Some(part) = self.selected_control_id.parts.first() {
            if let Some(project) = &self.project {
                if part.part_type == ControlIdPartType::Chart {
                    if let Some(time) = project
                        .cuts
                        .iter()
                        .find(|cut| cut.chart.id == part.name)
                        .map(|cut| cut.start_time)
                    {
                        return time + cursor_time;
                    }
                }
            }
        }
        cursor_time
    }

    pub fn start(&mut self) {
        self.cursor.start();
    }

    pub fn pause(&mut self) {
        self.cursor.pause();
    }

    pub fn set_time(&mut self, time: f32) {
        self.pause();
        self.cursor.set(time);
        self.cursor_time = time;
    }

    pub fn is_playing(&self) -> bool {
        self.cursor.is_playing()
    }
}
