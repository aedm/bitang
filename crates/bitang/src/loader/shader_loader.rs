use crate::control::controls::GlobalType;
use crate::loader::async_cache::AsyncCache;
use crate::loader::file_cache::{ContentHash, FileCache, FileCacheEntry};
use crate::loader::shader_compiler::{
    NamedResourceBinding, ShaderArtifact, ShaderCompilation, ShaderCompilationLocalUniform,
};
use crate::loader::{compute_hash, ResourcePath};
use crate::render::shader::{GlobalUniformMapping, ShaderKind};
use crate::tool::VulkanContext;
use anyhow::{anyhow, bail, ensure, Context, Error, Result};
use dashmap::{DashMap, DashSet};
use spirv_reflect::types::{ReflectDescriptorType, ReflectTypeFlags};
use std::mem::size_of;
use std::str::FromStr;
use std::sync::Arc;
use tokio::task::spawn_blocking;
use tracing::{debug, error, info, instrument, trace};
use vulkano::shader::ShaderModule;

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
    None(ShaderArtifact),

    /// The next step in the include chain
    NextInclude(ShaderTreeNode),
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct ShaderCacheKey {
    source_path: ResourcePath,
    kind: ShaderKind,
    macros: Vec<(String, String)>,
}

pub struct ShaderCache {
    file_hash_cache: Arc<FileCache>,
    shader_tree: Arc<DashMap<ShaderCacheKey, ShaderTreeNode>>,

    /// A shader cache that's only valid for the current load cycle. During the load cycle,
    /// file contents are assumed not to change, so we can use a cache key that consist only
    /// of the file path and compile macros.
    load_cycle_shader_cache: AsyncCache<ShaderCacheKey, ShaderArtifact>,
}

impl ShaderCache {
    pub fn new(file_hash_cache: &Arc<FileCache>) -> Self {
        Self {
            file_hash_cache: file_hash_cache.clone(),
            shader_tree: Arc::new(DashMap::new()),
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
        let header = self.load_source(&common_path).await?;
        let source = format!("{header}\n{}", self.load_source(&source_path).await?);

        let shaderc_kind = match kind {
            ShaderKind::Vertex => shaderc::ShaderKind::Vertex,
            ShaderKind::Fragment => shaderc::ShaderKind::Fragment,
        };

        let key = ShaderCacheKey {
            source_path: source_path.clone(),
            kind,
            macros: vec![],
        };

        // let source_path = source_path.clone();
        let shader_load_func = {
            let shader_tree = Arc::clone(&self.shader_tree);
            let context = Arc::clone(&context);
            let file_hash_cache = Arc::clone(&self.file_hash_cache);
            async move {
                // TODO: access cache

                let compile_task = spawn_blocking(move || {
                    ShaderCompilation::compile_shader(
                        &context,
                        &source_path,
                        &source,
                        shaderc_kind,
                        file_hash_cache,
                    )
                });
                let ShaderCompilation {
                    include_chain,
                    shader_artifact,
                } = compile_task.await??;

                // TODO: write result to cache

                Ok(Arc::new(shader_artifact))
            }
        };
        self.load_cycle_shader_cache
            .get(key, shader_load_func)
            .await
    }

    // #[instrument(skip(context, source, kind, file_hash_cache))]
    // fn compile_shader_module(
    //     context: &Arc<VulkanContext>,
    //     source: &str,
    //     path: &ResourcePath,
    //     kind: shaderc::ShaderKind,
    //     file_hash_cache: Arc<FileCache>,
    // ) -> Result<ShaderArtifact> {
    //     let path_str = path.to_string();
    //     let now = std::time::Instant::now();
    //     let compiler = shaderc::Compiler::new().context("Failed to create shader compiler")?;
    //     let mut options =
    //         shaderc::CompileOptions::new().context("Failed to create shader compiler options")?;
    //     options.set_target_env(
    //         shaderc::TargetEnv::Vulkan,
    //         shaderc::EnvVersion::Vulkan1_1 as u32,
    //     );
    //     // TODO: Enable optimization
    //     // options.set_optimization_level(shaderc::OptimizationLevel::Performance);
    //
    //     let deps = Arc::new(DashSet::<ResourcePath>::new());
    //     let deps_clone = deps.clone();
    //     options.set_include_callback(move |include_name, include_type, source_name, depth| {
    //         error!(
    //             "#include '{include_name}' ({include_type:?}) from '{source_name}' (depth: {depth})",
    //         );
    //         let source_path = ResourcePath::from_str(source_name).map_err(|err| err.to_string())?;
    //         let include_path = source_path.relative_path(include_name);
    //         deps_clone.insert(include_path.clone());
    //         let included_source_u8 = {
    //             let file_hash_cache = file_hash_cache.clone();
    //             let include_path = include_path.clone();
    //             tokio::runtime::Handle::current()
    //                 .block_on(async move { file_hash_cache.get(&include_path).await })
    //                 .map_err(|err| err.to_string())?
    //         };
    //         let content = String::from_utf8(included_source_u8.content.clone())
    //             .map_err(|err| err.to_string())?;
    //         Ok(shaderc::ResolvedInclude {
    //             resolved_name: include_path.to_string(),
    //             content,
    //         })
    //     });
    //
    //     let spirv = compiler.compile_into_spirv(source, kind, &path_str, "main", Some(&options))?;
    //     let spirv_binary = spirv.as_binary_u8();
    //     info!("compiled in {:?}.", now.elapsed());
    //
    //     // Extract metadata from SPIRV
    //     let reflect = spirv_reflect::ShaderModule::load_u8_data(spirv_binary)
    //         .map_err(|err| anyhow!("Failed to reflect SPIRV binary of shader '{path}': {err}"))?;
    //     let entry_point = reflect
    //         .enumerate_entry_points()
    //         .map_err(Error::msg)?
    //         .into_iter()
    //         .find(|ep| ep.name == "main")
    //         .with_context(|| format!("Failed to find entry point 'main' in '{path}'"))?;
    //
    //     let module = unsafe { ShaderModule::from_bytes(context.device.clone(), spirv_binary) }?;
    //
    //     let descriptor_set_index = match kind {
    //         shaderc::ShaderKind::Vertex => 0,
    //         shaderc::ShaderKind::Fragment => 1,
    //         _ => panic!("Unsupported shader kind"),
    //     };
    //
    //     let dependencies: Vec<_> = deps.iter().map(|dep| dep.clone()).collect();
    //
    //     // Find the descriptor set that belongs to the current shader stage
    //     let Some(descriptor_set) = entry_point
    //         .descriptor_sets
    //         .iter()
    //         .find(|ds| ds.set == descriptor_set_index) else {
    //         // The entire descriptor set is empty, so we can just use the module
    //         return Ok(ShaderArtifact {
    //             module,
    //             samplers: vec![],
    //             buffers: vec![],
    //             local_uniform_bindings: vec![],
    //             global_uniform_bindings: vec![],
    //             uniform_buffer_size: 0,
    //             dependencies
    //         });
    //     };
    //
    //     // Find all samplers
    //     let samplers: Vec<_> = descriptor_set
    //         .bindings
    //         .iter()
    //         .filter(|binding| {
    //             binding.descriptor_type == ReflectDescriptorType::CombinedImageSampler
    //         })
    //         .map(|binding| NamedResourceBinding {
    //             name: binding.name.clone(),
    //             binding: binding.binding,
    //         })
    //         .collect();
    //
    //     // Find all buffers
    //     let buffers: Vec<_> = descriptor_set
    //         .bindings
    //         .iter()
    //         .filter(|binding| binding.descriptor_type == ReflectDescriptorType::StorageBuffer)
    //         .map(|binding| NamedResourceBinding {
    //             name: binding.name.clone(),
    //             binding: binding.binding,
    //         })
    //         .collect();
    //     debug!(
    //         "Found {} samplers and {} buffers, SPIRV size: {}.",
    //         samplers.len(),
    //         buffers.len(),
    //         spirv_binary.len()
    //     );
    //
    //     // Find the uniform block that contains all local and global uniforms
    //     let uniform_block = &descriptor_set
    //         .bindings
    //         .iter()
    //         .find(|binding| binding.descriptor_type == ReflectDescriptorType::UniformBuffer);
    //
    //     // Find local and global uniforms
    //     let (local_uniform_bindings, global_uniform_bindings, uniform_buffer_size) =
    //         match uniform_block {
    //             Some(binding) => {
    //                 let members = &binding.block.members;
    //                 let local_uniform_bindings = members
    //                     .iter()
    //                     .filter(|var| !var.name.starts_with(GLOBAL_UNIFORM_PREFIX))
    //                     .map(|var| {
    //                         let Some(type_desc) = &var.type_description else {
    //                             bail!(
    //                                 "Failed to get type description for uniform variable {}",
    //                                 var.name
    //                             );
    //                         };
    //                         ensure!(
    //                             (type_desc.type_flags & !ReflectTypeFlags::VECTOR)
    //                                 == ReflectTypeFlags::FLOAT,
    //                             "Uniform variable {} is not a float or vector",
    //                             var.name
    //                         );
    //                         Ok(ShaderCompilationLocalUniform {
    //                             name: var.name.clone(),
    //                             f32_offset: var.offset as usize / size_of::<f32>(),
    //                             f32_count: var.size as usize / size_of::<f32>(),
    //                         })
    //                     })
    //                     .collect::<Result<Vec<_>>>()?;
    //                 let global_uniform_bindings = members
    //                     .iter()
    //                     .filter(|var| var.name.starts_with(GLOBAL_UNIFORM_PREFIX))
    //                     .map(|var| {
    //                         GlobalType::from_str(&var.name[(GLOBAL_UNIFORM_PREFIX.len())..]).map(
    //                             |global_type| GlobalUniformMapping {
    //                                 global_type,
    //                                 f32_offset: var.offset as usize / size_of::<f32>(),
    //                             },
    //                         )
    //                     })
    //                     .collect::<::core::result::Result<Vec<_>, _>>()?;
    //                 let uniform_buffer_size = binding.block.size as usize;
    //                 (
    //                     local_uniform_bindings,
    //                     global_uniform_bindings,
    //                     uniform_buffer_size,
    //                 )
    //             }
    //             None => {
    //                 trace!("WARNING: No uniform block found in '{:?}'", path_str);
    //                 (vec![], vec![], 0)
    //             }
    //         };
    //
    //     let result = ShaderArtifact {
    //         module,
    //         samplers,
    //         buffers,
    //         local_uniform_bindings,
    //         global_uniform_bindings,
    //         uniform_buffer_size,
    //         dependencies,
    //     };
    //
    //     trace!(
    //         "Local uniforms: {:?}",
    //         result
    //             .local_uniform_bindings
    //             .iter()
    //             .map(|u| &u.name)
    //             .collect::<Vec<_>>()
    //     );
    //     trace!(
    //         "Global uniforms: {:?}",
    //         result
    //             .global_uniform_bindings
    //             .iter()
    //             .map(|u| u.global_type)
    //             .collect::<Vec<_>>()
    //     );
    //     trace!(
    //         "Textures: {:?}",
    //         result.samplers.iter().map(|u| &u.name).collect::<Vec<_>>()
    //     );
    //     Ok(result)
    // }

    async fn load_source(&self, path: &ResourcePath) -> Result<String> {
        let cache_entry = self.file_hash_cache.get(path).await?;
        let FileCacheEntry {
            hash: _,
            content: source,
        } = cache_entry.as_ref();
        Ok(std::str::from_utf8(source)?.to_string())
    }

    pub fn display_load_errors(&self) {
        self.load_cycle_shader_cache.display_load_errors();
    }

    pub fn reset_load_cycle(&self, changed_files: Option<&Vec<ResourcePath>>) {
        self.load_cycle_shader_cache.clear();
        // if let Some(changed_files) = changed_files {
        //     for file in changed_files {
        //         self.shader_tree.remove(&ShaderTreeRootKey {
        //             source_hash: ContentHash::default(),
        //             kind: ShaderKind::Vertex,
        //         });
        //         self.shader_tree.remove(&ShaderTreeRootKey {
        //             source_hash: ContentHash::default(),
        //             kind: ShaderKind::Fragment,
        //         });
        //     }
        // }
    }
}
