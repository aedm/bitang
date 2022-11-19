#version 450

layout (set = 0, binding = 0) uniform Context {
    mat4 model_to_projection;
    mat4 model_to_camera;
} cx;