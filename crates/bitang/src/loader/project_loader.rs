use crate::control::controls::ControlRepository;
use crate::file::{chart_file, project_file};
use crate::loader::async_cache::LoadFuture;
use crate::loader::file_cache::{FileCache, FileManager};
use crate::loader::resource_cache::ResourceCache;
use crate::loader::resource_repository::ResourceRepository;
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
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, instrument, warn};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract,
};
use vulkano::format::Format;
use vulkano::image::{ImageDimensions, ImmutableImage, MipmapsCount};

// If loading fails, we want to retry periodically.
const LOAD_RETRY_INTERVAL: Duration = Duration::from_millis(500);

pub struct ProjectLoader {
    pub resource_repository: Arc<ResourceRepository>,
    cached_root: Option<Arc<Project>>,
    last_load_time: Instant,
    is_first_load: bool,
    file_loader: FileManager,
    async_runtime: tokio::runtime::Runtime,
}

impl ProjectLoader {
    pub fn try_new() -> Result<Self> {
        let file_loader = FileManager::new();
        let async_runtime = tokio::runtime::Runtime::new()?;
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

    fn run_project_loader(&mut self, context: &Arc<VulkanContext>) -> Result<Project> {
        self.async_runtime.block_on(async {
            let result = self.resource_repository.load_project(context).await;
            self.file_loader.update_watchers().await;
            result
        })
    }

    #[instrument(skip_all, name = "load")]
    pub fn get_or_load_project(&mut self, context: &Arc<VulkanContext>) -> Option<Arc<Project>> {
        let has_file_changes = self.file_loader.handle_file_changes();
        let needs_retry = self.cached_root.is_none()
            && self.file_loader.has_missing_files()
            && self.last_load_time.elapsed() > LOAD_RETRY_INTERVAL;
        if has_file_changes || self.is_first_load || needs_retry {
            let now = Instant::now();
            self.resource_repository
                .control_repository
                .reset_component_usage_counts();
            self.file_loader.file_cache.prepare_loading_cycle();
            match self.run_project_loader(context) {
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
