use crate::render::draw_call::DrawCall;
use serde::Deserialize;

pub struct Material {
    pub passes: Vec<Option<DrawCall>>,
}

impl Material {
    pub fn get_pass(&self, pass_id: usize) -> Option<&DrawCall> {
        self.passes[pass_id].as_ref()
    }
}
