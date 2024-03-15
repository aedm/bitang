use crate::loader::file_cache::FileManager;
use crate::loader::resource_repository::ResourceRepository;
use crate::render::project::Project;
use crate::tool::VulkanContext;
use anyhow::Result;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, instrument};

// If loading fails, we want to retry periodically.
const LOAD_RETRY_INTERVAL: Duration = Duration::from_millis(500);

/// Manages the load and reload cycles of the project.
pub struct ProjectLoader {
    pub resource_repository: Rc<ResourceRepository>,
    cached_root: Option<Rc<Project>>,
    last_load_time: Instant,
    file_loader: FileManager,
    async_runtime: tokio::runtime::Runtime,
}

impl ProjectLoader {
    pub fn try_new() -> Result<Self> {
        let file_loader = FileManager::new();
        let async_runtime = tokio::runtime::Runtime::new()?;
        Ok(Self {
            resource_repository: Rc::new(ResourceRepository::try_new(
                file_loader.file_cache.clone(),
            )?),
            cached_root: None,
            last_load_time: Instant::now() - LOAD_RETRY_INTERVAL,
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
    pub fn get_or_load_project(&mut self, context: &Arc<VulkanContext>) -> Option<Rc<Project>> {
        let changed_files = self.file_loader.handle_file_changes();
        let needs_retry = self.cached_root.is_none()
            && self.file_loader.has_missing_files()
            && self.last_load_time.elapsed() > LOAD_RETRY_INTERVAL;
        if changed_files.is_some() || needs_retry {
            let now = Instant::now();
            self.resource_repository
                .start_load_cycle(changed_files.as_ref());
            match self.run_project_loader(context) {
                Ok(project) => {
                    info!("Project length: {} seconds", project.length);
                    info!("Loading took {:?}", now.elapsed());
                    self.cached_root = Some(Rc::new(project));
                }
                Err(err) => {
                    if changed_files.is_some() {
                        error!("Failed to load project: {:?}", err);
                    }
                    self.resource_repository.display_load_errors();
                    self.cached_root = None;
                }
            };
            self.last_load_time = Instant::now();
        }
        self.cached_root.clone()
    }
}
