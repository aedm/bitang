use crate::control::spline::Spline;
use crate::control::{ControlId, RcHashRef};
use anyhow::anyhow;
use anyhow::Result;
use glam::Mat4;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::{array, mem, slice};

#[derive(Default)]
pub struct UsedControlsNode {
    pub id_prefix: ControlId,
    pub children: Vec<Rc<RefCell<UsedControlsNode>>>,
    pub control: Option<Rc<Control>>,
}

impl UsedControlsNode {
    fn insert(&mut self, control: Rc<Control>) {
        for i in 0..self.id_prefix.parts.len() {
            assert_eq!(self.id_prefix.parts[i], control.id.parts[i]);
        }

        if self.id_prefix.parts.len() == control.id.parts.len() {
            self.control = Some(control);
            return;
        }

        let child_prefix = control.id.prefix(self.id_prefix.parts.len() + 1);
        if let Some(child) = self
            .children
            .iter_mut()
            .find(|x| x.borrow().id_prefix == child_prefix)
        {
            child.borrow_mut().insert(control);
        } else {
            let child = Rc::new(RefCell::new(UsedControlsNode {
                id_prefix: child_prefix,
                ..UsedControlsNode::default()
            }));
            child.borrow_mut().insert(control);
            self.children.push(child);
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct Controls {
    pub by_id: HashMap<ControlId, Rc<Control>>,

    #[serde(skip)]
    pub used_controls_root: RefCell<UsedControlsNode>,

    #[serde(skip)]
    pub used_controls_list: Vec<Rc<Control>>,

    #[serde(skip)]
    used_control_collector: HashSet<RcHashRef<Control>>,
}

impl Controls {
    pub fn get_control(&mut self, id: &ControlId) -> Rc<Control> {
        if let Some(x) = self.by_id.get(id) {
            self.used_control_collector.insert(RcHashRef(x.clone()));
            return x.clone();
        }
        let control = Rc::new(Control::new(id.clone()));
        self.by_id.insert(id.clone(), control.clone());
        self.used_control_collector
            .insert(RcHashRef(control.clone()));
        control
    }

    pub fn reset_usage_collector(&mut self) {
        self.used_control_collector.clear();
        self.used_controls_list.clear();
        self.used_controls_root.borrow_mut().children.clear();
    }

    pub fn finish_load_cycle(&mut self) {
        let mut controls: Vec<_> = mem::take(&mut self.used_control_collector)
            .into_iter()
            .map(|x| x.0.clone())
            .collect();
        controls.sort_by(|a, b| a.id.cmp(&b.id));

        for control in controls {
            self.used_controls_list.push(control.clone());
            self.used_controls_root.borrow_mut().insert(control);
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Control {
    pub id: ControlId,
    pub components: RefCell<[ControlComponent; 4]>,
}

#[derive(Serialize, Deserialize)]
pub struct ControlComponent {
    pub value: f32,
    pub spline: Spline,
    pub use_spline: bool,
}

impl Control {
    pub fn new(id: ControlId) -> Self {
        Self {
            id,
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
