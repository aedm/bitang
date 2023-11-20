pub mod chart_file;
mod material;
pub mod project_file;
mod shader_context;

// TODO: remove
const COMMON_SHADER_FILE: &str = "common.glsl";

// Helper function to initialize a bool using serde
fn default_true() -> bool {
    true
}
