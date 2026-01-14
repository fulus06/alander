// 全景图转立方体贴图计算位着色器

@group(0) @binding(0)
var t_equirect: texture_2d<f32>;
@group(0) @binding(1)
var s_equirect: sampler;
@group(0) @binding(2)
var t_cube: texture_storage_2d_array<rgba32float, write>;

const PI: f32 = 3.14159265359;

// 计算给定 cubemap 面和 UV 坐标对应的世界空间方向
fn get_world_direction(face: u32, uv: vec2<f32>) -> vec3<f32> {
    let tex_coord = uv * 2.0 - 1.0;
    var dir: vec3<f32>;
    
    // WGPU 立方体贴图面顺序: +X, -X, +Y, -Y, +Z, -Z
    switch face {
        case 0u: { dir = vec3<f32>( 1.0, -tex_coord.y, -tex_coord.x); } // +X
        case 1u: { dir = vec3<f32>(-1.0, -tex_coord.y,  tex_coord.x); } // -X
        case 2u: { dir = vec3<f32>( tex_coord.x,  1.0,  tex_coord.y); } // +Y
        case 3u: { dir = vec3<f32>( tex_coord.x, -1.0, -tex_coord.y); } // -Y
        case 4u: { dir = vec3<f32>( tex_coord.x, -tex_coord.y,  1.0); } // +Z
        case 5u: { dir = vec3<f32>(-tex_coord.x, -tex_coord.y, -1.0); } // -Z
        default: { dir = vec3<f32>(0.0); }
    }
    return normalize(dir);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let face = id.z;
    let size = textureDimensions(t_cube).xy;
    
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    
    let uv = vec2<f32>(id.xy) / vec2<f32>(size - 1u);
    let dir = get_world_direction(face, uv);
    
    // 将方向转换为球面坐标 (phi, theta)
    // phi: 经度 [0, 2PI], theta: 纬度 [0, PI]
    let phi = atan2(dir.z, dir.x);
    let theta = acos(dir.y);
    
    // 映射到 [0, 1] UV 坐标
    let equirect_uv = vec2<f32>(
        (phi + PI) / (2.0 * PI),
        theta / PI
    );
    
    let color = textureSampleLevel(t_equirect, s_equirect, equirect_uv, 0.0);
    textureStore(t_cube, id.xy, face, color);
}
