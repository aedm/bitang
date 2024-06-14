pub mod chart_file;
mod material;
mod object;
pub mod project_file;
mod scene;
mod shader_context;

// Helper function to initialize a bool using serde
fn default_true() -> bool {
    true
}
