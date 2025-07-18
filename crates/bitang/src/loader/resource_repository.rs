use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{anyhow, ensure, Context, Result};
use image::GenericImageView;
use jxl_oxide::JxlImage;
use tracing::{info, instrument, warn};

use crate::engine::{
    BitangImage, Chart, ControlRepository, GpuContext, Mesh, PixelFormat, Project,
};
use crate::file::{chart_file, project_file};
use crate::loader::async_cache::LoadFuture;
use crate::loader::file_cache::FileCache;
use crate::loader::gltf_loader::load_mesh_collection;
use crate::loader::resource_cache::ResourceCache;
use crate::loader::resource_path::ResourcePath;
use crate::loader::shader_cache::ShaderCache;
use crate::loader::{CHARTS_FOLDER, CHART_FILE_NAME, PROJECT_FILE_NAME};

pub struct SceneNode {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub mesh: Arc<Mesh>,
}

pub struct SceneFile {
    pub nodes_by_name: HashMap<String, Arc<SceneNode>>,
}

pub struct ResourceRepository {
    pub root_path: Arc<PathBuf>,
    file_cache: Arc<FileCache>,
    texture_cache: Arc<ResourceCache<BitangImage>>,
    pub mesh_cache: Arc<ResourceCache<SceneFile>>,
    chart_file_cache: Arc<ResourceCache<chart_file::Chart>>,
    project_file_cache: Arc<ResourceCache<project_file::Project>>,
    pub shader_cache: ShaderCache,
    pub control_repository: Arc<ControlRepository>,
}

impl ResourceRepository {
    pub fn try_new(file_cache: Arc<FileCache>) -> Result<Self> {
        let control_repository = ControlRepository::load_control_files(&file_cache.root_path)?;
        Ok(Self {
            root_path: file_cache.root_path.clone(),
            texture_cache: Arc::new(ResourceCache::new(&file_cache, load_texture)),
            mesh_cache: Arc::new(ResourceCache::new(&file_cache, load_mesh_collection)),
            shader_cache: ShaderCache::new(&file_cache),
            chart_file_cache: Arc::new(ResourceCache::new(&file_cache, load_chart_file)),
            project_file_cache: Arc::new(ResourceCache::new(&file_cache, load_project_file)),
            file_cache,
            control_repository: Arc::new(control_repository),
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
        self.file_cache.start_load_cycle();
        self.texture_cache.start_load_cycle();
        self.mesh_cache.start_load_cycle();
        self.chart_file_cache.start_load_cycle();
        self.project_file_cache.start_load_cycle();
        self.control_repository.reset_component_usage_counts();
        self.shader_cache.reset_load_cycle(changed_files);
    }

    #[instrument(skip(self, context))]
    pub fn get_texture(
        self: &Arc<Self>,
        context: &Arc<GpuContext>,
        path: &ResourcePath,
    ) -> LoadFuture<BitangImage> {
        self.texture_cache.get_future(context, path)
    }

    // TODO: try make this pure async
    // TODO: use get_mesh_collection instead
    #[instrument(skip(self, context))]
    pub fn get_mesh(
        self: &Arc<Self>,
        context: &Arc<GpuContext>,
        path: &ResourcePath,
        selector: &str,
    ) -> LoadFuture<Mesh> {
        let mesh_cache = self.mesh_cache.clone();
        let context = context.clone();
        let path_clone = path.clone();
        let selector = selector.to_string();
        let loader = async move {
            let co = mesh_cache.load(&context, &path_clone).await?;
            let node =
                co.nodes_by_name.get(&selector).cloned().with_context(|| {
                    anyhow!("Could not find mesh '{selector}' in '{path_clone:?}'")
                })?;
            Ok(Arc::clone(&node.mesh))
        };
        LoadFuture::new(format!("mesh:{path:?}"), loader)
    }

    #[instrument(skip(self, context))]
    pub async fn load_chart(
        self: &Arc<Self>,
        id: &str,
        context: &Arc<GpuContext>,
    ) -> Result<Arc<Chart>> {
        let subdirectory = [CHARTS_FOLDER, id].iter().collect::<PathBuf>();
        let path = ResourcePath::new(&self.root_path, subdirectory, CHART_FILE_NAME);
        let chart = self.chart_file_cache.load(context, &path).await?;
        chart
            .load(id, context, self, &path)
            .await
            .with_context(|| anyhow!("Failed to load chart '{id}'"))
    }

    pub async fn load_project(self: &Arc<Self>, context: &Arc<GpuContext>) -> Result<Project> {
        let path = ResourcePath::new(&self.root_path, PathBuf::new(), PROJECT_FILE_NAME);
        let project = self.project_file_cache.load(context, &path).await?;
        project.load(context, self).await
    }
}

#[instrument(skip(context, content))]
fn load_texture(
    context: &Arc<GpuContext>,
    content: &[u8],
    resource_name: &str,
) -> Result<Arc<BitangImage>> {
    let now = Instant::now();

    let image = if resource_name.ends_with(".jxl") {
        let image =
            JxlImage::builder().read(content).map_err(|e| anyhow!("Can't load image {e}"))?;
        let size = [image.width(), image.height()];
        let render = image.render_frame(0).map_err(|e| anyhow!("Can't render image {e}"))?;
        let frame = render.image();
        let buf = frame.buf();
        ensure!(
            image.image_header().metadata.encoded_color_channels() == 3,
            "Only RGB images are supported"
        );
        // Map RGB to RGBA
        let mut raw = vec![0.0f32; (size[0] * size[1] * 4) as usize];
        for i in 0..((size[0] * size[1]) as usize) {
            for o in 0..3 {
                raw[i * 4 + o] = buf[i * 3 + o];
            }
        }
        BitangImage::immutable_from_pixel_data(
            resource_name,
            context,
            PixelFormat::Rgba32F,
            size,
            bytemuck::cast_slice(&raw),
        )
    } else {
        let image = image::load_from_memory(content)?;
        let size = [image.dimensions().0, image.dimensions().1];
        if resource_name.ends_with(".hdr") || resource_name.ends_with(".exr") {
            BitangImage::immutable_from_pixel_data(
                resource_name,
                context,
                PixelFormat::Rgba32F,
                size,
                bytemuck::cast_slice(&image.into_rgba32f().into_raw()),
            )
        } else {
            BitangImage::immutable_from_pixel_data(
                resource_name,
                context,
                PixelFormat::Rgba8Srgb,
                size,
                &image.into_rgba8().into_raw(),
            )
        }
    };
    info!("decoded in {:?}", now.elapsed());
    image
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
    _context: &Arc<GpuContext>,
    content: &[u8],
    _resource_name: &str,
) -> Result<Arc<chart_file::Chart>> {
    let chart = ron_loader().from_str::<chart_file::Chart>(std::str::from_utf8(content)?)?;
    Ok(Arc::new(chart))
}

#[instrument(skip_all)]
pub fn load_project_file(
    _context: &Arc<GpuContext>,
    content: &[u8],
    _resource_name: &str,
) -> Result<Arc<project_file::Project>> {
    let project = ron_loader().from_str::<project_file::Project>(std::str::from_utf8(content)?)?;
    Ok(Arc::new(project))
}
