use crate::control::controls::ControlSetBuilder;
use crate::control::ControlId;
use crate::loader::async_cache::LoadFuture;
use crate::loader::resource_repository::ResourceRepository;
use crate::render;
use crate::render::vulkan_window::VulkanContext;
use ahash::AHashMap;
use std::sync::Arc;

pub mod chart_file;
mod material;
pub mod project_file;

// Helper function to initialize a bool using serde
fn default_true() -> bool {
    true
}
