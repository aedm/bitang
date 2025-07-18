use std::cell::{Cell, RefCell};
use std::cmp::max;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::{array, mem};

use ahash::AHashSet;
use anyhow::{Context, Result};
use dashmap::mapref::entry::Entry::{Occupied, Vacant};
use dashmap::DashMap;
use glam::{Vec2, Vec3, Vec4};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tracing::{debug, info, instrument, warn};

use super::spline::Spline;
use super::ControlIdPartType::Chart;
use super::{ControlId, ControlIdPart, ControlIdPartType, RcHashRef};
use crate::engine::project::Project;
use crate::loader::CHARTS_FOLDER;

const CONTROLS_FILE_NAME: &str = "controls.ron";

#[derive(Default)]
pub struct UsedControlsNode {
    pub id_prefix: ControlId,
    pub children: Vec<UsedControlsNode>,
    pub control: Option<Arc<Control>>,
}

impl UsedControlsNode {
    fn insert(&mut self, control: Arc<Control>, chart_step_ids: &[String]) {
        for i in 0..self.id_prefix.parts.len() {
            assert_eq!(self.id_prefix.parts[i], control.id.parts[i]);
        }

        if self.id_prefix.parts.len() == control.id.parts.len() {
            self.control = Some(control);
            return;
        }

        let child_prefix = control.id.prefix(self.id_prefix.parts.len() + 1);
        if let Some(child) = self.children.iter_mut().find(|x| x.id_prefix == child_prefix) {
            child.insert(control, chart_step_ids);
        } else {
            let mut new_node = UsedControlsNode {
                id_prefix: child_prefix,
                ..UsedControlsNode::default()
            };
            new_node.insert(control, chart_step_ids);
            let n = self.id_prefix.parts.len();
            let new_part = &new_node.id_prefix.parts[n];
            let mut i = 0;
            while i < self.children.len() {
                let child_part = &self.children[i].id_prefix.parts[n];
                if new_part.part_type < child_part.part_type {
                    break;
                }
                if child_part.part_type == new_part.part_type {
                    // TODO: only sorts chart steps correctly.
                    // If two controls are on the same level, it checks their names in the chart_step_ids list.
                    let child_index = chart_step_ids.iter().position(|x| x == &child_part.name);
                    let new_index = chart_step_ids.iter().position(|x| x == &new_part.name);
                    if new_index < child_index {
                        break;
                    }
                }
                i += 1;
            }
            self.children.insert(i, new_node);
        }
    }
}

pub struct ControlSet {
    pub used_controls: Vec<Arc<Control>>,
    pub root_node: Mutex<UsedControlsNode>,
}

/// Builder for `ControlSet`, used during project loading.
pub struct ControlSetBuilder {
    control_repository: Arc<ControlRepository>,
    used_controls: RefCell<AHashSet<RcHashRef<Control>>>,
    used_control_list: RefCell<Vec<Arc<Control>>>,
    root_id: ControlId,
}

impl ControlSetBuilder {
    pub fn new(root_id: ControlId, control_repository: Arc<ControlRepository>) -> Self {
        Self {
            root_id,
            control_repository,
            used_controls: RefCell::new(AHashSet::new()),
            used_control_list: RefCell::new(vec![]),
        }
    }

    pub fn into_control_set(self, chart_step_ids: &[String]) -> ControlSet {
        let mut root_node = UsedControlsNode {
            id_prefix: self.root_id,
            ..UsedControlsNode::default()
        };
        let mut controls = vec![];
        let mut used_control_list = self.used_control_list.borrow_mut();
        for control in mem::take(&mut *used_control_list) {
            root_node.insert(control.clone(), chart_step_ids);
            controls.push(control);
        }
        ControlSet {
            used_controls: controls,
            root_node: Mutex::new(root_node),
        }
    }

    pub fn get_float_with_default(&self, id: &ControlId, default: f32) -> Arc<Control> {
        self.get_control(id, 1, &[default, 0.0, 0.0, 0.0])
    }

    pub fn get_vec(&self, id: &ControlId, component_count: usize) -> Arc<Control> {
        self.get_control(id, component_count, &[0.0; 4])
    }

    #[allow(dead_code)]
    pub fn get_vec2_with_default(&self, id: &ControlId, default: &[f32; 2]) -> Arc<Control> {
        self.get_control(id, 2, &[default[0], default[1], 0.0, 0.0])
    }

    pub fn get_vec3(&self, id: &ControlId) -> Arc<Control> {
        self.get_control(id, 3, &[0.0; 4])
    }

    pub fn get_vec3_with_default(&self, id: &ControlId, default: &[f32; 3]) -> Arc<Control> {
        self.get_control(id, 3, &[default[0], default[1], default[2], 0.0])
    }

    pub fn get_vec4(&self, id: &ControlId) -> Arc<Control> {
        self.get_control(id, 4, &[0.0; 4])
    }

    pub fn _get_vec4_with_default(&self, id: &ControlId, default: &[f32; 4]) -> Arc<Control> {
        self.get_control(id, 4, default)
    }

    fn get_control(
        &self,
        id: &ControlId,
        component_count: usize,
        default: &[f32; 4],
    ) -> Arc<Control> {
        let control = self.control_repository.get_control(id, default);
        {
            let mut component_count_lock = control.used_component_count.lock();
            *component_count_lock = max(*component_count_lock, component_count);
            if self.used_controls.borrow_mut().insert(RcHashRef(control.clone())) {
                self.used_control_list.borrow_mut().push(control.clone());
            }
        }
        control
    }
}

pub struct ControlRepository {
    by_id: DashMap<ControlId, Arc<Control>>,
}

// We want to serialize the controls by reference, but deserialize them by value to avoid cloning them.
#[derive(Serialize)]
struct SerializedControls {
    controls: Vec<Arc<Control>>,
}

#[derive(Deserialize)]
struct DeserializedControls {
    controls: Vec<Control>,
}

// TODO: move serialization out of "engine" mod.
impl ControlRepository {
    fn get_control(&self, id: &ControlId, default: &[f32; 4]) -> Arc<Control> {
        match self.by_id.entry(id.clone()) {
            Occupied(x) => x.get().clone(),
            Vacant(entry) => {
                let control = Arc::new(Control::new(id.clone(), default));
                entry.insert(control.clone());
                control
            }
        }
    }

    pub fn save_control_files(&self, project: &Project) -> Result<()> {
        for chart in project.charts_by_id.values() {
            let path =
                project.root_path.join(CHARTS_FOLDER).join(&chart.id).join(CONTROLS_FILE_NAME);
            let controls = self
                .by_id
                .iter()
                .filter(|it| it.id.parts[0].part_type == Chart && it.id.parts[0].name == chart.id)
                .map(|it| it.value().clone())
                .collect();
            let serialized = SerializedControls { controls };
            let ron = ron::ser::to_string_pretty(&serialized, ron::ser::PrettyConfig::default())?;
            std::fs::write(&path, ron)
                .with_context(|| format!("Failed to write controls to {path:?}."))?;
            debug!("Saved controls to {path:?}.");
        }
        Ok(())
    }

    #[instrument]
    pub fn load_control_files(root_path: &Path) -> Result<Self> {
        let by_id = DashMap::new();
        let path = root_path.join(CHARTS_FOLDER);
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let chart_id = path.file_name().unwrap().to_str().unwrap();
                let controls_path =
                    root_path.join(CHARTS_FOLDER).join(chart_id).join(CONTROLS_FILE_NAME);
                if let Ok(ron) = std::fs::read_to_string(&controls_path) {
                    info!("Loading {controls_path:?}.");
                    let deserialized: DeserializedControls = ron::de::from_str(&ron)?;
                    for mut control in deserialized.controls {
                        control.id.parts.insert(
                            0,
                            ControlIdPart {
                                part_type: Chart,
                                name: chart_id.to_string(),
                            },
                        );
                        by_id.insert(control.id.clone(), Arc::new(control));
                    }
                } else {
                    warn!("No controls file found at {controls_path:?}.");
                }
            }
        }
        Ok(Self { by_id })
    }

    pub fn reset_component_usage_counts(&self) {
        for it in self.by_id.iter() {
            *it.value().used_component_count.lock() = 0;
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

    // TODO: two Mutexes are unsound, merge them
    pub components: Mutex<[ControlComponent; 4]>,

    #[serde(skip)]
    pub used_component_count: Mutex<usize>,
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
    let parts =
        parts.into_iter().map(|(part_type, name)| ControlIdPart { part_type, name }).collect();
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
            components: Mutex::new(array::from_fn(|i| ControlComponent {
                value: value[i],
                spline: Spline::new(),
                use_spline: false,
            })),
            used_component_count: Mutex::new(0),
        }
    }

    pub fn set(&self, value: &[f32; 4]) {
        let mut components = self.components.lock();
        for i in 0..4 {
            components[i].value = value[i];
        }
    }

    pub fn evaluate_splines(&self, time: f32) {
        let mut components = self.components.lock();
        for component in components.iter_mut() {
            if component.use_spline {
                component.value = component.spline.get_value(time);
            }
        }
    }

    pub fn as_float(&self) -> f32 {
        self.components.lock()[0].value
    }

    #[allow(dead_code)]
    pub fn as_vec2(&self) -> Vec2 {
        let components = self.components.lock();
        Vec2::new(components[0].value, components[1].value)
    }

    pub fn as_vec3(&self) -> Vec3 {
        let components = self.components.lock();
        Vec3::new(
            components[0].value,
            components[1].value,
            components[2].value,
        )
    }

    pub fn as_vec4(&self) -> Vec4 {
        let components = self.components.lock();
        Vec4::new(
            components[0].value,
            components[1].value,
            components[2].value,
            components[3].value,
        )
    }
}
