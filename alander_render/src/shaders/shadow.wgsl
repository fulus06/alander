struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

struct ModelUniform {
    model_matrix: mat4x4<f32>,
};

struct LightSpaceUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> light_space: LightSpaceUniform;

@group(1) @binding(0)
var<uniform> model: ModelUniform;

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = light_space.view_proj * model.model_matrix * vec4<f32>(position, 1.0);
    return out;
}

// 片元着色器为空，因为我们只关心深度
@fragment
fn fs_main() {}
