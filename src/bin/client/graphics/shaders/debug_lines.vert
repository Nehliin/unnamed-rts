#version 450
#extension GL_EXT_scalar_block_layout: require

layout(location = 0) in vec3 position;
layout(location = 5) in vec4 m0;
layout(location = 6) in vec4 m1;
layout(location = 7) in vec4 m2;
layout(location = 8) in vec4 m3;

layout(location = 0) out vec3 fragment_position;

layout(set=0, binding=0, std430)
uniform CameraUniforms {
    mat4 view;
    mat4 projection;
    vec3 view_pos;
};


void main() {
    mat4 model_mat = mat4(m0, m1, m2, m3);
    fragment_position = vec3(model_mat * vec4(position, 1.0));
    gl_Position = projection * view * vec4(fragment_position, 1.0);
}