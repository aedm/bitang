use super::Vertex3;
use crate::tool::GpuContext;
use anyhow::Result;
use wgpu::util::DeviceExt;

use super::MeshIndex;

#[derive(Clone)]
pub struct Mesh {
    pub vertex_buffer: wgpu::Buffer,
    pub vertex_count: u32,
    pub index_buffer: Option<wgpu::Buffer>,
    pub index_count: u32,
}

impl Mesh {
    pub fn try_new(
        context: &GpuContext,
        vertices: Vec<Vertex3>,
        indices: Option<Vec<MeshIndex>>,
    ) -> Result<Mesh> {
        let vertex_buffer = context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let (index_buffer, index_count) = if let Some(indices) = &indices {
            let buffer = context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            (Some(buffer), indices.len() as u32)
        } else {
            (None, 0)
        };

        Ok(Mesh {
            vertex_buffer,
            vertex_count: vertices.len() as u32,
            index_buffer,
            index_count,
        })
    }
}
