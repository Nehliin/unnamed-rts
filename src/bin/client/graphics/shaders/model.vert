#version 450
#extension GL_EXT_scalar_block_layout: require

layout(location=0) in vec3 a_position;
layout(location=1) in vec3 a_normal;
layout(location=2) in vec3 a_tan;
layout(location=3) in vec2 tex_coords;

layout(location=4) in vec4 m0;
layout(location=5) in vec4 m1;
layout(location=6) in vec4 m2;
layout(location=7) in vec4 m3;

layout(location=0) out vec2 out_tex_coords;
layout(location=1) out vec3 normal;
layout(location=2) out vec3 fragment_position;
layout(location=3) out vec3 out_view_pos;
layout(location=4) out mat3 tbn;

layout(set=0, binding=0, std430)
uniform CameraUniforms {
    mat4 view;
    mat4 projection;
    vec3 view_pos;
};

void main() {
    out_tex_coords = tex_coords;
    out_view_pos = view_pos;
    mat4 model = mat4(m0, m1, m2, m3);
    fragment_position = vec3(model * vec4(a_position, 1.0));
    // TODO: move this to cpu
    mat3 inv_tran = mat3(transpose(inverse(mat3(model))));
    // TODO: Do the opposite and convert the others to tan space
    normal = inv_tran * a_normal; 
    vec3 tan = inv_tran * a_tan.xyz;
    vec3 bi_tan = cross(normal, tan); //* a_tan.w;
    tbn = mat3(normalize(tan), normalize(bi_tan), normalize(normal));
    gl_Position = projection * view * vec4(fragment_position, 1.0);
}