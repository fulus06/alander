struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

struct ModelUniform {
    model_matrix: mat4x4<f32>,
    has_skinning: u32,
};

struct LightSpaceUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> light_space: LightSpaceUniform;

@group(1) @binding(0)
var<uniform> model: ModelUniform;

@group(1) @binding(1)
var<uniform> bones: array<mat4x4<f32>, 128>;

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
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

    out.clip_position = light_space.view_proj * model.model_matrix * skin_matrix * vec4<f32>(position, 1.0);
    return out;
}

// 片元着色器为空，因为我们只关心深度
@fragment
fn fs_main() {}
