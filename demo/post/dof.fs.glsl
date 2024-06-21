#version 450

layout (location = 0) in vec2 v_uv;

layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 0) uniform Uniforms {
    float g_z_near;
    float g_app_time;
    float g_pixel_size;
    mat4 g_camera_from_world;
    float focus_distance;
    float focus_scale;
};

layout (set = 1, binding = 1) uniform sampler2D color_texture;
layout (set = 1, binding = 2) uniform sampler2D depth_texture;

const float GOLDEN_ANGLE = 2.39996323;
const float MAX_BLUR_SIZE = 10.0;
const float RAD_SCALE = 0.5; // Smaller = nicer blur, larger = faster

float getBlurSize(float depth, float focusPoint, float focusScale)
{
    float coc = clamp((1.0 / focusPoint - 1.0 / depth)*focusScale, -1.0, 1.0);
    return abs(coc) * MAX_BLUR_SIZE;
}

float depth_sample_to_z2( float buffer_value) {
    return g_z_near / (1 - buffer_value);
}

vec3 depthOfField(vec2 texCoord, float focusPoint, float focusScale)
{
    float centerDepth = depth_sample_to_z2(texture(depth_texture, texCoord).r);
    float centerSize = getBlurSize(centerDepth, focusPoint, focusScale);
    vec3 color = texture(color_texture, v_uv).rgb;
    float tot = 1.0;
    float radius = RAD_SCALE;
    float ic = 0;
    for (float ang = 0.0; radius<MAX_BLUR_SIZE; ang += GOLDEN_ANGLE)
    {
        vec2 tc = texCoord + vec2(cos(ang), sin(ang)) * g_pixel_size * radius;
        vec3 sampleColor = min(texture(color_texture, tc).rgb, 1);
        float sampleDepth = depth_sample_to_z2(texture(depth_texture, tc).r);

        float sampleSize = getBlurSize(sampleDepth, focusPoint, focusScale);
        if (sampleDepth > centerDepth) sampleSize = clamp(sampleSize, 0.0, centerSize*2.0);
        float m = smoothstep(radius-0.5, radius+0.5, sampleSize);
        color += mix(color/tot, sampleColor, m);
        tot += 1.0;
        radius += RAD_SCALE/radius;
        ic += 1;
    }
    return color /= tot;
}

void main() {
    vec4 center = g_camera_from_world * vec4(0,0,0,1);
    f_color = vec4(depthOfField(v_uv, focus_distance, focus_scale), 1);
}
