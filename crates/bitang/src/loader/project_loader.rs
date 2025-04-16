use crate::loader::file_cache::{FileCache, FileChangeHandler};
use crate::loader::resource_repository::ResourceRepository;
use crate::engine::project::Project;
use crate::engine::GpuContext;
use anyhow::{ensure, Result};
use dunce::canonicalize;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, info, instrument};

// If loading fails, we want to retry periodically.
const LOAD_RETRY_INTERVAL: Duration = Duration::from_millis(500);

/// Manages the load and reload cycles of the project.
pub struct ProjectLoader {
    // TODO: remove if not needed
    pub _root_path: Arc<PathBuf>,
    pub resource_repository: Rc<ResourceRepository>,
    cached_root: Option<Rc<Project>>,
    last_load_time: Instant,
    file_change_handler: FileChangeHandler,
    async_runtime: tokio::runtime::Runtime,
}

impl ProjectLoader {
    pub fn try_new(root_path: &str) -> Result<Self> {
        let pwd = std::env::current_dir()?;
        let root_path = Arc::new(canonicalize(pwd.join(PathBuf::from(root_path)))?);
        ensure!(root_path.exists());

        let file_cache = Arc::new(FileCache::new(&root_path));
        let file_change_handler = FileChangeHandler::new(&file_cache);
        let async_runtime = tokio::runtime::Runtime::new()?;
        Ok(Self {
            _root_path: root_path,
            resource_repository: Rc::new(ResourceRepository::try_new(Arc::clone(&file_cache))?),
            cached_root: None,
            last_load_time: Instant::now() - LOAD_RETRY_INTERVAL,
            file_change_handler,
            async_runtime,
        })
    }

    fn run_project_loader(&mut self, context: &Arc<GpuContext>) -> Result<Project> {
        self.async_runtime.block_on(async {
            let result = self.resource_repository.load_project(context).await;
            self.file_change_handler.update_watchers().await;
            result
        })
    }

    #[instrument(skip_all, name = "load")]
    pub fn get_or_load_project(&mut self, context: &Arc<GpuContext>) -> Option<Rc<Project>> {
        let changed_files = self.file_change_handler.handle_file_changes();
        let needs_retry = self.cached_root.is_none()
            && self.file_change_handler.has_missing_files()
            && self.last_load_time.elapsed() > LOAD_RETRY_INTERVAL;
        if changed_files.is_some() || needs_retry {
            let now = Instant::now();
            self.resource_repository.start_load_cycle(changed_files.as_ref());
            match self.run_project_loader(context) {
                Ok(project) => {
                    info!("Project length: {} seconds", project.length);
                    info!("Load time {:?}", now.elapsed());
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
