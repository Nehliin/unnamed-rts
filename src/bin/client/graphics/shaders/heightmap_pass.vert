#version 450
#extension GL_EXT_scalar_block_layout: require

layout(location=0) in vec2 position;
layout(location=1) in vec2 tex_coords;

layout(location=5) in vec4 m0;
layout(location=6) in vec4 m1;
layout(location=7) in vec4 m2;
layout(location=8) in vec4 m3;

layout(location=0) out float normalizedHeight;

layout(set=0, binding=0) uniform texture2D dis_texture;
layout(set=0, binding=1) uniform sampler dis_sampler;

layout(set=1, binding=0, std430)
uniform CameraUniforms {
    mat4 view;
    mat4 projection;
    vec3 view_pos;
};

void main() {
    mat4 model = mat4(m0, m1, m2, m3);
    // Vec3 isn't needed for pos,
    normalizedHeight = texture(sampler2D(dis_texture, dis_sampler), tex_coords).r;
    vec3 fragment_position = vec3(model * vec4(position.x, position.y, 0.0, 1.0));
    fragment_position = vec3(fragment_position.x, fragment_position.y + normalizedHeight * 5.0, fragment_position.z);
    gl_Position = projection * view * vec4(fragment_position, 1.0);
}
