const PI: f32 = 3.14159265359;

fn translate_matrix(translate_vector: vec3<f32>) -> mat4x4<f32> {
    var matrix: mat4x4<f32>;
    matrix[0] = vec4<f32>(1.0, 0.0, 0.0, 0.0);
    matrix[1] = vec4<f32>(0.0, 1.0, 0.0, 0.0);
    matrix[2] = vec4<f32>(0.0, 0.0, 1.0, 0.0);
    matrix[3] = vec4<f32>(translate_vector, 1.0);
    return matrix;
}

fn rotate_x_matrix(angle: f32) -> mat4x4<f32> {
    let s = sin(angle);
    let c = cos(angle);
    var matrix: mat4x4<f32>;
    matrix[0] = vec4<f32>(1.0, 0.0, 0.0, 0.0);
    matrix[1] = vec4<f32>(0.0, c, s, 0.0);
    matrix[2] = vec4<f32>(0.0, -s, c, 0.0);
    matrix[3] = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    return matrix;
}

fn rotate_y_matrix(angle: f32) -> mat4x4<f32> {
    let s = sin(angle);
    let c = cos(angle);
    var matrix: mat4x4<f32>;
    matrix[0] = vec4<f32>(c, 0.0, s, 0.0);
    matrix[1] = vec4<f32>(0.0, 1.0, 0.0, 0.0);
    matrix[2] = vec4<f32>(-s, 0.0, c, 0.0);
    matrix[3] = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    return matrix;
}

fn rotate_z_matrix(angle: f32) -> mat4x4<f32> {
    let s = sin(angle);
    let c = cos(angle);
    var matrix: mat4x4<f32>;
    matrix[0] = vec4<f32>(c, s, 0.0, 0.0);
    matrix[1] = vec4<f32>(-s, c, 0.0, 0.0);
    matrix[2] = vec4<f32>(0.0, 0.0, 1.0, 0.0);
    matrix[3] = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    return matrix;
}

fn rotate_xyz_matrix(angle: vec3<f32>) -> mat4x4<f32> {
    return rotate_x_matrix(angle.x) * rotate_y_matrix(angle.y) * rotate_z_matrix(angle.z);
}

fn calculate_camera_pos_worldspace(camera_from_world: mat4x4<f32>) -> vec3<f32> {
    let inverse_rotation = transpose(mat3x3<f32>(camera_from_world[0].xyz, camera_from_world[1].xyz, camera_from_world[2].xyz));
    return inverse_rotation * -camera_from_world[3].xyz;
}

// fn calculate_camera_pos_worldspace(camera_from_world: mat4x4<f32>) -> vec3<f32> {
//     let myMat3x3 = mat3x3(camera_from_world[0].xyz, camera_from_world[1].xyz, camera_from_world[2].xyz);
//     let inverse_rotation = transpose(myMat3x3);
//     return inverse_rotation * -camera_from_world[3].xyz;
// }