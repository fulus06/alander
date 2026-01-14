// PBR 网格着色器 (Physically Based Rendering)
// 实现 Cook-Torrance BRDF 模型

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) world_tangent: vec4<f32>,
};

struct Camera {
    view_proj: mat4x4<f32>,
    view_position: vec3<f32>,
};

struct Light {
    position: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
    range: f32,
};

struct LightBuffer {
    lights: array<Light, 4>,
    light_count: u32,
};

@group(0) @binding(0)
var<uniform> camera: Camera;
@group(0) @binding(1)
var<uniform> light_buffer: LightBuffer;
@group(0) @binding(2)
var t_irradiance: texture_cube<f32>;
@group(0) @binding(3)
var t_prefilter: texture_cube<f32>;
@group(0) @binding(4)
var s_ibl: sampler;

@group(1) @binding(0)
var<uniform> model: mat4x4<f32>;

@group(2) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(2) @binding(1)
var t_normal: texture_2d<f32>;
@group(2) @binding(2)
var t_metallic_roughness: texture_2d<f32>;
@group(2) @binding(3)
var s_common: sampler;

struct Material {
    base_color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    has_normal_map: u32,
    has_metallic_roughness_map: u32,
    emissive: vec4<f32>,
};

@group(3) @binding(0)
var<uniform> material: Material;

const PI: f32 = 3.14159265359;

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = model * vec4<f32>(position, 1.0);
    out.world_position = world_pos.xyz;
    out.world_normal = normalize((model * vec4<f32>(normal, 0.0)).xyz);
    out.world_tangent = vec4<f32>(normalize((model * vec4<f32>(tangent.xyz, 0.0)).xyz), tangent.w);
    out.clip_position = camera.view_proj * world_pos;
    out.uv = uv;

    return out;
}

// ----------------------------------------------------------------------------
// PBR 函数
// ----------------------------------------------------------------------------

// Trowbridge-Reitz GGX (法线分布函数)
fn DistributionGGX(N: vec3<f32>, H: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let NdotH = max(dot(N, H), 0.0);
    let NdotH2 = NdotH * NdotH;

    let num = a2;
    var denom = (NdotH2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;

    return num / denom;
}

// Smith's Schlick-GGX (几何函数)
fn GeometrySchlickGGX(NdotV: f32, roughness: f32) -> f32 {
    let r = (roughness + 1.0);
    let k = (r * r) / 8.0;

    let num = NdotV;
    let denom = NdotV * (1.0 - k) + k;

    return num / denom;
}

fn GeometrySmith(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, roughness: f32) -> f32 {
    let NdotV = max(dot(N, V), 0.0);
    let NdotL = max(dot(N, L), 0.0);
    let ggx2 = GeometrySchlickGGX(NdotV, roughness);
    let ggx1 = GeometrySchlickGGX(NdotL, roughness);

    return ggx1 * ggx2;
}

// Fresnel-Schlick (菲涅尔方程)
fn fresnelSchlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_common, in.uv);
    let albedo = tex_color.rgb * material.base_color.rgb;
    
    // 获取法线
    var N = normalize(in.world_normal);
    if (material.has_normal_map > 0u) {
        let tangent_normal = textureSample(t_normal, s_common, in.uv).rgb * 2.0 - 1.0;
        
        let T = normalize(in.world_tangent.xyz);
        let B = normalize(cross(N, T) * in.world_tangent.w);
        let TBN = mat3x3<f32>(T, B, N);
        N = normalize(TBN * tangent_normal);
    }
    
    let V = normalize(camera.view_position - in.world_position);

    // 获取金属度和粗糙度
    var metallic = material.metallic;
    var roughness = material.roughness;
    if (material.has_metallic_roughness_map > 0u) {
        let mr_sample = textureSample(t_metallic_roughness, s_common, in.uv);
        // glTF 标准：金属度在 B 通道，粗糙度在 G 通道
        metallic = metallic * mr_sample.b;
        roughness = roughness * mr_sample.g;
    }

    // 基础反射率模型：非金属 0.04，金属使用 albedo
    var F0 = vec3<f32>(0.04); 
    F0 = mix(F0, albedo, metallic);

    var Lo = vec3<f32>(0.0);
    
    // 遍历所有光源
    for (var i: u32 = 0u; i < light_buffer.light_count; i = i + 1u) {
        let light = light_buffer.lights[i];
        
        // 计算光照方向和距离衰减
        let L = normalize(light.position - in.world_position);
        let H = normalize(V + L);
        let distance = length(light.position - in.world_position);
        
        // 简单的距离平方反比衰减 + 范围截断
        let attenuation = 1.0 / (distance * distance);
        let radiance = light.color * light.intensity * attenuation;

        // Cook-Torrance BRDF
        let NDF = DistributionGGX(N, H, roughness);   
        let G   = GeometrySmith(N, V, L, roughness);      
        let F   = fresnelSchlick(max(dot(H, V), 0.0), F0);           
        
        let numerator    = NDF * G * F; 
        let denominator = 4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0) + 0.0001; // 防止除以0
        let specular = numerator / denominator;
        
        // kS 是镜面反射部分，kD 是漫反射部分
        let kS = F;
        var kD = vec3<f32>(1.0) - kS;
        kD = kD * (1.0 - metallic);	  
            
        let NdotL = max(dot(N, L), 0.0);        

        Lo = Lo + (kD * albedo / PI + specular) * radiance * NdotL;
    }   
    
    // 环境光部分 - IBL
    let irradiance = textureSample(t_irradiance, s_ibl, N).rgb;
    let kS = fresnelSchlick(max(dot(N, V), 0.0), F0);
    let kD = (vec3<f32>(1.0) - kS) * (1.0 - metallic);
    let diffuse_ibl = irradiance * albedo * kD;
    
    // IBL 镜面反射 (环境反射) - 包含粗糙度对应的 Mip 采样
    let R = reflect(-V, N);
    // 假设 prefilter map 有 5 级 mip (0 到 4)
    let prefilteredColor = textureSampleLevel(t_prefilter, s_ibl, R, roughness * 4.0).rgb;
    let specular_ibl = prefilteredColor * (kS * 0.5 + 0.5); // 简化版菲涅尔反射

    // 即使没有 IBL 贴图，也保证一点基础环境光
    let ambient = (diffuse_ibl + specular_ibl) + vec3<f32>(0.03) * albedo;
    
    var color = ambient + Lo + material.emissive.rgb;

    // HDR Tonemapping (Reinhard)
    color = color / (color + vec3<f32>(1.0));

    return vec4<f32>(color, tex_color.a * material.base_color.a);
}
