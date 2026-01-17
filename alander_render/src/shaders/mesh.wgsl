// PBR 网格着色器 (Physically Based Rendering)
// 实现 Cook-Torrance BRDF 模型

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
    @location(5) shadow_pos0: vec3<f32>,
    @location(6) shadow_pos1: vec3<f32>,
    @location(7) shadow_pos2: vec3<f32>,
    @location(8) shadow_pos3: vec3<f32>,
    @location(9) view_z: f32,
};

struct Camera {
    view_proj: mat4x4<f32>,
    view_position: vec3<f32>,
};

struct DirectionalLight {
    direction: vec3<f32>,
    shadow_bias: f32,
    color: vec3<f32>,
    intensity: f32,
};

struct Light {
    position: vec3<f32>,
    light_type: u32, // 0: Point, 1: Spot
    color: vec3<f32>,
    intensity: f32,
    range: f32,
    inner_angle: f32,
    outer_angle: f32,
    shadow_bias: f32,
    direction: vec3<f32>,
};

struct LightBuffer {
    dir_light: DirectionalLight,
    lights: array<Light, 4>,
    light_count: u32,
};

struct LightSpace {
    view_projs: array<mat4x4<f32>, 4>,
    split_distances: vec4<f32>,
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

// 阴影相关绑定
@group(0) @binding(5)
var t_shadow: texture_depth_2d;
@group(0) @binding(6)
var s_shadow: sampler_comparison;
@group(0) @binding(7)
var<uniform> light_space: LightSpace;
@group(0) @binding(8)
var t_shadow_cube: texture_depth_cube;
@group(0) @binding(9)
var t_ssao: texture_2d<f32>;

struct Model {
    matrix: mat4x4<f32>,
    has_skinning: u32,
};

@group(1) @binding(0)
var<uniform> model: Model;

@group(1) @binding(1)
var<uniform> bones: array<mat4x4<f32>, 128>;

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
    @location(4) joint_indices: vec4<u32>,
    @location(5) joint_weights: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;

    var skin_matrix = mat4x4<f32>(
        vec4<f32>(1.0, 0.0, 0.0, 0.0),
        vec4<f32>(0.0, 1.0, 0.0, 0.0),
        vec4<f32>(0.0, 0.0, 1.0, 0.0),
        vec4<f32>(0.0, 0.0, 0.0, 1.0)
    );

    if (model.has_skinning > 0u) {
        skin_matrix = 
            bones[joint_indices.x] * joint_weights.x +
            bones[joint_indices.y] * joint_weights.y +
            bones[joint_indices.z] * joint_weights.z +
            bones[joint_indices.w] * joint_weights.w;
    }

    let world_pos = model.matrix * skin_matrix * vec4<f32>(position, 1.0);
    out.world_position = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;
    out.uv = uv;

    // TBN
    let normal_matrix = mat3x3<f32>(
        (model.matrix * skin_matrix)[0].xyz,
        (model.matrix * skin_matrix)[1].xyz,
        (model.matrix * skin_matrix)[2].xyz
    );
    let N_world = normalize(normal_matrix * normal);
    let T_world = normalize(normal_matrix * tangent.xyz);
    let B_world = normalize(cross(N_world, T_world) * tangent.w);
    out.normal = N_world;
    out.tangent = T_world;
    out.bitangent = B_world;

    // 阴影坐标 (CSM)
    let pos0 = light_space.view_projs[0] * world_pos;
    out.shadow_pos0 = vec3<f32>(pos0.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5), pos0.z);
    
    let pos1 = light_space.view_projs[1] * world_pos;
    out.shadow_pos1 = vec3<f32>(pos1.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5), pos1.z);
    
    let pos2 = light_space.view_projs[2] * world_pos;
    out.shadow_pos2 = vec3<f32>(pos2.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5), pos2.z);
    
    let pos3 = light_space.view_projs[3] * world_pos;
    out.shadow_pos3 = vec3<f32>(pos3.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5), pos3.z);

    // 视图空间深度 (用于级联选择)
    out.view_z = out.clip_position.z; // Use clip_position.z for view space depth (after projection)

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

// Poisson Disk 样本点 (归一化范围)
const POISSON_DISK: array<vec2<f32>, 16> = array<vec2<f32>, 16>(
    vec2<f32>(-0.94201624, -0.39906216),
    vec2<f32>(0.94558609, -0.76890725),
    vec2<f32>(-0.09418410, -0.92938870),
    vec2<f32>(0.34495938, 0.29387760),
    vec2<f32>(-0.91588581, 0.45771432),
    vec2<f32>(-0.81544232, -0.87912464),
    vec2<f32>(-0.38277543, 0.27676845),
    vec2<f32>(0.97484398, 0.75648379),
    vec2<f32>(0.44323325, -0.97511554),
    vec2<f32>(0.53742981, -0.47373420),
    vec2<f32>(-0.26496911, -0.41893023),
    vec2<f32>(0.79197514, 0.19090188),
    vec2<f32>(-0.24188840, 0.99706507),
    vec2<f32>(-0.81409955, 0.91437590),
    vec2<f32>(0.19984126, 0.78641367),
    vec2<f32>(0.14383161, -0.14100790),
    // vec2<f32>(-0.62120641, -0.12933457), // Original had 18, but array size is 16. Removing last two.
    // vec2<f32>(0.65825488, 0.53754854)
);

// 阴影采样 (Poisson Disk PCF)
fn fetch_shadow(shadow_pos: vec3<f32>, bias: f32) -> f32 {
    var visibility = 0.0;
    let size = 1.0 / 2048.0; // 同步初始化时的分辨率
    let filter_radius = 2.0;

    var poisson = POISSON_DISK;
    for (var i = 0; i < 16; i++) {
        let offset = poisson[i] * size * filter_radius;
        visibility += textureSampleCompare(t_shadow, s_shadow, shadow_pos.xy + offset, shadow_pos.z - bias);
    }
    
    let result = visibility / 16.0;
    let in_bounds = shadow_pos.x >= 0.0 && shadow_pos.x <= 1.0 && shadow_pos.y >= 0.0 && shadow_pos.y <= 1.0;
    return select(1.0, result, in_bounds);
}

// 点光源阴影采样 (全向)
fn fetch_point_shadow(light_pos: vec3<f32>, world_pos: vec3<f32>, range: f32, bias: f32) -> f32 {
    let light_to_frag = world_pos - light_pos;
    let distance = length(light_to_frag);
    
    // 归一化深度
    let depth = distance / range;
    
    // 采样 CubeMap
    let sampled_depth = textureSample(t_shadow_cube, s_common, normalize(light_to_frag));
    
    return select(0.0, 1.0, sampled_depth >= depth - bias);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_common, in.uv);
    let albedo = tex_color.rgb * material.base_color.rgb;
    
    // 获取法线
    var N = normalize(in.normal);
    if (material.has_normal_map > 0u) {
        let tangent_normal = textureSample(t_normal, s_common, in.uv).rgb * 2.0 - 1.0;
        
        let TBN = mat3x3<f32>(in.tangent, in.bitangent, in.normal);
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

    // 1. 平行光计算 (阴影 + BRDF)
    if (light_buffer.dir_light.intensity > 0.0) {
        let L = normalize(-light_buffer.dir_light.direction);
        let H = normalize(V + L);
        
        // 级联选择
        var shadow_pos_selected = in.shadow_pos0;
        if (in.view_z > light_space.split_distances.x) {
            shadow_pos_selected = in.shadow_pos1;
        }
        if (in.view_z > light_space.split_distances.y) {
            shadow_pos_selected = in.shadow_pos2;
        }
        if (in.view_z > light_space.split_distances.z) {
            shadow_pos_selected = in.shadow_pos3;
        }

        let shadow = fetch_shadow(shadow_pos_selected, light_buffer.dir_light.shadow_bias);
        let radiance = light_buffer.dir_light.color * light_buffer.dir_light.intensity;

        let NDF = DistributionGGX(N, H, roughness);
        let G   = GeometrySmith(N, V, L, roughness);
        let F   = fresnelSchlick(max(dot(H, V), 0.0), F0);

        let numerator    = NDF * G * F;
        let denominator = 4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0) + 0.0001;
        let specular = numerator / denominator;

        let kS = F;
        var kD = vec3<f32>(1.0) - kS;
        kD = kD * (1.0 - metallic);

        let NdotL = max(dot(N, L), 0.0);
        Lo += (kD * albedo / PI + specular) * radiance * NdotL * shadow;
    }

    // 2. 遍历所有点光源和聚光灯
    for (var i: u32 = 0u; i < light_buffer.light_count; i = i + 1u) {
        let light = light_buffer.lights[i];
        
        let light_to_pos = light.position - in.world_position;
        let L = normalize(light_to_pos);
        let H = normalize(V + L);
        let distance = length(light_to_pos);
        
        if (distance > light.range && light.range > 0.0) {
            continue;
        }

        // 简单的距离平方反比衰减 + 范围截断
        var attenuation = 1.0 / (distance * distance + 1.0);
        
        // 聚光灯锥形衰减
        if (light.light_type == 1u) {
            let theta = dot(L, normalize(-light.direction));
            let epsilon = light.inner_angle - light.outer_angle;
            let intensity_factor = clamp((theta - light.outer_angle) / epsilon, 0.0, 1.0);
            attenuation = attenuation * intensity_factor;
        }

        let radiance = light.color * light.intensity * attenuation;

        var shadow = 1.0;
        if (light.light_type == 0u) {
            // 目前只支持第一个点光源的阴影作为演示
            if (i == 0u) {
                shadow = fetch_point_shadow(light.position, in.world_position, light.range, light.shadow_bias);
            }
        }

        // Cook-Torrance BRDF
        let NDF = DistributionGGX(N, H, roughness);   
        let G   = GeometrySmith(N, V, L, roughness);      
        let F   = fresnelSchlick(max(dot(H, V), 0.0), F0);           
        
        let numerator    = NDF * G * F; 
        let denominator = 4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0) + 0.0001;
        let specular = numerator / denominator;
        
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
    // SSAO 采样
    let ssao = textureSample(t_ssao, s_common, in.uv).r;
    let ambient = ((diffuse_ibl + specular_ibl) + vec3<f32>(0.03) * albedo) * ssao;
    
    var color = ambient + Lo + material.emissive.rgb;

    // HDR Tonemapping (Reinhard)
    color = color / (color + vec3<f32>(1.0));

    return vec4<f32>(color, tex_color.a * material.base_color.a);
}
