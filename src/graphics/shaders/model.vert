#version 450
#extension GL_EXT_scalar_block_layout: require

layout(location=0) in vec3 a_position;
layout(location=1) in vec3 a_normal;
layout(location=2) in vec3 a_tan;
layout(location=3) in float tan_handedness;
layout(location=4) in vec2 tex_coords;

layout(location=5) in vec4 m0;
layout(location=6) in vec4 m1;
layout(location=7) in vec4 m2;
layout(location=8) in vec4 m3;

layout(location=9) in vec3 n0;
layout(location=10) in vec3 n1;
layout(location=11) in vec3 n2;

layout(location=0) out vec2 out_tex_coords;
layout(location=2) out vec3 fragment_position;
layout(location=3) out vec3 out_view_pos;
layout(location=4) out mat3 tbn;

layout(set=0, binding=0, std430)
uniform CameraUniform {
    mat4 view;
    mat4 projection;
    vec3 view_pos;
    mat4 viewInv;
    mat4 projInv;
};

void main() {
    out_tex_coords = tex_coords;
    out_view_pos = view_pos;
    mat4 m_model = mat4(m0, m1, m2, m3);
    mat3 m_normal_matrix = mat3(n0, n1, n2);
    fragment_position = vec3(m_model * vec4(a_position, 1.0));
    // TODO: Do the opposite and convert the others to tan space
    vec3 normal = normalize(m_normal_matrix * a_normal);
    vec3 tan = normalize(m_normal_matrix * a_tan);
    tan = normalize(tan - dot(tan, normal) * normal);
    vec3 bi_tan = cross(normal, tan) * tan_handedness;
    tbn = mat3(tan, bi_tan, normal);
    gl_Position = projection * view * vec4(fragment_position, 1.0);
}