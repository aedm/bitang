use crate::loader::async_cache::AsyncCache;
use crate::loader::file_cache::{ContentHash, FileCache};
use crate::loader::shader_compiler::{IncludeChainLink, ShaderArtifact, ShaderCompilation};
use crate::loader::ResourcePath;
use crate::render::shader::ShaderKind;
use crate::tool::VulkanContext;
use anyhow::Result;

use dashmap::DashMap;

use std::sync::Arc;
use tokio::task::spawn_blocking;
use tracing::trace;

/// Shader cache is a tree structure.
///
/// For a given set of shader source code, the order of #include directives is assumed
/// to be deterministic.
///
/// Each node represents the next shader source path to be included. The children of that node
/// represent different possible contents of that shader source file. Each child is a tree itself,
/// allowing us to traverse the dependency tree and find an already compiled version of the shader
/// without invoking the shader compiler.
///
/// This mechanism relies on the file cache that stores the hash of each file and only reacts to
/// file changes, allowing fast tree traversal.
struct ShaderTreeNode {
    source_path: ResourcePath,
    subtrees_by_file_content: DashMap<ContentHash, ShaderDependency>,
}

enum ShaderDependency {
    /// The shader has no further include dependencies
    None(Arc<ShaderArtifact>),

    /// The next step in the include chain
    NextInclude(Arc<ShaderTreeNode>),
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct ShaderCacheKey {
    source_path: ResourcePath,
    kind: ShaderKind,
    macros: Vec<(String, String)>,
}

pub struct ShaderCache {
    file_hash_cache: Arc<FileCache>,
    shader_tree_roots: Arc<DashMap<ShaderCacheKey, Arc<ShaderTreeNode>>>,

    /// A shader cache that's only valid for the current load cycle. During the load cycle,
    /// file contents are assumed not to change, so we can use a cache key that consist only
    /// of the file path and compile macros.
    load_cycle_shader_cache: AsyncCache<ShaderCacheKey, ShaderArtifact>,
}

impl ShaderCache {
    pub fn new(file_hash_cache: &Arc<FileCache>) -> Self {
        Self {
            file_hash_cache: file_hash_cache.clone(),
            shader_tree_roots: Arc::new(DashMap::new()),
            load_cycle_shader_cache: AsyncCache::new(),
        }
    }

    pub async fn get(
        &self,
        context: Arc<VulkanContext>,
        source_path: ResourcePath,
        kind: ShaderKind,
        common_path: ResourcePath,
    ) -> Result<Arc<ShaderArtifact>> {
        let shaderc_kind = match kind {
            ShaderKind::Vertex => shaderc::ShaderKind::Vertex,
            ShaderKind::Fragment => shaderc::ShaderKind::Fragment,
        };

        let key = ShaderCacheKey {
            source_path: source_path.clone(),
            kind,
            macros: vec![],
        };

        let shader_tree_root = self
            .shader_tree_roots
            .entry(key.clone())
            .or_insert_with(|| {
                Arc::new(ShaderTreeNode {
                    source_path: source_path.clone(),
                    subtrees_by_file_content: DashMap::new(),
                })
            })
            .value()
            .clone();

        let shader_load_func = {
            let context = Arc::clone(&context);
            let file_hash_cache = Arc::clone(&self.file_hash_cache);

            async move {
                // Traverse the cache tree
                let mut node = shader_tree_root.clone();

                loop {
                    let file = file_hash_cache.get(&node.source_path).await?;
                    let new_node = match node.subtrees_by_file_content.get(&file.hash) {
                        Some(dep) => match dep.value() {
                            ShaderDependency::None(artifact) => {
                                trace!("Cache hit for shader '{:?}'", source_path);
                                return Ok(Arc::clone(artifact));
                            }
                            ShaderDependency::NextInclude(tree_node) => tree_node.clone(),
                        },
                        None => {
                            trace!("Cache miss for shader {source_path:?}");
                            break;
                        }
                    };
                    node = new_node;
                }

                let source_file = file_hash_cache.get(&source_path).await?;
                let header_file = file_hash_cache.get(&common_path).await?;

                let source_str = std::str::from_utf8(&source_file.content)?;
                let header_str = std::str::from_utf8(&header_file.content)?;
                let source = format!("{header_str}\n{source_str}",);

                // No cache hit found, so we need to compile the shader
                let compile_task = {
                    let source_path = source_path.clone();
                    spawn_blocking(move || {
                        ShaderCompilation::compile_shader(
                            &context,
                            &source_path,
                            &source,
                            shaderc_kind,
                            file_hash_cache,
                        )
                    })
                };

                let ShaderCompilation {
                    mut include_chain,
                    shader_artifact,
                } = compile_task.await??;

                let shader_artifact = Arc::new(shader_artifact);
                include_chain.insert(
                    0,
                    IncludeChainLink {
                        resource_path: common_path.clone(),
                        hash: header_file.hash,
                    },
                );

                let mut node = shader_tree_root.clone();
                let mut hash = source_file.hash;

                for dep in include_chain {
                    let next_node = match node
                        .subtrees_by_file_content
                        .entry(hash)
                        .or_insert_with(|| {
                            ShaderDependency::NextInclude(Arc::new(ShaderTreeNode {
                                source_path: dep.resource_path.clone(),
                                subtrees_by_file_content: DashMap::new(),
                            }))
                        })
                        .value()
                    {
                        ShaderDependency::None(_) => panic!("Unexpected cache hit"),
                        ShaderDependency::NextInclude(next) => Arc::clone(next),
                    };
                    node = next_node;
                    hash = dep.hash;
                }
                node.subtrees_by_file_content
                    .insert(hash, ShaderDependency::None(Arc::clone(&shader_artifact)));

                Ok(shader_artifact)
            }
        };
        self.load_cycle_shader_cache
            .get(key, shader_load_func)
            .await
    }

    pub fn display_load_errors(&self) {
        self.load_cycle_shader_cache.display_load_errors();
    }

    pub fn reset_load_cycle(&self, _changed_files: Option<&Vec<ResourcePath>>) {
        self.load_cycle_shader_cache.clear();
    }
}
