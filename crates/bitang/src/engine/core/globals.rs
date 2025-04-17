use std::slice;

use glam::{Mat4, Vec2, Vec3};
use strum::EnumString;

#[derive(Copy, Clone, EnumString, Debug)]
#[strum(serialize_all = "snake_case")]
pub enum GlobalType {
    // TODO: Rename to something that makes more sense in simulation context
    /// Total elapsed time since the app started. During simulation, it's the simulation time.
    AppTime,

    /// Elapsed time relative to the current chart.
    ChartTime,

    ProjectionFromModel,
    LightProjectionFromModel,
    LightProjectionFromWorld,
    ProjectionFromCamera,
    ProjectionFromWorld,
    CameraFromModel,
    CameraFromWorld,
    WorldFromModel,
    InstanceCount,
    PixelSize,
    AspectRatio,
    ZNear,
    FieldOfView,

    /// Direction of light from the light source at infinite distance.
    LightDirWorldspaceNorm,
    LightDirCamspaceNorm,

    ShadowMapSize,

    /// The ratio between two consecutive frames in the simulation. 0..=1.
    /// During rendering, simulation buffers must be blended using this ratio
    /// between Current and Next states.
    SimulationFrameRatio,

    SimulationStepSeconds,
}

// TODO: move to engine
#[derive(Default, Copy, Clone, Debug)]
pub struct Globals {
    pub projection_from_model: Mat4,
    pub camera_from_model: Mat4,
    pub projection_from_camera: Mat4,
    pub projection_from_world: Mat4,
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
    pub light_dir_worldspace_norm: Vec3,
    pub light_dir_camspace_norm: Vec3,
    pub shadow_map_size: f32,
    pub simulation_frame_ratio: f32,
    pub simulation_step_seconds: f32,

    // TODO: find a better place for these
    pub simulation_elapsed_time_since_last_render: f32,
    pub is_paused: bool,
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
            GlobalType::LightDirWorldspaceNorm => self.light_dir_worldspace_norm.as_ref(),
            GlobalType::LightDirCamspaceNorm => self.light_dir_camspace_norm.as_ref(),
            GlobalType::ShadowMapSize => slice::from_ref(&self.shadow_map_size),
            GlobalType::SimulationFrameRatio => slice::from_ref(&self.simulation_frame_ratio),
            GlobalType::SimulationStepSeconds => slice::from_ref(&self.simulation_step_seconds),
            GlobalType::ProjectionFromWorld => self.projection_from_world.as_ref(),
        }
    }

    pub fn update_compound_matrices(&mut self) {
        self.camera_from_model = self.camera_from_world * self.world_from_model;
        self.projection_from_model = self.projection_from_camera * self.camera_from_model;
        self.light_projection_from_model = self.light_projection_from_world * self.world_from_model;
        self.projection_from_world = self.projection_from_camera * self.camera_from_world;
    }
}
