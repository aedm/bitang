use crate::control::controls::GlobalType;
use crate::file::file_hash_cache::{hash_content, ContentHash, FileCache, FileCacheEntry};
use crate::file::ResourcePath;
use crate::render::vulkan_window::VulkanContext;
use anyhow::{anyhow, Context, Error, Result};
use spirv_reflect::types::ReflectDescriptorType;
use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::size_of;
use std::rc::Rc;
use std::sync::Arc;
use tracing::{debug, info, instrument, trace};
use vulkano::shader::ShaderModule;
use crate::render::shader::GlobalUniformMapping;

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
    file_hash_cache: Rc<RefCell<FileCache>>,
    shader_cache: HashMap<ShaderCacheKey, ShaderCacheValue>,
}

const GLOBAL_UNIFORM_PREFIX: &str = "g_";

impl ShaderCache {
    pub fn new(file_hash_cache: &Rc<RefCell<FileCache>>) -> Self {
        Self {
            file_hash_cache: file_hash_cache.clone(),
            shader_cache: HashMap::new(),
        }
    }

    pub fn get_or_load(
        &mut self,
        context: &VulkanContext,
        vs_path: &ResourcePath,
        fs_path: &ResourcePath,
        common_path: &ResourcePath,
    ) -> Result<&ShaderCacheValue> {
        let header = self.load_source(common_path)?;
        let vs_source = format!("{header}\n{}", self.load_source(vs_path)?);
        let fs_source = format!("{header}\n{}", self.load_source(fs_path)?);

        let vs_hash = hash_content(vs_source.as_bytes());
        let fs_hash = hash_content(fs_source.as_bytes());
        let key = ShaderCacheKey {
            vertex_shader_hash: vs_hash,
            fragment_shader_hash: fs_hash,
        };
        if !self.shader_cache.contains_key(&key) {
            let vs_result = Self::compile_shader_module(
                context,
                &vs_source,
                vs_path,
                shaderc::ShaderKind::Vertex,
            )?;
            let fs_result = Self::compile_shader_module(
                context,
                &fs_source,
                fs_path,
                shaderc::ShaderKind::Fragment,
            )?;
            let value = ShaderCacheValue {
                vertex_shader: vs_result,
                fragment_shader: fs_result,
            };
            self.shader_cache.insert(key.clone(), value);
        }
        // Unwrap is safe: we just inserted the shader
        Ok(self.shader_cache.get(&key).unwrap())
    }

    #[instrument(skip(context, source))]
    fn compile_shader_module(
        context: &VulkanContext,
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

        let module =
            unsafe { ShaderModule::from_bytes(context.context.device().clone(), spirv_binary) }?;

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
                        .map(|var| ShaderCompilationLocalUniform {
                            name: var.name.clone(),
                            // TODO: assert that type is FLOAT
                            f32_offset: var.offset as usize / size_of::<f32>(),
                            f32_count: var.size as usize / size_of::<f32>(),
                        })
                        .collect();
                    let global_uniform_bindings = members
                        .iter()
                        .filter(|var| var.name.starts_with(GLOBAL_UNIFORM_PREFIX))
                        .map(|var| {
                            GlobalType::from_str(&var.name[(GLOBAL_UNIFORM_PREFIX.len())..]).map(
                                |global_type| GlobalUniformMapping {
                                    global_type,
                                    offset: var.offset as usize,
                                },
                            )
                        })
                        .collect::<Result<Vec<_>>>()?;
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

    fn load_source(&mut self, path: &ResourcePath) -> Result<String> {
        let mut file_cache = self.file_hash_cache.borrow_mut();
        let FileCacheEntry {
            hash: _,
            content: vs_source,
        } = file_cache.get(path, true)?;
        Ok(
            std::str::from_utf8(&vs_source.context("Failed to read vertex shader source")?)?
                .to_string(),
        )
    }
}
