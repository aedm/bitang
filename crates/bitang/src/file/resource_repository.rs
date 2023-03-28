use crate::control::controls::ControlsAndGlobals;
use crate::file::binary_file_cache::BinaryFileCache;
use crate::file::file_hash_cache::FileCache;
use crate::file::shader_loader::{ShaderCache, ShaderCompilationResult};
use crate::render::material::{
    LocalUniformMapping, Material, MaterialStep, SamplerBinding, Shader,
};
use crate::render::mesh::Mesh;
use crate::render::vulkan_window::VulkanContext;
use crate::render::{RenderObject, Texture, Vertex3};
use anyhow::{anyhow, Result};

use crate::file::chart_file;
use crate::render::chart::Chart;
use bitang_utils::blend_loader::load_blend_buffer;
use serde::Deserialize;
use serde::Serialize;
use std::array;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, instrument};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract,
};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{ImageDimensions, ImmutableImage, MipmapsCount};

pub struct ResourceRepository {
    file_hash_cache: Rc<RefCell<FileCache>>,

    texture_cache: BinaryFileCache<Arc<Texture>>,
    mesh_cache: BinaryFileCache<Arc<Mesh>>,
    root_ron_file_cache: BinaryFileCache<Arc<chart_file::Chart>>,

    pub shader_cache: ShaderCache,
    // vertex_shader_cache: BinaryFileCache<Arc<ShaderModule>>,
    // fragment_shader_cache: BinaryFileCache<Arc<ShaderModule>>,
    pub controls: ControlsAndGlobals,

    cached_root: Option<Arc<RenderObject>>,
}

impl ResourceRepository {
    pub fn try_new() -> Result<Self> {
        let file_hash_cache = Rc::new(RefCell::new(FileCache::new()?));

        Ok(Self {
            texture_cache: BinaryFileCache::new(&file_hash_cache, load_texture),
            mesh_cache: BinaryFileCache::new(&file_hash_cache, load_mesh),
            shader_cache: ShaderCache::new(&file_hash_cache),
            root_ron_file_cache: BinaryFileCache::new(&file_hash_cache, load_chart_file),
            file_hash_cache,
            cached_root: None,
            controls: ControlsAndGlobals::new(),
        })
    }

    // #[instrument(skip(self, context))]
    #[instrument(skip_all, name = "load")]
    pub fn load_root_document(
        &mut self,
        context: &VulkanContext,
        controls: &mut ControlsAndGlobals,
    ) -> Result<Arc<RenderObject>> {
        let has_file_changes = self.file_hash_cache.borrow_mut().handle_file_changes();
        match (has_file_changes, &self.cached_root) {
            (false, Some(cached_root)) => Ok(cached_root.clone()),
            _ => {
                let now = std::time::Instant::now();
                controls.start_load_cycle();
                let result = self
                    .load_root_chart(context, controls)
                    .and_then(|render_object| {
                        let render_object = Arc::new(render_object);
                        self.cached_root = Some(render_object.clone());
                        Ok(render_object)
                    });
                controls.finish_load_cycle();
                self.file_hash_cache.borrow_mut().update_watchers()?;
                info!("Loading took {:?}", now.elapsed());
                result
            }
        }
    }

    #[instrument(skip(self, context))]
    pub fn get_texture(&mut self, context: &VulkanContext, path: &str) -> Result<&Arc<Texture>> {
        self.texture_cache.get_or_load(context, &path)
    }

    #[instrument(skip(self, context))]
    pub fn get_mesh(&mut self, context: &VulkanContext, path: &str) -> Result<&Arc<Mesh>> {
        self.mesh_cache.get_or_load(context, &path)
    }

    pub fn load_root_chart(
        &mut self,
        context: &VulkanContext,
        controls: &mut ControlsAndGlobals,
    ) -> Result<Chart> {
        let chart = self
            .root_ron_file_cache
            .get_or_load(context, "test-chart/chart.ron")?
            .clone();
        let c2 = chart.load(context, self, controls);
    }

    // pub fn load_root_chart(
    //     &mut self,
    //     context: &VulkanContext,
    //     controls: &mut ControlsAndGlobals,
    // ) -> Result<RenderObject> {
    //     let chart = self
    //         .root_ron_file_cache
    //         .get_or_load(context, "test-chart/chart.ron")?
    //         .clone();
    //
    //     // Set uniforms
    //     for (uniform_name, uniform_value) in &object.uniforms {
    //         if uniform_value.is_empty() {
    //             return Err(anyhow!(
    //                 "Uniform '{}' has no values. Object id '{}'",
    //                 uniform_name,
    //                 object.id
    //             ));
    //         }
    //         let _control_id = Self::make_control_id_for_object(&object.id, uniform_name);
    //         let _value: [f32; 4] = array::from_fn(|i| uniform_value[i % uniform_value.len()]);
    //         // controls.get_control(&control_id).set_scalar(value);
    //     }
    //
    //     let mesh = self.get_mesh(context, &object.mesh_path)?.clone();
    //     let texture = self.get_texture(context, &object.texture_path)?.clone();
    //     let solid_step = self.make_material_step(context, controls, &object, &texture)?;
    //     let material = Material {
    //         passes: [None, None, Some(solid_step)],
    //     };
    //
    //     let render_object = RenderObject {
    //         mesh,
    //         material,
    //         position: Default::default(),
    //         rotation: Default::default(),
    //     };
    //     Ok(render_object)
    // }

    fn make_control_id_for_object(object_id: &str, uniform_name: &str) -> String {
        format!("ob/{}/{}", object_id, uniform_name)
    }
}

#[instrument(skip_all)]
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

#[instrument(skip_all)]
fn load_texture(context: &VulkanContext, content: &[u8]) -> Result<Arc<Texture>> {
    let now = Instant::now();
    let rgba = image::load_from_memory(content)?.to_rgba8();
    info!("Decoded image in {:?}", now.elapsed());
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

#[instrument(skip_all)]
pub fn load_chart_file(_context: &VulkanContext, content: &[u8]) -> Result<Arc<chart_file::Chart>> {
    let chart = ron::from_str::<chart_file::Chart>(std::str::from_utf8(content)?)?;
    Ok(Arc::new(chart))
}
