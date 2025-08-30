alias Quaternion = vec4f;

fn q_from_axis_angle(axis: vec3<f32>, angle: f32) -> Quaternion {
    let half_angle = angle * 0.5;
    let s = sin(half_angle);
    let c = cos(half_angle);
    return Quaternion(axis.x * s, axis.y * s, axis.z * s, c);
}

fn q_multiply(q1: Quaternion, q2: Quaternion) -> Quaternion {
    let w1 = q1.w;
    let v1 = q1.xyz;
    let w2 = q2.w;
    let v2 = q2.xyz;

    let w_new = w1 * w2 - dot(v1, v2);
    let v_new = w1 * v2 + w2 * v1 + cross(v1, v2);
    
    return Quaternion(v_new, w_new);
}

/**
 * Rotates a 3D vector by a unit vec4f.
 * This function uses the efficient formula: v' = v + 2w(u X v) + 2(u X (u X v))
 *
 * @param v The vec3<f32> vector to rotate.
 * @param q The unit vec4f to rotate by.
 * @returns The rotated vec3<f32> vector.
 */
fn q_rotate(v: vec3<f32>, q: Quaternion) -> vec3<f32> {
    let u = q.xyz;
    let w = q.w;
    let cross1 = cross(u, v);
    let cross2 = cross(u, cross1);
    return v + 2.0 * w * cross1 + 2.0 * cross2;
}

/**
 * Performs a spherical linear interpolation (slerp) between two unit vec4fs.
 * This finds the shortest path of rotation between the two vec4fs.
 *
 * @param q1 The starting unit vec4f.
 * @param q2 The ending unit vec4f.
 * @param t The interpolation factor, a float between 0.0 and 1.0.
 * @returns The interpolated unit vec4f.
 */
fn q_slerp(q1: Quaternion, q2_in: Quaternion, t: f32) -> Quaternion {
    var q2 = q2_in;

    // Calculate the cosine of the angle between the two vec4fs.
    var cos_theta = dot(q1, q2);

    // If the dot product is negative, the vec4fs are more than 90 degrees
    // apart. To take the shorter path, we can invert one of the vec4fs.
    if (cos_theta < 0.0) {
        q2 = -q2;
        cos_theta = -cos_theta;
    }

    // If the vec4fs are very close, use linear interpolation to avoid
    // division by a very small number (from sin(theta)).
    if (cos_theta > 0.9995) {
        return normalize(mix(q1, q2, t));
    }

    // Standard slerp calculation
    let theta = acos(cos_theta);
    let sin_theta = sin(theta);
    let scale1 = sin((1.0 - t) * theta) / sin_theta;
    let scale2 = sin(t * theta) / sin_theta;
    
    return q1 * scale1 + q2 * scale2;
}


// --- Example Usage (conceptual, within a compute or vertex shader) ---
/*
struct Uniforms {
    // A vec4f passed from the CPU to the shader.
    rotation: vec4f,
};
@group(0) @binding(0) var<uniform> uni: Uniforms;
@group(0) @binding(1) var<uniform> time: f32;

@vertex
fn main(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    // Define start and end rotations
    let axis_y = vec3<f32>(0.0, 1.0, 0.0);
    let axis_z = vec3<f32>(0.0, 0.0, 1.0);

    // Create a vec4f for a 0-degree rotation around the Y-axis (identity)
    let q_start = q_from_axis_angle(axis_y, 0.0);
    
    // Create a vec4f for a 180-degree rotation around the Z-axis
    let q_end = q_from_axis_angle(axis_z, 3.14159);

    // Interpolate between the start and end rotations based on time
    // Use abs(sin(time)) to create a looping animation effect
    let t = abs(sin(time * 0.5));
    let q_interpolated = q_slerp(q_start, q_end, t);

    // Apply the interpolated rotation to the model's vertex position
    let rotated_position = q_rotate(position, q_interpolated);

    // Final position for rendering (omitting view/projection for simplicity)
    return vec4<f32>(rotated_position, 1.0);
}
*/
