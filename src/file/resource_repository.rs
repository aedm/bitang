use crate::control::controls::{Controls, GlobalType};
use crate::file::binary_file_cache::BinaryFileCache;
use crate::file::blend_loader::load_blend_buffer;
use crate::file::file_hash_cache::FileCache;
use crate::file::shader_loader::ShaderCache;
use crate::render::material::{
    GlobalUniformMapping, Material, MaterialStep, Shader, TextureBinding,
};
use crate::render::mesh::Mesh;
use crate::render::vulkan_window::VulkanContext;
use crate::render::{RenderObject, Texture, Vertex3};
use anyhow::Result;
use serde::Deserialize;
use std::cell::RefCell;
use std::env;
use std::io::Cursor;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract,
};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{ImageDimensions, ImmutableImage, MipmapsCount};
use vulkano::shader::ShaderModule;

#[derive(Debug, Deserialize)]
pub struct RonObject {
    mesh_path: String,
    texture_path: String,
    // mesh_selector: String,
    vertex_shader: String,
    fragment_shader: String,
    depth_test: bool,
    depth_write: bool,
}

pub struct ResourceRepository {
    file_hash_cache: Rc<RefCell<FileCache>>,

    texture_cache: BinaryFileCache<Arc<Texture>>,
    mesh_cache: BinaryFileCache<Arc<Mesh>>,
    root_ron_file_cache: BinaryFileCache<Arc<RonObject>>,

    shader_cache: ShaderCache,
    // vertex_shader_cache: BinaryFileCache<Arc<ShaderModule>>,
    // fragment_shader_cache: BinaryFileCache<Arc<ShaderModule>>,
    pub controls: Controls,

    cached_root: Option<Arc<RenderObject>>,
}

impl ResourceRepository {
    pub fn try_new() -> Result<Self> {
        let file_hash_cache = Rc::new(RefCell::new(FileCache::new()?));

        Ok(Self {
            texture_cache: BinaryFileCache::new(&file_hash_cache, load_texture),
            mesh_cache: BinaryFileCache::new(&file_hash_cache, load_mesh),
            // vertex_shader_cache: BinaryFileCache::new(&file_hash_cache, |context, content| {
            //     load_shader_module(context, content, shaderc::ShaderKind::Vertex)
            // }),
            // fragment_shader_cache: BinaryFileCache::new(&file_hash_cache, |context, content| {
            //     load_shader_module(context, content, shaderc::ShaderKind::Fragment)
            // }),
            shader_cache: ShaderCache::new(&file_hash_cache),
            root_ron_file_cache: BinaryFileCache::new(&file_hash_cache, load_ron_file),
            file_hash_cache,
            cached_root: None,
            controls: Controls::new(),
        })
    }

    pub fn load_root_document(&mut self, context: &VulkanContext) -> Result<Arc<RenderObject>> {
        let has_changes = self.file_hash_cache.borrow_mut().start_load_cycle();
        if !has_changes {
            if let Some(cached_root) = &self.cached_root {
                return Ok(cached_root.clone());
            }
        }
        let result = Arc::new(self.load_render_object(context)?);
        self.cached_root = Some(result.clone());
        self.file_hash_cache.borrow_mut().end_load_cycle()?;
        Ok(result)
    }

    pub fn get_texture(&mut self, context: &VulkanContext, path: &str) -> Result<&Arc<Texture>> {
        self.texture_cache.get_or_load(context, &path)
    }

    pub fn get_mesh(&mut self, context: &VulkanContext, path: &str) -> Result<&Arc<Mesh>> {
        self.mesh_cache.get_or_load(context, &path)
    }

    // pub fn get_vertex_shader(
    //     &mut self,
    //     context: &VulkanContext,
    //     path: &str,
    // ) -> Result<&Arc<ShaderModule>> {
    //     // self.vertex_shader_cache.get_or_load(context, &path)
    // }
    //
    // pub fn get_fragment_shader(
    //     &mut self,
    //     context: &VulkanContext,
    //     path: &str,
    // ) -> Result<&Arc<ShaderModule>> {
    //     self.fragment_shader_cache.get_or_load(context, &path)
    // }

    pub fn load_render_object(&mut self, context: &VulkanContext) -> Result<RenderObject> {
        // let source = std::fs::read_to_string("app/demo.ron")?;
        // let object = ron::from_str::<RonObject>(&source)?;
        let object = self
            .root_ron_file_cache
            .get_or_load(context, "app/demo.ron")?
            .clone();

        let mesh = self.get_mesh(context, &object.mesh_path)?.clone();
        let texture = self.get_texture(context, &object.texture_path)?.clone();

        // let vs = self
        //     .get_vertex_shader(context, &object.vertex_shader)?
        //     .clone();
        // let fs = self
        //     .get_fragment_shader(context, &object.fragment_shader)?
        //     .clone();
        let shader_modules = self.shader_cache.get_or_load(
            context,
            &object.vertex_shader,
            &object.fragment_shader,
        )?;

        let vertex_shader = Shader {
            shader_module: shader_modules.vertex_shader,
            texture_bindings: vec![],
            local_uniform_bindings: vec![],
            global_uniform_bindings: vec![
                GlobalUniformMapping {
                    global_type: GlobalType::ModelToProjection,
                    offset: 0,
                },
                GlobalUniformMapping {
                    global_type: GlobalType::ModelToCamera,
                    offset: 16,
                },
            ],
        };

        let fragment_shader = Shader {
            shader_module: shader_modules.fragment_shader,
            texture_bindings: vec![TextureBinding {
                texture,
                descriptor_set_binding: 1,
            }],
            local_uniform_bindings: vec![],
            global_uniform_bindings: vec![
                GlobalUniformMapping {
                    global_type: GlobalType::ModelToProjection,
                    offset: 0,
                },
                GlobalUniformMapping {
                    global_type: GlobalType::ModelToCamera,
                    offset: 16,
                },
            ],
        };

        let solid_step = MaterialStep {
            vertex_shader,
            fragment_shader,
            depth_test: object.depth_test,
            depth_write: object.depth_write,
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
}

fn load_mesh(context: &VulkanContext, content: &[u8]) -> Result<Arc<Mesh>> {
    let blend_file = load_blend_buffer(content)?;
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
    Ok(Arc::new(Mesh::try_new(context, vertices)?))
}

fn load_texture(context: &VulkanContext, content: &[u8]) -> Result<Arc<Texture>> {
    let rgba = image::io::Reader::new(Cursor::new(content))
        .with_guessed_format()?
        .decode()?
        .to_rgba8();
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

pub fn load_ron_file(_context: &VulkanContext, content: &[u8]) -> Result<Arc<RonObject>> {
    let object = ron::from_str::<RonObject>(std::str::from_utf8(content)?)?;
    Ok(Arc::new(object))
}
