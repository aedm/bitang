use crate::render::shader::ShaderKind;
use crate::render::vulkan_window::VulkanContext;
use anyhow::anyhow;
use anyhow::Result;
use notify::{RecommendedWatcher, Watcher};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use vulkano::shader::ShaderModule;

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
    cache: HashMap<PathBuf, Arc<ShaderModule>>,
    shader_kind: ShaderKind,

    watcher: RecommendedWatcher,
    receiver: Receiver<Result<notify::Event, notify::Error>>,
}

fn to_absolute_path(path: &str) -> Result<PathBuf> {
    let path2 = std::path::Path::new(path);
    if path2.is_absolute() {
        Ok(path2.to_path_buf())
    } else {
        Ok(env::current_dir()?.join(path2))
    }
}

impl ShaderModuleCache {
    pub fn new(shader_kind: ShaderKind) -> ShaderModuleCache {
        let (tx, receiver) = std::sync::mpsc::channel();
        let watcher = notify::recommended_watcher(tx).unwrap();
        ShaderModuleCache {
            cache: HashMap::new(),
            shader_kind,
            watcher,
            receiver,
        }
    }

    pub fn get_shader_module(
        &mut self,
        context: &VulkanContext,
        path: &str,
    ) -> Result<Arc<ShaderModule>> {
        let path = to_absolute_path(path)?;
        for res in self.receiver.try_iter() {
            match res {
                Ok(event) => {
                    for path in event.paths {
                        println!("Removing file: {:?}", path);
                        self.cache.remove(&path);
                    }
                }
                Err(e) => println!("watch error: {:?}", e),
            }
        }

        if let Some(shader_module) = self.cache.get(&path) {
            return Ok(shader_module.clone());
        }
        let result = self.load_shader_module(context, &path);
        if let Ok(module) = &result {
            println!("Watching file: {:?}", path);
            if self
                .watcher
                .watch(&path, notify::RecursiveMode::NonRecursive)
                .is_err()
            {
                println!("Failed to watch file: {:?}", path);
            };
            self.cache.insert(path, module.clone());
        }
        result
    }

    fn load_shader_module(
        &self,
        context: &VulkanContext,
        path: &PathBuf,
    ) -> Result<Arc<ShaderModule>> {
        let kind = match self.shader_kind {
            ShaderKind::Vertex => shaderc::ShaderKind::Vertex,
            ShaderKind::Fragment => shaderc::ShaderKind::Fragment,
        };
        let source = std::fs::read_to_string(path)?;
        let header = std::fs::read_to_string("app/header.glsl")?;
        let combined = format!("{header}\n{source}");

        let compiler = shaderc::Compiler::new().unwrap();

        let input_file_name = path.to_str().ok_or(anyhow!("Invalid file name"))?;
        let spirv = compiler.compile_into_spirv(&combined, kind, input_file_name, "main", None)?;
        let spirv_binary = spirv.as_binary_u8();

        // let reflect = spirv_reflect::ShaderModule::load_u8_data(spirv_binary).unwrap();
        // let _ep = &reflect.enumerate_entry_points().unwrap()[0];
        // println!("SPIRV Metadata: {:#?}", ep);

        println!("Shader '{path:?}' SPIRV size: {}", spirv_binary.len());

        let module =
            unsafe { ShaderModule::from_bytes(context.context.device().clone(), spirv_binary) };

        Ok(module?)
    }
}
