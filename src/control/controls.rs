use anyhow::anyhow;
use anyhow::Result;
use glam::Mat4;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use std::slice;

pub struct Controls {
    pub controls_by_id: HashMap<String, Rc<Control>>,
    pub globals: Globals,
}

impl Controls {
    pub fn new() -> Self {
        Self {
            controls_by_id: HashMap::new(),
            globals: Globals::default(),
        }
    }

    pub fn get_control(&mut self, id: &str) -> Rc<Control> {
        if let Some(x) = self.controls_by_id.get(id) {
            return x.clone();
        }
        let control = Rc::new(Control::new());
        self.controls_by_id.insert(id.to_string(), control.clone());
        control
    }
}

pub struct Control {
    value: RefCell<ControlValue>,
}

pub enum ControlValue {
    Scalars([f32; 4]),
    Splines(),
}

impl Control {
    pub fn new() -> Self {
        Self {
            value: RefCell::new(ControlValue::Scalars([0.0; 4])),
        }
    }

    pub fn get_value(&self, index: usize, _time: f32) -> f32 {
        match self.value.borrow().deref() {
            ControlValue::Scalars(x) => x[index],
            ControlValue::Splines() => 0.0,
        }
    }

    pub fn set_scalar(&self, value: [f32; 4]) {
        *self.value.borrow_mut() = ControlValue::Scalars(value);
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
