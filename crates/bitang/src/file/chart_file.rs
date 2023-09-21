use crate::control::controls::ControlSetBuilder;
use crate::control::{ControlId, ControlIdPartType};
use crate::loader::async_cache::LoadFuture;
use crate::loader::resource_repository::ResourceRepository;
use crate::loader::ResourcePath;
use crate::render::buffer_generator::BufferGeneratorType;
use crate::render::image::ImageSizeRule;
use crate::render::vulkan_window::VulkanContext;
use crate::render::SCREEN_RENDER_TARGET_ID;
use crate::{file, render};
use anyhow::{anyhow, Context, Result};
use futures::future::join_all;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct Chart {
    #[serde(default)]
    pub images: Vec<Image>,

    #[serde(default)]
    pub buffer_generators: Vec<BufferGenerator>,

    pub steps: Vec<Draw>,
}

impl Chart {
    pub async fn load(
        &self,
        id: &str,
        context: &Arc<VulkanContext>,
        resource_repository: &Arc<ResourceRepository>,
        path: &ResourcePath,
    ) -> Result<Arc<render::chart::Chart>> {
        let control_id = ControlId::default().add(ControlIdPartType::Chart, id);
        let control_set_builder = ControlSetBuilder::new(
            control_id.clone(),
            resource_repository.control_repository.clone(),
        );

        let mut images_by_id = self
            .images
            .iter()
            .map(|image_desc| {
                let image = LoadFuture::new_from_value(image_desc.load());
                Ok((image_desc.id.clone(), image))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        // Add swapchain image to the image map
        images_by_id.insert(
            SCREEN_RENDER_TARGET_ID.to_string(),
            LoadFuture::new_from_value(context.final_render_target.clone()),
        );

        let buffer_generators_by_id = self
            .buffer_generators
            .iter()
            .map(|buffer_generator| {
                let generator = buffer_generator.load(context, &control_id, &control_set_builder);
                (buffer_generator.id.clone(), Arc::new(generator))
            })
            .collect::<HashMap<_, _>>();

        let pass_futures = self.steps.iter().map(|pass| async {
            pass.load(
                context,
                resource_repository,
                &control_set_builder,
                &images_by_id,
                &buffer_generators_by_id,
                &control_id,
                path,
            )
            .await
        });
        // Load all passes in parallel.
        let passes = join_all(pass_futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

        let image_futures = images_by_id
            .into_values()
            .map(|image_future| async move { image_future.get().await });
        let images = join_all(image_futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

        let buffer_generators = buffer_generators_by_id.into_values().collect::<Vec<_>>();

        let chart = render::chart::Chart::new(
            id,
            &control_id,
            control_set_builder,
            images,
            buffer_generators,
            passes,
        );
        Ok(Arc::new(chart))
    }
}

#[derive(Debug, Deserialize)]
pub struct Image {
    pub id: String,
    pub size: ImageSizeRule,
    pub format: render::image::ImageFormat,
}

impl Image {
    pub fn load(&self) -> Arc<render::image::Image> {
        let image = render::image::Image::new_attachment(&self.id, self.format, self.size);
        image
    }
}

fn default_clear_color() -> Option<[f32; 4]> {
    Some([0.03, 0.03, 0.03, 1.0])
}

/// Represents a draw step in the chart sequence.
#[derive(Debug, Deserialize)]
pub struct Draw {
    pub id: String,
    pub passes: Vec<Pass>,
    pub objects: Vec<Object>,
}

impl Draw {
    #[allow(clippy::too_many_arguments)]
    pub async fn load(
        &self,
        context: &Arc<VulkanContext>,
        resource_repository: &Arc<ResourceRepository>,
        control_set_builder: &ControlSetBuilder,
        images_by_id: &HashMap<String, LoadFuture<render::image::Image>>,
        buffer_generators_by_id: &HashMap<String, Arc<render::buffer_generator::BufferGenerator>>,
        chart_id: &ControlId,
        path: &ResourcePath,
    ) -> Result<render::draw::Draw> {
        let control_prefix = chart_id.add(ControlIdPartType::ChartStep, &self.id);
        let chart_id = chart_id.add(ControlIdPartType::ChartValues, "Chart Values");
        let pass_futures = self
            .passes
            .iter()
            .map(|pass| pass.load(context, images_by_id));

        // Pass render targets don't need to be loaded, no problem resolving it early
        let passes = join_all(pass_futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

        let object_futures = self.objects.iter().map(|object| {
            // let resource_repository = resource_repository.clone();
            object.load(
                &control_prefix,
                &chart_id,
                context,
                resource_repository,
                control_set_builder,
                images_by_id,
                buffer_generators_by_id,
                path,
                &passes,
            )
        });
        let objects = join_all(object_futures)
            .await
            .into_iter()
            .collect::<Result<_>>()?;

        let light_dir_id = control_prefix.add(ControlIdPartType::Value, "light_dir");
        let shadow_map_size_id = control_prefix.add(ControlIdPartType::Value, "shadow_map_size");

        let light_dir = control_set_builder.get_vec3(&light_dir_id);
        let shadow_map_size = control_set_builder.get_vec3(&shadow_map_size_id);

        let draw = render::draw::Draw::new(&self.id, passes, objects, light_dir, shadow_map_size)?;
        Ok(draw)
    }
}

#[derive(Debug, Deserialize)]
pub enum ImageSelector {
    /// Level 0 of the image
    Image(String),
}

impl ImageSelector {
    pub async fn load(
        &self,
        images_by_id: &HashMap<String, LoadFuture<render::image::Image>>,
    ) -> Result<render::pass::ImageSelector> {
        match self {
            ImageSelector::Image(id) => {
                let image_future = images_by_id
                    .get(id)
                    .with_context(|| anyhow!("Render target not found: {id}"))?;
                let image = image_future.get().await?;
                Ok(render::pass::ImageSelector::Image(image))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Pass {
    pub id: String,
    pub depth_buffer: Option<ImageSelector>,
    pub color_buffers: Vec<ImageSelector>,

    #[serde(default = "default_clear_color")]
    pub clear_color: Option<[f32; 4]>,
}

impl Pass {
    pub async fn load(
        &self,
        context: &Arc<VulkanContext>,
        render_targets_by_id: &HashMap<String, LoadFuture<render::image::Image>>,
    ) -> Result<render::pass::Pass> {
        let depth_buffer = match &self.depth_buffer {
            Some(selector) => Some(selector.load(render_targets_by_id).await?),
            None => None,
        };

        let color_buffer_futures = self
            .color_buffers
            .iter()
            .map(|color_buffer| color_buffer.load(render_targets_by_id));
        let color_buffers = join_all(color_buffer_futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

        render::pass::Pass::new(
            &self.id,
            context,
            color_buffers,
            depth_buffer,
            self.clear_color,
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct BufferGenerator {
    id: String,
    size: u32,
    generator: BufferGeneratorType,
}

impl BufferGenerator {
    pub fn load(
        &self,
        context: &Arc<VulkanContext>,
        parent_id: &ControlId,
        control_set_builder: &ControlSetBuilder,
    ) -> render::buffer_generator::BufferGenerator {
        let control_id = parent_id.add(ControlIdPartType::BufferGenerator, &self.id);

        render::buffer_generator::BufferGenerator::new(
            self.size,
            context,
            &control_id,
            control_set_builder,
            &self.generator,
        )
    }
}

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
    #[allow(clippy::too_many_arguments)]
    pub async fn load(
        &self,
        parent_id: &ControlId,
        chart_id: &ControlId,
        context: &Arc<VulkanContext>,
        resource_repository: &Arc<ResourceRepository>,
        control_set_builder: &ControlSetBuilder,
        images_by_id: &HashMap<String, LoadFuture<render::image::Image>>,
        buffer_generators_by_id: &HashMap<String, Arc<render::buffer_generator::BufferGenerator>>,
        path: &ResourcePath,
        passes: &[render::pass::Pass],
    ) -> Result<Arc<crate::render::render_object::RenderObject>> {
        let control_id = parent_id.add(ControlIdPartType::Object, &self.id);
        let mesh_future = resource_repository.get_mesh(
            context,
            &path.relative_path(&self.mesh_file),
            &self.mesh_name,
        );

        let material_future = self.material.load(
            context,
            resource_repository,
            images_by_id,
            path,
            passes,
            control_set_builder,
            &self.control_map,
            &control_id,
            chart_id,
            buffer_generators_by_id,
        );

        let position_id = control_id.add(ControlIdPartType::Value, "position");
        let rotation_id = control_id.add(ControlIdPartType::Value, "rotation");
        let instances_id = control_id.add(ControlIdPartType::Value, "instances");

        // Wait for resources to be loaded
        let mesh = mesh_future.get().await?;
        let material = material_future.await?;

        let object = crate::render::render_object::RenderObject {
            id: self.id.clone(),
            mesh,
            material,
            position: control_set_builder.get_vec3(&position_id),
            rotation: control_set_builder.get_vec3(&rotation_id),
            instances: control_set_builder.get_float_with_default(&instances_id, 1.),
        };
        Ok(Arc::new(object))
    }
}
