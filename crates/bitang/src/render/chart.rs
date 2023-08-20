use crate::control::controls::{ControlSet, ControlSetBuilder};
use crate::control::{ControlId, ControlIdPartType};
use crate::render::buffer_generator::BufferGenerator;
use crate::render::camera::Camera;
use crate::render::draw::Draw;
use crate::render::image::Image;
use crate::render::vulkan_window::RenderContext;
use anyhow::Result;
use std::rc::Rc;
use std::sync::Arc;

pub struct Chart {
    pub id: String,
    pub controls: Rc<ControlSet>,
    camera: Camera,
    images: Vec<Arc<Image>>,
    buffer_generators: Vec<Arc<BufferGenerator>>,
    pub steps: Vec<Draw>,
}

impl Chart {
    pub fn new(
        id: &str,
        control_id: &ControlId,
        mut control_set_builder: ControlSetBuilder,
        images: Vec<Arc<Image>>,
        buffer_generators: Vec<Arc<BufferGenerator>>,
        passes: Vec<Draw>,
    ) -> Self {
        let _camera = Camera::new(
            &mut control_set_builder,
            &control_id.add(ControlIdPartType::Camera, "camera"),
        );
        let controls = Rc::new(control_set_builder.into_control_set());
        Chart {
            id: id.to_string(),
            camera: _camera,
            images,
            buffer_generators,
            steps: passes,
            controls,
        }
    }

    pub fn render(&self, context: &mut RenderContext) -> Result<()> {
        for image in &self.images {
            image.enforce_size_rule(context)?;
        }
        for buffer_generator in &self.buffer_generators {
            buffer_generator.generate()?;
        }
        for draw in &self.steps {
            draw.render(context, &self.camera)?;
        }
        Ok(())
    }
}
