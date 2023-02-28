use crate::control::controls::GlobalType;
use crate::file::file_hash_cache::{hash_content, ContentHash, FileCache, FileCacheEntry};
use crate::render::material::GlobalUniformMapping;
use crate::render::vulkan_window::VulkanContext;
use anyhow::{Context, Result};
use spirv_reflect::types::{ReflectDescriptorType, ReflectResourceType};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use vulkano::buffer::BufferContents;
use vulkano::shader::ShaderModule;

struct ShaderCacheKey {
    vertex_shader_hash: ContentHash,
    fragment_shader_hash: ContentHash,
}

#[derive(Clone)]
pub struct ShaderCacheValue {
    pub vertex_shader: Arc<ShaderModule>,
    pub fragment_shader: Arc<ShaderModule>,
}

pub struct ShaderCompilationResult {
    pub module: Arc<ShaderModule>,
    pub texture_bindings: Vec<ShaderCompilationTextureBinding>,
    pub global_uniform_bindings: Vec<GlobalUniformMapping>,
    pub local_uniform_bindings: Vec<ShaderCompilationLocalUniform>,
}

// Metadata of a texture binding extracted from the compiled shader
pub struct ShaderCompilationTextureBinding {
    pub name: String,
    pub binding: u32,
}

// Metadata of a local uniform extracted from the compiled shader
pub struct ShaderCompilationLocalUniform {
    pub name: String,
    pub offset: u32,
    pub size: u32,
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
        vs_path: &str,
        fs_path: &str,
    ) -> Result<ShaderCacheValue> {
        let header = self.load_source("app/header.glsl")?;
        let vs_source = format!("{header}\n{}", self.load_source(vs_path)?);
        let fs_source = format!("{header}\n{}", self.load_source(fs_path)?);

        let vs_hash = hash_content(vs_source.as_bytes());
        let fs_hash = hash_content(fs_source.as_bytes());
        let key = ShaderCacheKey {
            vertex_shader_hash: vs_hash,
            fragment_shader_hash: fs_hash,
        };
        if self.shader_cache.contains_key(&key) {
            Ok(self.shader_cache.get(&key).unwrap().clone())
        } else {
            let vs_module = load_shader_module(
                context,
                vs_source.as_bytes(),
                vs_path,
                shaderc::ShaderKind::Vertex,
            )?;
            let fs_module = load_shader_module(
                context,
                fs_source.as_bytes(),
                fs_path,
                shaderc::ShaderKind::Fragment,
            )?;
            let value = ShaderCacheValue {
                vertex_shader: vs_module,
                fragment_shader: fs_module,
            };
            self.shader_cache.insert(key, value.clone());
            Ok(value)
        }
    }

    fn compile_shader_module(
        context: &VulkanContext,
        source: &str,
        path: &str,
        kind: shaderc::ShaderKind,
    ) -> Result<ShaderCompilationResult> {
        let compiler = shaderc::Compiler::new().unwrap();
        let spirv = compiler.compile_into_spirv(&source, kind, path, "main", None)?;
        let spirv_binary = spirv.as_binary_u8();
        println!("Shader '{path:?}' SPIRV size: {}", spirv_binary.len());

        // Extract metadata from SPIRV
        let reflect = spirv_reflect::ShaderModule::load_u8_data(spirv_binary).unwrap();
        let entry_point = reflect
            .enumerate_entry_points()?
            .into_iter()
            .find(|ep| ep.name == "main")
            .with_context(|| format!("Failed to find entry point 'main' in '{path}'"))?;

        let descriptor_set_index = match kind {
            shaderc::ShaderKind::Vertex => 0,
            shaderc::ShaderKind::Fragment => 1,
            _ => panic!("Unsupported shader kind"),
        };

        // Find the descriptor set that belongs to the current shader stage
        let descriptor_set = entry_point
            .descriptor_sets
            .iter()
            .find(|ds| ds.set == descriptor_set_index)
            .unwrap();

        // Find all texture bindings
        let texture_bindings = descriptor_set
            .bindings
            .iter()
            .filter(|binding| {
                binding.descriptor_type == ReflectDescriptorType::CombinedImageSampler
            })
            .map(|binding| ShaderCompilationTextureBinding {
                name: binding.name.clone(),
                binding: binding.binding,
            })
            .collect();

        // Find the uniform block that contains all local and global uniforms
        let uniform_block = &descriptor_set
            .bindings
            .iter()
            .find(|binding| binding.descriptor_type == ReflectDescriptorType::UniformBuffer)
            .with_context(|| format!("Failed to find uniform buffer in '{path}'"))?
            .block
            .members;

        // Split local and global uniforms
        let local_uniform_bindings = uniform_block
            .iter()
            .filter(|var| !var.name.starts_with(GLOBAL_UNIFORM_PREFIX))
            .map(|var| ShaderCompilationLocalUniform {
                name: var.name.clone(),
                offset: var.offset,
                size: var.size,
            })
            .collect();

        let global_uniform_bindings = uniform_block
            .iter()
            .filter(|var| var.name.starts_with(GLOBAL_UNIFORM_PREFIX))
            .map(|var| GlobalUniformMapping {
                global_type: GlobalType::from_str(&var.name[(GLOBAL_UNIFORM_PREFIX.len())..])?,
                offset: var.offset,
            })
            .collect();

        println!("SPIRV Metadata: {:#?}", entry_point);

        let module =
            unsafe { ShaderModule::from_bytes(context.context.device().clone(), spirv_binary) }?;

        let result = ShaderCompilationResult {
            module,
            texture_bindings,
            local_uniform_bindings,
            global_uniform_bindings,
        };
        Ok(result)
    }

    fn load_source(&mut self, path: &str) -> Result<String> {
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

// pub fn load_shader_module(
//     context: &VulkanContext,
//     content: &[u8],
//     kind: shaderc::ShaderKind,
// ) -> Result<Arc<ShaderModule>> {
//     let source = std::str::from_utf8(content)?;
//     let header = std::fs::read_to_string("app/header.glsl")?;
//     let combined = format!("{header}\n{source}");
//
//     let compiler = shaderc::Compiler::new().unwrap();
//
//     // let input_file_name = path.to_str().ok_or(anyhow!("Invalid file name"))?;
//     let spirv = compiler.compile_into_spirv(&combined, kind, "input_file_name", "main", None)?;
//     let spirv_binary = spirv.as_binary_u8();
//
//     let reflect = spirv_reflect::ShaderModule::load_u8_data(spirv_binary).unwrap();
//     let ep = &reflect.enumerate_entry_points().unwrap()[0];
//     println!("SPIRV Metadata: {:#?}", ep);
//
//     // println!("Shader '{path:?}' SPIRV size: {}", spirv_binary.len());
//
//     let module =
//         unsafe { ShaderModule::from_bytes(context.context.device().clone(), spirv_binary) };
//
//     Ok(module?)
// }
