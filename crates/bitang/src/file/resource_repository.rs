use crate::control::controls::ControlRepository;
use crate::file::binary_file_cache::BinaryFileCache;
use crate::file::file_hash_cache::FileCache;
use crate::file::shader_loader::ShaderCache;
use crate::file::{chart_file, project_file, ResourcePath};
use crate::render::chart::Chart;
use crate::render::mesh::Mesh;
use crate::render::project::Project;
use crate::render::vulkan_window::VulkanContext;
use crate::render::{Texture, Vertex3};
use anyhow::Result;
use bitang_utils::blend_loader::load_blend_buffer;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, instrument};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract,
};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{ImageDimensions, ImmutableImage, MipmapsCount};

pub struct ResourceRepository {
    file_hash_cache: Rc<RefCell<FileCache>>,
    texture_cache: BinaryFileCache<Arc<Texture>>,
    mesh_cache: BinaryFileCache<Arc<Mesh>>,
    chart_file_cache: BinaryFileCache<Arc<chart_file::Chart>>,
    project_file_cache: BinaryFileCache<Arc<project_file::Project>>,

    pub shader_cache: ShaderCache,
    pub control_repository: Rc<RefCell<ControlRepository>>,

    cached_root: Option<Arc<Project>>,
    last_load_time: Instant,
    is_first_load: bool,
}

// If loading fails, we want to retry periodically.
const LOAD_RETRY_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

const PROJECT_FILE_NAME: &str = "project.ron";
pub const CHARTS_FOLDER: &str = "charts";
const CHART_FILE_NAME: &str = "chart.ron";

impl ResourceRepository {
    pub fn try_new() -> Result<Self> {
        let file_hash_cache = Rc::new(RefCell::new(FileCache::new()?));
        let control_repository = ControlRepository::load_control_files()?;

        Ok(Self {
            texture_cache: BinaryFileCache::new(&file_hash_cache, load_texture),
            mesh_cache: BinaryFileCache::new(&file_hash_cache, load_mesh),
            shader_cache: ShaderCache::new(&file_hash_cache),
            chart_file_cache: BinaryFileCache::new(&file_hash_cache, load_chart_file),
            project_file_cache: BinaryFileCache::new(&file_hash_cache, load_project_file),
            file_hash_cache,
            cached_root: None,
            control_repository: Rc::new(RefCell::new(control_repository)),
            last_load_time: Instant::now() - LOAD_RETRY_INTERVAL,
            is_first_load: true,
        })
    }

    #[instrument(skip_all, name = "load")]
    pub fn get_or_load_project(&mut self, context: &VulkanContext) -> Option<Arc<Project>> {
        let has_file_changes = self.file_hash_cache.borrow_mut().handle_file_changes();
        if has_file_changes
            || (self.cached_root.is_none() && self.last_load_time.elapsed() > LOAD_RETRY_INTERVAL)
        {
            let now = Instant::now();
            let result = self.load_project(context);
            self.file_hash_cache.borrow_mut().update_watchers();
            match result {
                Ok(project) => {
                    self.cached_root = Some(Arc::new(project));
                    info!("Loading took {:?}", now.elapsed());
                }
                Err(err) => {
                    if self.is_first_load || has_file_changes {
                        error!("Error loading root document: {:?}", err);
                    }
                    self.cached_root = None;
                }
            };
            self.last_load_time = Instant::now();
            self.is_first_load = false;
        }
        self.cached_root.clone()
    }

    #[instrument(skip(self, context))]
    pub fn get_texture(
        &mut self,
        context: &VulkanContext,
        path: &ResourcePath,
    ) -> Result<&Arc<Texture>> {
        self.texture_cache.get_or_load(context, path)
    }

    #[instrument(skip(self, context))]
    pub fn get_mesh(&mut self, context: &VulkanContext, path: &ResourcePath) -> Result<&Arc<Mesh>> {
        self.mesh_cache.get_or_load(context, path)
    }

    pub fn load_chart(&mut self, id: &str, context: &VulkanContext) -> Result<Chart> {
        let path = ResourcePath::new(&format!("{CHARTS_FOLDER}/{id}"), CHART_FILE_NAME);
        let chart = self.chart_file_cache.get_or_load(context, &path)?.clone();
        chart.load(id, context, self, &path)
    }

    pub fn load_project(&mut self, context: &VulkanContext) -> Result<Project> {
        let path = ResourcePath::new("", PROJECT_FILE_NAME);
        let project = self.project_file_cache.get_or_load(context, &path)?.clone();
        project.load(context, self)
    }
}

#[instrument(skip_all)]
fn load_mesh(context: &VulkanContext, content: &[u8]) -> Result<Arc<Mesh>> {
    let blend_file = load_blend_buffer(content)?;
    let vertices = blend_file
        .mesh
        .faces
        .iter()
        .flatten()
        .map(|v| Vertex3 {
            a_position: [v.0[0], v.0[1], v.0[2]],
            a_normal: [v.1[0], v.1[1], v.1[2]],
            a_tangent: [0.0, 0.0, 0.0],
            a_uv: [v.2[0], v.2[1]],
            a_padding: 0.0,
        })
        .collect::<Vec<Vertex3>>();
    Ok(Arc::new(Mesh::try_new(context, vertices)?))
}

#[instrument(skip_all)]
fn load_texture(context: &VulkanContext, content: &[u8]) -> Result<Arc<Texture>> {
    let now = Instant::now();
    let rgba = image::load_from_memory(content)?.to_rgba8();
    info!("Decoded image in {:?}", now.elapsed());
    let dimensions = ImageDimensions::Dim2d {
        width: rgba.dimensions().0,
        height: rgba.dimensions().0,
        array_layers: 1,
    };

    let mut cbb = AutoCommandBufferBuilder::primary(
        &context.command_buffer_allocator,
        context.context.graphics_queue().queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )?;

    let image = ImmutableImage::from_iter(
        context.context.memory_allocator(),
        rgba.into_raw(),
        dimensions,
        MipmapsCount::One,
        Format::R8G8B8A8_SRGB,
        &mut cbb,
    )?;
    let _fut = cbb
        .build()?
        .execute(context.context.graphics_queue().clone())?;

    Ok(ImageView::new_default(image)?)
}

#[instrument(skip_all)]
pub fn load_chart_file(_context: &VulkanContext, content: &[u8]) -> Result<Arc<chart_file::Chart>> {
    let chart = ron::from_str::<chart_file::Chart>(std::str::from_utf8(content)?)?;
    Ok(Arc::new(chart))
}

#[instrument(skip_all)]
pub fn load_project_file(
    _context: &VulkanContext,
    content: &[u8],
) -> Result<Arc<project_file::Project>> {
    let project = ron::from_str::<project_file::Project>(std::str::from_utf8(content)?)?;
    Ok(Arc::new(project))
}
