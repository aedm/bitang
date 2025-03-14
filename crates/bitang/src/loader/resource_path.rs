use anyhow::{anyhow, Result};
use anyhow::{bail, Context};
use dunce::canonicalize;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The path of a resource file
#[derive(Hash, Eq, PartialEq, Clone)]
pub struct ResourcePath {
    /// Root path of the project
    pub root_path: Arc<PathBuf>,

    /// Relative subfolder of the file without the file name
    pub subdirectory: PathBuf,

    /// The name of the file without the directory
    pub file_name: String,
}

impl ResourcePath {
    pub fn new(root_path: &Arc<PathBuf>, subdirectory: PathBuf, file_name: &str) -> Self {
        Self {
            root_path: Arc::clone(root_path),
            subdirectory,
            file_name: file_name.to_string(),
        }
    }

    pub fn from_pathbuf(root_path: &Arc<PathBuf>, path: &Path) -> anyhow::Result<Self> {
        let relative_path = path.strip_prefix(root_path.as_path())?;
        let subdirectory = relative_path.parent().unwrap_or(Path::new("")).to_path_buf();
        let file_name = relative_path.file_name().unwrap_or_default().to_string_lossy().to_string();
        Ok(Self {
            root_path: Arc::clone(root_path),
            subdirectory,
            file_name,
        })
    }

    /// Returns a new ResourcePath for a file relative to the current one.
    /// Path starting with '/' are relative to the root folder.
    pub fn relative_path(&self, file_name: &str) -> anyhow::Result<Self> {
        let path = Path::new(file_name);
        let components = path.components().collect::<Vec<_>>();

        // let parts = file_name.split('/').collect::<Vec<_>>();
        let subdirectory = if file_name.starts_with(['/', '\\']) {
            components[1..components.len() - 1].iter().collect::<PathBuf>()
        } else {
            self.subdirectory.join(
                // TODO: there was an underflow crash here once
                components[0..components.len() - 1].iter().collect::<PathBuf>(),
            )
        };

        let Some(file_name) = components.last() else {
            bail!("Invalid file name '{}'", file_name);
        };
        let file_name = file_name.as_os_str().to_string_lossy().to_string();

        Ok(Self {
            root_path: Arc::clone(&self.root_path),
            subdirectory,
            file_name,
        })
    }

    /// Returns the path relative to the present working directory.
    /// The purpose is to log the path in a way that is recognizable by the user and the IDE.
    /// If the file is not in the present working directory, the path is absolute.
    pub fn to_pwd_relative_path(&self) -> Result<String> {
        let absolute_path = self.absolute_path()?;
        let pwd = std::env::current_dir()?;
        let path = if let Ok(relative_path) = absolute_path.strip_prefix(&pwd) {
            relative_path
        } else {
            &absolute_path
        };
        Ok(path
            .to_str()
            .with_context(|| format!("Failed to convert path to string: {:?}", path))?
            .to_string())
    }

    /// Makes a ResourcePath from path string relative to the present working directory,
    /// eg. "demo/folder/file.txt".
    /// This is the inverse of `to_pwd_relative_path`, so the path is allowed be absolute.
    #[allow(dead_code)]
    pub fn from_pwd_relative_path(
        root_path: &Arc<PathBuf>,
        path_str: &str,
    ) -> anyhow::Result<Self> {
        let pwd = std::env::current_dir()?;
        let full_path = pwd.join(path_str);
        let Ok(relative) = full_path.strip_prefix(root_path.as_path()) else {
            bail!("Path must be relative to the project root.")
        };
        Ok(Self {
            root_path: Arc::clone(root_path),
            subdirectory: relative.parent().unwrap_or(Path::new("")).to_path_buf(),
            file_name: relative.file_name().unwrap_or_default().to_string_lossy().to_string(),
        })
    }

    pub fn absolute_path(&self) -> Result<PathBuf> {
        canonicalize(self.root_path.join(&self.subdirectory).join(&self.file_name)).map_err(|e| {
            anyhow!(
                "Failed to get absolute path for '{:?}/{:?}/{:?}': {}",
                self.root_path,
                self.subdirectory,
                self.file_name,
                e
            )
        })
    }
}

impl fmt::Debug for ResourcePath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Ok(relative_path) = self.to_pwd_relative_path() {
            write!(f, "{}", relative_path)
        } else if let Ok(absolute_path) = self.absolute_path() {
            write!(f, "{}", absolute_path.to_string_lossy())
        } else {
            let path = self.root_path.join(&self.subdirectory).join(&self.file_name);
            write!(f, "{path:?}")
        }
    }
}
