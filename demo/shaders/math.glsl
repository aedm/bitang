#pragma once

mat4 translate_matrix(vec3 translate_vector) {
    mat4 matrix;
    matrix[0] = vec4(1, 0, 0, 0);
    matrix[1] = vec4(0, 1, 0, 0);
    matrix[2] = vec4(0, 0, 1, 0);
    matrix[3] = vec4(translate_vector, 1);
    return matrix;
}

mat4 rotate_x_matrix(float angle) {
    float s = sin(angle);
    float c = cos(angle);
    mat4 matrix;
    matrix[0] = vec4(1, 0, 0, 0);
    matrix[1] = vec4(0, c, s, 0);
    matrix[2] = vec4(0, -s, c, 0);
    matrix[3] = vec4(0, 0, 0, 1);
    return matrix;
}

mat4 rotate_y_matrix(float angle) {
    float s = sin(angle);
    float c = cos(angle);
    mat4 matrix;
    matrix[0] = vec4(c, 0, s, 0);
    matrix[1] = vec4(0, 1, 0, 0);
    matrix[2] = vec4(-s, 0, c, 0);
    matrix[3] = vec4(0, 0, 0, 1);
    return matrix;
}

mat4 rotate_z_matrix(float angle) {
    float s = sin(angle);
    float c = cos(angle);
    mat4 matrix;
    matrix[0] = vec4(c, s, 0, 0);
    matrix[1] = vec4(-s, c, 0, 0);
    matrix[2] = vec4(0, 0, 1, 0);
    matrix[3] = vec4(0, 0, 0, 1);
    return matrix;
}

mat4 rotate_xyz_matrix(vec3 angle) {
    return rotate_x_matrix(angle.x) * rotate_y_matrix(angle.y) * rotate_z_matrix(angle.z);
}
