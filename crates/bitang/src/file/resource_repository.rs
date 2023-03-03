use crate::control::controls::Controls;
use crate::file::binary_file_cache::BinaryFileCache;
use crate::file::blend_loader::load_blend_buffer;
use crate::file::file_hash_cache::FileCache;
use crate::file::shader_loader::{ShaderCache, ShaderCompilationResult};
use crate::render::material::{
    LocalUniformMapping, Material, MaterialStep, Shader, TextureBinding,
};
use crate::render::mesh::Mesh;
use crate::render::vulkan_window::VulkanContext;
use crate::render::{RenderObject, Texture, Vertex3};
use anyhow::{anyhow, Result};

use serde::Deserialize;
use serde::Serialize;
use std::array;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;
use std::rc::Rc;
use std::sync::Arc;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract,
};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{ImageDimensions, ImmutableImage, MipmapsCount};

#[derive(Debug, Deserialize, Serialize)]
pub struct RonObject {
    id: String,
    mesh_path: String,
    texture_path: String,
    // mesh_selector: String,
    vertex_shader: String,
    fragment_shader: String,
    depth_test: bool,
    depth_write: bool,
    uniforms: HashMap<String, Vec<f32>>,
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
            shader_cache: ShaderCache::new(&file_hash_cache),
            root_ron_file_cache: BinaryFileCache::new(&file_hash_cache, load_ron_file),
            file_hash_cache,
            cached_root: None,
            controls: Controls::new(),
        })
    }

    pub fn load_root_document(
        &mut self,
        context: &VulkanContext,
        controls: &mut Controls,
    ) -> Result<Arc<RenderObject>> {
        let has_file_changes = self.file_hash_cache.borrow_mut().handle_file_changes();
        match (has_file_changes, &self.cached_root) {
            (false, Some(cached_root)) => Ok(cached_root.clone()),
            _ => {
                controls.start_load_cycle();
                let result = self
                    .load_render_object(context, controls)
                    .and_then(|render_object| {
                        let render_object = Arc::new(render_object);
                        self.cached_root = Some(render_object.clone());
                        Ok(render_object)
                    });
                controls.finish_load_cycle();
                self.file_hash_cache.borrow_mut().update_watchers()?;
                result
            }
        }
    }

    pub fn get_texture(&mut self, context: &VulkanContext, path: &str) -> Result<&Arc<Texture>> {
        self.texture_cache.get_or_load(context, &path)
    }

    pub fn get_mesh(&mut self, context: &VulkanContext, path: &str) -> Result<&Arc<Mesh>> {
        self.mesh_cache.get_or_load(context, &path)
    }

    pub fn load_render_object(
        &mut self,
        context: &VulkanContext,
        controls: &mut Controls,
    ) -> Result<RenderObject> {
        let object = self
            .root_ron_file_cache
            .get_or_load(context, "app/demo.ron")?
            .clone();

        // Set uniforms
        for (uniform_name, uniform_value) in &object.uniforms {
            if uniform_value.is_empty() {
                return Err(anyhow!(
                    "Uniform '{}' has no values. Object id '{}'",
                    uniform_name,
                    object.id
                ));
            }
            let _control_id = Self::make_control_id_for_object(&object.id, uniform_name);
            let _value: [f32; 4] = array::from_fn(|i| uniform_value[i % uniform_value.len()]);
            // controls.get_control(&control_id).set_scalar(value);
        }

        let mesh = self.get_mesh(context, &object.mesh_path)?.clone();
        let texture = self.get_texture(context, &object.texture_path)?.clone();
        let solid_step = self.make_material_step(context, controls, &object, &texture)?;
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

    fn make_material_step(
        &mut self,
        context: &VulkanContext,
        controls: &mut Controls,
        object: &Arc<RonObject>,
        texture: &Arc<Texture>,
    ) -> Result<MaterialStep> {
        let shaders = self.shader_cache.get_or_load(
            context,
            &object.vertex_shader,
            &object.fragment_shader,
        )?;

        let vertex_shader = Self::make_shader(controls, &object, &shaders.vertex_shader, &texture);
        let fragment_shader =
            Self::make_shader(controls, &object, &shaders.fragment_shader, &texture);

        let material_step = MaterialStep {
            vertex_shader,
            fragment_shader,
            depth_test: object.depth_test,
            depth_write: object.depth_write,
        };
        Ok(material_step)
    }

    fn make_control_id_for_object(object_id: &str, uniform_name: &str) -> String {
        format!("ob/{}/{}", object_id, uniform_name)
    }

    fn make_shader(
        controls: &mut Controls,
        object: &RonObject,
        compilation_result: &ShaderCompilationResult,
        texture: &Arc<Texture>,
    ) -> Shader {
        let local_mapping = compilation_result
            .local_uniform_bindings
            .iter()
            .map(|binding| {
                let control_id = Self::make_control_id_for_object(&object.id, &binding.name);
                let control = controls.get_control(&control_id);
                LocalUniformMapping {
                    control,
                    f32_count: binding.f32_count,
                    f32_offset: binding.f32_offset,
                }
            })
            .collect::<Vec<_>>();

        // Bind all samplers to the same texture for now
        let texture_bindings = compilation_result
            .texture_bindings
            .iter()
            .map(|binding| TextureBinding {
                texture: texture.clone(),
                descriptor_set_binding: binding.binding,
            })
            .collect::<Vec<_>>();

        Shader {
            shader_module: compilation_result.module.clone(),
            texture_bindings,
            local_uniform_bindings: local_mapping,
            global_uniform_bindings: compilation_result.global_uniform_bindings.clone(),
            uniform_buffer_size: compilation_result.uniform_buffer_size,
        }
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
