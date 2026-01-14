// 天空盒着色器

struct Camera {
    view_proj: mat4x4<f32>,
    view_position: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) view_dir: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var pos = array<vec3<f32>, 36>(
        vec3<f32>(-1.0,  1.0, -1.0), vec3<f32>(-1.0, -1.0, -1.0), vec3<f32>( 1.0, -1.0, -1.0),
        vec3<f32>( 1.0, -1.0, -1.0), vec3<f32>( 1.0,  1.0, -1.0), vec3<f32>(-1.0,  1.0, -1.0),
        vec3<f32>(-1.0, -1.0,  1.0), vec3<f32>(-1.0, -1.0, -1.0), vec3<f32>(-1.0,  1.0, -1.0),
        vec3<f32>(-1.0,  1.0, -1.0), vec3<f32>(-1.0,  1.0,  1.0), vec3<f32>(-1.0, -1.0,  1.0),
        vec3<f32>( 1.0, -1.0, -1.0), vec3<f32>( 1.0, -1.0,  1.0), vec3<f32>( 1.0,  1.0,  1.0),
        vec3<f32>( 1.0,  1.0,  1.0), vec3<f32>( 1.0,  1.0, -1.0), vec3<f32>( 1.0, -1.0, -1.0),
        vec3<f32>(-1.0, -1.0,  1.0), vec3<f32>( 1.0, -1.0,  1.0), vec3<f32>( 1.0,  1.0,  1.0),
        vec3<f32>( 1.0,  1.0,  1.0), vec3<f32>(-1.0,  1.0,  1.0), vec3<f32>(-1.0, -1.0,  1.0),
        vec3<f32>(-1.0,  1.0, -1.0), vec3<f32>( 1.0,  1.0, -1.0), vec3<f32>( 1.0,  1.0,  1.0),
        vec3<f32>( 1.0,  1.0,  1.0), vec3<f32>(-1.0,  1.0,  1.0), vec3<f32>(-1.0,  1.0, -1.0),
        vec3<f32>(-1.0, -1.0, -1.0), vec3<f32>(-1.0, -1.0,  1.0), vec3<f32>( 1.0, -1.0, -1.0),
        vec3<f32>( 1.0, -1.0, -1.0), vec3<f32>(-1.0, -1.0,  1.0), vec3<f32>( 1.0, -1.0,  1.0)
    );

    var out: VertexOutput;
    var p = pos[vertex_index];
    
    // 让天空盒围绕相机位置
    out.clip_position = (camera.view_proj * vec4<f32>(p + camera.view_position, 1.0)).xyww;
    out.view_dir = p;
    return out;
}

@group(1) @binding(0)
var t_skybox: texture_cube<f32>;
@group(1) @binding(1)
var s_skybox: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_skybox, s_skybox, in.view_dir).rgb;
    
    // 快速 HDR 映射
    var mapped = color / (color + vec3<f32>(1.0));

    return vec4<f32>(mapped, 1.0);
}
