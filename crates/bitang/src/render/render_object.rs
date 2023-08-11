use crate::control::controls::Control;
use crate::render::material::Material;
use crate::render::mesh::Mesh;
use std::rc::Rc;
use std::sync::Arc;
use glam::{EulerRot, Mat4};
use crate::render::vulkan_window::RenderContext;

#[derive(Clone)]
pub struct RenderObject {
    pub id: String,
    pub mesh: Arc<Mesh>,
    pub material: Material,
    pub position: Rc<Control>,
    pub rotation: Rc<Control>,
    pub instances: Rc<Control>,
}

impl RenderObject {
    pub fn render(
        &self,
        context: &mut RenderContext,
        material_pass_index: usize,
    ) -> Result<()> {
        let saved_globals = context.globals;
        self.apply_transformations(context);

        let Some(material_pass) = self.material.get_pass(material_pass_index) else {
            return Ok(());
        };
        
        let instance_count = self.instances.as_float().round() as u32;
        context.globals.instance_count = instance_count as f32;

        let result = component.render(
            context,
            material_step,
            &self.render_object.mesh,
            instance_count,
        );
        context.globals = saved_globals;

        result
    }

    fn apply_transformations(&self, context: &mut RenderContext) {
        let rotation = self.rotation.as_vec3();
        let rotation_matrix = Mat4::from_euler(EulerRot::ZXY, rotation.z, rotation.x, rotation.y);

        let position = self.position.as_vec3();
        let translation_matrix = Mat4::from_translation(position);

        context.globals.world_from_model = translation_matrix * rotation_matrix;
        context.globals.update_compound_matrices();
    }
}