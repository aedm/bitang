use crate::loader::resource_repository::{SceneFile, SceneNode};
use crate::render::mesh::Mesh;
use crate::render::Vertex3;
use crate::tool::VulkanContext;
use anyhow::{Context, Result};
use gltf::Gltf;
use log::warn;
use std::array;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

struct GltfLoader {}

impl GltfLoader {
    pub fn from_file(path_buf: PathBuf) -> Self {
        Self {}
    }
}

// glTf is right-handed, y up
fn gltf_to_left_handed_y_up(v: &[f32; 3]) -> [f32; 3] {
    [v[0], v[1], -v[2]]
}

#[instrument(skip(context, content))]
pub fn load_mesh_collection(
    context: &Arc<VulkanContext>,
    content: &[u8],
    resource_name: &str,
) -> Result<Arc<SceneFile>> {
    info!("Loading GLTF resource {resource_name}");
    if resource_name.ends_with("plane_xz_301.glb") {
        let x = 1;
        println!("x = {}", x);
    }
    // let gltf = Gltf::from_slice(content)?;
    // debug!(
    //     "GLTF resource {resource_name} has {} scenes",
    //     gltf.scenes().count()
    // );

    let (gltf, buffers, _) = gltf::import_slice(content)?;
    let scene = gltf.default_scene().context("No default scene found")?;
    // for scene in gltf.scenes() {
    //     for node in scene.nodes() {
    //         println!(
    //             "[{resource_name}] Node {:?} (#{}) has {} children",
    //             node.name(),
    //             node.index(),
    //             node.children().count(),
    //         );
    //     }
    // }

    let mut nodes_by_name = HashMap::new();

    for node in scene.nodes() {
        let Some(name) = node.name() else { continue };
        let Some(mesh) = node.mesh() else { continue };

        let Some(primitive) = mesh
            .primitives()
            .find(|p| p.mode() == gltf::mesh::Mode::Triangles)
        else {
            warn!("Mesh '{name}' has no triangle primitives.");
            continue;
        };

        debug!("Loading mesh '{name}'");
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
        if let Some(indices) = reader.read_indices() {
            vertices = indices
                .into_u32()
                .map(|i| vertices[i as usize])
                .collect::<Vec<_>>();
        }

        debug!("Loaded {} vertices", vertices.len());
        // println!("Vertices: {:#?}", vertices);

        let mesh = Arc::new(Mesh::try_new(context, vertices)?);

        let (translation, r, _scale) = node.transform().decomposed();
        // let rotation = glam::Quat::from_array(rotation).to_euler(glam::EulerRot::XZY);
        // let rotation = glam::Quat::from_array(rotation).to_euler(glam::EulerRot::XYZ);

        let rotation =
            glam::Quat::from_array([r[0], r[1], -r[2], r[3]]).to_euler(glam::EulerRot::XYZ);
        // let rotation = glam::Quat::from_array(rotation).to_euler(glam::EulerRot::ZYX);
        // let rotation = glam::Quat::from_array(rotation).to_euler(glam::EulerRot::YXZ);
        // let rotation = glam::Quat::from_array(rotation).to_euler(glam::EulerRot::YZX);

        error!("{name} Rotation: {:?}, quat: {r:?}", rotation);
        let node = SceneNode {
            position: gltf_to_left_handed_y_up(&translation),
            rotation: rotation.into(),
            mesh,
        };
        nodes_by_name.insert(name.to_string(), Arc::new(node));
    }

    Ok(Arc::new(SceneFile { nodes_by_name }))
}

// fn load_mesh_collection(
//     context: &Arc<VulkanContext>,
//     content: &[u8],
//     _resource_name: &str,
// ) -> anyhow::Result<Arc<MeshCollection>> {
//     let now = Instant::now();
//     let scene = Scene::from_buffer(
//         content,
//         vec![
//             PostProcess::CalculateTangentSpace,
//             PostProcess::Triangulate,
//             PostProcess::JoinIdenticalVertices,
//             PostProcess::SortByPrimitiveType,
//             PostProcess::FlipUVs,
//             PostProcess::OptimizeMeshes,
//         ],
//         "",
//     )?;
//     debug!("Mesh count: {}", scene.meshes.len());
//     let mut meshes_by_name = HashMap::new();
//     for mesh in scene.meshes {
//         let name = mesh.name.clone();
//         if mesh.vertices.is_empty() {
//             warn!("No vertices found in mesh '{name}'");
//             continue;
//         }
//         if mesh.normals.is_empty() {
//             warn!("No normals found in mesh '{name}'");
//             continue;
//         }
//         if mesh.texture_coords.is_empty() {
//             warn!("No texture coordinates found in mesh '{name}'");
//             continue;
//         }
//         if mesh.tangents.is_empty() {
//             warn!("No tangents found in mesh '{name}'");
//             continue;
//         }
//         let uvs = mesh.texture_coords[0]
//             .as_ref()
//             .context("No texture coordinates found")?;
//         let mut vertices = vec![];
//         for (index, face) in mesh.faces.iter().enumerate() {
//             ensure!(
//                 face.0.len() == 3,
//                 "Face {index} in mesh '{name}' has {} vertices, expected 3",
//                 face.0.len()
//             );
//             for i in 0..3 {
//                 let vertex = Vertex3 {
//                     a_position: to_vec3_b(&mesh.vertices[face.0[i] as usize]),
//                     a_normal: to_vec3_b(&mesh.normals[face.0[i] as usize]),
//                     a_tangent: to_vec3_neg(&mesh.tangents[face.0[i] as usize]),
//                     a_uv: to_vec2(&uvs[face.0[i] as usize]),
//                     a_padding: 0.0,
//                 };
//                 vertices.push(vertex);
//             }
//         }
//         let mesh = Arc::new(Mesh::try_new(context, vertices)?);
//         meshes_by_name.insert(name, mesh);
//     }
//
//     info!(
//         "Meshes loaded: '{}' in {:?}",
//         meshes_by_name.keys().join(", "),
//         now.elapsed()
//     );
//     Ok(Arc::new(MeshCollection { meshes_by_name }))
// }
