use crate::control::controls::ControlSetBuilder;
use crate::control::{ControlId, ControlIdPartType};
use crate::file::material::Material;
use crate::file::resource_repository::ResourceRepository;
use crate::file::shader_loader::ShaderCompilationResult;
use crate::file::ResourcePath;
use crate::render::buffer_generator::BufferGeneratorType;
use crate::render::image::ImageSizeRule;
use crate::render::vulkan_window::VulkanContext;
use crate::{file, render};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::instrument;

#[derive(Debug, Deserialize)]
pub struct Chart {
    #[serde(default)]
    pub images: Vec<Image>,

    #[serde(default)]
    pub buffer_generators: Vec<BufferGenerator>,

    pub steps: Vec<Draw>,
}

impl Chart {
    pub fn load(
        &self,
        id: &str,
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
        path: &ResourcePath,
    ) -> Result<render::chart::Chart> {
        let control_id = ControlId::default().add(ControlIdPartType::Chart, id);
        let mut control_set_builder = ControlSetBuilder::new(
            control_id.clone(),
            resource_repository.control_repository.clone(),
        );

        let mut images_by_id = self
            .images
            .iter()
            .map(|image_desc| {
                let image = image_desc.load()?;
                Ok((image_desc.id.clone(), image))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        // Add default swapchain images to the image map
        for (id, image) in &context.swapchain_render_targets_by_id {
            images_by_id.insert(id.clone(), image.clone());
        }

        let buffer_generators_by_id = self
            .buffer_generators
            .iter()
            .map(|buffer_generator| {
                let generator =
                    buffer_generator.load(context, &control_id, &mut control_set_builder);
                (buffer_generator.id.clone(), Arc::new(generator))
            })
            .collect::<HashMap<_, _>>();

        let passes = self
            .steps
            .iter()
            .map(|pass| {
                pass.load(
                    context,
                    resource_repository,
                    &mut control_set_builder,
                    &images_by_id,
                    &buffer_generators_by_id,
                    &control_id,
                    path,
                )
            })
            .collect::<Result<Vec<_>>>()?;

        let images = images_by_id.into_values().collect::<Vec<_>>();
        let buffer_generators = buffer_generators_by_id.into_values().collect::<Vec<_>>();

        let chart = render::chart::Chart::new(
            id,
            &control_id,
            control_set_builder,
            images,
            buffer_generators,
            passes,
        );
        Ok(chart)
    }
}

#[derive(Debug, Deserialize)]
pub struct Image {
    pub id: String,
    // pub path: Option<String>,
    // pub generate_mipmaps: Option<bool>,
    pub size: ImageSizeRule,
    pub format: render::image::ImageFormat,
}

impl Image {
    pub fn load(&self) -> Result<Arc<render::image::Image>> {
        let image = render::image::Image::new_attachment(&self.id, self.format, self.size);
        Ok(image)
    }
}

fn default_clear_color() -> Option<[f32; 4]> {
    Some([0.03, 0.03, 0.03, 1.0])
}

/// Represents a draw step in the chart sequence.
#[derive(Debug, Deserialize)]
pub struct Draw {
    pub id: String,
    // pub render_targets: Vec<String>,
    pub passes: Vec<Pass>,
    pub objects: Vec<Object>,
    // #[serde(default = "default_clear_color")]
    // pub clear_color: Option<[f32; 4]>,
}

impl Draw {
    #[allow(clippy::too_many_arguments)]
    pub fn load(
        &self,
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
        control_set_builder: &mut ControlSetBuilder,
        images_by_id: &HashMap<String, Arc<render::image::Image>>,
        buffer_generators_by_id: &HashMap<String, Arc<render::buffer_generator::BufferGenerator>>,
        chart_id: &ControlId,
        path: &ResourcePath,
    ) -> Result<render::draw::Draw> {
        let control_prefix = chart_id.add(ControlIdPartType::ChartStep, &self.id);
        let chart_id = chart_id.add(ControlIdPartType::ChartValues, "Chart Values");
        let passes = self
            .passes
            .iter()
            .map(|pass| pass.load(context, images_by_id))
            .collect::<Result<Vec<_>>>()?;
        // let render_targets = self
        //     .render_targets
        //     .iter()
        //     .map(|render_target_id| {
        //         render_targets_by_id
        //             .get(render_target_id)
        //             .or_else(|| context.swapchain_render_targets_by_id.get(render_target_id))
        //             .cloned()
        //             .with_context(|| anyhow!("Render target '{}' not found", render_target_id))
        //     })
        //     .collect::<Result<Vec<_>>>()?;

        let objects = self
            .objects
            .iter()
            .map(|object| {
                object.load(
                    &control_prefix,
                    &chart_id,
                    context,
                    resource_repository,
                    control_set_builder,
                    images_by_id,
                    buffer_generators_by_id,
                    path,
                    &passes,
                )
            })
            .collect::<Result<Vec<_>>>()?;

        let draw = render::draw::Draw::new(&self.id, passes, objects)?;
        Ok(draw)
    }
}

#[derive(Debug, Deserialize)]
pub enum ImageSelector {
    /// Level 0 of the image
    Image(String),
}

impl ImageSelector {
    pub fn load(
        &self,
        images_by_id: &HashMap<String, Arc<render::image::Image>>,
    ) -> Result<render::pass::ImageSelector> {
        match self {
            ImageSelector::Image(id) => images_by_id
                .get(id)
                .cloned()
                .map(render::pass::ImageSelector::Image)
                .ok_or_else(|| anyhow!("Render target not found: {}", id)),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Pass {
    pub id: String,
    pub depth_buffer: Option<ImageSelector>,
    pub color_buffers: Vec<ImageSelector>,

    #[serde(default = "default_clear_color")]
    pub clear_color: Option<[f32; 4]>,
}

impl Pass {
    pub fn load(
        &self,
        context: &VulkanContext,
        render_targets_by_id: &HashMap<String, Arc<render::image::Image>>,
    ) -> Result<render::pass::Pass> {
        let depth_buffer = self
            .depth_buffer
            .as_ref()
            .map(|selector| selector.load(render_targets_by_id))
            .transpose()?;
        let color_buffers = self
            .color_buffers
            .iter()
            .map(|color_buffer| color_buffer.load(render_targets_by_id))
            .collect::<Result<Vec<_>>>()?;

        let pass = render::pass::Pass::new(
            &self.id,
            context,
            color_buffers,
            depth_buffer,
            self.clear_color,
        );
        pass
    }
}

#[derive(Debug, Deserialize)]
pub struct BufferGenerator {
    id: String,
    size: u32,
    generator: BufferGeneratorType,
}

impl BufferGenerator {
    pub fn load(
        &self,
        context: &VulkanContext,
        parent_id: &ControlId,
        control_set_builder: &mut ControlSetBuilder,
    ) -> render::buffer_generator::BufferGenerator {
        let control_id = parent_id.add(ControlIdPartType::BufferGenerator, &self.id);

        render::buffer_generator::BufferGenerator::new(
            self.size,
            context,
            &control_id,
            control_set_builder,
            &self.generator,
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct Object {
    pub id: String,
    pub mesh_file: String,
    pub mesh_name: String,
    pub material: file::material::Material,
    // pub vertex_shader: String,
    // pub fragment_shader: String,
    //
    // #[serde(default = "default_true")]
    // pub depth_test: bool,
    //
    // #[serde(default = "default_true")]
    // pub depth_write: bool,
    //
    // #[serde(default)]
    // pub textures: HashMap<String, TextureMapping>,
    //
    // #[serde(default)]
    // pub buffers: HashMap<String, BufferMapping>,
    #[serde(default)]
    pub control_map: HashMap<String, String>,
}

impl Object {
    #[allow(clippy::too_many_arguments)]
    pub fn load(
        &self,
        parent_id: &ControlId,
        chart_id: &ControlId,
        context: &VulkanContext,
        resource_repository: &mut ResourceRepository,
        control_set_builder: &mut ControlSetBuilder,
        images_by_id: &HashMap<String, Arc<render::image::Image>>,
        buffer_generators_by_id: &HashMap<String, Arc<render::buffer_generator::BufferGenerator>>,
        path: &ResourcePath,
        passes: &Vec<render::pass::Pass>,
    ) -> Result<Arc<crate::render::render_object::RenderObject>> {
        let control_id = parent_id.add(ControlIdPartType::Object, &self.id);
        let mesh = resource_repository
            .get_mesh(
                context,
                &path.relative_path(&self.mesh_file),
                &self.mesh_name,
            )?
            .clone();

        let material = self.material.load(
            context,
            resource_repository,
            images_by_id,
            path,
            passes,
            control_set_builder,
            &self.control_map,
            &control_id,
            chart_id,
            buffer_generators_by_id,
        )?; // TODO: pass control_id and chart_id to material.load(

        let position_id = control_id.add(ControlIdPartType::Value, "position");
        let rotation_id = control_id.add(ControlIdPartType::Value, "rotation");
        let instances_id = control_id.add(ControlIdPartType::Value, "instances");

        let object = crate::render::render_object::RenderObject {
            id: self.id.clone(),
            mesh,
            material,
            position: control_set_builder.get_vec3(&position_id),
            rotation: control_set_builder.get_vec3(&rotation_id),
            instances: control_set_builder.get_float_with_default(&instances_id, 1.),
        };
        Ok(Arc::new(object))
    }

    // #[allow(clippy::too_many_arguments)]
    // fn make_material_step(
    //     &self,
    //     context: &VulkanContext,
    //     resource_repository: &mut ResourceRepository,
    //     control_set_builder: &mut ControlSetBuilder,
    //     parent_id: &ControlId,
    //     chart_id: &ControlId,
    //     control_map: &HashMap<String, String>,
    //     sampler_sources_by_id: &HashMap<String, DescriptorSource>,
    //     buffer_sources_by_id: &HashMap<String, DescriptorSource>,
    //     path: &ResourcePath,
    // ) -> Result<MaterialPass> {
    //     let shaders = resource_repository.shader_cache.get_or_load(
    //         context,
    //         &path.relative_path(&self.vertex_shader),
    //         &path.relative_path(&self.fragment_shader),
    //         &path.relative_path(COMMON_SHADER_FILE),
    //     )?;
    //
    //     let vertex_shader = make_shader(
    //         control_set_builder,
    //         parent_id,
    //         chart_id,
    //         control_map,
    //         &shaders.vertex_shader,
    //         sampler_sources_by_id,
    //         buffer_sources_by_id,
    //     )?;
    //     let fragment_shader = make_shader(
    //         control_set_builder,
    //         parent_id,
    //         chart_id,
    //         control_map,
    //         &shaders.fragment_shader,
    //         sampler_sources_by_id,
    //         buffer_sources_by_id,
    //     )?;
    //
    //     let material_step = MaterialPass {
    //         vertex_shader,
    //         fragment_shader,
    //         depth_test: self.depth_test,
    //         depth_write: self.depth_write,
    //         blend_mode: self.blend_mode.load(),
    //         sampler_address_mode: self.sampler_address_mode.load(),
    //     };
    //     Ok(material_step)
    // }
}

// #[instrument(skip_all)]
// fn make_shader(
//     control_set_builder: &mut ControlSetBuilder,
//     parent_id: &ControlId,
//     chart_id: &ControlId,
//     control_map: &HashMap<String, String>,
//     compilation_result: &ShaderCompilationResult,
//     sampler_sources_by_id: &HashMap<String, DescriptorSource>,
//     buffer_sources_by_id: &HashMap<String, DescriptorSource>,
// ) -> Result<Shader> {
//     let local_mapping = compilation_result
//         .local_uniform_bindings
//         .iter()
//         .map(|binding| {
//             let control_id = if let Some(mapped_name) = control_map.get(&binding.name) {
//                 chart_id.add(ControlIdPartType::Value, mapped_name)
//             } else {
//                 parent_id.add(ControlIdPartType::Value, &binding.name)
//             };
//             let control = control_set_builder.get_vec(&control_id, binding.f32_count);
//             LocalUniformMapping {
//                 control,
//                 f32_count: binding.f32_count,
//                 f32_offset: binding.f32_offset,
//             }
//         })
//         .collect::<Vec<_>>();
//
//     let mut sampler_bindings = compilation_result
//         .samplers
//         .iter()
//         .map(|sampler| {
//             let sampler_source = sampler_sources_by_id
//                 .get(&sampler.name)
//                 .cloned()
//                 .with_context(|| format!("Sampler binding '{}' not found", sampler.name))?;
//             Ok(DescriptorBinding {
//                 descriptor_source: sampler_source,
//                 descriptor_set_binding: sampler.binding,
//             })
//         })
//         .collect::<Result<Vec<DescriptorBinding>>>()?;
//
//     let mut buffer_bindings = compilation_result
//         .buffers
//         .iter()
//         .map(|buffer| {
//             let buffer_source = buffer_sources_by_id
//                 .get(&buffer.name)
//                 .cloned()
//                 .with_context(|| format!("Buffer binding '{}' not found", buffer.name))?;
//             Ok(DescriptorBinding {
//                 descriptor_source: buffer_source,
//                 descriptor_set_binding: buffer.binding,
//             })
//         })
//         .collect::<Result<Vec<DescriptorBinding>>>()?;
//
//     let mut descriptor_bindings = vec![];
//     descriptor_bindings.append(&mut sampler_bindings);
//     descriptor_bindings.append(&mut buffer_bindings);
//
//     Ok(Shader {
//         shader_module: compilation_result.module.clone(),
//         descriptor_bindings,
//         local_uniform_bindings: local_mapping,
//         global_uniform_bindings: compilation_result.global_uniform_bindings.clone(),
//         uniform_buffer_size: compilation_result.uniform_buffer_size,
//     })
// }
