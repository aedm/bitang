use std::sync::Arc;

use anyhow::{bail, ensure, Result};
use glam::{Mat4, Vec2, Vec3};

use super::{
    Camera, Control, FrameContext, Globals, Pass, RenderObject, RenderPassContext, Scene, Viewport,
};
use crate::engine::RenderStage;

pub enum DrawItem {
    Object(Arc<RenderObject>),
    Scene(Arc<Scene>),
}

/// Represents a draw step in the chart sequence.
pub struct Draw {
    pub id: String,
    pub passes: Vec<Pass>,
    pub items: Vec<DrawItem>,
    pub light_dir_worldspace: Arc<Control>,
    pub shadow_map_size: Arc<Control>,
}

impl Draw {
    pub fn new(
        id: &str,
        passes: Vec<Pass>,
        items: Vec<DrawItem>,
        light_dir_worldspace: Arc<Control>,
        shadow_map_size: Arc<Control>,
    ) -> Result<Draw> {
        Ok(Draw {
            id: id.to_string(),
            passes,
            items,
            light_dir_worldspace,
            shadow_map_size,
        })
    }

    fn render_items(&self, context: &mut RenderPassContext, pass_index: usize) -> Result<()> {
        for object in &self.items {
            match object {
                DrawItem::Object(object) => object.render(context, pass_index)?,
                DrawItem::Scene(scene) => scene.render(context, pass_index)?,
            }
        }
        Ok(())
    }

    fn set_common_globals(&self, globals: &mut Globals) {
        let light_dir_worldspace_norm = self.light_dir_worldspace.as_vec3().normalize();
        globals.light_dir_worldspace_norm = light_dir_worldspace_norm;
    }

    fn set_globals_for_shadow_map_rendering(&self, globals: &mut Globals) {
        let shadow_map_size = self.shadow_map_size.as_float();

        globals.pixel_size = Vec2::new(1.0 / shadow_map_size, 1.0 / shadow_map_size);
        globals.aspect_ratio = 1.0;
        globals.field_of_view = 0.0;
        globals.z_near = -shadow_map_size;
        globals.shadow_map_size = shadow_map_size;

        globals.projection_from_camera = Mat4::orthographic_lh(
            -shadow_map_size,
            shadow_map_size,
            -shadow_map_size,
            shadow_map_size,
            -shadow_map_size,
            shadow_map_size,
        );

        // TODO: position shadow center to camera target, fix artifacts
        globals.camera_from_world =
            Mat4::look_to_lh(Vec3::ZERO, -globals.light_dir_worldspace_norm, Vec3::Y);

        // When camera space is the light source space, the direction of light is always forward
        globals.light_dir_camspace_norm = Vec3::Z;

        // Render objects should take care of their model-to-world transformation
        globals.world_from_model = Mat4::IDENTITY;
        globals.light_projection_from_world =
            globals.projection_from_camera * globals.camera_from_world;

        globals.update_compound_matrices();
    }

    pub fn render_offscreen(
        &self,
        frame_context: &mut FrameContext,
        camera: &Camera,
    ) -> Result<()> {
        let RenderStage::Offscreen(_) = &mut frame_context.render_stage else {
            bail!("Render stage is not offscreen");
        };

        // Render each pass
        for (pass_index, pass) in self.passes.iter().enumerate() {
            let Pass::OffScreen(render_target) = pass else {
                continue;
            };

            let (viewport, canvas_size) =
                render_target.get_viewport_and_canvas_size(frame_context)?;

            // Set globals unspecific to pass
            self.set_common_globals(&mut frame_context.globals);

            // Set pass-specific globals
            if render_target.id == "shadow" {
                self.set_globals_for_shadow_map_rendering(&mut frame_context.globals);
            } else {
                camera.set_globals(&mut frame_context.globals, canvas_size);
            }

            let mut render_pass =
                render_target.make_render_pass_context(&mut frame_context.render_stage)?;

            let Viewport { x, y, size } = viewport;
            render_pass.set_viewport(x as f32, y as f32, size[0] as f32, size[1] as f32, 0.0, 1.0);

            let mut render_pass_context = RenderPassContext {
                gpu_context: &frame_context.gpu_context,
                pass: &mut render_pass,
                globals: &mut frame_context.globals,
            };
            self.render_items(&mut render_pass_context, pass_index)?;
        }

        Ok(())
    }

    pub fn render_onscreen(&self, frame_context: &mut FrameContext, camera: &Camera) -> Result<()> {
        let RenderStage::Onscreen(render_pass) = &mut frame_context.render_stage else {
            bail!("Render stage is not onscreen");
        };

        // Render each pass
        for (pass_index, pass) in self.passes.iter().enumerate() {
            let Pass::Screen = pass else {
                continue;
            };

            let viewport = frame_context.screen_viewport;

            // Set globals unspecific to pass
            self.set_common_globals(&mut frame_context.globals);

            // Set pass-specific globals
            camera.set_globals(&mut frame_context.globals, viewport.size);

            let Viewport { x, y, size } = viewport;
            render_pass.set_viewport(x as f32, y as f32, size[0] as f32, size[1] as f32, 0.0, 1.0);

            let mut render_pass_context = RenderPassContext {
                gpu_context: &frame_context.gpu_context,
                pass: render_pass,
                globals: &mut frame_context.globals,
            };
            self.render_items(&mut render_pass_context, pass_index)?;
        }

        Ok(())
    }

    pub fn render(&self, frame_context: &mut FrameContext, camera: &Camera) -> Result<()> {
        ensure!(!self.passes.is_empty(), "Draw '{}' has no passes", self.id);

        match &frame_context.render_stage {
            RenderStage::Offscreen(_) => self.render_offscreen(frame_context, camera),
            RenderStage::Onscreen(_) => self.render_onscreen(frame_context, camera),
        }
    }
}
