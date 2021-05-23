[[block]]
struct CameraUniform {
    view: mat4x4<f32>;
    projection: mat4x4<f32>;
    view_pos: vec3<f32>;
    view_inv: mat4x4<f32>;
    proj_inv: mat4x4<f32>;
};

[[group(1), binding(0)]]
var<uniform> camera: CameraUniform;

struct VertexInput {
 [[location(0)]] position: vec3<f32>;
 [[location(1)]] tex_coords: vec2<f32>;
 [[location(5)]] m0: vec4<f32>;
 [[location(6)]] m1: vec4<f32>;
 [[location(7)]] m2: vec4<f32>;
 [[location(8)]] m3: vec4<f32>;
};

[[group(0), binding(0)]] var color_tex: texture_2d<f32>;
[[group(0), binding(1)]] var color_sampler: sampler;

struct VertexOutput {
 [[builtin(position)]] position: vec4<f32>;
 [[location(0)]] tex_coords: vec2<f32>;
};

[[stage(vertex)]]
fn vs_main(in: VertexInput) -> VertexOutput {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    let world_pos = model * vec4<f32>(in.position.x, in.position.y, in.position.z, 1.0);
    var out: VertexOutput;
    out.position = camera.projection * camera.view * world_pos;  
    out.tex_coords = in.tex_coords;
    return out;
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
   let color = vec4<f32>(textureSample(color_tex, color_sampler, in.tex_coords).rbg, 0.0);
   return color; 
}