use std::collections::HashMap;
use std::rc::Rc;

use anyhow::Result;
use serde::Deserialize;

use crate::engine::{ControlId, ControlIdPartType};
use crate::file::chart_file::ChartContext;
use crate::{engine, file};

#[derive(Debug, Deserialize)]
pub struct Object {
    pub id: String,
    pub mesh_file: String,
    pub mesh_name: String,
    pub material: file::material::Material,

    #[serde(default)]
    pub control_map: HashMap<String, String>,
}

impl Object {
    pub async fn load(
        &self,
        chart_context: &ChartContext,
        parent_id: &ControlId,
        passes: &[engine::Pass],
    ) -> Result<Rc<engine::RenderObject>> {
        let object_cid = parent_id.add(ControlIdPartType::Object, &self.id);
        let mesh_future = chart_context.resource_repository.get_mesh(
            &chart_context.gpu_context,
            &chart_context.path.relative_path(&self.mesh_file)?,
            &self.mesh_name,
        );

        // Load material
        let material =
            self.material.load(chart_context, passes, &self.control_map, &object_cid).await?;

        // Wait for resources to be loaded
        let mesh = mesh_future.get().await?;

        let position_id = object_cid.add(ControlIdPartType::Value, "position");
        let rotation_id = object_cid.add(ControlIdPartType::Value, "rotation");
        let instances_id = object_cid.add(ControlIdPartType::Value, "instances");

        let object = crate::engine::RenderObject {
            _id: self.id.clone(),
            mesh,
            material,
            position: chart_context.control_set_builder.get_vec3(&position_id),
            rotation: chart_context.control_set_builder.get_vec3(&rotation_id),
            instances: chart_context.control_set_builder.get_float_with_default(&instances_id, 1.),
        };
        Ok(Rc::new(object))
    }
}
