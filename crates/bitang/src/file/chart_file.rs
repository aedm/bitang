use crate::control::controls::ControlSetBuilder;
use crate::control::{ControlId, ControlIdPartType};
use crate::file::ResourcePath;
use crate::loader::resource_repository::ResourceRepository;
use crate::render::buffer_generator::BufferGeneratorType;
use crate::render::image::ImageSizeRule;
use crate::render::vulkan_window::VulkanContext;
use crate::render::SCREEN_RENDER_TARGET_ID;
use crate::{file, render};
use anyhow::{anyhow, Result};
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
    pub fn load(
        &self,
        id: &str,
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
        path: &ResourcePath,
    ) -> Result<render::chart::Chart> {
        let control_id = ControlId::default().add(ControlIdPartType::Chart, id);
        let mut control_set_builder = ControlSetBuilder::new(
            control_id.clone(),
            resource_repository.control_repository.clone(),
        );

        let mut images_by_id = self
            .images
            .iter()
            .map(|image_desc| {
                let image = image_desc.load()?;
                Ok((image_desc.id.clone(), image))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        // Add swapchain image to the image map
        images_by_id.insert(
            SCREEN_RENDER_TARGET_ID.to_string(),
            context.final_render_target.clone(),
        );

        let buffer_generators_by_id = self
            .buffer_generators
            .iter()
            .map(|buffer_generator| {
                let generator =
                    buffer_generator.load(context, &control_id, &mut control_set_builder);
                (buffer_generator.id.clone(), Arc::new(generator))
            })
            .collect::<HashMap<_, _>>();

        let passes = self
            .steps
            .iter()
            .map(|pass| {
                pass.load(
                    context,
                    resource_repository,
                    &mut control_set_builder,
                    &images_by_id,
                    &buffer_generators_by_id,
                    &control_id,
                    path,
                )
            })
            .collect::<Result<Vec<_>>>()?;

        let images = images_by_id.into_values().collect::<Vec<_>>();
        let buffer_generators = buffer_generators_by_id.into_values().collect::<Vec<_>>();

        let chart = render::chart::Chart::new(
            id,
            &control_id,
            control_set_builder,
            images,
            buffer_generators,
            passes,
        );
        Ok(chart)
    }
}

#[derive(Debug, Deserialize)]
pub struct Image {
    pub id: String,
    pub size: ImageSizeRule,
    pub format: render::image::ImageFormat,
}

impl Image {
    pub fn load(&self) -> Result<Arc<render::image::Image>> {
        let image = render::image::Image::new_attachment(&self.id, self.format, self.size);
        Ok(image)
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
    pub fn load(
        &self,
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
        control_set_builder: &mut ControlSetBuilder,
        images_by_id: &HashMap<String, Arc<render::image::Image>>,
        buffer_generators_by_id: &HashMap<String, Arc<render::buffer_generator::BufferGenerator>>,
        chart_id: &ControlId,
        path: &ResourcePath,
    ) -> Result<render::draw::Draw> {
        let control_prefix = chart_id.add(ControlIdPartType::ChartStep, &self.id);
        let chart_id = chart_id.add(ControlIdPartType::ChartValues, "Chart Values");
        let passes = self
            .passes
            .iter()
            .map(|pass| pass.load(context, images_by_id))
            .collect::<Result<Vec<_>>>()?;

        let objects = self
            .objects
            .iter()
            .map(|object| {
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
            })
            .collect::<Result<Vec<_>>>()?;

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
    pub fn load(
        &self,
        images_by_id: &HashMap<String, Arc<render::image::Image>>,
    ) -> Result<render::pass::ImageSelector> {
        match self {
            ImageSelector::Image(id) => images_by_id
                .get(id)
                .cloned()
                .map(render::pass::ImageSelector::Image)
                .ok_or_else(|| anyhow!("Render target not found: {id}")),
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
    pub fn load(
        &self,
        context: &VulkanContext,
        render_targets_by_id: &HashMap<String, Arc<render::image::Image>>,
    ) -> Result<render::pass::Pass> {
        let depth_buffer = self
            .depth_buffer
            .as_ref()
            .map(|selector| selector.load(render_targets_by_id))
            .transpose()?;
        let color_buffers = self
            .color_buffers
            .iter()
            .map(|color_buffer| color_buffer.load(render_targets_by_id))
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
        context: &VulkanContext,
        parent_id: &ControlId,
        control_set_builder: &mut ControlSetBuilder,
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
    pub fn load(
        &self,
        parent_id: &ControlId,
        chart_id: &ControlId,
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
        control_set_builder: &mut ControlSetBuilder,
        images_by_id: &HashMap<String, Arc<render::image::Image>>,
        buffer_generators_by_id: &HashMap<String, Arc<render::buffer_generator::BufferGenerator>>,
        path: &ResourcePath,
        passes: &[render::pass::Pass],
    ) -> Result<Arc<crate::render::render_object::RenderObject>> {
        let control_id = parent_id.add(ControlIdPartType::Object, &self.id);
        let mesh = resource_repository
            .get_mesh(
                context,
                &path.relative_path(&self.mesh_file),
                &self.mesh_name,
            )?
            .clone();

        let material = self.material.load(
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
        )?;

        let position_id = control_id.add(ControlIdPartType::Value, "position");
        let rotation_id = control_id.add(ControlIdPartType::Value, "rotation");
        let instances_id = control_id.add(ControlIdPartType::Value, "instances");

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
