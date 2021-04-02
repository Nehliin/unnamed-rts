#version 450

layout(location = 0) in float normalizedHeight;
layout(location=0) out vec4 f_color;

layout(set=0, binding=0) uniform texture2D color_texture;
layout(set=0, binding=1) uniform sampler color_sampler;

void main() {
    vec3 color = texture(sampler2D(color_texture, color_sampler), tex_coords).rgb;
    f_color = vec4(color, 1);
}