#version 450

layout(location=0) in vec2 tex_coords;
layout(location=0) out vec4 f_color;

layout(set=0, binding=2) uniform texture2D color_texture;
layout(set=0, binding=3) uniform sampler color_sampler;
layout(set=0, binding=4) uniform texture2D decal_texture;
layout(set=0, binding=5) uniform sampler decal_sampler;

void main() {
    vec4 color = vec4(texture(sampler2D(color_texture, color_sampler), tex_coords).rgb, 0.0);
    vec4 decal_color = texture(sampler2D(decal_texture, decal_sampler), tex_coords).rgba;
    color = decal_color + color * ( 1 - decal_color.a );
    f_color = color;
}