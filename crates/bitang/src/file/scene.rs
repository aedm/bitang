use crate::engine::{ControlId, ControlIdPartType};
use crate::file::chart_file::ChartContext;
use crate::file::material::Material;
use crate::{engine};
use anyhow::Context;
use serde::Deserialize;
use std::collections::HashMap;
use std::rc::Rc;
use tracing::instrument;

#[derive(Debug, Deserialize)]
pub(crate) struct Scene {
    id: String,
    file: String,
    material: Material,

    #[serde(default)]
    pub control_map: HashMap<String, String>,
}

impl Scene {
    #[instrument(skip_all)]
    pub async fn load(
        &self,
        parent_id: &ControlId,
        chart_context: &ChartContext,
        passes: &[engine::Pass],
    ) -> anyhow::Result<Rc<engine::Scene>> {
        let scene_cid = parent_id.add(ControlIdPartType::Scene, &self.id);
        let mesh_collection_future = tokio::spawn({
            let mesh_cache = chart_context.resource_repository.mesh_cache.clone();
            let vulkan_context = chart_context.gpu_context.clone();
            let path = chart_context.path.relative_path(&self.file)?;
            async move { mesh_cache.load(&vulkan_context, &path).await }
        });

        // Load material
        let material = self
            .material
            .load(chart_context, passes, &self.control_map, &scene_cid)
            .await
            .with_context(|| format!("Failed to load material for scene '{}'", self.id))?;

        // Wait for resources to be loaded
        let mesh_collection = mesh_collection_future.await??;

        let objects = mesh_collection
            .nodes_by_name
            .iter()
            .map(|(mesh_id, scene_node)| {
                let object_cid = scene_cid.add(ControlIdPartType::Object, mesh_id);
                let position_id = object_cid.add(ControlIdPartType::Value, "position");
                let rotation_id = object_cid.add(ControlIdPartType::Value, "rotation");
                let instances_id = object_cid.add(ControlIdPartType::Value, "instances");

                let node_pos = scene_node.position;
                let position = chart_context.control_set_builder.get_vec3(&position_id);
                position.set(&[node_pos[0], node_pos[1], node_pos[2], 0.0]);

                let node_rot = scene_node.rotation;
                let rotation = chart_context.control_set_builder.get_vec3(&rotation_id);
                rotation.set(&[node_rot[0], node_rot[1], node_rot[2], 0.0]);

                engine::RenderObject {
                    _id: mesh_id.clone(),
                    mesh: scene_node.mesh.clone(),
                    material: material.clone(),
                    position,
                    rotation,
                    instances: chart_context
                        .control_set_builder
                        .get_float_with_default(&instances_id, 1.),
                }
            })
            .collect();

        let scene = engine::Scene {
            _id: self.id.clone(),
            objects,
        };
        Ok(Rc::new(scene))
    }
}
