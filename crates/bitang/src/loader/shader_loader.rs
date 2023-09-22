use crate::control::controls::GlobalType;
use crate::loader::async_cache::AsyncCache;
use crate::loader::file_cache::{ContentHash, FileCache, FileCacheEntry};
use crate::loader::{compute_hash, ResourcePath};
use crate::render::shader::GlobalUniformMapping;
use crate::render::vulkan_window::VulkanContext;
use anyhow::{anyhow, bail, ensure, Context, Error, Result};
use spirv_reflect::types::{ReflectDescriptorType, ReflectTypeFlags};
use std::mem::size_of;
use std::str::FromStr;
use std::sync::Arc;
use tokio::task::spawn_blocking;
use tracing::{debug, info, instrument, trace};
use vulkano::shader::ShaderModule;

#[derive(Hash, PartialEq, Eq, Clone)]
struct ShaderCacheKey {
    vertex_shader_hash: ContentHash,
    fragment_shader_hash: ContentHash,
}

pub struct ShaderCacheValue {
    pub vertex_shader: ShaderCompilationResult,
    pub fragment_shader: ShaderCompilationResult,
}

#[derive(Debug)]
pub struct ShaderCompilationResult {
    pub module: Arc<ShaderModule>,
    pub samplers: Vec<ShaderCompilationResource>,
    pub buffers: Vec<ShaderCompilationResource>,
    pub global_uniform_bindings: Vec<GlobalUniformMapping>,
    pub local_uniform_bindings: Vec<ShaderCompilationLocalUniform>,
    pub uniform_buffer_size: usize,
}

// A descriptor binding extracted from the compiled shader
#[derive(Debug)]
pub struct ShaderCompilationResource {
    pub name: String,
    pub binding: u32,
}

// Metadata of a local uniform extracted from the compiled shader
#[derive(Debug)]
pub struct ShaderCompilationLocalUniform {
    pub name: String,
    pub f32_offset: usize,
    pub f32_count: usize,
}

pub struct ShaderCache {
    file_hash_cache: Arc<FileCache>,
    shader_cache: AsyncCache<ShaderCacheKey, ShaderCacheValue>,
}

const GLOBAL_UNIFORM_PREFIX: &str = "g_";

impl ShaderCache {
    pub fn new(file_hash_cache: &Arc<FileCache>) -> Self {
        Self {
            file_hash_cache: file_hash_cache.clone(),
            shader_cache: AsyncCache::new(),
        }
    }

    pub async fn get(
        &self,
        context: &Arc<VulkanContext>,
        vs_path: &ResourcePath,
        fs_path: &ResourcePath,
        common_path: &ResourcePath,
    ) -> Result<Arc<ShaderCacheValue>> {
        let header = self.load_source(common_path).await?;
        let vs_source = format!("{header}\n{}", self.load_source(vs_path).await?);
        let fs_source = format!("{header}\n{}", self.load_source(fs_path).await?);

        let vs_hash = compute_hash(vs_source.as_bytes());
        let fs_hash = compute_hash(fs_source.as_bytes());
        let key = ShaderCacheKey {
            vertex_shader_hash: vs_hash,
            fragment_shader_hash: fs_hash,
        };

        let context = context.clone();
        let vs_path = vs_path.clone();
        let fs_path = fs_path.clone();

        let _key_clone = key.clone();
        let shader_load_func = async move {
            let vs_handle = {
                let context = context.clone();
                spawn_blocking(move || {
                    Self::compile_shader_module(
                        &context,
                        &vs_source,
                        &vs_path,
                        shaderc::ShaderKind::Vertex,
                    )
                })
            };

            let fs_handle = {
                let context = context.clone();
                spawn_blocking(move || {
                    Self::compile_shader_module(
                        &context,
                        &fs_source,
                        &fs_path,
                        shaderc::ShaderKind::Fragment,
                    )
                })
            };

            let value = ShaderCacheValue {
                vertex_shader: vs_handle.await??,
                fragment_shader: fs_handle.await??,
            };

            Ok(Arc::new(value))
        };
        self.shader_cache.get(key, shader_load_func).await
    }

    #[instrument(skip(context, source))]
    fn compile_shader_module(
        context: &Arc<VulkanContext>,
        source: &str,
        path: &ResourcePath,
        kind: shaderc::ShaderKind,
    ) -> Result<ShaderCompilationResult> {
        let path = path.to_string();
        let now = std::time::Instant::now();
        let compiler = shaderc::Compiler::new().context("Failed to create shader compiler")?;
        let spirv = compiler.compile_into_spirv(source, kind, &path, "main", None)?;
        let spirv_binary = spirv.as_binary_u8();
        info!(
            "compiled in {:?}, SPIRV size: {}.",
            now.elapsed(),
            spirv_binary.len()
        );

        // Extract metadata from SPIRV
        let reflect = spirv_reflect::ShaderModule::load_u8_data(spirv_binary)
            .map_err(|err| anyhow!("Failed to reflect SPIRV binary of shader '{path}': {err}"))?;
        let entry_point = reflect
            .enumerate_entry_points()
            .map_err(Error::msg)?
            .into_iter()
            .find(|ep| ep.name == "main")
            .with_context(|| format!("Failed to find entry point 'main' in '{path}'"))?;

        let module = unsafe {
            ShaderModule::from_bytes(context.vulkano_context.device().clone(), spirv_binary)
        }?;

        let descriptor_set_index = match kind {
            shaderc::ShaderKind::Vertex => 0,
            shaderc::ShaderKind::Fragment => 1,
            _ => panic!("Unsupported shader kind"),
        };

        // Find the descriptor set that belongs to the current shader stage
        let Some(descriptor_set) = entry_point
            .descriptor_sets
            .iter()
            .find(|ds| ds.set == descriptor_set_index) else {
            // The entire descriptor set is empty, so we can just use the module
            return Ok(ShaderCompilationResult {
                module,
                samplers: vec![],
                buffers: vec![],
                local_uniform_bindings: vec![],
                global_uniform_bindings: vec![],
                uniform_buffer_size: 0,
            });
        };

        // Find all samplers
        let samplers: Vec<_> = descriptor_set
            .bindings
            .iter()
            .filter(|binding| {
                binding.descriptor_type == ReflectDescriptorType::CombinedImageSampler
            })
            .map(|binding| ShaderCompilationResource {
                name: binding.name.clone(),
                binding: binding.binding,
            })
            .collect();

        // Find all buffers
        let buffers: Vec<_> = descriptor_set
            .bindings
            .iter()
            .filter(|binding| binding.descriptor_type == ReflectDescriptorType::StorageBuffer)
            .map(|binding| ShaderCompilationResource {
                name: binding.name.clone(),
                binding: binding.binding,
            })
            .collect();
        debug!(
            "Found {} samplers and {} buffers",
            samplers.len(),
            buffers.len()
        );

        // Find the uniform block that contains all local and global uniforms
        let uniform_block = &descriptor_set
            .bindings
            .iter()
            .find(|binding| binding.descriptor_type == ReflectDescriptorType::UniformBuffer);

        // Find local and global uniforms
        let (local_uniform_bindings, global_uniform_bindings, uniform_buffer_size) =
            match uniform_block {
                Some(binding) => {
                    let members = &binding.block.members;
                    let local_uniform_bindings = members
                        .iter()
                        .filter(|var| !var.name.starts_with(GLOBAL_UNIFORM_PREFIX))
                        .map(|var| {
                            let Some(type_desc) = &var.type_description else {
                                bail!(
                                    "Failed to get type description for uniform variable {}",
                                    var.name
                                );
                            };
                            ensure!(
                                (type_desc.type_flags & !ReflectTypeFlags::VECTOR)
                                    == ReflectTypeFlags::FLOAT,
                                "Uniform variable {} is not a float or vector",
                                var.name
                            );
                            Ok(ShaderCompilationLocalUniform {
                                name: var.name.clone(),
                                f32_offset: var.offset as usize / size_of::<f32>(),
                                f32_count: var.size as usize / size_of::<f32>(),
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    let global_uniform_bindings = members
                        .iter()
                        .filter(|var| var.name.starts_with(GLOBAL_UNIFORM_PREFIX))
                        .map(|var| {
                            GlobalType::from_str(&var.name[(GLOBAL_UNIFORM_PREFIX.len())..]).map(
                                |global_type| GlobalUniformMapping {
                                    global_type,
                                    f32_offset: var.offset as usize / size_of::<f32>(),
                                },
                            )
                        })
                        .collect::<::core::result::Result<Vec<_>, _>>()?;
                    let uniform_buffer_size = binding.block.size as usize;
                    (
                        local_uniform_bindings,
                        global_uniform_bindings,
                        uniform_buffer_size,
                    )
                }
                None => {
                    trace!("WARNING: No uniform block found in '{:?}'", path);
                    (vec![], vec![], 0)
                }
            };

        let result = ShaderCompilationResult {
            module,
            samplers,
            buffers,
            local_uniform_bindings,
            global_uniform_bindings,
            uniform_buffer_size,
        };

        debug!(
            "Local uniforms: {:?}",
            result
                .local_uniform_bindings
                .iter()
                .map(|u| &u.name)
                .collect::<Vec<_>>()
        );
        debug!(
            "Global uniforms: {:?}",
            result
                .global_uniform_bindings
                .iter()
                .map(|u| u.global_type)
                .collect::<Vec<_>>()
        );
        debug!(
            "Textures: {:?}",
            result.samplers.iter().map(|u| &u.name).collect::<Vec<_>>()
        );
        Ok(result)
    }

    async fn load_source(&self, path: &ResourcePath) -> Result<String> {
        let cache_entry = self.file_hash_cache.get(path).await?;
        let FileCacheEntry {
            hash: _,
            content: source,
        } = cache_entry.as_ref();
        Ok(std::str::from_utf8(source)?.to_string())
    }

    pub fn display_load_errors(&self) {
        self.shader_cache.display_load_errors();
    }

    pub fn reset_load_cycle(&self) {
        self.shader_cache.start_load_cycle();
    }
}
