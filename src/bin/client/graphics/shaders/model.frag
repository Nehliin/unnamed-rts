#version 450
#extension GL_EXT_scalar_block_layout: require

layout(location=0) in vec2 v_tex_coords;
layout(location=1) in vec3 in_normal;
layout(location=2) in vec3 fragment_position;
layout(location=3) in vec3 view_pos;
layout(location=4) in mat3 tbn;

layout(location=0) out vec4 f_color;

layout(set=1, binding=0) uniform texture2D base_texture;
layout(set=1, binding=1) uniform sampler base_sampler;
layout(set=1, binding=2) uniform texture2D metallic_texture;
layout(set=1, binding=3) uniform sampler metallic_sampler;
layout(set=1, binding=4) uniform texture2D occulusion_texture;
layout(set=1, binding=5) uniform sampler occulusion_sampler;
layout(set=1, binding=6) uniform texture2D normal_texture;
layout(set=1, binding=7) uniform sampler normal_sampler;

layout(set=1, binding=8, std430) uniform MaterialFactors {
    vec4 base_color_factor;
    float metallic_factor;
    float roughness_factor;
    float occulusion_strenght;
    float normal_scale;
};

const float PI = 3.14159265359;

struct Light {
   vec3 position;
   vec3 color;   
};
const int light_count = 2;
const Light lights[light_count] = Light[](
    Light(vec3(-1.0, 1.0,0.0), vec3(1.0,1.0,1.0)),
    Light(vec3(1.0, 1.0,0.0), vec3(1.0,1.0,1.0))
   // Light(vec3(7.0, 10.0, 6.0), vec3(1.0,1.0,1.0)),
   // Light(vec3(7.0, 10.0,-6.0), vec3(1.0,1.0,1.0))
);

vec3 fresnelSchlick(float cosTheta, vec3 F0) {
    return F0 + (1.0 - F0) * pow(max(1.0 - cosTheta, 0.0), 5.0);
}

float distributionGGX(vec3 N, vec3 H, float roughness) {
    float a      = roughness*roughness;
    float a2     = a*a;
    float NdotH  = max(dot(N, H), 0.0);
    float NdotH2 = NdotH*NdotH;
	
    float num   = a2;
    float denom = (NdotH2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;
	
    return num / denom;
}

float geometrySchlickGGX(float NdotV, float roughness) {
    float r = (roughness + 1.0);
    float k = (r*r) / 8.0;

    float num   = NdotV;
    float denom = NdotV * (1.0 - k) + k;
	
    return num / denom;
}

float geometrySmith(vec3 N, vec3 V, vec3 L, float roughness) {
    float NdotV = max(dot(N, V), 0.0);
    float NdotL = max(dot(N, L), 0.0);
    float ggx2  = geometrySchlickGGX(NdotV, roughness);
    float ggx1  = geometrySchlickGGX(NdotL, roughness);
	
    return ggx1 * ggx2;
}

void main() {
    vec4 base_tex_color = texture(sampler2D(base_texture, base_sampler), v_tex_coords);
    vec2 metal_tex_color = texture(sampler2D(metallic_texture, metallic_sampler), v_tex_coords).bg;
    vec3 albedo = pow(base_tex_color.rgb, vec3(2.2)) * base_color_factor.rgb;
    float metallic = metal_tex_color.x * metallic_factor;
    float roughness = metal_tex_color.y * roughness_factor;
    float ao = texture(sampler2D(occulusion_texture, occulusion_sampler), v_tex_coords).r * occulusion_strenght;

    vec3 normal = texture(sampler2D(normal_texture, normal_sampler), v_tex_coords).rgb * normal_scale;
    // convert between 0-1 to -1-1
    normal = normalize(normal * 2 - 1);
    vec3 N = normalize(tbn * normal);
    vec3 V = normalize(view_pos - fragment_position);

    // Irradiance
    vec3 Lo = vec3(0.0);
    for(int i = 0; i < light_count; ++i) {
        vec3 L = normalize(lights[i].position - fragment_position);
        vec3 H = normalize(V + L);
        float distance = length(lights[i].position - fragment_position);
        float attenuation = 1.0 / (distance * distance);
        vec3 radiance = lights[i].color * attenuation;

        vec3 F0 = vec3(0.04);
        F0 = mix(F0, albedo, metallic);
        vec3 F = fresnelSchlick(max(dot(H,V), 0.0), F0);
        float ndf = distributionGGX(N, H, roughness);
        float G = geometrySmith(N, V, L, roughness);

        vec3 numerator = ndf * G * F;
        float denominator = 4.0 * max(dot(N,V), 0.0) * max(dot(N,L), 0.0);
        vec3 specular = numerator / max(denominator, 0.001);

        vec3 kS = F;
        vec3 kD = vec3(1.0) - kS;

        kD *= 1.0 - metallic;
        float NdotL = max(dot(N,L), 0.0);
        Lo += (kD * albedo / PI + specular) * radiance * NdotL;
    }
    vec3 ambient = vec3(0.01) * albedo * ao;
    vec3 color = ambient + Lo;
    // HDR
    color = color / (color + vec3(1.0));
    color = pow(color, vec3(1.0/2.2));  
    f_color = vec4(color, 1.0);
}
