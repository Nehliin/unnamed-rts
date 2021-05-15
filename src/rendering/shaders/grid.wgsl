
[[block]]
struct CameraUniform {
    view: mat4x4<f32>;
    projection: mat4x4<f32>;
    view_pos: vec3<f32>;
    view_inv: mat4x4<f32>;
    proj_inv: mat4x4<f32>;
};


[[group(0), binding(0)]]
var<uniform> camera: CameraUniform;

struct VertexOutput {
 [[builtin(position)]] proj_position: vec4<f32>;
 [[location(1)]] near_point: vec3<f32>;
 [[location(2)]] far_point: vec3<f32>;
 [[location(3)]] view_proj: mat4x4<f32>;
};



// calculate world position of the point 
fn get_world_coords(x: f32, y:f32, z: f32, unprojection_mat: mat4x4<f32>) -> vec3<f32> {
    let unprojected_point = unprojection_mat * vec4<f32>(x, y, z, 1.0);
    // perspective division is done here since this is something openGL would do 
    // automatically (presumably any shader). It has do be done manually here
    return unprojected_point.xyz / unprojected_point.w;
}

let GRID_PLANE: array<vec3<f32>, 6> = array<vec3<f32>, 6>(
    vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(-1.0, -1.0, 0.0), vec3<f32>(-1.0, 1.0, 0.0), 
    vec3<f32>(-1.0, -1.0, 0.0), vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(1.0, -1.0, 0.0)
);

[[stage(vertex)]]
fn vs_main([[builtin(vertex_index)]] vertex_index: u32) -> VertexOutput {
    let point = GRID_PLANE[vertex_index].xyz;
    // Reversed matrix multiplication order since it's the inverses we have to 
    // undo the last applied first
    let unprojection_mat = camera.view_inv * camera.proj_inv;
    var out: VertexOutput;
    out.near_point = get_world_coords(point.x, point.y, 0.0, unprojection_mat).xyz;
    out.far_point = get_world_coords(point.x, point.y, 1.0, unprojection_mat).xyz;
    out.view_proj = camera.projection * camera.view;
    out.proj_position = vec4<f32>(point, 1.0);
    return out;
}

// FRAGMENT

// source: http://asliceofrendering.com/scene%20helper/2020/01/05/InfiniteGrid/
fn grid(frag_pos: vec3<f32>, step_size: f32) -> vec4<f32> {
    // step size is used to set distance between lines, lower value -> larger space
    let coord = frag_pos.xz * step_size;
    // fwidth = abs(dFdx(coord) + dFdy(coord))
    // dFdx = partial derivative with respect to x axis (screen space)
    // i.e what's the change going one pixel to the right
    // so basically p(x) - p(x + 1) is used as an approximation
    let derivative = abs(dpdx(coord)) + abs(dpdy(coord));
    // fract = x - floor(x) 
    // a = fract(coord - 0.5) guarentees a will between [0, 0.5]
    // abs(a - 0.5) will give how close a is to 0.5
    // without dividing with derivative the lines won't be sharp
    let grid = abs(fract(coord - 0.5) - 0.5) / derivative;
    // the minimum value of the x and y (smallest is the closest to 0.5 / derivative)
    let line = min(grid.x, grid.y);
    let minimumz = min(derivative.y, 1.0);
    let minimumx = min(derivative.x, 1.0);
    // opacity = 1 - min(closest to 0.5 / derivative, 1)
    // so if the value is very close to 0.5 / derivative the opacity will be very close to 1
    var color: vec4<f32> = vec4<f32>(0.2, 0.2, 0.2, 1.0 - min(line, 1.0));
    // z axis
    if(frag_pos.x > -0.1 * minimumx && frag_pos.x < 0.1 * minimumx) {
        color.z = 1.0;
    }
    // x axis
    if(frag_pos.z > -0.1 * minimumz && frag_pos.z < 0.1 * minimumz) {
        color.x = 1.0;
    }
    return color;
}

fn compute_depth(pos: vec3<f32>, view_proj: mat4x4<f32>) -> f32 {
    let clip_space_position = view_proj * vec4<f32>(pos.xyz, 1.0);
    // The prespective division has do be done manually since it's in the fragment shader 
    return clip_space_position.z / clip_space_position.w; 
}

struct FragmentOutput {
    [[builtin(frag_depth)]] frag_depth: f32;
    [[location(0)]] color: vec4<f32>;
};

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> FragmentOutput {
    // parametric equation of a line
    // y = nearPoint.y + t * (farPoint.y - nearPoint.y)
    // y = 0 -> t = -nearPoint.y / (farPoint.y - nearPoint.y)
    let t = -in.near_point.y / (in.far_point.y - in.near_point.y);
    let frag_pos = in.near_point + t * (in.far_point - in.near_point);
    var output: FragmentOutput;
    output.frag_depth = compute_depth(frag_pos, in.view_proj);
    // only show the grid when t > 0 meaning it stretches out to infinity from the camera
    output.color = grid(frag_pos, 2.0) * sign(t);
    return output;
}