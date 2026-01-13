// 基础网格着色器 - 简化版本
// 仅包含最基本的顶点和片段着色器

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct Camera {
    view_proj: mat4x4<f32>,
    position: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var<uniform> model: mat4x4<f32>;

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;

    // 世界空间位置
    out.world_position = (model * vec4<f32>(position, 1.0)).xyz;

    // 世界空间法线
    out.world_normal = normalize((model * vec4<f32>(normal, 0.0)).xyz);

    // 裁剪空间位置
    out.clip_position = camera.view_proj * vec4<f32>(out.world_position, 1.0);

    // 传递UV
    out.uv = uv;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 简化的朗伯光照模型
    let albedo = vec3<f32>(0.8, 0.3, 0.2); // 基础颜色

    // 简单的方向光
    let light_dir = normalize(vec3<f32>(1.0, 1.0, 0.5));
    let n = normalize(in.world_normal);

    // 环境光
    let ambient = vec3<f32>(0.1);

    // 漫反射
    let diff = max(dot(n, light_dir), 0.0);
    let diffuse = diff * albedo;

    // 组合颜色
    let color = ambient + diffuse;

    return vec4<f32>(color, 1.0);
}
