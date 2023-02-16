use crate::render::shader_context::ContextUniforms;
use crate::render::vulkan_window::VulkanContext;
use crate::render::Texture;
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use std::sync::Arc;
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer, CpuBufferPool};
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::image::view::ImageView;
use vulkano::image::ImmutableImage;
use vulkano::memory::allocator::MemoryUsage;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::sampler::Sampler;
use vulkano::shader::ShaderModule;

type UniformBufferPool = CpuBufferPool<ContextUniforms>;

pub enum DescriptorSetIds {
    Vertex = 0,
    Fragment = 1,
}

pub struct Shader {
    pub shader_module: Arc<ShaderModule>,
    pub texture_bindings: Vec<TextureBinding>,
}

struct TextureBinding {
    texture: Arc<Texture>,
    sampler: Arc<Sampler>,
}

impl Shader {
    pub fn new(
        context: &VulkanContext,
        shader_module: &Arc<ShaderModule>,
        texture_bindings: Vec<TextureBinding>,
        // layout: &Arc<DescriptorSetLayout>,
    ) -> Self {
        let uniform_buffer_pool = UniformBufferPool::new(
            context.context.memory_allocator().clone(),
            BufferUsage {
                uniform_buffer: true,
                ..BufferUsage::empty()
            },
            MemoryUsage::Upload,
        );

        Self {
            shader_module: shader_module.clone(),
            texture_bindings,
        }
    }

    pub fn make_descriptor_set(
        &self,
        context: &VulkanContext,
        uniform_values: &ContextUniforms,
    ) -> Arc<PersistentDescriptorSet> {
        let uniform_buffer_subbuffer = self.uniform_buffer_pool.from_data(*uniform_values).unwrap();

        let mut descriptors = vec![WriteDescriptorSet::buffer(0, uniform_buffer_subbuffer)];
        descriptors.extend(
            self.texture_bindings
                .iter()
                .enumerate()
                .map(|(i, texture_binding)| {
                    WriteDescriptorSet::image_view_sampler(
                        i as u32 + 1,
                        texture_binding.texture.clone(),
                        texture_binding.sampler.clone(),
                    )
                }),
        );

        PersistentDescriptorSet::new(
            &context.descriptor_set_allocator,
            self.layout.clone(),
            descriptors,
        )
        .unwrap()
    }
}