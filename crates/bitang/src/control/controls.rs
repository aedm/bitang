use crate::control::spline::Spline;
use crate::control::ControlIdPartType::Chart;
use crate::control::{ControlId, ControlIdPart, ControlIdPartType, RcHashRef};
use crate::file::resource_repository::CHARTS_FOLDER;
use crate::file::ROOT_FOLDER;
use crate::render::project::Project;
use anyhow::Result;
use anyhow::{anyhow, Context};
use glam::{Mat4, Vec2, Vec3, Vec4};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cell::{Cell, RefCell};
use std::cmp::max;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
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
    used_control_list: Vec<Rc<Control>>,
    root_id: ControlId,
}

impl ControlSetBuilder {
    pub fn new(root_id: ControlId, control_repository: Rc<RefCell<ControlRepository>>) -> Self {
        Self {
            root_id,
            control_repository,
            used_controls: HashSet::new(),
            used_control_list: vec![],
        }
    }

    pub fn into_control_set(self) -> ControlSet {
        let mut root_node = UsedControlsNode {
            id_prefix: self.root_id,
            ..UsedControlsNode::default()
        };
        let mut controls = vec![];
        for control in self.used_control_list {
            root_node.insert(control.clone());
            controls.push(control);
        }
        ControlSet {
            used_controls: controls,
            root_node: RefCell::new(root_node),
        }
    }

    pub fn get_float_with_default(&mut self, id: &ControlId, default: f32) -> Rc<Control> {
        self.get_control(id, 1, &[default, 0.0, 0.0, 0.0])
    }

    pub fn get_vec(&mut self, id: &ControlId, component_count: usize) -> Rc<Control> {
        self.get_control(id, component_count, &[0.0; 4])
    }

    pub fn get_vec3(&mut self, id: &ControlId) -> Rc<Control> {
        self.get_control(id, 3, &[0.0; 4])
    }

    pub fn get_vec3_with_default(&mut self, id: &ControlId, default: &[f32; 3]) -> Rc<Control> {
        self.get_control(id, 3, &[default[0], default[1], default[2], 0.0])
    }

    pub fn get_vec4_with_default(&mut self, id: &ControlId, default: &[f32; 4]) -> Rc<Control> {
        self.get_control(id, 4, default)
    }

    fn get_control(
        &mut self,
        id: &ControlId,
        component_count: usize,
        default: &[f32; 4],
    ) -> Rc<Control> {
        let control = self
            .control_repository
            .borrow_mut()
            .get_control(id, default);
        control
            .used_component_count
            .set(max(control.used_component_count.get(), component_count));
        if self.used_controls.insert(RcHashRef(control.clone())) {
            self.used_control_list.push(control.clone());
        }
        control
    }
}

pub struct ControlRepository {
    by_id: HashMap<ControlId, Rc<Control>>,
}

// We want to serialize the controls by reference, but deserialize them by value to avoid cloning them.
#[derive(Serialize)]
struct SerializedControls {
    controls: Vec<Rc<Control>>,
}

#[derive(Deserialize)]
struct DeserializedControls {
    controls: Vec<Control>,
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
                    let deserialized: DeserializedControls = ron::de::from_str(&ron)?;
                    for mut control in deserialized.controls {
                        control.id.parts.insert(
                            0,
                            ControlIdPart {
                                part_type: Chart,
                                name: chart_id.to_string(),
                            },
                        );
                        by_id.insert(control.id.clone(), Rc::new(control));
                    }
                } else {
                    warn!("No controls file found at '{}'.", controls_path);
                }
            }
        }
        Ok(Self { by_id })
    }

    pub fn reset_component_usage_counts(&self) {
        for control in self.by_id.values() {
            control.used_component_count.set(0);
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Control {
    #[serde(
        serialize_with = "serialize_control_id",
        deserialize_with = "deserialize_control_id"
    )]
    pub id: ControlId,
    pub components: RefCell<[ControlComponent; 4]>,

    #[serde(skip)]
    pub used_component_count: Cell<usize>,
}

fn serialize_control_id<S>(id: &ControlId, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let parts = id
        .parts
        .iter()
        .skip_while(|x| x.part_type == ControlIdPartType::Chart)
        .map(|x| (x.part_type, &x.name))
        .collect::<Vec<_>>();
    let text = ron::ser::to_string(&parts).unwrap();
    s.serialize_str(&text)
}

fn deserialize_control_id<'de, D>(d: D) -> Result<ControlId, D::Error>
where
    D: Deserializer<'de>,
{
    let text = String::deserialize(d)?;
    let parts: Vec<(ControlIdPartType, String)> = ron::de::from_str(&text).unwrap();
    let parts = parts
        .into_iter()
        .map(|(part_type, name)| ControlIdPart { part_type, name })
        .collect();
    Ok(ControlId { parts })
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
            used_component_count: Cell::new(0),
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

    pub fn as_float(&self) -> f32 {
        self.components.borrow()[0].value
    }

    pub fn as_vec3(&self) -> Vec3 {
        let components = self.components.borrow();
        Vec3::new(
            components[0].value,
            components[1].value,
            components[2].value,
        )
    }

    pub fn as_vec4(&self) -> Vec4 {
        let components = self.components.borrow();
        Vec4::new(
            components[0].value,
            components[1].value,
            components[2].value,
            components[3].value,
        )
    }
}

// TODO: generate this automatically from the Globals struct somehow
#[derive(Copy, Clone, Debug)]
pub enum GlobalType {
    AppTime,
    ChartTime,
    ProjectionFromModel,
    CameraFromModel,
    InstanceCount,
    PixelSize,
}

impl GlobalType {
    pub fn from_str(s: &str) -> Result<GlobalType> {
        match s {
            "app_time" => Ok(GlobalType::AppTime),
            "instance_count" => Ok(GlobalType::InstanceCount),
            "chart_time" => Ok(GlobalType::ChartTime),
            "projection_from_model" => Ok(GlobalType::ProjectionFromModel),
            "camera_from_model" => Ok(GlobalType::CameraFromModel),
            "pixel_size" => Ok(GlobalType::PixelSize),
            _ => Err(anyhow!("Unknown global type: {}", s)),
        }
    }
}

#[derive(Default, Copy, Clone, Debug)]
pub struct Globals {
    pub app_time: f32,
    pub chart_time: f32,
    pub instance_count: f32,
    pub projection_from_model: Mat4,
    pub camera_from_model: Mat4,
    pub projection_from_camera: Mat4,
    pub camera_from_world: Mat4,
    pub world_from_model: Mat4,
    pub pixel_size: Vec2,
}

impl Globals {
    pub fn get(&self, global_type: GlobalType) -> &[f32] {
        match global_type {
            GlobalType::AppTime => slice::from_ref(&self.app_time),
            GlobalType::ChartTime => slice::from_ref(&self.chart_time),
            GlobalType::ProjectionFromModel => self.projection_from_model.as_ref(),
            GlobalType::CameraFromModel => self.camera_from_model.as_ref(),
            GlobalType::InstanceCount => slice::from_ref(&self.instance_count),
            GlobalType::PixelSize => self.pixel_size.as_ref(),
        }
    }

    pub fn update_compound_matrices(&mut self) {
        self.camera_from_model = self.camera_from_world * self.world_from_model;
        self.projection_from_model = self.projection_from_camera * self.camera_from_model;
    }
}
