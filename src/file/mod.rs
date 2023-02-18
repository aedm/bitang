pub mod blend_loader;

use crate::render::shader::ShaderKind;
use crate::render::vulkan_window::VulkanContext;
use anyhow::anyhow;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use vulkano::shader::ShaderModule;

pub type ContentHash = String;

pub struct ResourceCache {
    vertex_shader_cache: ShaderModuleCache,
    fragment_shader_cache: ShaderModuleCache,
}

impl ResourceCache {
    pub fn new() -> ResourceCache {
        ResourceCache {
            vertex_shader_cache: ShaderModuleCache::new(ShaderKind::Vertex),
            fragment_shader_cache: ShaderModuleCache::new(ShaderKind::Fragment),
        }
    }

    pub fn get_vertex_shader(
        &mut self,
        context: &VulkanContext,
        path: &str,
    ) -> Result<Arc<ShaderModule>> {
        self.vertex_shader_cache.get_shader_module(context, path)
    }

    pub fn get_fragment_shader(
        &mut self,
        context: &VulkanContext,
        path: &str,
    ) -> Result<Arc<ShaderModule>> {
        self.fragment_shader_cache.get_shader_module(context, path)
    }
}

pub struct ShaderModuleCache {
    cache: HashMap<String, Arc<ShaderModule>>,
    shader_kind: ShaderKind,
}

impl ShaderModuleCache {
    pub fn new(shader_kind: ShaderKind) -> ShaderModuleCache {
        ShaderModuleCache {
            cache: HashMap::new(),
            shader_kind,
        }
    }

    pub fn get_shader_module(
        &mut self,
        context: &VulkanContext,
        path: &str,
    ) -> Result<Arc<ShaderModule>> {
        if let Some(shader_module) = self.cache.get(path) {
            return Ok(shader_module.clone());
        }
        let result = self.load_shader_module(context, path);
        if let Ok(module) = &result {
            self.cache.insert(path.to_string(), module.clone());
        }
        result
    }

    fn load_shader_module(
        &self,
        context: &VulkanContext,
        file_name: &str,
    ) -> Result<Arc<ShaderModule>> {
        let kind = match self.shader_kind {
            ShaderKind::Vertex => shaderc::ShaderKind::Vertex,
            ShaderKind::Fragment => shaderc::ShaderKind::Fragment,
        };
        let source = std::fs::read_to_string(file_name)?;
        let header = std::fs::read_to_string("app/header.glsl")?;
        let combined = format!("{header}\n{source}");

        let compiler = shaderc::Compiler::new().unwrap();

        let spirv = compiler.compile_into_spirv(&combined, kind, file_name, "main", None)?;
        let spirv_binary = spirv.as_binary_u8();

        // let reflect = spirv_reflect::ShaderModule::load_u8_data(spirv_binary).unwrap();
        // let _ep = &reflect.enumerate_entry_points().unwrap()[0];
        // println!("SPIRV Metadata: {:#?}", ep);

        Ok(unsafe { ShaderModule::from_bytes(context.context.device().clone(), spirv_binary) }?)
    }

    pub fn invalidate(&mut self, path: &str) {
        self.cache.remove(path);
        todo!()
    }
}
