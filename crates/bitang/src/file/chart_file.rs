use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use ahash::AHashMap;
use anyhow::{anyhow, Context, Result};
use futures::future::join_all;
use serde::Deserialize;
use tracing::{instrument, trace};

use crate::engine::{
    ControlId, ControlIdPartType, ControlSetBuilder, GpuContext, ImageSizeRule, ShaderKind,
};
use crate::file::shader_context::{BufferSource, ShaderContext, Texture};
use crate::loader::resource_path::ResourcePath;
use crate::loader::resource_repository::ResourceRepository;
use crate::{engine, file};

/// A context for loading a chart.
pub struct ChartContext {
    pub gpu_context: Arc<GpuContext>,
    pub resource_repository: Rc<ResourceRepository>,
    pub images_by_id: AHashMap<String, Arc<engine::BitangImage>>,
    pub control_set_builder: ControlSetBuilder,
    pub chart_control_id: ControlId,
    pub values_control_id: ControlId,
    pub buffers_by_id: HashMap<String, Rc<engine::DoubleBuffer>>,
    pub path: ResourcePath,
}

#[derive(Debug, Deserialize)]
pub struct Chart {
    #[serde(default)]
    pub images: Vec<Image>,

    #[serde(default)]
    pub buffers: Vec<DoubleBuffer>,

    /// Simulation should run this long before starting the demo
    #[serde(default)]
    pub simulation_precalculation_time: f32,

    pub steps: Vec<ChartStep>,
}

impl Chart {
    #[instrument(skip(self, context, resource_repository, chart_file_path))]
    pub async fn load(
        &self,
        id: &str,
        context: &Arc<GpuContext>,
        resource_repository: &Rc<ResourceRepository>,
        chart_file_path: &ResourcePath,
    ) -> Result<Rc<engine::Chart>> {
        trace!("Loading chart {}", id);
        let chart_control_id = ControlId::default().add(ControlIdPartType::Chart, id);
        let control_set_builder = ControlSetBuilder::new(
            chart_control_id.clone(),
            resource_repository.control_repository.clone(),
        );

        let images_by_id = self
            .images
            .iter()
            .map(|image_desc| Ok((image_desc.id.clone(), image_desc.load())))
            .collect::<Result<AHashMap<_, _>>>()?;

        let buffers_by_id = self
            .buffers
            .iter()
            .map(|buffer_desc| {
                let buffer = buffer_desc.load(context);
                (buffer_desc.id.clone(), Rc::new(buffer))
            })
            .collect::<HashMap<_, _>>();

        let chart_context = ChartContext {
            gpu_context: context.clone(),
            resource_repository: resource_repository.clone(),
            images_by_id,
            control_set_builder,
            values_control_id: chart_control_id.add(ControlIdPartType::ChartValues, "Chart Values"),
            chart_control_id,
            buffers_by_id,
            path: chart_file_path.clone(),
        };

        let chart_step_futures =
            self.steps.iter().map(|pass| async { pass.load(context, &chart_context).await });
        // Load all passes in parallel.
        let chart_steps =
            join_all(chart_step_futures).await.into_iter().collect::<Result<Vec<_>>>()?;

        let images = chart_context.images_by_id.values().cloned().collect::<Vec<_>>();

        let chart = engine::Chart::new(
            id,
            &chart_context.chart_control_id,
            chart_context.control_set_builder,
            images,
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
    pub format: engine::PixelFormat,

    #[serde(default)]
    pub has_mipmaps: bool,
}

impl Image {
    pub fn load(&self) -> Arc<engine::BitangImage> {
        engine::BitangImage::new_attachment(&self.id, self.format, self.size, self.has_mipmaps)
    }
}

fn default_clear_color() -> Option<[f32; 4]> {
    Some([0.03, 0.03, 0.03, 1.0])
}

#[derive(Debug, Deserialize)]
pub enum ChartStep {
    Draw(Draw),
    Compute(Compute),
    GenerateMipLevels(GenerateMipLevels),
}

impl ChartStep {
    async fn load(
        &self,
        context: &Arc<GpuContext>,
        chart_context: &ChartContext,
    ) -> Result<engine::ChartStep> {
        match self {
            ChartStep::Draw(draw) => {
                let draw = draw.load(chart_context).await?;
                Ok(engine::ChartStep::Draw(draw))
            }
            ChartStep::Compute(compute) => {
                let compute = compute.load(context, chart_context).await?;
                Ok(engine::ChartStep::Compute(compute))
            }
            ChartStep::GenerateMipLevels(generate_mip_levels) => {
                let generate_mip_levels = generate_mip_levels.load(chart_context).await?;
                Ok(engine::ChartStep::GenerateMipLevels(generate_mip_levels))
            }
        }
    }
}

/// Represents a mipmap generation step in the chart sequence.
#[derive(Debug, Deserialize)]
pub struct GenerateMipLevels {
    pub id: String,
    pub image_id: String,
}

impl GenerateMipLevels {
    pub async fn load(&self, chart_context: &ChartContext) -> Result<engine::GenerateMipLevels> {
        let image = chart_context.images_by_id.get(&self.image_id).with_context(|| {
            anyhow!(
                "Image id not found: '{}' (mipmap generation step: '{}')",
                self.image_id,
                self.id
            )
        })?;

        Ok(engine::GenerateMipLevels::new(
            &chart_context.gpu_context,
            &self.id,
            image,
        ))
    }
}

#[derive(Debug, Deserialize)]
pub enum DrawItem {
    Object(file::object::Object),
    Scene(file::scene::Scene),
}

impl DrawItem {
    pub async fn load(
        &self,
        chart_context: &ChartContext,
        draw_control_id: &ControlId,
        passes: &[engine::Pass],
    ) -> Result<engine::DrawItem> {
        match self {
            DrawItem::Object(object) => {
                let object = object.load(chart_context, draw_control_id, passes).await?;
                Ok(engine::DrawItem::Object(object))
            }
            DrawItem::Scene(scene) => {
                let scene = scene.load(draw_control_id, chart_context, passes).await?;
                Ok(engine::DrawItem::Scene(scene))
            }
        }
    }
}

/// Represents a draw step in the chart sequence.
#[derive(Debug, Deserialize)]
pub struct Draw {
    pub id: String,
    pub passes: Vec<Pass>,
    pub items: Vec<DrawItem>,
}

impl Draw {
    #[allow(clippy::too_many_arguments)]
    pub async fn load(&self, chart_context: &ChartContext) -> Result<engine::Draw> {
        let draw_control_id =
            chart_context.chart_control_id.add(ControlIdPartType::ChartStep, &self.id);
        let pass_futures = self.passes.iter().map(|pass| pass.load(chart_context));

        // Pass render targets rarely need an image to be loaded, no problem resolving it early
        let passes = join_all(pass_futures).await.into_iter().collect::<Result<Vec<_>>>()?;

        let draw_item_futures =
            self.items.iter().map(|object| object.load(chart_context, &draw_control_id, &passes));
        let objects = join_all(draw_item_futures).await.into_iter().collect::<Result<_>>()?;

        let light_dir_id = draw_control_id.add(ControlIdPartType::Value, "light_dir");
        let shadow_map_size_id = draw_control_id.add(ControlIdPartType::Value, "shadow_map_size");

        let light_dir = chart_context.control_set_builder.get_vec3(&light_dir_id);
        let shadow_map_size = chart_context.control_set_builder.get_vec3(&shadow_map_size_id);

        let draw = engine::Draw::new(&self.id, passes, objects, light_dir, shadow_map_size)?;
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
    textures: HashMap<String, Texture>,

    #[serde(default)]
    buffers: HashMap<String, BufferSource>,

    #[serde(default)]
    control_map: HashMap<String, String>,
}

impl Compute {
    async fn load(
        &self,
        context: &Arc<GpuContext>,
        chart_context: &ChartContext,
    ) -> Result<engine::Compute> {
        let run = match &self.run {
            ComputeRun::Init(buffer_id) => {
                let buffer = chart_context
                    .buffers_by_id
                    .get(buffer_id)
                    .with_context(|| anyhow!("Buffer not found: {buffer_id}"))?;
                engine::Run::Init(buffer.clone())
            }
            ComputeRun::Simulation(buffer_id) => {
                let buffer = chart_context
                    .buffers_by_id
                    .get(buffer_id)
                    .with_context(|| anyhow!("Buffer not found: {buffer_id}"))?;
                engine::Run::Simulate(buffer.clone())
            }
        };

        let control_id = chart_context.chart_control_id.add(ControlIdPartType::Compute, &self.id);

        let shader_context = ShaderContext::new(
            chart_context,
            &self.control_map,
            &control_id,
            &self.textures,
            &self.buffers,
        )?;

        let shader =
            shader_context.make_shader(chart_context, ShaderKind::Compute, &self.shader).await?;

        engine::Compute::new(context, &self.id, shader, run)
    }
}

// TODO: get rid of this, use a plain string id instead
#[derive(Debug, Deserialize)]
pub enum ImageSelector {
    /// Level 0 of the image
    Image(String),

    /// The swapchain image
    Screen,
}

impl ImageSelector {
    pub fn load(&self, chart_context: &ChartContext) -> Result<Arc<engine::BitangImage>> {
        match self {
            ImageSelector::Image(id) => {
                let image = chart_context
                    .images_by_id
                    .get(id)
                    .with_context(|| anyhow!("Render target not found: {id}"))?;
                Ok(Arc::clone(image))
            }
            ImageSelector::Screen => {
                let swapchain_image = chart_context.gpu_context.final_render_target.clone();
                Ok(swapchain_image)
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
    pub async fn load(&self, chart_context: &ChartContext) -> Result<engine::Pass> {
        let depth_buffer = match &self.depth_image {
            Some(selector) => Some(selector.load(chart_context)?),
            None => None,
        };

        let color_buffers = self
            .color_images
            .iter()
            .map(|color_buffer| color_buffer.load(chart_context))
            .collect::<Result<Vec<_>>>()?;

        engine::Pass::new(&self.id, color_buffers, depth_buffer, self.clear_color)
    }
}

#[derive(Debug, Deserialize)]
pub struct DoubleBuffer {
    id: String,
    item_size_in_vec4: usize,
    item_count: usize,
}

impl DoubleBuffer {
    pub fn load(&self, context: &Arc<GpuContext>) -> engine::DoubleBuffer {
        engine::DoubleBuffer::new(context, self.item_size_in_vec4, self.item_count)
    }
}
