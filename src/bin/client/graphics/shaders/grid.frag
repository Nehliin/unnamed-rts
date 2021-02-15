
#version 450
layout(location = 1) in vec3 nearPoint; // nearPoint calculated in vertex shader
layout(location = 2) in vec3 farPoint; // farPoint calculated in vertex shader
layout(location = 3) in mat4 view;
// layout 7 because of the size of the previous matrix
layout(location = 7) in mat4 proj;

layout(location = 0) out vec4 outColor;
// source: http://asliceofrendering.com/scene%20helper/2020/01/05/InfiniteGrid/
vec4 grid(vec3 frag_pos, float step_size) {
    // step size is used to set distance between lines, lower value -> larger space
    vec2 coord = frag_pos.xz * step_size;
    // fwidth = abs(dFdx(coord) + dFdy(coord))
    // dFdx = partial derivative with respect to x axis (screen space)
    // i.e what's the change going one pixel to the right
    // so basically p(x) - p(x + 1) is used as an approximation
    vec2 derivative = fwidth(coord);
    // fract = x - floor(x) 
    // a = fract(coord - 0.5) guarentees a will between [0, 0.5]
    // abs(a - 0.5) will give how close a is to 0.5
    // without dividing with derivative the lines won't be sharp
    vec2 grid = abs(fract(coord - 0.5) - 0.5) / derivative;
    // the minimum value of the x and y (smallest is the closest to 0.5 / derivative)
    float line = min(grid.x, grid.y);
    float minimumz = min(derivative.y, 1);
    float minimumx = min(derivative.x, 1);
    // opacity = 1 - min(closest to 0.5 / derivative, 1)
    // so if the value is very close to 0.5 / derivative the opacity will be very close to 1
    vec4 color = vec4(0.2, 0.2, 0.2, 1.0 - min(line, 1.0));
    // z axis
    if(frag_pos.x > -0.1 * minimumx && frag_pos.x < 0.1 * minimumx)
        color.z = 1.0;
    // x axis
    if(frag_pos.z > -0.1 * minimumz && frag_pos.z < 0.1 * minimumz)
        color.x = 1.0;
    return color;
}

float computeDepth(vec3 pos) {
    vec4 clip_space_position = proj * view * vec4(pos.xyz, 1.0);
    // The prespective division has do be done manually since it's in the fragment shader 
    return clip_space_position.z / clip_space_position.w;
}

void main() {
    // parametric equation of a line
    // y = nearPoint.y + t * (farPoint.y - nearPoint.y)
    // y = 0 -> t = -nearPoint.y / (farPoint.y - nearPoint.y)
    float t = -nearPoint.y / (farPoint.y - nearPoint.y);
    vec3 frag_pos = nearPoint + t * (farPoint - nearPoint);
    gl_FragDepth = computeDepth(frag_pos);
    // only show the grid when t > 0 meaning it stretches out to infinity from the camera
    outColor = grid(frag_pos, 2)* float(t > 0);
}
