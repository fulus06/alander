// SSAO 采样设置
struct SSAOUniform {
    samples: array<vec4<f32>, 64>, // 采样核
    projection: mat4x4<f32>,
    inv_projection: mat4x4<f32>,
    view: mat4x4<f32>,
    radius: f32,
    bias: f32,
    screen_size: vec2<f32>,
    _padding: vec4<f32>,
};

@group(0) @binding(0) var<uniform> ssao_uniform: SSAOUniform;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var normal_tex: texture_2d<f32>;
@group(0) @binding(3) var noise_tex: texture_2d<f32>;
@group(0) @binding(4) var s_linear: sampler;
@group(0) @binding(5) var s_nearest: sampler;
@group(0) @binding(6) var ssao_input_tex: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // 标准大三角形方案
    let uv = vec2<f32>(f32((vertex_index << 1u) & 2u), f32(vertex_index & 2u));
    out.uv = uv;
    out.clip_position = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    return out;
}

// 从深度图重建视图空间位置
fn get_view_pos(uv: vec2<f32>) -> vec3<f32> {
    let depth = textureSample(depth_tex, s_nearest, uv);
    let clip_pos = vec4<f32>(uv.x * 2.0 - 1.0, (1.0 - uv.y) * 2.0 - 1.0, depth, 1.0);
    let view_pos_h = ssao_uniform.inv_projection * clip_pos;
    return view_pos_h.xyz / view_pos_h.w;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) f32 {
    let view_pos = get_view_pos(in.uv);
    let normal = textureSample(normal_tex, s_linear, in.uv).xyz * 2.0 - 1.0;
    
    // 获取旋转噪声
    let noise_scale = ssao_uniform.screen_size / 4.0;
    let random_vec = textureSample(noise_tex, s_linear, in.uv * noise_scale).xyz * 2.0 - 1.0;

    // 构造 TBN 矩阵（Gram-Schmidt 过程）
    let tangent = normalize(random_vec - normal * dot(random_vec, normal));
    let bitangent = cross(normal, tangent);
    let tbn = mat3x3<f32>(tangent, bitangent, normal);

    var occlusion = 0.0;
    for (var i = 0; i < 64; i = i + 1) {
        // 转换采样点到视图空间
        let sample_pos_view = view_pos + tbn * ssao_uniform.samples[i].xyz * ssao_uniform.radius;
        
        // 投影到屏幕空间
        var offset = ssao_uniform.projection * vec4<f32>(sample_pos_view, 1.0);
        offset.x = offset.x / offset.w;
        offset.y = offset.y / offset.w;
        let offset_uv = vec2<f32>(offset.x * 0.5 + 0.5, 1.0 - (offset.y * 0.5 + 0.5));

        // 获取该点的实际深度
        let sample_depth = get_view_pos(offset_uv).z;
        
        // 范围检查以平滑过渡
        let range_check = smoothstep(0.0, 1.0, ssao_uniform.radius / abs(view_pos.z - sample_depth));
        if (sample_depth >= sample_pos_view.z + ssao_uniform.bias) {
            occlusion += 1.0 * range_check;
        }
    }

    return 1.0 - (occlusion / 64.0);
}

// 双向模糊（Bilateral Blur）
@fragment
fn fs_blur(in: VertexOutput) -> @location(0) f32 {
    let texel_size = 1.0 / ssao_uniform.screen_size;
    var result = 0.0;
    
    for (var x = -2; x < 2; x = x + 1) {
        for (var y = -2; y < 2; y = y + 1) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            result += textureSample(ssao_input_tex, s_linear, in.uv + offset).r;
        }
    }
    
    return result / 16.0;
}
