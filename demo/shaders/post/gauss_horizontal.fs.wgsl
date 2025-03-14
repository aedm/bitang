struct Uniforms {
    g_pixel_size: vec2<f32>,
}

@group(1) @binding(0) var<uniform> uniforms: Uniforms;
@group(1) @binding(1) var base_color: texture_2d<f32>;
@group(1) @binding(3) var sampler_clamp_to_edge: sampler;

const MIP: f32 = 3.0;
const POW2MIP: f32 = 8.0; // pow(2.0, MIP)

const gauss_kernel_size: i32 = 20;
var<private> gauss_weight: array<f32, 41> = array<f32, 41>(
    0.0003, 0.0004, 0.0007, 0.0012, 0.0019, 0.0029, 0.0044, 0.0064, 0.0090, 0.0124, 
    0.0166, 0.0216, 0.0274, 0.0337, 0.0404, 0.0470, 0.0532, 0.0587, 0.0629, 0.0655, 
    0.0665, 0.0655, 0.0629, 0.0587, 0.0532, 0.0470, 0.0404, 0.0337, 0.0274, 0.0216, 
    0.0166, 0.0124, 0.0090, 0.0064, 0.0044, 0.0029, 0.0019, 0.0012, 0.0007, 0.0004, 
    0.0003
);

struct VertexOutput {
    @location(0) v_uv: vec2<f32>,
    @builtin(position) position: vec4<f32>,
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let texture_dimensions = textureDimensions(base_color, 0);
    let base_color_pixel_size = 1.0 / f32(texture_dimensions.x);
    let uvstep = base_color_pixel_size * POW2MIP;
    
    var result = vec3<f32>(0.0, 0.0, 0.0);
    var d = in.v_uv - vec2<f32>(uvstep * f32(gauss_kernel_size), 0.0);
    
    for (var i = 0; i < gauss_kernel_size * 2 + 1; i++) {
        let c = textureSampleLevel(
            base_color, 
            sampler_clamp_to_edge, 
            d, 
            MIP
        ).rgb * gauss_weight[i];
        
        result += c;
        d.x += uvstep;
    }
    
    return vec4<f32>(result, 1.0);
}
