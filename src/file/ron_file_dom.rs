use crate::file::blend_loader::load_blend_file;
use crate::file::resource_cache::ResourceCache;
use crate::render::material::{Material, MaterialStep};
use crate::render::mesh::Mesh;
use crate::render::render_unit::RenderUnit;
use crate::render::shader::Shader;
use crate::render::vulkan_window::VulkanContext;
use crate::render::{RenderObject, Texture, Vertex3};
use anyhow::Result;
use image::io::Reader as ImageReader;
use serde::Deserialize;
use std::sync::Arc;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract,
};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{ImageDimensions, ImmutableImage, MipmapsCount};

#[derive(Debug, Deserialize)]
struct Object {
    mesh_path: String,
    texture_path: String,
    // mesh_selector: String,
    vertex_shader: String,
    fragment_shader: String,
    depth_test: bool,
    depth_write: bool,
}

pub fn load_render_object(
    cache: &mut ResourceCache,
    context: &VulkanContext,
) -> Result<RenderObject> {
    let source = std::fs::read_to_string("app/demo.ron")?;
    let object = ron::from_str::<Object>(&source)?;

    let mesh = load_mesh(context, &object.mesh_path)?;
    let texture = load_texture(context, &object.texture_path)?;

    let vs = cache.get_vertex_shader(context, &object.vertex_shader)?;
    let fs = cache.get_fragment_shader(context, &object.fragment_shader)?;

    let vertex_shader = Shader {
        shader_module: vs,
        textures: vec![],
    };

    let fragment_shader = Shader {
        shader_module: fs,
        textures: vec![texture],
    };

    let solid_step = MaterialStep {
        vertex_shader,
        fragment_shader,
        depth_test: true,
        depth_write: true,
    };

    let material = Material {
        passes: [None, None, Some(solid_step)],
    };

    let render_object = RenderObject {
        mesh,
        material,
        position: Default::default(),
        rotation: Default::default(),
    };

    Ok(render_object)
}

fn load_mesh(context: &VulkanContext, path: &str) -> Result<Mesh> {
    let blend_file = load_blend_file(path)?;
    let vertices = blend_file
        .mesh
        .faces
        .iter()
        .flatten()
        .map(|v| Vertex3 {
            a_position: [v.0[0], v.0[1], v.0[2]],
            a_normal: [v.1[0], v.1[1], v.1[2]],
            a_tangent: [0.0, 0.0, 0.0],
            a_uv: [v.2[0], v.2[1]],
            a_padding: 0.0,
        })
        .collect::<Vec<Vertex3>>();
    Mesh::try_new(context, vertices)
}

fn load_texture(context: &VulkanContext, path: &str) -> Result<Arc<Texture>> {
    let rgba = ImageReader::open(path)?.decode()?.to_rgba8();
    let dimensions = ImageDimensions::Dim2d {
        width: rgba.dimensions().0,
        height: rgba.dimensions().0,
        array_layers: 1,
    };

    let mut cbb = AutoCommandBufferBuilder::primary(
        &context.command_buffer_allocator,
        context.context.graphics_queue().queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )?;

    let image = ImmutableImage::from_iter(
        context.context.memory_allocator(),
        rgba.into_raw(),
        dimensions,
        MipmapsCount::One,
        Format::R8G8B8A8_SRGB,
        &mut cbb,
    )?;
    let _fut = cbb
        .build()
        .unwrap()
        .execute(context.context.graphics_queue().clone())
        .unwrap();

    Ok(ImageView::new_default(image)?)
}
