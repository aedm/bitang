use anyhow::anyhow;
use anyhow::Result;
use glam::{Mat4, Vec4};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use std::slice;

pub struct Controls {
    controls_by_id: HashMap<String, Rc<Control>>,
    globals: Globals,
}

impl Controls {
    pub fn new() -> Self {
        Self {
            controls_by_id: HashMap::new(),
            globals: Globals::default(),
        }
    }

    fn get_control(&mut self, id: &str) -> Option<Rc<Control>> {
        if let Some(x) = self.controls_by_id.get(id) {
            return Some(x.clone());
        }
        let control = Rc::new(Control::new());
        self.controls_by_id.insert(id.to_string(), control.clone());
        Some(control)
    }
}

pub struct Control {
    value: RefCell<ControlValue>,
}

pub enum ControlValue {
    Scalars([f32; 16]), // Can store up to a 4x4 matrix
    Splines(),
}

impl Control {
    pub fn new() -> Self {
        Self {
            value: RefCell::new(ControlValue::Scalars([0.0; 16])),
        }
    }

    pub fn get_value(&self, index: usize, _time: f32) -> f32 {
        match self.value.borrow().deref() {
            ControlValue::Scalars(x) => x[index],
            ControlValue::Splines() => 0.0,
        }
    }
}

// TODO: generate this automatically from the Globals struct somehow
#[derive(Copy, Clone, Debug)]
pub enum GlobalType {
    AppTime,
    ModelToProjection,
    ModelToCamera,
}

impl GlobalType {
    pub fn from_str(s: &str) -> Result<GlobalType> {
        match s {
            "app_time" => Ok(GlobalType::AppTime),
            "model_to_projection" => Ok(GlobalType::ModelToProjection),
            "model_to_camera" => Ok(GlobalType::ModelToCamera),
            _ => Err(anyhow!("Unknown global type: {}", s)),
        }
    }
}

#[derive(Default)]
pub struct Globals {
    pub app_time: f32,
    pub model_to_projection: Mat4,
    pub model_to_camera: Mat4,
}

impl Globals {
    pub fn get(&self, global_type: GlobalType) -> &[f32] {
        match global_type {
            GlobalType::AppTime => slice::from_ref(&self.app_time),
            GlobalType::ModelToProjection => self.model_to_projection.as_ref(),
            GlobalType::ModelToCamera => self.model_to_camera.as_ref(),
        }
    }
}
