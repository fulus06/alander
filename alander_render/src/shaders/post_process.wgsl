struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;
    // 生成一个覆盖全屏的三角形
    let x = f32(i32(in_vertex_index << 1u) & 2) * 2.0 - 1.0;
    let y = f32(i32(in_vertex_index & 2u)) * 2.0 - 1.0;
    out.uv = vec2<f32>(x * 0.5 + 0.5, 1.0 - (y * 0.5 + 0.5));
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;
@group(0) @binding(2)
var t_bloom: texture_2d<f32>;

struct BloomSettings {
    threshold: f32,
    intensity: f32,
}
@group(0) @binding(3)
var<uniform> settings: BloomSettings;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let raw_color = textureSample(t_diffuse, s_diffuse, in.uv).rgb;
    let bloom_color = textureSample(t_bloom, s_diffuse, in.uv).rgb;
    
    // 合成 HDR 颜色与泛光
    let hdr_color = raw_color + bloom_color * settings.intensity;
    
    // 1. Reinhard Tone Mapping
    let mapped = hdr_color / (hdr_color + vec3<f32>(1.0));
    
    // 2. Gamma Correction
    let final_color = pow(mapped, vec3<f32>(1.0 / 2.2));
    
    return vec4<f32>(final_color, 1.0);
}
