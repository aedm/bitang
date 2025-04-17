use std::sync::{Arc, OnceLock};

use anyhow::Result;
use smallvec::SmallVec;

use super::image::BitangImage;

static MIPMAP_GENERATOR: OnceLock<MipmapGeneratorCore> = OnceLock::new();

pub struct MipmapGenerator {
    image: Arc<BitangImage>,
    pipeline: wgpu::RenderPipeline,
}

impl MipmapGenerator {
    pub fn new(device: &wgpu::Device, image: Arc<BitangImage>) -> Self {
        let mipmap_generator = MIPMAP_GENERATOR.get_or_init(|| MipmapGeneratorCore::new(device));

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &mipmap_generator.shader_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &mipmap_generator.shader_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(image.pixel_format.wgpu_format().into())],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self { image, pipeline }
    }

    pub fn generate(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
    ) -> Result<()> {
        let mipmap_generator = MIPMAP_GENERATOR.get_or_init(|| MipmapGeneratorCore::new(device));

        let mip_count = self.image.mip_levels()?;
        let views = (0..mip_count)
            .map(|mip| self.image.view_mip_level(mip))
            .collect::<Result<SmallVec<[_; 32]>>>()?;

        let bind_group_layout = self.pipeline.get_bind_group_layout(0);

        for target_mip in 1..mip_count as usize {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&views[target_mip - 1]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&mipmap_generator.sampler),
                    },
                ],
                label: None,
            });

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &views[target_mip],
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        Ok(())
    }
}

struct MipmapGeneratorCore {
    shader_module: wgpu::ShaderModule,
    sampler: wgpu::Sampler,
}

impl MipmapGeneratorCore {
    fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("blit.wgsl"));

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mip"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            shader_module: shader,
            sampler,
        }
    }
}
