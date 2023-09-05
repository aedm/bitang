use crate::control::controls::ControlRepository;
use crate::file::binary_file_cache::BinaryFileCache;
use crate::file::file_hash_cache::FileCache;
use crate::file::shader_loader::ShaderCache;
use crate::file::{chart_file, project_file, ResourcePath};
use crate::render::chart::Chart;
use crate::render::image::Image;
use crate::render::mesh::Mesh;
use crate::render::project::Project;
use crate::render::vulkan_window::VulkanContext;
use crate::render::Vertex3;
use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use russimp::scene::{PostProcess, Scene};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
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
    file_hash_cache: Rc<RefCell<FileCache>>,
    texture_cache: BinaryFileCache<Arc<Image>>,
    mesh_cache: BinaryFileCache<MeshCollection>,
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
            mesh_cache: BinaryFileCache::new(&file_hash_cache, load_mesh_collection),
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
            || self.is_first_load
            || (self.cached_root.is_none()
                && self.file_hash_cache.borrow_mut().has_missing_files
                && self.last_load_time.elapsed() > LOAD_RETRY_INTERVAL)
        {
            let now = Instant::now();
            self.control_repository
                .borrow()
                .reset_component_usage_counts();
            self.file_hash_cache.borrow_mut().prepare_loading_cycle();
            let result = self.load_project(context);
            self.file_hash_cache.borrow_mut().update_watchers();
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

    #[instrument(skip(self, context))]
    pub fn get_texture(
        &mut self,
        context: &VulkanContext,
        path: &ResourcePath,
    ) -> Result<&Arc<Image>> {
        self.texture_cache.get_or_load(context, path)
    }

    #[instrument(skip(self, context))]
    pub fn get_mesh(
        &mut self,
        context: &VulkanContext,
        path: &ResourcePath,
        selector: &str,
    ) -> Result<&Arc<Mesh>> {
        let co = self.mesh_cache.get_or_load(context, path)?;
        co.meshes_by_name.get(selector).with_context(|| {
            anyhow!(
                "Could not find mesh '{}' in '{}'",
                selector,
                path.to_string()
            )
        })
    }

    #[instrument(skip(self, context))]
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
    context: &VulkanContext,
    content: &[u8],
    _resource_name: &str,
) -> Result<MeshCollection> {
    let scene = Scene::from_buffer(
        content,
        vec![
            PostProcess::CalculateTangentSpace,
            PostProcess::Triangulate,
            PostProcess::JoinIdenticalVertices,
            PostProcess::SortByPrimitiveType,
            // PostProcess::GenerateSmoothNormals,
            PostProcess::FlipUVs,
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
            if face.0.len() != 3 {
                return Err(anyhow!(
                    "Face {index} in mesh '{name}' has {} vertices, expected 3",
                    face.0.len()
                ));
            }
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
    Ok(MeshCollection { meshes_by_name })
}

#[instrument(skip_all)]
fn load_texture(
    context: &VulkanContext,
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
        context.context.graphics_queue().queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )?;

    let image = ImmutableImage::from_iter(
        context.context.memory_allocator(),
        rgba.into_raw(),
        dimensions,
        MipmapsCount::Log2,
        Format::R8G8B8A8_UNORM,
        &mut cbb,
    )?;
    let _fut = cbb
        .build()?
        .execute(context.context.graphics_queue().clone())?;

    let image = Image::new_immutable(resource_name, image);
    Ok(image)
}

#[instrument(skip_all)]
pub fn load_chart_file(
    _context: &VulkanContext,
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
    _context: &VulkanContext,
    content: &[u8],
    _resource_name: &str,
) -> Result<Arc<project_file::Project>> {
    let ron =
        ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    let project = ron.from_str::<project_file::Project>(std::str::from_utf8(content)?)?;
    Ok(Arc::new(project))
}
