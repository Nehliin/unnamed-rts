[[block]]
struct CameraUniform {
    view: mat4x4<f32>;
    projection: mat4x4<f32>;
    view_pos: vec3<f32>;
    view_inv: mat4x4<f32>;
    proj_inv: mat4x4<f32>;
};

[[group(0), binding(0)]]
var<uniform> camera: CameraUniform;

struct VertexInput {
 [[location(0)]] position: vec3<f32>;
 [[location(1)]] normal: vec3<f32>;
 [[location(2)]] tangent: vec3<f32>;
 [[location(3)]] tanget_handedness: f32;
 [[location(4)]] tex_coords: vec2<f32>;
 [[location(5)]] m0: vec4<f32>;
 [[location(6)]] m1: vec4<f32>;
 [[location(7)]] m2: vec4<f32>;
 [[location(8)]] m3: vec4<f32>;
 [[location(9)]] n0: vec3<f32>;
 [[location(10)]] n1: vec3<f32>;
 [[location(11)]] n2: vec3<f32>;
};

struct VertexOutput {
 [[builtin(position)]] position: vec4<f32>;
 [[location(0)]] tex_coords: vec2<f32>;
 [[location(2)]] frag_position: vec3<f32>; // why not 1?
 [[location(3)]] view_pos: vec3<f32>;
 [[location(4)]] tbn0: vec3<f32>;
 [[location(5)]] tbn1: vec3<f32>;
 [[location(6)]] tbn2: vec3<f32>;
};

[[stage(vertex)]]
fn vs_main(in: VertexInput) -> VertexOutput {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    let normal_matrix = mat3x3<f32>(in.n0, in.n1, in.n2);
    let f_position = model * vec4<f32>(in.position, 1.0);
    let normal = normalize(normal_matrix * in.normal);
    let pre_tangent = normalize(normal_matrix * in.tangent);
    let tangent = normalize(pre_tangent - dot(pre_tangent, normal) * normal);
    let bi_tan = cross(normal, tangent) * in.tanget_handedness;
    var out: VertexOutput;
    out.position = camera.projection * camera.view * f_position;  
    out.frag_position = vec3<f32>(f_position.x, f_position.y, f_position.z); 
    out.view_pos = camera.view_pos;
    out.tbn0 = tangent;
    out.tbn1 = bi_tan;
    out.tbn2 = normal;
    out.tex_coords = in.tex_coords;
    return out;
}


// FRAGMENT SHADER

let MAX_LIGHTS: i32 = 5;
let PI: f32 = 3.14159265359;

// TODO: reuse the sampler!
[[group(1), binding(0)]] var base_texture: texture_2d<f32>;
[[group(1), binding(1)]] var base_sampler: sampler;
[[group(1), binding(2)]] var metallic_texture: texture_2d<f32>;
[[group(1), binding(3)]] var metallic_sampler: sampler;
[[group(1), binding(4)]] var occulusion_texture: texture_2d<f32>;
[[group(1), binding(5)]] var occulusion_sampler: sampler;
[[group(1), binding(6)]] var normal_texture: texture_2d<f32>;
[[group(1), binding(7)]] var normal_sampler: sampler;

[[block]]
struct MaterialFactors {
    base_color_factor: vec4<f32>;
    metallic_factor: f32;
    roughness_factor: f32;
    occulusion_strength: f32;
    normal_scale: f32;
};

[[group(1), binding(8)]]
var<uniform> material_factors: MaterialFactors;

struct PointLight {
     color: vec3<f32>;
     position: vec3<f32>; 
};

[[block]]
struct PointLights {
    inner: array<PointLight, MAX_LIGHTS>;
};

[[block]]
struct PointLightCount {
    count: i32;
};

[[group(2), binding(0)]]
var<uniform> point_lights: PointLights;

[[group(2), binding(1)]]
var<uniform> point_light_count: PointLightCount;

fn fresnelSchlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (1.0 - F0) * pow(max(1.0 - cosTheta, 0.0), 5.0);
}

fn distributionGGX(N: vec3<f32>, H: vec3<f32>, roughness: f32) -> f32 {
    let a      = roughness*roughness;
    let a2     = a*a;
    let NdotH  = max(dot(N, H), 0.0);
    let NdotH2 = NdotH*NdotH;
	
    let num   = a2;
    var denom: f32 = (NdotH2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;
	
    return num / denom;
}

fn geometrySchlickGGX(NdotV: f32, roughness: f32) -> f32 {
    let r = (roughness + 1.0);
    let k = (r*r) / 8.0;

    let num   = NdotV;
    let denom = NdotV * (1.0 - k) + k;
	
    return num / denom;
}

fn geometrySmith(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, roughness: f32) -> f32 {
    let NdotV = max(dot(N, V), 0.0);
    let NdotL = max(dot(N, L), 0.0);
    let ggx2  = geometrySchlickGGX(NdotV, roughness);
    let ggx1  = geometrySchlickGGX(NdotL, roughness);
	
    return ggx1 * ggx2;
}


[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
   let tbn = mat3x3<f32>(in.tbn0, in.tbn1, in.tbn2);
   let base_tex_color = textureSample(base_texture, base_sampler, in.tex_coords);
   let metal_tex_color = textureSample(metallic_texture, metallic_sampler, in.tex_coords).bg; //.xy
   let albedo = pow(base_tex_color.rbg * material_factors.base_color_factor.rgb, vec3<f32>(2.2));
   let metallic = metal_tex_color.x * material_factors.metallic_factor;
   let roughness = metal_tex_color.y * material_factors.roughness_factor;
   let ao = textureSample(occulusion_texture, occulusion_sampler, in.tex_coords).r * material_factors.occulusion_strength;

   let pre_normal = textureSample(normal_texture, normal_sampler, in.tex_coords).rgb * material_factors.normal_scale;
   // convert between 0-1 to -1-1
   let normal = pre_normal * 2.0 - 1.0;
   let N = normalize(tbn * normal);
   let V = normalize(in.view_pos - in.frag_position);

   var F0: vec3<f32> = vec3<f32>(0.04);
   F0 = mix(F0, albedo, vec3<f32>(metallic));
   // Irradiance
   var Lo: vec3<f32> = vec3<f32>(0.0);
   var i: i32 = 0;
   loop {
       if (i >= point_light_count.count) {
           break;
       }
       let L = normalize(point_lights.inner[i].position - in.frag_position);
       let H = normalize(V + L);
       let dist = length(point_lights.inner[i].position - in.frag_position);
       let attenuation = 1.0 / (dist * dist);
       let radiance = point_lights.inner[i].color * attenuation;
       
       let F = fresnelSchlick(max(dot(H,V), 0.0), F0);
       let ndf = distributionGGX(N, H, roughness);
       let G = geometrySmith(N, V, L, roughness);

       let numerator = ndf * G * F;
       let denominator = 4.0 * max(dot(N,V), 0.0) * max(dot(N,L), 0.0);
       let specular = numerator / max(denominator, 0.001);

       let kS = F;
       let pre_kD = vec3<f32>(1.0) - kS;

       let kD = pre_kD * (1.0 - metallic);
       let NdotL = max(dot(N,L), 0.0);
       Lo = Lo + (kD * albedo / PI + specular) * radiance * NdotL;
       i = i + 1;
   }
   let ambient = vec3<f32>(0.01) * albedo * ao;
   var color: vec3<f32> = ambient + Lo;
   // HDR
   color = color / (color + vec3<f32>(1.0));
   color = pow(color, vec3<f32>(1.0/2.2));  
   return vec4<f32>(color, 1.0);
}