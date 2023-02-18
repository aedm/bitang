pub mod blend_loader;

use anyhow::anyhow;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use vulkano::shader::ShaderModule;

pub type ContentHash = String;

pub enum CachedResource {
    ShaderModule(Arc<ShaderModule>),
}

//
// pub struct ResourceId {
//     id: usize,
//     resource_type: Resource,
// }
//
// struct ResolvableResource<T> {}

// struct ResourceCacheKey {
//     file_content_hash: ContentHash,
//     dependency_resource_ids: Vec<ResourceId>,
// }

pub struct ResourceCache {
    // cache: HashMap<ResourceCacheKey, Arc<V>>,
    cache: HashMap<String, CachedResource>,
}

impl ResourceCache {
    pub fn new() -> ResourceCache {
        ResourceCache {
            cache: HashMap::new(),
        }
    }

    pub fn get_shader_module(&mut self, path: &str) -> Result<Arc<ShaderModule>> {
        if let Some(cached) = self.cache.get(path) {
            if let CachedResource::ShaderModule(shader_module) = cached {
                return Ok(shader_module.clone());
            }
            return Err(anyhow!("Cached resource '{path}' is not a shader module."));
        }
        Ok(Self::load_shader_module(path))
    }

    fn load_shader_module(path: &str) -> Arc<ShaderModule> {
        todo!()
    }

    pub fn invalidate(&mut self, path: &str) {
        self.cache.remove(path);
        todo!()
    }
}

type ShaderModuleCache = ResourceCache<Arc<ShaderModule>>;
