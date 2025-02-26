use crate::loader::resource_repository::{SceneFile, SceneNode};
use crate::render::mesh::Mesh;
use crate::render::Vertex3;
use crate::tool::GpuContext;
use anyhow::{Context, Result};
use itertools::Itertools;
use log::warn;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, instrument};

// gltf is right-handed, y up
fn gltf_to_left_handed_y_up(v: &[f32; 3]) -> [f32; 3] {
    [v[0], v[1], -v[2]]
}

#[instrument(skip(context, content))]
pub fn load_mesh_collection(
    context: &Arc<GpuContext>,
    content: &[u8],
    path: &str,
) -> Result<Arc<SceneFile>> {
    info!("Loading mesh collection");
    let now = Instant::now();

    let (gltf, buffers, _) = gltf::import_slice(content)?;
    let scene = gltf.default_scene().context("No default scene found")?;
    let mut nodes_by_name = HashMap::new();

    for node in scene.nodes() {
        let Some(name) = node.name() else { continue };
        debug!("Loading mesh '{name}'");

        let Some(mesh) = node.mesh() else { continue };

        let primitives = mesh
            .primitives()
            .filter(|p| p.mode() == gltf::mesh::Mode::Triangles)
            .collect_vec();

        for (pi, primitive) in primitives.into_iter().enumerate() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            // Read positions
            let mut vertices = if let Some(iter) = reader.read_positions() {
                iter.map(|p| Vertex3 {
                    a_position: gltf_to_left_handed_y_up(&p),
                    ..Default::default()
                })
                .collect::<Vec<_>>()
            } else {
                info!("Mesh '{name}' has no vertex positions, ignoring.");
                continue;
            };
            let vertex_count = vertices.len();

            // Read normals
            if let Some(iter) = reader.read_normals() {
                for (i, normal) in iter.take(vertex_count).enumerate() {
                    vertices[i].a_normal = gltf_to_left_handed_y_up(&normal);
                }
            } else {
                warn!("Mesh '{name}' has no vertex normals.");
            };

            // Read tangents
            if let Some(iter) = reader.read_tangents() {
                for (i, tangent) in iter.take(vertex_count).enumerate() {
                    // The fourth component of the tangent is the handedness, but I'm feeling lucky and ignore it.
                    vertices[i].a_tangent =
                        gltf_to_left_handed_y_up(&[tangent[0], tangent[1], tangent[2]]);
                }
            } else {
                warn!("Mesh '{name}' has no vertex tangents.");
            };

            // Read texture coordinates
            if let Some(iter) = reader.read_tex_coords(0) {
                for (i, uv) in iter.into_f32().take(vertex_count).enumerate() {
                    vertices[i].a_uv = uv;
                }
            } else {
                warn!("Mesh '{name}' has no texture coordinates.");
            };

            // Read indices
            let indices = reader
                .read_indices()
                .and_then(|indices| Some(indices.into_u32().collect_vec()));

            debug!("Loaded {} vertices", vertices.len());

            let (translation, r, _scale) = node.transform().decomposed();

            let rotation =
                glam::Quat::from_xyzw(r[0], r[1], r[2], r[3]).to_euler(glam::EulerRot::ZXY);

            // Bake scaling into vertices
            for vertex in &mut vertices {
                vertex.a_position[0] *= _scale[0];
                vertex.a_position[1] *= _scale[1];
                vertex.a_position[2] *= _scale[2];
            }

            let mesh = Arc::new(Mesh::try_new(context, vertices, indices)?);

            let node = SceneNode {
                position: gltf_to_left_handed_y_up(&translation),
                rotation: [-rotation.1, -rotation.2, rotation.0],
                mesh,
            };

            let mesh_name = if pi > 0 { format!("{name}.{pi}") } else { name.to_string() };

            nodes_by_name.insert(mesh_name, Arc::new(node));
        }
    }
    info!("Load time {:?}", now.elapsed());

    Ok(Arc::new(SceneFile { nodes_by_name }))
}
