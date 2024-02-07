use crate::control::controls::ControlSetBuilder;
use crate::control::{ControlId, ControlIdPartType};
use crate::file::shader_context::{BufferSource, Sampler, ShaderContext};
use crate::loader::async_cache::LoadFuture;
use crate::loader::resource_repository::ResourceRepository;
use crate::loader::ResourcePath;
use crate::render::buffer_generator::BufferGeneratorType;
use crate::render::image::ImageSizeRule;
use crate::render::shader::ShaderKind;
use crate::render::SCREEN_RENDER_TARGET_ID;
use crate::tool::VulkanContext;
use crate::{file, render};
use ahash::AHashMap;
use anyhow::{anyhow, Context, Result};
use futures::future::join_all;
use serde::Deserialize;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// A context for loading a chart.
pub struct ChartContext {
    pub vulkan_context: Arc<VulkanContext>,
    pub resource_repository: Arc<ResourceRepository>,
    pub image_futures_by_id: AHashMap<String, LoadFuture<render::image::Image>>,
    pub control_set_builder: ControlSetBuilder,
    pub chart_control_id: ControlId,
    pub values_control_id: ControlId,
    pub buffers_by_id: HashMap<String, Rc<render::buffer::Buffer>>,
    pub buffer_generators_by_id: HashMap<String, Rc<render::buffer_generator::BufferGenerator>>,
    pub path: ResourcePath,
}

#[derive(Debug, Deserialize)]
pub struct Chart {
    #[serde(default)]
    pub images: Vec<Image>,

    #[serde(default)]
    pub buffer_generators: Vec<BufferGenerator>,

    #[serde(default)]
    pub buffers: Vec<Buffer>,

    /// Simulation should run this long before starting the demo
    #[serde(default)]
    pub simulation_precalculation_time: f32,

    pub steps: Vec<ChartStep>,
}

impl Chart {
    pub async fn load(
        &self,
        id: &str,
        context: &Arc<VulkanContext>,
        resource_repository: &Arc<ResourceRepository>,
        chart_file_path: &ResourcePath,
    ) -> Result<Rc<render::chart::Chart>> {
        let chart_control_id = ControlId::default().add(ControlIdPartType::Chart, id);
        let control_set_builder = ControlSetBuilder::new(
            chart_control_id.clone(),
            resource_repository.control_repository.clone(),
        );

        let mut image_futures_by_id = self
            .images
            .iter()
            .map(|image_desc| {
                let image = LoadFuture::new_from_value(image_desc.load());
                Ok((image_desc.id.clone(), image))
            })
            .collect::<Result<AHashMap<_, _>>>()?;

        // Add swapchain image to the image map
        image_futures_by_id.insert(
            SCREEN_RENDER_TARGET_ID.to_string(),
            LoadFuture::new_from_value(context.final_render_target.clone()),
        );

        let buffer_generators_by_id = self
            .buffer_generators
            .iter()
            .map(|buffer_generator| {
                let generator =
                    buffer_generator.load(context, &chart_control_id, &control_set_builder);
                (buffer_generator.id.clone(), Rc::new(generator))
            })
            .collect::<HashMap<_, _>>();

        let buffers_by_id = self
            .buffers
            .iter()
            .map(|buffer_desc| {
                let buffer = buffer_desc.load(context);
                (buffer_desc.id.clone(), Rc::new(buffer))
            })
            .collect::<HashMap<_, _>>();

        let chart_context = ChartContext {
            vulkan_context: context.clone(),
            resource_repository: resource_repository.clone(),
            image_futures_by_id,
            control_set_builder,
            values_control_id: chart_control_id.add(ControlIdPartType::ChartValues, "Chart Values"),
            chart_control_id,
            buffers_by_id,
            buffer_generators_by_id,
            path: chart_file_path.clone(),
        };

        let chart_step_futures = self
            .steps
            .iter()
            .map(|pass| async { pass.load(context, &chart_context).await });
        // Load all passes in parallel.
        let chart_steps = join_all(chart_step_futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

        let image_futures = chart_context
            .image_futures_by_id
            .into_values()
            .map(|image_future| async move { image_future.get().await });
        let images = join_all(image_futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

        let buffer_generators = chart_context
            .buffer_generators_by_id
            .into_values()
            .collect::<Vec<_>>();

        let chart = render::chart::Chart::new(
            id,
            &chart_context.chart_control_id,
            chart_context.control_set_builder,
            images,
            buffer_generators,
            chart_steps,
            self.simulation_precalculation_time,
        );
        Ok(Rc::new(chart))
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
        render::image::Image::new_attachment(&self.id, self.format, self.size)
    }
}

fn default_clear_color() -> Option<[f32; 4]> {
    Some([0.03, 0.03, 0.03, 1.0])
}

#[derive(Debug, Deserialize)]
pub enum ChartStep {
    Draw(Draw),
    Compute(Compute),
}

impl ChartStep {
    async fn load(
        &self,
        context: &Arc<VulkanContext>,
        chart_context: &ChartContext,
    ) -> Result<render::chart::ChartStep> {
        match self {
            ChartStep::Draw(draw) => {
                let draw = draw.load(chart_context).await?;
                Ok(render::chart::ChartStep::Draw(draw))
            }
            ChartStep::Compute(compute) => {
                let compute = compute.load(context, chart_context).await?;
                Ok(render::chart::ChartStep::Compute(compute))
            }
        }
    }
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
    pub async fn load(&self, chart_context: &ChartContext) -> Result<render::draw::Draw> {
        let draw_control_id = chart_context
            .chart_control_id
            .add(ControlIdPartType::ChartStep, &self.id);
        let pass_futures = self.passes.iter().map(|pass| pass.load(chart_context));

        // Pass render targets rarely need an image to be loaded, no problem resolving it early
        let passes = join_all(pass_futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

        let object_futures = self
            .objects
            .iter()
            .map(|object| object.load(chart_context, &draw_control_id, &passes));
        let objects = join_all(object_futures)
            .await
            .into_iter()
            .collect::<Result<_>>()?;

        let light_dir_id = draw_control_id.add(ControlIdPartType::Value, "light_dir");
        let shadow_map_size_id = draw_control_id.add(ControlIdPartType::Value, "shadow_map_size");

        let light_dir = chart_context.control_set_builder.get_vec3(&light_dir_id);
        let shadow_map_size = chart_context
            .control_set_builder
            .get_vec3(&shadow_map_size_id);

        let draw = render::draw::Draw::new(&self.id, passes, objects, light_dir, shadow_map_size)?;
        Ok(draw)
    }
}

#[derive(Debug, Deserialize)]
pub enum ComputeRun {
    Init(String),
    Simulation(String),
}

#[derive(Debug, Deserialize)]
pub enum BufferSelector {
    Current,
    Next,
}

#[derive(Debug, Deserialize)]
pub struct Compute {
    id: String,
    shader: String,
    run: ComputeRun,

    #[serde(default)]
    samplers: HashMap<String, Sampler>,

    #[serde(default)]
    buffers: HashMap<String, BufferSource>,

    #[serde(default)]
    control_map: HashMap<String, String>,
}

impl Compute {
    async fn load(
        &self,
        context: &Arc<VulkanContext>,
        chart_context: &ChartContext,
    ) -> Result<render::compute::Compute> {
        let run = match &self.run {
            ComputeRun::Init(buffer_id) => {
                let buffer = chart_context
                    .buffers_by_id
                    .get(buffer_id)
                    .with_context(|| anyhow!("Buffer not found: {buffer_id}"))?;
                render::compute::Run::Init(buffer.clone())
            }
            ComputeRun::Simulation(buffer_id) => {
                let buffer = chart_context
                    .buffers_by_id
                    .get(buffer_id)
                    .with_context(|| anyhow!("Buffer not found: {buffer_id}"))?;
                render::compute::Run::Simulate(buffer.clone())
            }
        };

        let control_id = chart_context
            .chart_control_id
            .add(ControlIdPartType::Compute, &self.id);

        let shader_context = ShaderContext::new(
            chart_context,
            &self.control_map,
            &control_id,
            &self.samplers,
            &self.buffers,
        )?;

        let shader = shader_context
            .make_shader(chart_context, ShaderKind::Compute, &self.shader)
            .await?;

        render::compute::Compute::new(context, &self.id, shader, run)
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
        images_by_id: &AHashMap<String, LoadFuture<render::image::Image>>,
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
    pub depth_image: Option<ImageSelector>,
    pub color_images: Vec<ImageSelector>,

    #[serde(default = "default_clear_color")]
    pub clear_color: Option<[f32; 4]>,
}

impl Pass {
    pub async fn load(&self, chart_context: &ChartContext) -> Result<render::pass::Pass> {
        let depth_buffer = match &self.depth_image {
            Some(selector) => Some(selector.load(&chart_context.image_futures_by_id).await?),
            None => None,
        };

        let color_buffer_futures = self
            .color_images
            .iter()
            .map(|color_buffer| color_buffer.load(&chart_context.image_futures_by_id));
        let color_buffers = join_all(color_buffer_futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

        render::pass::Pass::new(
            &self.id,
            &chart_context.vulkan_context,
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
pub struct Buffer {
    id: String,
    item_size_in_vec4: usize,
    item_count: usize,
}

impl Buffer {
    pub fn load(&self, context: &Arc<VulkanContext>) -> render::buffer::Buffer {
        render::buffer::Buffer::new(context, self.item_size_in_vec4, self.item_count)
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
        chart_context: &ChartContext,
        parent_id: &ControlId,
        passes: &[render::pass::Pass],
    ) -> Result<Arc<crate::render::render_object::RenderObject>> {
        let object_cid = parent_id.add(ControlIdPartType::Object, &self.id);
        let mesh_future = chart_context.resource_repository.get_mesh(
            &chart_context.vulkan_context,
            &chart_context.path.relative_path(&self.mesh_file),
            &self.mesh_name,
        );

        let material_future =
            self.material
                .load(chart_context, passes, &self.control_map, &object_cid);

        let position_id = object_cid.add(ControlIdPartType::Value, "position");
        let rotation_id = object_cid.add(ControlIdPartType::Value, "rotation");
        let instances_id = object_cid.add(ControlIdPartType::Value, "instances");

        // Wait for resources to be loaded
        let material = material_future.await?;
        let mesh = mesh_future.get().await?;

        let object = crate::render::render_object::RenderObject {
            id: self.id.clone(),
            mesh,
            material,
            position: chart_context.control_set_builder.get_vec3(&position_id),
            rotation: chart_context.control_set_builder.get_vec3(&rotation_id),
            instances: chart_context
                .control_set_builder
                .get_float_with_default(&instances_id, 1.),
        };
        Ok(Arc::new(object))
    }
}
