use crate::control::controls::ControlRepository;
use crate::file::{chart_file, project_file};
use crate::loader::async_cache::LoadFuture;
use crate::loader::file_cache::FileCache;
use crate::loader::gltf_loader::load_mesh_collection;
use crate::loader::resource_cache::ResourceCache;
use crate::loader::shader_loader::ShaderCache;
use crate::loader::{ResourcePath, CHARTS_FOLDER, CHART_FILE_NAME, PROJECT_FILE_NAME};
use crate::render::chart::Chart;
use crate::render::image::BitangImage;
use crate::render::mesh::Mesh;
use crate::render::project::Project;
use crate::tool::VulkanContext;
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, instrument, warn};
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferToImageInfo,
    PrimaryCommandBufferAbstract,
};
use vulkano::format::Format;
use vulkano::image::{Image, ImageType, ImageUsage};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};

pub struct SceneNode {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub mesh: Arc<Mesh>,
}

pub struct SceneFile {
    pub nodes_by_name: HashMap<String, Arc<SceneNode>>,
}

pub struct ResourceRepository {
    file_hash_cache: Arc<FileCache>,
    texture_cache: Arc<ResourceCache<BitangImage>>,
    pub mesh_cache: Arc<ResourceCache<SceneFile>>,
    chart_file_cache: Arc<ResourceCache<chart_file::Chart>>,
    project_file_cache: Arc<ResourceCache<project_file::Project>>,
    pub shader_cache: ShaderCache,
    pub control_repository: Rc<ControlRepository>,
}

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
            control_repository: Rc::new(control_repository),
        })
    }

    pub fn display_load_errors(&self) {
        self.texture_cache.display_load_errors();
        self.mesh_cache.display_load_errors();
        self.shader_cache.display_load_errors();
        self.chart_file_cache.display_load_errors();
        self.project_file_cache.display_load_errors();
    }

    pub fn start_load_cycle(&self, changed_files: Option<&Vec<ResourcePath>>) {
        self.file_hash_cache.start_load_cycle();
        self.texture_cache.start_load_cycle();
        self.mesh_cache.start_load_cycle();
        self.chart_file_cache.start_load_cycle();
        self.project_file_cache.start_load_cycle();
        self.control_repository.reset_component_usage_counts();
        self.shader_cache.reset_load_cycle(changed_files);
    }

    #[instrument(skip(self, context))]
    pub fn get_texture(
        self: &Rc<Self>,
        context: &Arc<VulkanContext>,
        path: &ResourcePath,
    ) -> LoadFuture<BitangImage> {
        self.texture_cache.get_future(context, path)
    }

    // TODO: try make this pure async
    // TODO: use get_mesh_collection instead
    #[instrument(skip(self, context))]
    pub fn get_mesh(
        self: &Rc<Self>,
        context: &Arc<VulkanContext>,
        path: &ResourcePath,
        selector: &str,
    ) -> LoadFuture<Mesh> {
        let mesh_cache = self.mesh_cache.clone();
        let context = context.clone();
        let path_clone = path.clone();
        let selector = selector.to_string();
        let loader = async move {
            let co = mesh_cache.load(&context, &path_clone).await?;
            let node = co.nodes_by_name.get(&selector).cloned().with_context(|| {
                anyhow!(
                    "Could not find mesh '{selector}' in '{}'",
                    path_clone.to_string()
                )
            })?;
            Ok(Arc::clone(&node.mesh))
        };
        LoadFuture::new(format!("mesh:{path}"), loader)
    }

    #[instrument(skip(self, context))]
    pub async fn load_chart(
        self: &Rc<Self>,
        id: &str,
        context: &Arc<VulkanContext>,
    ) -> Result<Rc<Chart>> {
        let path = ResourcePath::new(&format!("{CHARTS_FOLDER}/{id}"), CHART_FILE_NAME);
        let chart = self.chart_file_cache.load(context, &path).await?;
        chart
            .load(id, context, self, &path)
            .await
            .with_context(|| anyhow!("Failed to load chart '{id}'"))
    }

    pub async fn load_project(self: &Rc<Self>, context: &Arc<VulkanContext>) -> Result<Project> {
        let path = ResourcePath::new("", PROJECT_FILE_NAME);
        let project = self.project_file_cache.load(context, &path).await?;
        project.load(context, self).await
    }
}

#[instrument(skip(context, content))]
fn load_texture(
    context: &Arc<VulkanContext>,
    content: &[u8],
    resource_name: &str,
) -> Result<Arc<BitangImage>> {
    let now = Instant::now();
    let rgba = image::load_from_memory(content)?.to_rgba8();
    info!("decoded in {:?}", now.elapsed());
    let dimensions = [rgba.dimensions().0, rgba.dimensions().1, 1];

    let mut cbb = AutoCommandBufferBuilder::primary(
        &context.command_buffer_allocator,
        context.gfx_queue.queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )?;

    let image = Image::new(
        context.memory_allocator.clone(),
        vulkano::image::ImageCreateInfo {
            image_type: ImageType::Dim2d,
            format: Format::R8G8B8A8_UNORM,
            extent: dimensions,
            usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
            // TODO: generate mipmaps
            mip_levels: 1,
            ..Default::default()
        },
        AllocationCreateInfo::default(),
    )?;

    // TODO: move buffer operations to BitangImage.
    let upload_buffer = Buffer::from_iter(
        context.memory_allocator.clone(),
        BufferCreateInfo {
            usage: BufferUsage::TRANSFER_SRC,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_HOST
                | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ..Default::default()
        },
        rgba.into_raw(),
    )
    .unwrap();

    cbb.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
        upload_buffer,
        image.clone(),
    ))
    .unwrap();

    let _fut = cbb.build()?.execute(context.gfx_queue.clone())?;

    let image = BitangImage::new_immutable(resource_name, image);
    Ok(image)
}

fn ron_loader() -> ron::Options {
    ron::Options::default().with_default_extension(
        ron::extensions::Extensions::IMPLICIT_SOME
            | ron::extensions::Extensions::UNWRAP_NEWTYPES
            | ron::extensions::Extensions::UNWRAP_VARIANT_NEWTYPES,
    )
}

#[instrument(skip_all)]
pub fn load_chart_file(
    _context: &Arc<VulkanContext>,
    content: &[u8],
    _resource_name: &str,
) -> Result<Arc<chart_file::Chart>> {
    let chart = ron_loader().from_str::<chart_file::Chart>(std::str::from_utf8(content)?)?;
    Ok(Arc::new(chart))
}

#[instrument(skip_all)]
pub fn load_project_file(
    _context: &Arc<VulkanContext>,
    content: &[u8],
    _resource_name: &str,
) -> Result<Arc<project_file::Project>> {
    let project = ron_loader().from_str::<project_file::Project>(std::str::from_utf8(content)?)?;
    Ok(Arc::new(project))
}
