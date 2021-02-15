#version 450

layout(location=0) in vec3 a_position;
layout(location=1) in vec3 a_normal;
layout(location=2) in vec2 tex_coords;

layout(location=3) in vec4 m0;
layout(location=4) in vec4 m1;
layout(location=5) in vec4 m2;
layout(location=6) in vec4 m3;

layout(location=0) out vec2 out_tex_coords;
layout(location=1) out vec3 normal;
layout(location=2) out vec3 fragment_position;
layout(location=3) out vec3 out_view_pos;


layout(set=0, binding=0, std140)
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
    normal = mat3(transpose(inverse(mat3(model)))) * a_normal; //make sure surface normals doesn't become fucked when scaling;
    gl_Position = projection * view * vec4(fragment_position, 1.0);
}