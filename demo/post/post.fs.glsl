layout (location = 0) in vec2 v_uv;
layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 1) uniform sampler2D original;

layout (set = 1, binding = 0) uniform Uniforms {
    vec2 g_pixel_size;
    float do_clamp;
};

void main() {
    f_color = texture(original, v_uv);
    return;

    vec2 adjust = g_pixel_size * 0.25;
    vec4 c1 = texture(original, v_uv + vec2(-adjust.x, -adjust.y));
    vec4 c2 = texture(original, v_uv + vec2(adjust.x, -adjust.y));
    vec4 c3 = texture(original, v_uv + vec2(adjust.x, adjust.y));
    vec4 c4 = texture(original, v_uv + vec2(-adjust.x, adjust.y));

    if (do_clamp > 0.5) {
        f_color = (clamp(c1-1,0,1) + clamp(c2-1,0,1) + clamp(c3-1,0,1) + clamp(c4-1,0,1)) * 0.25;
    } else {
        f_color = (c1 + c2 + c3 + c4) * 0.25;
    }
}
