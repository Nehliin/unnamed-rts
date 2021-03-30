#version 450

layout(location = 0) in float normalizedHeight;
layout(location=0) out vec4 f_color;

void main() {
    f_color = vec4(normalizedHeight, normalizedHeight, normalizedHeight, 1);
}