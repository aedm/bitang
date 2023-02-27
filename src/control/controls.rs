use glam::{Mat4, Vec4};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use std::slice;

pub struct Control {
    value: RefCell<ControlValue>,
}

pub enum ControlValue {
    Scalars([f32; 16]), // Can store up to a 4x4 matrix
    Splines(),
}

pub struct Controls {
    controls_by_id: HashMap<String, Rc<Control>>,
    globals: HashMap<String, Rc<Control>>,
}

pub enum GlobalType {
    AppTime,
    ModelToProjection,
    ModelToCamera,
}

pub struct Globals {
    pub app_time: f32,
    pub model_to_projection: Mat4,
    pub model_to_camera: Mat4,
}

impl Controls {
    pub fn new() -> Self {
        let globals = HashMap::from([
            ("app_time".to_string(), Rc::new(Control::new())),
            ("model_to_projection".to_string(), Rc::new(Control::new())),
            ("model_to_camera".to_string(), Rc::new(Control::new())),
        ]);
        Self {
            controls_by_id: HashMap::new(),
            globals,
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

    fn set_global_matrix(&mut self, id: &str, value: Mat4) {
        if let Some(control) = self.globals.get(id) {
            *control.value.borrow_mut() = ControlValue::Scalars(value.to_cols_array());
        } else {
            println!("ERROR: Unknown global: {}", id);
        }
    }
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

impl Globals {
    pub fn get_global(&self, global_type: GlobalType) -> &[f32] {
        match global_type {
            GlobalType::AppTime => slice::from_ref(&self.app_time),
            GlobalType::ModelToProjection => self.model_to_projection.as_ref(),
            GlobalType::ModelToCamera => self.model_to_camera.as_ref(),
        }
    }
}
