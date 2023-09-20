use crate::control::spline::Spline;
use crate::control::ControlIdPartType::Chart;
use crate::control::{ArcHashRef, ControlId, ControlIdPart, ControlIdPartType};
use crate::loader::resource_repository::CHARTS_FOLDER;
use crate::loader::ROOT_FOLDER;
use crate::render::project::Project;
use anyhow::Context;
use anyhow::Result;
use dashmap::mapref::entry::Entry::{Occupied, Vacant};
use dashmap::{DashMap, DashSet};
use glam::{Mat4, Vec2, Vec3, Vec4};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cell::{Cell, RefCell};
use std::cmp::max;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::{array, slice};
use strum::EnumString;
use tracing::{debug, info, instrument, warn};

const CONTROLS_FILE_NAME: &str = "controls.ron";

#[derive(Default)]
pub struct UsedControlsNode {
    pub id_prefix: ControlId,
    pub children: Vec<Arc<RefCell<UsedControlsNode>>>,
    pub control: Option<Arc<Control>>,
}

impl UsedControlsNode {
    fn insert(&mut self, control: Arc<Control>) {
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
            let child = Arc::new(RefCell::new(UsedControlsNode {
                id_prefix: child_prefix,
                ..UsedControlsNode::default()
            }));
            child.borrow_mut().insert(control);
            self.children.push(child);
        }
    }
}

pub struct ControlSet {
    pub used_controls: Vec<Arc<Control>>,
    pub root_node: RefCell<UsedControlsNode>,
}

pub struct ControlSetBuilder {
    control_repository: Arc<ControlRepository>,
    used_controls: DashSet<ArcHashRef<Control>>,
    used_control_list: Mutex<Vec<Arc<Control>>>,
    root_id: ControlId,
}

impl ControlSetBuilder {
    pub fn new(root_id: ControlId, control_repository: Arc<ControlRepository>) -> Self {
        Self {
            root_id,
            control_repository,
            used_controls: DashSet::new(),
            used_control_list: Mutex::new(vec![]),
        }
    }

    pub fn into_control_set(self) -> ControlSet {
        let mut root_node = UsedControlsNode {
            id_prefix: self.root_id,
            ..UsedControlsNode::default()
        };
        let mut controls = vec![];
        let mut used_control_list = self.used_control_list.lock().unwrap();
        for control in *used_control_list {
            root_node.insert(control.clone());
            controls.push(control);
        }
        ControlSet {
            used_controls: controls,
            root_node: RefCell::new(root_node),
        }
    }

    pub fn get_float_with_default(&self, id: &ControlId, default: f32) -> Arc<Control> {
        self.get_control(id, 1, &[default, 0.0, 0.0, 0.0])
    }

    pub fn get_vec(&self, id: &ControlId, component_count: usize) -> Arc<Control> {
        self.get_control(id, component_count, &[0.0; 4])
    }

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
        control
            .used_component_count
            .set(max(control.used_component_count.get(), component_count));
        if self.used_controls.insert(ArcHashRef(control.clone())) {
            let mut used_control_list = self.used_control_list.lock().unwrap();
            used_control_list.push(control.clone());
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
            let path = format!(
                "{ROOT_FOLDER}/{CHARTS_FOLDER}/{}/{CONTROLS_FILE_NAME}",
                chart.id
            );
            let controls = self
                .by_id
                .iter()
                .filter(|it| it.id.parts[0].part_type == Chart && it.id.parts[0].name == chart.id)
                .map(|it| it.value().clone())
                .collect();
            let serialized = SerializedControls { controls };
            let ron = ron::ser::to_string_pretty(&serialized, ron::ser::PrettyConfig::default())?;
            std::fs::write(&path, ron)
                .with_context(|| format!("Failed to write controls to '{path}'."))?;
            debug!("Saved controls to '{path}'.");
        }
        Ok(())
    }

    #[instrument]
    pub fn load_control_files() -> Result<Self> {
        let mut by_id = DashMap::new();
        let path = format!("{ROOT_FOLDER}/{CHARTS_FOLDER}/");
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let chart_id = path.file_name().unwrap().to_str().unwrap();
                let controls_path = format!(
                    "{ROOT_FOLDER}/{CHARTS_FOLDER}/{}/{CONTROLS_FILE_NAME}",
                    chart_id
                );
                if let Ok(ron) = std::fs::read_to_string(&controls_path) {
                    info!("Loading '{controls_path}'.");
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
                    warn!("No controls file found at '{controls_path}'.");
                }
            }
        }
        Ok(Self { by_id })
    }

    pub fn reset_component_usage_counts(&self) {
        for it in self.by_id.iter() {
            it.value().used_component_count.set(0);
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

    pub fn as_vec2(&self) -> Vec2 {
        let components = self.components.borrow();
        Vec2::new(components[0].value, components[1].value)
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

#[derive(Copy, Clone, EnumString, Debug)]
#[strum(serialize_all = "snake_case")]
pub enum GlobalType {
    AppTime,
    ChartTime,
    ProjectionFromModel,
    LightProjectionFromModel,
    LightProjectionFromWorld,
    ProjectionFromCamera,
    CameraFromModel,
    CameraFromWorld,
    WorldFromModel,
    InstanceCount,
    PixelSize,
    AspectRatio,
    ZNear,
    FieldOfView,
    LightDir,
    ShadowMapSize,
}

#[derive(Default, Copy, Clone, Debug)]
pub struct Globals {
    pub projection_from_model: Mat4,
    pub camera_from_model: Mat4,
    pub projection_from_camera: Mat4,
    pub camera_from_world: Mat4,
    pub world_from_model: Mat4,
    pub light_projection_from_world: Mat4,
    pub light_projection_from_model: Mat4,
    pub pixel_size: Vec2,
    pub app_time: f32,
    pub chart_time: f32,
    pub instance_count: f32,
    pub aspect_ratio: f32,
    pub z_near: f32,
    pub field_of_view: f32,
    pub light_dir: Vec3,
    pub shadow_map_size: f32,
}

impl Globals {
    pub fn get(&self, global_type: GlobalType) -> &[f32] {
        match global_type {
            GlobalType::AppTime => slice::from_ref(&self.app_time),
            GlobalType::ChartTime => slice::from_ref(&self.chart_time),
            GlobalType::ProjectionFromModel => self.projection_from_model.as_ref(),
            GlobalType::LightProjectionFromModel => self.light_projection_from_model.as_ref(),
            GlobalType::LightProjectionFromWorld => self.light_projection_from_world.as_ref(),
            GlobalType::ProjectionFromCamera => self.projection_from_camera.as_ref(),
            GlobalType::CameraFromModel => self.camera_from_model.as_ref(),
            GlobalType::CameraFromWorld => self.camera_from_world.as_ref(),
            GlobalType::WorldFromModel => self.world_from_model.as_ref(),
            GlobalType::InstanceCount => slice::from_ref(&self.instance_count),
            GlobalType::PixelSize => self.pixel_size.as_ref(),
            GlobalType::AspectRatio => slice::from_ref(&self.aspect_ratio),
            GlobalType::ZNear => slice::from_ref(&self.z_near),
            GlobalType::FieldOfView => slice::from_ref(&self.field_of_view),
            GlobalType::LightDir => self.light_dir.as_ref(),
            GlobalType::ShadowMapSize => slice::from_ref(&self.shadow_map_size),
        }
    }

    pub fn update_compound_matrices(&mut self) {
        self.camera_from_model = self.camera_from_world * self.world_from_model;
        self.projection_from_model = self.projection_from_camera * self.camera_from_model;
        self.light_projection_from_model = self.light_projection_from_world * self.world_from_model;
    }
}
