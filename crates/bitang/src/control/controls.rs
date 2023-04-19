use crate::control::spline::Spline;
use crate::control::ControlIdPartType::Chart;
use crate::control::{ControlId, RcHashRef};
use crate::file::resource_repository::CHARTS_FOLDER;
use crate::file::ROOT_FOLDER;
use crate::render::project::Project;
use anyhow::Result;
use anyhow::{anyhow, Context};
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::{array, slice};
use tracing::{debug, info, instrument, warn};

const CONTROLS_FILE_NAME: &str = "controls.ron";

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

pub struct ControlSet {
    pub used_controls: Vec<Rc<Control>>,
    pub root_node: RefCell<UsedControlsNode>,
}

pub struct ControlSetBuilder {
    control_repository: Rc<RefCell<ControlRepository>>,
    used_controls: HashSet<RcHashRef<Control>>,
    root_id: ControlId,
}

impl ControlSetBuilder {
    pub fn new(root_id: ControlId, control_repository: Rc<RefCell<ControlRepository>>) -> Self {
        Self {
            root_id,
            control_repository,
            used_controls: HashSet::new(),
        }
    }

    pub fn into_control_set(self) -> ControlSet {
        let mut root_node = UsedControlsNode {
            id_prefix: self.root_id,
            ..UsedControlsNode::default()
        };
        let mut controls = vec![];
        for control in &self.used_controls {
            root_node.insert(control.0.clone());
            controls.push(control.0.clone());
        }
        ControlSet {
            used_controls: controls,
            root_node: RefCell::new(root_node),
        }
    }

    pub fn get_control(&mut self, id: &ControlId) -> Rc<Control> {
        let control = self
            .control_repository
            .borrow_mut()
            .get_control(id, &[0.0; 4]);
        self.used_controls.insert(RcHashRef(control.clone()));
        control
    }

    pub fn get_control_with_default(&mut self, id: &ControlId, default: &[f32; 4]) -> Rc<Control> {
        let control = self
            .control_repository
            .borrow_mut()
            .get_control(id, default);
        self.used_controls.insert(RcHashRef(control.clone()));
        control
    }
}

// #[derive(Default)]
pub struct ControlRepository {
    by_id: HashMap<ControlId, Rc<Control>>,
}

#[derive(Serialize, Deserialize)]
struct SerializedControls {
    controls: Vec<Rc<Control>>,
}

impl ControlRepository {
    fn get_control(&mut self, id: &ControlId, default: &[f32; 4]) -> Rc<Control> {
        if let Some(x) = self.by_id.get(id) {
            return x.clone();
        }
        let control = Rc::new(Control::new(id.clone(), default));
        self.by_id.insert(id.clone(), control.clone());
        control
    }

    pub fn save_control_files(&self, project: &Project) -> Result<()> {
        for chart in project.charts_by_id.values() {
            let path = format!(
                "{}/{}/{}/{}",
                ROOT_FOLDER, CHARTS_FOLDER, chart.id, CONTROLS_FILE_NAME
            );
            let controls = self
                .by_id
                .iter()
                .filter(|(id, _)| id.parts[0].part_type == Chart && id.parts[0].name == chart.id)
                .map(|(_, x)| x.clone())
                .collect();
            let serialized = SerializedControls { controls };
            let ron = ron::ser::to_string_pretty(&serialized, ron::ser::PrettyConfig::default())?;
            std::fs::write(&path, ron)
                .with_context(|| format!("Failed to write controls to '{}'.", path))?;
            debug!("Saved controls to '{path}'.");
        }
        Ok(())
    }

    #[instrument]
    pub fn load_control_files() -> Result<Self> {
        let mut by_id = HashMap::new();
        let path = format!("{}/{}/", ROOT_FOLDER, CHARTS_FOLDER);
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let chart_id = path.file_name().unwrap().to_str().unwrap();
                let controls_path = format!(
                    "{}/{}/{}/{}",
                    ROOT_FOLDER, CHARTS_FOLDER, chart_id, CONTROLS_FILE_NAME
                );
                if let Ok(ron) = std::fs::read_to_string(&controls_path) {
                    info!("Loading '{}'.", controls_path);
                    let serialized: SerializedControls = ron::de::from_str(&ron)?;
                    for control in serialized.controls {
                        by_id.insert(control.id.clone(), control);
                    }
                } else {
                    warn!("No controls file found at '{}'.", controls_path);
                }
            }
        }
        Ok(Self { by_id })
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
    pub fn new(id: ControlId, value: &[f32; 4]) -> Self {
        Self {
            id,
            components: RefCell::new(array::from_fn(|i| ControlComponent {
                value: value[i],
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

    pub fn as_vec3(&self) -> Vec3 {
        let components = self.components.borrow();
        Vec3::new(
            components[0].value,
            components[1].value,
            components[2].value,
        )
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

#[derive(Default, Copy, Clone, Debug)]
pub struct Globals {
    pub app_time: f32,
    pub projection_from_model: Mat4,
    pub camera_from_model: Mat4,
    pub projection_from_camera: Mat4,
    pub camera_from_world: Mat4,
    pub world_from_model: Mat4,
}

impl Globals {
    pub fn get(&self, global_type: GlobalType) -> &[f32] {
        match global_type {
            GlobalType::AppTime => slice::from_ref(&self.app_time),
            GlobalType::ProjectionFromModel => self.projection_from_model.as_ref(),
            GlobalType::CameraFromModel => self.camera_from_model.as_ref(),
        }
    }

    pub fn update_compound_matrices(&mut self) {
        self.projection_from_model =
            self.projection_from_camera * self.camera_from_world * self.world_from_model;
        self.camera_from_model = self.camera_from_world * self.world_from_model;
    }
}
