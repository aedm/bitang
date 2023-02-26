use glam::Vec4;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;

pub struct Control {
    value: RefCell<ControlValue>,
}

pub enum ControlValue {
    Vector(Vec4),
    Spline(),
}

pub struct Controls {
    controls_by_id: HashMap<String, Rc<Control>>,
}

impl Controls {
    pub fn new() -> Self {
        Self {
            controls_by_id: HashMap::new(),
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

impl Control {
    pub fn new() -> Self {
        Self {
            value: RefCell::new(ControlValue::Vector(Vec4::new(0.0, 0.0, 0.0, 0.0))),
        }
    }

    pub fn get_value(&self, _time: f32) -> Vec4 {
        match self.value.borrow().deref() {
            ControlValue::Vector(x) => *x,
            ControlValue::Spline() => Vec4::new(0.0, 0.0, 0.0, 0.0),
        }
    }
}
