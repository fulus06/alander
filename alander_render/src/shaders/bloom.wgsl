struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(i32(in_vertex_index << 1u) & 2) * 2.0 - 1.0;
    let y = f32(i32(in_vertex_index & 2u)) * 2.0 - 1.0;
    out.uv = vec2<f32>(x * 0.5 + 0.5, 1.0 - (y * 0.5 + 0.5));
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    return out;
}

@group(0) @binding(0)
var t_input: texture_2d<f32>;
@group(0) @binding(1)
var s_input: sampler;

struct BloomSettings {
    threshold: f32,
    intensity: f32,
}
@group(0) @binding(2)
var<uniform> settings: BloomSettings;

// 1. 亮度提取片元着色器
@fragment
fn fs_extract(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_input, s_input, in.uv).rgb;
    // 使用相对亮度公式计算
    let brightness = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    
    if (brightness > settings.threshold) {
        return vec4<f32>(color, 1.0);
    } else {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
}

// 2. 高斯模糊片元着色器 (分离式: 水平/垂直)
@fragment
fn fs_blur_h(in: VertexOutput) -> @location(0) vec4<f32> {
    var weight = array<f32, 5>(0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);
    let tex_offset = 1.0 / vec2<f32>(textureDimensions(t_input));
    
    var result = textureSample(t_input, s_input, in.uv).rgb * weight[0];
    for(var i = 1; i < 5; i = i + 1) {
        result += textureSample(t_input, s_input, in.uv + vec2<f32>(tex_offset.x * f32(i), 0.0)).rgb * weight[i];
        result += textureSample(t_input, s_input, in.uv - vec2<f32>(tex_offset.x * f32(i), 0.0)).rgb * weight[i];
    }
    return vec4<f32>(result, 1.0);
}

@fragment
fn fs_blur_v(in: VertexOutput) -> @location(0) vec4<f32> {
    var weight = array<f32, 5>(0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);
    let tex_offset = 1.0 / vec2<f32>(textureDimensions(t_input));
    
    var result = textureSample(t_input, s_input, in.uv).rgb * weight[0];
    for(var i = 1; i < 5; i = i + 1) {
        result += textureSample(t_input, s_input, in.uv + vec2<f32>(0.0, tex_offset.y * f32(i))).rgb * weight[i];
        result += textureSample(t_input, s_input, in.uv - vec2<f32>(0.0, tex_offset.y * f32(i))).rgb * weight[i];
    }
    return vec4<f32>(result, 1.0);
}
