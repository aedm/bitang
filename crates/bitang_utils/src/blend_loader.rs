use anyhow::Result;
use blend::{Blend, Instance};
use std::collections::HashMap;

type Vertex = ([f32; 3], [f32; 3], [f32; 2]);
type Face = [Vertex; 3];

#[derive(Debug)]
pub struct Mesh {
    pub faces: Vec<Face>,
}

#[derive(Debug)]
pub struct ExternalObject {
    pub name: String,
    pub location: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    pub mesh: Mesh,
}

pub struct ExternalObjectCollection {
    pub objects_by_name: HashMap<String, ExternalObject>,
}

// This is only valid for meshes with triangular faces
fn instance_to_mesh(mesh: Instance) -> Option<Mesh> {
    if !mesh.is_valid("mpoly")
        || !mesh.is_valid("mloop")
        || !mesh.is_valid("mloopuv")
        || !mesh.is_valid("mvert")
    {
        return None;
    }

    let faces = mesh.get_iter("mpoly").collect::<Vec<_>>();
    let loops = mesh.get_iter("mloop").collect::<Vec<_>>();
    let uvs = mesh.get_iter("mloopuv").collect::<Vec<_>>();
    let verts = mesh.get_iter("mvert").collect::<Vec<_>>();

    let mut index_count = 0;
    let mut face_indice_count = 0;
    for face in &faces {
        let len = face.get_i32("totloop");
        let mut indexi = 1;
        while indexi < len {
            face_indice_count += 3;
            indexi += 2;
        }
    }

    let mut uv_buffer = vec![0f32; face_indice_count * 2];
    let mut normal_buffer = vec![0f32; face_indice_count * 3];
    let mut verts_array_buff = vec![0f32; face_indice_count * 3];

    for face in &faces {
        let len = face.get_i32("totloop");
        let start = face.get_i32("loopstart");
        let mut indexi = 1;

        while indexi < len {
            let mut index;

            for l in 0..3 {
                if (indexi - 1) + l < len {
                    index = start + (indexi - 1) + l;
                } else {
                    index = start;
                }

                let v = loops[index as usize].get_i32("v");
                let vert = &verts[v as usize];

                let co = vert.get_f32_vec("co");
                // Blend files use right-handed, z-up coordinates.
                // We use left-handed, y-up coordinates.
                // A simple switch of the y and z coordinates is enough to convert.
                verts_array_buff[index_count * 3] = co[0];
                verts_array_buff[index_count * 3 + 1] = co[2];
                verts_array_buff[index_count * 3 + 2] = co[1];

                //Normals are compressed into 16 bit integers
                if vert.is_valid("no") {
                    let no = vert.get_i16_vec("no");
                    normal_buffer[index_count * 3] = f32::from(no[0]) / 32767.0;
                    normal_buffer[index_count * 3 + 2] = f32::from(no[1]) / 32767.0;
                    normal_buffer[index_count * 3 + 1] = f32::from(no[2]) / 32767.0;
                } else {
                    normal_buffer[index_count * 3] = 0.0;
                    normal_buffer[index_count * 3 + 1] = 0.0;
                    normal_buffer[index_count * 3 + 2] = 0.0;
                }

                let uv = uvs[index as usize].get_f32_vec("uv");
                let uv_x = uv[0];
                let uv_y = 1.0 - uv[1];
                uv_buffer[index_count * 2] = uv_x;
                uv_buffer[index_count * 2 + 1] = uv_y;

                index_count += 1;
            }

            indexi += 2;
        }
    }

    let faces: Vec<_> = verts_array_buff[..]
        .chunks(3)
        .enumerate()
        .map(|(i, pos)| {
            (
                [pos[0], pos[1], pos[2]],
                [
                    normal_buffer[i * 3],
                    normal_buffer[i * 3 + 1],
                    normal_buffer[i * 3 + 2],
                ],
                [uv_buffer[i * 2], uv_buffer[i * 2 + 1]],
            )
        })
        .collect::<Vec<Vertex>>();

    let faces: Vec<_> = faces.chunks(3).map(|f| [f[0], f[1], f[2]]).collect();

    Some(Mesh { faces })
}

fn load_blend(blend: Blend) -> Result<ExternalObjectCollection> {
    let mut objects_by_name = HashMap::new();
    let object_marker = "OB".to_string();

    for obj in blend.get_by_code(*b"OB") {
        let name = obj.get("id").get_string("name");
        // println!("-- name {name:?}");

        if !name.starts_with(&object_marker) {
            continue;
        }
        let name = name[object_marker.len()..].to_string();

        let _loc = obj.get_f32_vec("loc");
        let _id = obj.get("id");
        // for field in &id.fields {
        //     println!("NAME {}: VALUE {:?}", field.0, field.1);
        // }
        // println!("OBJ: {:#?}", obj);
        // println!("ID: {:#?}", id);
        // println!("NAME: {:#?}", name);

        // println!("\"{}\" at {:?}", name, loc);
        // let id = obj.get("id").get("newid");
        // println!("ID: {:#?}", id.is_valid("orig_id"));
        // println!("ID: {:#?}", id.is_valid("newid"));
        // println!("OBJ: {:#?}", obj);

        if obj.is_valid("data") && obj.get("data").code()[0..=1] == *b"ME" {
            let loc = obj.get_f32_vec("loc");
            let rot = obj.get_f32_vec("rot");
            let size = obj.get_f32_vec("size");
            let data = obj.get("data");

            if let Some(mesh) = instance_to_mesh(data) {
                objects_by_name.insert(
                    name,
                    ExternalObject {
                        name: obj.get("id").get_string("name"),
                        location: [loc[0], loc[1], loc[2]],
                        rotation: [rot[0], rot[1], rot[2]],
                        scale: [size[0], size[1], size[2]],
                        mesh,
                    },
                );
            }
        }

        // for (name, _value) in &obj.fields {
        //     if obj.is_valid(name) {
        //         // println!(" -- NAME {}: VALUE {:?}", name, value);
        //         // if name == "data" {
        //         //     println!(" -- NAME {}: VALUE {:?}", name, value);
        //         //     println!(" + {:#?}", obj.get("drawdata").fields.get("first").unwrap());
        //         // }
        //     }
        //     // println!("NAME {}: VALUE {:?}", field.0, field.1);
        // }
    }
    Ok(ExternalObjectCollection {
        objects_by_name: objects_by_name,
    })
}

pub fn load_blend_buffer(buffer: &[u8]) -> Result<ExternalObjectCollection> {
    let blend = Blend::new(buffer);
    load_blend(blend)
}
