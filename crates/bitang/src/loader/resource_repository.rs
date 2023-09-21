use crate::control::controls::ControlRepository;
use crate::file::{chart_file, project_file};
use crate::loader::async_cache::LoadFuture;
use crate::loader::file_cache::{FileCache, FileLoader};
use crate::loader::resource_cache::ResourceCache;
use crate::loader::shader_loader::ShaderCache;
use crate::loader::ResourcePath;
use crate::render::chart::Chart;
use crate::render::image::Image;
use crate::render::mesh::Mesh;
use crate::render::project::Project;
use crate::render::vulkan_window::VulkanContext;
use crate::render::Vertex3;
use anyhow::{anyhow, ensure, Context, Result};
use itertools::Itertools;
use russimp::scene::{PostProcess, Scene};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, instrument, warn};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract,
};
use vulkano::format::Format;
use vulkano::image::{ImageDimensions, ImmutableImage, MipmapsCount};

struct MeshCollection {
    meshes_by_name: HashMap<String, Arc<Mesh>>,
}

pub struct ResourceRepository {
    file_hash_cache: Arc<FileCache>,
    texture_cache: Arc<ResourceCache<Image>>,
    mesh_cache: Arc<ResourceCache<MeshCollection>>,
    chart_file_cache: Arc<ResourceCache<chart_file::Chart>>,
    project_file_cache: Arc<ResourceCache<project_file::Project>>,
    pub shader_cache: ShaderCache,
    pub control_repository: Arc<ControlRepository>,
}

pub struct ResourceLoader {
    pub resource_repository: Arc<ResourceRepository>,
    cached_root: Option<Arc<Project>>,
    last_load_time: Instant,
    is_first_load: bool,
    file_loader: FileLoader,
    async_runtime: tokio::runtime::Runtime,
}

// If loading fails, we want to retry periodically.
const LOAD_RETRY_INTERVAL: Duration = Duration::from_millis(500);

const PROJECT_FILE_NAME: &str = "project.ron";
pub const CHARTS_FOLDER: &str = "charts";
const CHART_FILE_NAME: &str = "chart.ron";

impl ResourceRepository {
    pub fn try_new(file_hash_cache: Arc<FileCache>) -> Result<Self> {
        let control_repository = ControlRepository::load_control_files()?;
        Ok(Self {
            texture_cache: Arc::new(ResourceCache::new(&file_hash_cache, load_texture)),
            mesh_cache: Arc::new(ResourceCache::new(&file_hash_cache, load_mesh_collection)),
            shader_cache: ShaderCache::new(&file_hash_cache),
            chart_file_cache: Arc::new(ResourceCache::new(&file_hash_cache, load_chart_file)),
            project_file_cache: Arc::new(ResourceCache::new(&file_hash_cache, load_project_file)),
            file_hash_cache,
            control_repository: Arc::new(control_repository),
        })
    }

    #[instrument(skip(self, context))]
    pub fn get_texture(
        self: &Arc<Self>,
        context: &Arc<VulkanContext>,
        path: &ResourcePath,
    ) -> LoadFuture<Image> {
        self.texture_cache.get_future(context, path)
    }

    #[instrument(skip(self, context))]
    pub fn get_mesh(
        self: &Arc<Self>,
        context: &Arc<VulkanContext>,
        path: &ResourcePath,
        selector: &str,
    ) -> LoadFuture<Mesh> {
        let mesh_cache = self.mesh_cache.clone();
        let context = context.clone();
        let path = path.clone();
        let selector = selector.to_string();
        let loader = async move {
            let co = mesh_cache.load(&context, &path).await?;
            co.meshes_by_name.get(&selector).cloned().with_context(|| {
                anyhow!("Could not find mesh '{selector}' in '{}'", path.to_string())
            })
        };
        LoadFuture::new(loader)
    }

    #[instrument(skip(self, context))]
    pub async fn load_chart(
        self: &Arc<Self>,
        id: &str,
        context: &Arc<VulkanContext>,
    ) -> Result<Arc<Chart>> {
        let path = ResourcePath::new(&format!("{CHARTS_FOLDER}/{id}"), CHART_FILE_NAME);
        let chart = self.chart_file_cache.load(context, &path).await?;
        chart.load(id, context, self, &path).await
    }

    pub async fn load_project(self: &Arc<Self>, context: &Arc<VulkanContext>) -> Result<Project> {
        let path = ResourcePath::new("", PROJECT_FILE_NAME);
        let project = self.project_file_cache.load(context, &path).await?;
        project.load(context, self).await
    }
}

impl ResourceLoader {
    pub fn try_new() -> Result<Self> {
        let file_loader = FileLoader::new();
        let async_runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        Ok(Self {
            resource_repository: Arc::new(ResourceRepository::try_new(
                file_loader.file_cache.clone(),
            )?),
            cached_root: None,
            last_load_time: Instant::now() - LOAD_RETRY_INTERVAL,
            is_first_load: true,
            file_loader,
            async_runtime,
        })
    }

    fn run_project_loader(&self, context: &Arc<VulkanContext>) -> Result<Project> {
        let resource_repository = self.resource_repository.clone();
        let context = context.clone();
        self.async_runtime
            .block_on(async move { resource_repository.load_project(&context).await })
    }

    #[instrument(skip_all, name = "load")]
    pub fn get_or_load_project(&mut self, context: &Arc<VulkanContext>) -> Option<Arc<Project>> {
        let has_file_changes = self.file_loader.handle_file_changes();
        if has_file_changes
            || self.is_first_load
            || (self.cached_root.is_none()
                && self
                    .file_loader
                    .file_cache
                    .has_missing_files
                    .load(Ordering::Relaxed)
                && self.last_load_time.elapsed() > LOAD_RETRY_INTERVAL)
        {
            let now = Instant::now();
            self.resource_repository
                .control_repository
                .reset_component_usage_counts();
            self.file_loader.file_cache.prepare_loading_cycle();
            let result = self.run_project_loader(context);
            self.file_loader.update_watchers();
            match result {
                Ok(project) => {
                    info!("Project length: {} seconds", project.length);
                    info!("Loading took {:?}", now.elapsed());
                    self.cached_root = Some(Arc::new(project));
                }
                Err(err) => {
                    if self.is_first_load || has_file_changes {
                        error!("Error loading project: {:?}", err);
                    }
                    self.cached_root = None;
                }
            };
            self.last_load_time = Instant::now();
            self.is_first_load = false;
        }
        self.cached_root.clone()
    }
}

fn to_vec3_neg(v: &russimp::Vector3D) -> [f32; 3] {
    [v.x, v.y, -v.z]
}

fn to_vec3_b(v: &russimp::Vector3D) -> [f32; 3] {
    [v.x, v.y, -v.z]
}
fn to_vec2(v: &russimp::Vector3D) -> [f32; 2] {
    [v.x, v.y]
}

#[instrument(skip_all)]
fn load_mesh_collection(
    context: &Arc<VulkanContext>,
    content: &[u8],
    _resource_name: &str,
) -> Result<Arc<MeshCollection>> {
    let scene = Scene::from_buffer(
        content,
        vec![
            PostProcess::CalculateTangentSpace,
            PostProcess::Triangulate,
            PostProcess::JoinIdenticalVertices,
            PostProcess::SortByPrimitiveType,
            PostProcess::FlipUVs,
            PostProcess::OptimizeMeshes,
        ],
        "",
    )?;
    debug!("Meshes in file: {}", scene.meshes.len());
    let mut meshes_by_name = HashMap::new();
    for mesh in scene.meshes {
        let name = mesh.name.clone();
        if mesh.vertices.is_empty() {
            warn!("No vertices found in mesh '{name}'");
            continue;
        }
        if mesh.normals.is_empty() {
            warn!("No normals found in mesh '{name}'");
            continue;
        }
        if mesh.texture_coords.is_empty() {
            warn!("No texture coordinates found in mesh '{name}'");
            continue;
        }
        if mesh.tangents.is_empty() {
            warn!("No tangents found in mesh '{name}'");
            continue;
        }
        let uvs = mesh.texture_coords[0]
            .as_ref()
            .context("No texture coordinates found")?;
        let mut vertices = vec![];
        for (index, face) in mesh.faces.iter().enumerate() {
            ensure!(
                face.0.len() == 3,
                "Face {index} in mesh '{name}' has {} vertices, expected 3",
                face.0.len()
            );
            for i in 0..3 {
                let vertex = Vertex3 {
                    a_position: to_vec3_b(&mesh.vertices[face.0[i] as usize]),
                    a_normal: to_vec3_b(&mesh.normals[face.0[i] as usize]),
                    a_tangent: to_vec3_neg(&mesh.tangents[face.0[i] as usize]),
                    a_uv: to_vec2(&uvs[face.0[i] as usize]),
                    a_padding: 0.0,
                };
                vertices.push(vertex);
            }
        }
        let mesh = Arc::new(Mesh::try_new(context, vertices)?);
        meshes_by_name.insert(name, mesh);
    }

    info!("Meshes loaded: {}", meshes_by_name.keys().join(", "));
    Ok(Arc::new(MeshCollection { meshes_by_name }))
}

#[instrument(skip_all)]
fn load_texture(
    context: &Arc<VulkanContext>,
    content: &[u8],
    resource_name: &str,
) -> Result<Arc<Image>> {
    let now = Instant::now();
    let rgba = image::load_from_memory(content)?.to_rgba8();
    info!("Decoded image in {:?}", now.elapsed());
    let dimensions = ImageDimensions::Dim2d {
        width: rgba.dimensions().0,
        height: rgba.dimensions().1,
        array_layers: 1,
    };

    let mut cbb = AutoCommandBufferBuilder::primary(
        &context.command_buffer_allocator,
        context
            .vulkano_context
            .graphics_queue()
            .queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )?;

    let image = ImmutableImage::from_iter(
        context.vulkano_context.memory_allocator(),
        rgba.into_raw(),
        dimensions,
        MipmapsCount::Log2,
        Format::R8G8B8A8_UNORM,
        &mut cbb,
    )?;
    let _fut = cbb
        .build()?
        .execute(context.vulkano_context.graphics_queue().clone())?;

    let image = Image::new_immutable(resource_name, image);
    Ok(image)
}

#[instrument(skip_all)]
pub fn load_chart_file(
    _context: &Arc<VulkanContext>,
    content: &[u8],
    _resource_name: &str,
) -> Result<Arc<chart_file::Chart>> {
    let ron =
        ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    let chart = ron.from_str::<chart_file::Chart>(std::str::from_utf8(content)?)?;
    Ok(Arc::new(chart))
}

#[instrument(skip_all)]
pub fn load_project_file(
    _context: &Arc<VulkanContext>,
    content: &[u8],
    _resource_name: &str,
) -> Result<Arc<project_file::Project>> {
    let ron =
        ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    let project = ron.from_str::<project_file::Project>(std::str::from_utf8(content)?)?;
    Ok(Arc::new(project))
}
