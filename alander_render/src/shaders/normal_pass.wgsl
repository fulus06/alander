// 法线预传透着色器 (Normal Pre-pass)
// 用于 SSAO 计算，输出世界空间法线到纹理

struct Camera {
    view_proj: mat4x4<f32>,
    view_position: vec3<f32>,
};

struct Model {
    matrix: mat4x4<f32>,
    has_skinning: u32,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var<uniform> model: Model;

@group(1) @binding(1)
var<uniform> bones: array<mat4x4<f32>, 128>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(5) joint_indices: vec4<u32>,
    @location(6) joint_weights: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var world_matrix = model.matrix;
    
    // 处理骨骼动画
    if (model.has_skinning == 1u) {
        let skin_matrix = 
            input.joint_weights.x * bones[input.joint_indices.x] +
            input.joint_weights.y * bones[input.joint_indices.y] +
            input.joint_weights.z * bones[input.joint_indices.z] +
            input.joint_weights.w * bones[input.joint_indices.w];
        world_matrix = world_matrix * skin_matrix;
    }

    var out: VertexOutput;
    out.clip_position = camera.view_proj * world_matrix * vec4<f32>(input.position, 1.0);
    
    // 计算法线矩阵 (假设无非等比缩放)
    let normal_matrix = mat3x3<f32>(world_matrix[0].xyz, world_matrix[1].xyz, world_matrix[2].xyz);
    out.normal = normalize(normal_matrix * input.normal);
    
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 将法线从 [-1, 1] 映射到 [0, 1] 以存储在普通纹理中
    return vec4<f32>(in.normal * 0.5 + 0.5, 1.0);
}
