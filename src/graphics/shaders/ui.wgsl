[[block]]
struct ScreenUniform {
    u_screen_size: vec2<f32>;
};

[[group(0), binding(0)]]
var<uniform> screen_uniform: ScreenUniform;

struct VertexInput {
 [[location(0)]] position: vec2<f32>;
 [[location(1)]] tex_coords: vec2<f32>;
 [[location(2)]] color: u32;
};

struct VertexOutput {
 [[builtin(position)]] position: vec4<f32>;
 [[location(0)]] tex_coords: vec2<f32>;
 [[location(1)]] v_color: vec4<f32>;
};

let CUTOFF: vec3<f32> = vec3<f32>(10.31475, 10.31475, 10.31475);
let LOWER: vec3<f32> = vec3<f32>(3294.6, 3294.6, 3294.6);
let H1: vec3<f32> = vec3<f32>(14.025, 14.025, 14.025);
let H2: vec3<f32> = vec3<f32>(269.025, 269.025, 269.025);
let H3: vec3<f32> = vec3<f32>(2.4, 2.4, 2.4);

fn linear_from_srgb(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = srgb < CUTOFF;
    let lower = srgb / LOWER;
    let higher = pow((srgb + H1) / H2, H3);
    return select(lower, higher, cutoff);
}

[[stage(vertex)]]
fn vs_main(in: VertexInput) -> VertexOutput {
    // [u8; 4] SRGB as u32 -> [r, g, b, a]
    let mask: u32 = 255u; // 0xFF
    let temp = vec4<u32>(in.color & mask, (in.color >> 8u) & mask, (in.color >> 16u) & mask, (in.color >> 24u) & mask);
    let color = vec4<f32>(temp);
    let v_color = vec4<f32>(linear_from_srgb(color.rgb), color.a / 255.0);
    var out: VertexOutput;
    out.v_color = v_color;  
    out.tex_coords = in.tex_coords;  
    out.position = vec4<f32>(2.0 * in.position.x / screen_uniform.u_screen_size.x - 1.0, 1.0 - 2.0 * in.position.y / screen_uniform.u_screen_size.y, 0.0, 1.0);  
    return out;
}

[[group(1), binding(0)]] var ui_texture: texture_2d<f32>;
[[group(0), binding(1)]] var ui_sampler: sampler;

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
   return in.v_color * textureSample(ui_texture, ui_sampler, in.tex_coords);
}