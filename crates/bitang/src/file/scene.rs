use crate::control::{ControlId, ControlIdPartType};
use crate::file::chart_file::ChartContext;
use crate::file::material::Material;
use crate::loader::resource_repository::ResourceRepository;
use crate::loader::ResourcePath;
use crate::render;
use crate::tool::VulkanContext;
use serde::Deserialize;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
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
        passes: &[render::pass::Pass],
    ) -> anyhow::Result<Rc<render::scene::Scene>> {
        let scene_cid = parent_id.add(ControlIdPartType::Scene, &self.id);
        let mesh_collection_future = tokio::spawn({
            let mesh_cache = chart_context.resource_repository.mesh_cache.clone();
            let vulkan_context = chart_context.vulkan_context.clone();
            let path = chart_context.path.relative_path(&self.file);
            async move { mesh_cache.load(&vulkan_context, &path).await }
        });

        // Load material
        let material = self
            .material
            .load(chart_context, passes, &self.control_map, &scene_cid)
            .await?;

        // Wait for resources to be loaded
        let mesh_collection = mesh_collection_future.await??;

        let objects = mesh_collection
            .meshes_by_name
            .iter()
            .map(|(mesh_id, mesh)| {
                let object_cid = scene_cid.add(ControlIdPartType::Object, mesh_id);
                let position_id = object_cid.add(ControlIdPartType::Value, "position");
                let rotation_id = object_cid.add(ControlIdPartType::Value, "rotation");
                let instances_id = object_cid.add(ControlIdPartType::Value, "instances");

                let object = render::render_object::RenderObject {
                    id: mesh_id.clone(),
                    mesh: mesh.clone(),
                    material: material.clone(),
                    position: chart_context.control_set_builder.get_vec3(&position_id),
                    rotation: chart_context.control_set_builder.get_vec3(&rotation_id),
                    instances: chart_context
                        .control_set_builder
                        .get_float_with_default(&instances_id, 1.),
                };
                object
            })
            .collect();

        let scene = render::scene::Scene {
            id: self.id.clone(),
            objects,
        };
        Ok(Rc::new(scene))
    }
}
