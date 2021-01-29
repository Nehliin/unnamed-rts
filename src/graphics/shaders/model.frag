
#version 450


layout(location=1) in vec3 normal;
layout(location=2) in vec3 fragment_position;
layout(location=3) in vec3 view_pos;

layout(location=0) out vec4 f_color;


// handle multiple textures?
/*layout(set = 0, binding = 0) uniform texture2D t_diffuse;
layout(set = 0, binding = 1) uniform sampler s_diffuse;
layout(set = 1, binding = 0) uniform texture2D t_specular;
layout(set = 1, binding = 1) uniform sampler s_specular;


layout(set = 2, binding = 0) uniform texture2DArray t_shadow;
layout(set = 2, binding = 1) uniform samplerShadow s_shadow;

const int MAX_POINT_LIGHTS = 16;
layout(set=4, binding=0) uniform PointLights {
    int lights_used;
    PointLight pointLights[MAX_POINT_LIGHTS];
};


float calc_shadow(int light_id, vec4 homo_coords) {
    vec3 projCoords = homo_coords.xyz / homo_coords.w;
    if (projCoords.z > 1.0) {
        return 0.0;
    } 
    const vec2 flip_correction = vec2(0.5, -0.5);
    // PCF for softer shadows:
    float shadow = 0.0;
    const vec3 texelSize = 1.0 / textureSize(t_shadow, 0);
    float currentDepth = projCoords.z;
    for (int x = -1; x <= 1; ++x) {
        for(int y = -1; y <= 1; ++y) {
            float pcfDepth = texture(sampler2DArrayShadow(t_shadow, s_shadow), vec4(projCoords.xy * flip_correction + 0.5 + vec2(x, y) * texelSize.xy, light_id, projCoords.z));
            shadow += currentDepth > pcfDepth ? 1.0 : 0.0;
        }
    }
    return shadow / 9.0;
}


float calculate_attenuation(vec3 light_position, float constant, float linear, float quadratic) {
    float distance = length(light_position - fragment_position);
    return 1.0 / (constant + (linear * distance) + quadratic * (distance * distance));
}


vec3 calculate_point_light(PointLight light, vec3 normal, float shadow_value) {

    vec3 direction_to_light = normalize(light.position - fragment_position);

    vec3 result = vec3(0.0);

    vec3 viewDir = normalize(view_pos - fragment_position);
    vec3 halfwayDir = normalize(direction_to_light + viewDir);

    float spec = pow(max(dot(normal, halfwayDir), 0.0), 32.0);
    float diff = max(dot(normal, direction_to_light), 0.0);
    float attenuation = calculate_attenuation(light.position, light.constant, light.linear, light.quadratic);


    result += (1.0 - shadow_value) * light.specular * attenuation * spec * texture(sampler2D(t_specular, s_specular), v_tex_coords).rgb;


    result += (1.0 - shadow_value) * light.diffuse * attenuation * diff * texture(sampler2D(t_diffuse, s_diffuse), v_tex_coords).rgb;


    result += light.ambient * attenuation * texture(sampler2D(t_diffuse, s_diffuse), v_tex_coords).rgb;

    return result;
}
*/
const mat4 CONVERSION = mat4(
1.0, 0.0, 0.0, 0.0,
0.0, 1.0, 0.0, 0.0,
0.0, 0.0, 0.5, 0.0,
0.0, 0.0, 0.5, 1.0);


void main() {

   /* vec3 norm = normalize(normal);
    vec3 result = vec3(0.0);
   
    for(int i = 0; i < lights_used; i++) {
        vec4 light_space_pos = CONVERSION * pointLights[i].light_space_matrix * vec4(fragment_position, 1.0);
        float shadow_value = calc_shadow(i, light_space_pos);
        result += calculate_point_light(pointLights[i], norm, shadow_value);
    }*/
    f_color = vec4(1.0);
}