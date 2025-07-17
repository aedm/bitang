use std::rc::Rc;
use std::sync::Arc;

use anyhow::Result;
use glam::{EulerRot, Mat4};

use super::{Control, Material, Mesh, RenderPassContext};

pub struct RenderObject {
    pub _id: String,
    pub mesh: Arc<Mesh>,
    pub material: Arc<Material>,
    pub position: Arc<Control>,
    pub rotation: Arc<Control>,
    pub instances: Arc<Control>,
}

impl RenderObject {
    pub fn render(
        &self,
        context: &mut RenderPassContext,
        material_pass_index: usize,
    ) -> Result<()> {
        let Some(material_pass) = self.material.get_pass(material_pass_index) else {
            return Ok(());
        };

        let saved_globals = *context.globals;
        self.apply_transformations(context);

        context.globals.instance_count = self.instances.as_float().round();

        let result = material_pass.render(context, &self.mesh);
        *context.globals = saved_globals;

        result
    }

    fn apply_transformations(&self, context: &mut RenderPassContext) {
        let rotation = self.rotation.as_vec3();
        let rotation_matrix = Mat4::from_euler(EulerRot::ZXY, rotation.z, rotation.x, rotation.y);

        let position = self.position.as_vec3();
        let translation_matrix = Mat4::from_translation(position);

        context.globals.world_from_model = translation_matrix * rotation_matrix;
        context.globals.update_compound_matrices();
    }
}
