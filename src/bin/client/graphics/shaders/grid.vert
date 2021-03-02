#version 450
#extension GL_EXT_scalar_block_layout: require

layout(set=0, binding=0, std430)
uniform CameraUniforms {
    mat4 view;
    mat4 projection;
    vec3 view_pos;
};

layout(location = 1) out vec3 nearPoint;
layout(location = 2) out vec3 farPoint;
layout(location = 3) out mat4 frag_view;
// 7 because of the size of the previous matrix
layout(location = 7) out mat4 frag_proj;

const vec3 gridPlane[6] = vec3[](
    vec3(1, 1, 0), vec3(-1, -1, 0), vec3(-1, 1, 0),
    vec3(-1, -1, 0), vec3(1, 1, 0), vec3(1, -1, 0)
);

// This gets the world coordinates of a point
vec3 get_world_cords(float x, float y, float z, mat4 view, mat4 projection) {
    mat4 viewInv = inverse(view);
    mat4 projInv = inverse(projection);
    // Reversed matrix multiplication order since it's the inverses we have to 
    // undo the last applied first
    vec4 unprojected_point = viewInv * projInv * vec4(x, y, z, 1.0); 
    // perspective division is done here since this is something openGL would do 
    // automatically (presumably any glsl shader). It has do be done manually here
    return unprojected_point.xyz / unprojected_point.w;
}

void main() {
    // local coords
    vec3 p = gridPlane[gl_VertexIndex].xyz;
    // unclear why 0 instead of -1
    // since it's supposed to emulate an infinate grid the near (z = 0)
    // and far (z = 1) clip space coordinates
    nearPoint = get_world_cords(p.x, p.y, 0.0, view, projection).xyz;
    farPoint = get_world_cords(p.x, p.y, 1.0, view, projection).xyz;
    frag_view = view;
    frag_proj = projection;
    // pretend they are already in clip space
    gl_Position = vec4(p, 1.0);
}