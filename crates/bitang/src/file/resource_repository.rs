use crate::control::controls::Controls;
use crate::control::ControlId;
use crate::file::binary_file_cache::BinaryFileCache;
use crate::file::file_hash_cache::FileCache;
use crate::file::shader_loader::ShaderCache;
use crate::file::{chart_file, load_controls};
use crate::render::chart::Chart;
use crate::render::mesh::Mesh;
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
    blender_cache: BinaryFileCache<Arc<russimp::scene::Scene>>,
    root_ron_file_cache: BinaryFileCache<Arc<chart_file::Chart>>,

    pub shader_cache: ShaderCache,
    pub controls: Controls,

    cached_root: Option<Arc<Chart>>,
    last_load_time: Instant,
    is_first_load: bool,
}

// If loading fails, we want to retry periodically.
const LOAD_RETRY_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

impl ResourceRepository {
    pub fn try_new() -> Result<Self> {
        let file_hash_cache = Rc::new(RefCell::new(FileCache::new()?));

        Ok(Self {
            texture_cache: BinaryFileCache::new(&file_hash_cache, load_texture),
            mesh_cache: BinaryFileCache::new(&file_hash_cache, load_mesh),
            blender_cache: BinaryFileCache::new(&file_hash_cache, load_blender_file),
            shader_cache: ShaderCache::new(&file_hash_cache),
            root_ron_file_cache: BinaryFileCache::new(&file_hash_cache, load_chart_file),
            file_hash_cache,
            cached_root: None,
            controls: load_controls(),
            last_load_time: Instant::now() - LOAD_RETRY_INTERVAL,
            is_first_load: true,
        })
    }

    #[instrument(skip_all, name = "load")]
    pub fn load_root_document(&mut self, context: &VulkanContext) -> Option<Arc<Chart>> {
        let has_file_changes = self.file_hash_cache.borrow_mut().handle_file_changes();
        if has_file_changes
            || (self.cached_root.is_none() && self.last_load_time.elapsed() > LOAD_RETRY_INTERVAL)
        {
            let now = Instant::now();
            self.controls.reset_usage_collector();
            let result = self.load_root_chart(context);
            self.file_hash_cache.borrow_mut().update_watchers();
            match result {
                Ok(chart) => {
                    self.cached_root = Some(Arc::new(chart));
                    self.controls.finish_load_cycle();
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
    pub fn get_texture(&mut self, context: &VulkanContext, path: &str) -> Result<&Arc<Texture>> {
        self.texture_cache.get_or_load(context, &path)
    }

    #[instrument(skip(self, context))]
    pub fn get_mesh(&mut self, context: &VulkanContext, path: &str) -> Result<&Arc<Mesh>> {
        self.mesh_cache.get_or_load(context, &path)
    }

    #[instrument(skip(self, context))]
    pub fn get_blender_mesh(&mut self, context: &VulkanContext, path: &str) -> Result<()> {
        let scene = self.blender_cache.get_or_load(context, &path)?;
        for mesh in &scene.meshes {
            // println!("Mesh: {:?}", mesh.name);
        }
        Ok(())
    }

    pub fn load_root_chart(&mut self, context: &VulkanContext) -> Result<Chart> {
        let root_folder = "app";
        let chart_folder = "test-chart";
        let chart = self
            .root_ron_file_cache
            .get_or_load(context, &format!("{root_folder}/{chart_folder}/chart.ron"))?
            .clone();
        chart.load(chart_folder, context, &ControlId::default(), self)
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
fn load_blender_file(
    context: &VulkanContext,
    content: &[u8],
) -> Result<Arc<russimp::scene::Scene>> {
    let scene = russimp::scene::Scene::from_buffer(content, vec![], "")?;
    Ok(Arc::new(scene))
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
