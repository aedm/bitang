use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RenderObject {
    mesh_path: String,
    mesh_selector: String,
    vertex_shader: String,
    texture_path: String,
    fragment_shader: String,
    depth_test: bool,
    depth_write: bool,
}
