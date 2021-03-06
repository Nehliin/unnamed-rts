#version 450
#extension GL_EXT_scalar_block_layout: require

layout(location=0) in vec2 v_tex_coords;
layout(location=1) in vec3 normal;
layout(location=2) in vec3 fragment_position;
layout(location=3) in vec3 view_pos;

layout(location=0) out vec4 f_color;

layout(set=1, binding=0) uniform texture2D base_texture;
layout(set=1, binding=1) uniform sampler base_sampler;
layout(set=1, binding=2) uniform texture2D metallic_texture;
layout(set=1, binding=3) uniform sampler metallic_sampler;

layout(set=1, binding=4, std430) uniform MaterialFactors {
    vec4 base_color_factor;
    float metallic_factor;
    float roughness_factor;
};

void main() {
    vec4 base_tex_color = texture(sampler2D(base_texture, base_sampler), v_tex_coords);
    vec2 metal_tex_color = texture(sampler2D(metallic_texture, metallic_sampler), v_tex_coords).bg;
    f_color = base_tex_color * base_color_factor;
}
