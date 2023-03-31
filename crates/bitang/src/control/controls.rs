use crate::control::spline::Spline;
use crate::control::RcHashRef;
use anyhow::anyhow;
use anyhow::Result;
use glam::Mat4;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::rc::Rc;
use std::{array, mem, slice};
use tracing::{debug, error};

#[derive(Serialize, Deserialize, Default)]
pub struct Controls {
    pub by_id: HashMap<String, Rc<Control>>,

    #[serde(skip)]
    pub used_controls: Vec<Rc<Control>>,

    #[serde(skip)]
    used_control_collector: HashSet<RcHashRef<Control>>,
}

impl Controls {
    pub fn get_control(&mut self, id: &str) -> Rc<Control> {
        if let Some(x) = self.by_id.get(id) {
            self.used_control_collector.insert(RcHashRef(x.clone()));
            return x.clone();
        }
        let control = Rc::new(Control::new(id));
        self.by_id.insert(id.to_string(), control.clone());
        self.used_control_collector
            .insert(RcHashRef(control.clone()));
        control
    }

    pub fn start_load_cycle(&mut self) {
        self.used_control_collector.clear();
    }

    pub fn finish_load_cycle(&mut self) {
        self.used_controls = mem::take(&mut self.used_control_collector)
            .into_iter()
            .map(|x| x.0.clone())
            .collect();
        self.used_controls.sort_by(|a, b| a.id.cmp(&b.id));
        debug!(
            "Used controls: {:?}",
            self.used_controls.iter().map(|x| &x.id).collect::<Vec<_>>()
        );
    }
}

#[derive(Serialize, Deserialize)]
pub struct Control {
    pub id: String,
    pub components: RefCell<[ControlComponent; 4]>,
}

#[derive(Serialize, Deserialize)]
pub struct ControlComponent {
    pub value: f32,
    pub spline: Spline,
    pub use_spline: bool,
}

impl Control {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            // value: RefCell::new(ControlValue::Scalars([0.0; 4])),
            components: RefCell::new(array::from_fn(|_| ControlComponent {
                value: 0.0,
                spline: Spline::new(),
                use_spline: false,
            })),
        }
    }

    pub fn evaluate_splines(&self, time: f32) {
        let mut components = self.components.borrow_mut();
        for component in components.iter_mut() {
            if component.use_spline {
                component.value = component.spline.get_value(time);
            }
        }
    }
}

impl ControlComponent {
    pub fn update(&mut self, time: f32) {
        if self.use_spline {
            self.value = self.spline.get_value(time);
        }
    }
}

// TODO: generate this automatically from the Globals struct somehow
#[derive(Copy, Clone, Debug)]
pub enum GlobalType {
    AppTime,
    ProjectionFromModel,
    CameraFromModel,
}

impl GlobalType {
    pub fn from_str(s: &str) -> Result<GlobalType> {
        match s {
            "app_time" => Ok(GlobalType::AppTime),
            "projection_from_model" => Ok(GlobalType::ProjectionFromModel),
            "camera_from_model" => Ok(GlobalType::CameraFromModel),
            _ => Err(anyhow!("Unknown global type: {}", s)),
        }
    }
}

#[derive(Default)]
pub struct Globals {
    pub app_time: f32,
    pub projection_from_model: Mat4,
    pub camera_from_model: Mat4,
}

impl Globals {
    pub fn get(&self, global_type: GlobalType) -> &[f32] {
        match global_type {
            GlobalType::AppTime => slice::from_ref(&self.app_time),
            GlobalType::ProjectionFromModel => self.projection_from_model.as_ref(),
            GlobalType::CameraFromModel => self.camera_from_model.as_ref(),
        }
    }
}
