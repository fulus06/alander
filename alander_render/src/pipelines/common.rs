

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

/// 顶点结构
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub tangent: [f32; 4],
    pub joint_indices: [u32; 4],
    pub joint_weights: [f32; 4],
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 24, shader_location: 2, format: wgpu::VertexFormat::Float32x2 },
                wgpu::VertexAttribute { offset: 32, shader_location: 3, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: 48, shader_location: 4, format: wgpu::VertexFormat::Uint32x4 },
                wgpu::VertexAttribute { offset: 64, shader_location: 5, format: wgpu::VertexFormat::Float32x4 },
            ],
        }
    }
}

/// 相机缓冲区
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraBuffer {
    view_proj: [[f32; 4]; 4],
    position: [f32; 3],
    _padding: u32,
}

impl CameraBuffer {
    pub fn new(view: cgmath::Matrix4<f32>, proj: cgmath::Matrix4<f32>, position: [f32; 3]) -> Self {
        let view_proj = OPENGL_TO_WGPU_MATRIX * proj * view;
        Self {
            view_proj: view_proj.into(),
            position,
            _padding: 0,
        }
    }
}

/// 光源结构
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Light {
    pub position: [f32; 3],
    pub light_type: u32, // 0: Point, 1: Spot
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub inner_angle: f32,
    pub outer_angle: f32,
    pub shadow_bias: f32,
    pub direction: [f32; 3],
    pub _padding: f32,
}

impl Light {
    pub fn point(position: [f32; 3], color: [f32; 3], intensity: f32, range: f32) -> Self {
        Self {
            position,
            light_type: 0,
            color,
            intensity,
            range,
            inner_angle: 0.0,
            outer_angle: 0.0,
            shadow_bias: 0.0,
            direction: [0.0, -1.0, 0.0],
            _padding: 0.0,
        }
    }

    pub fn spot(position: [f32; 3], color: [f32; 3], intensity: f32, range: f32, direction: [f32; 3], inner: f32, outer: f32, bias: f32) -> Self {
        Self {
            position,
            light_type: 1,
            color,
            intensity,
            range,
            inner_angle: inner,
            outer_angle: outer,
            shadow_bias: bias,
            direction,
            _padding: 0.0,
        }
    }
}

/// 平行光结构 (用于渲染)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DirectionalLight {
    pub direction: [f32; 3],
    pub shadow_bias: f32,
    pub color: [f32; 3],
    pub intensity: f32,
    pub shadow_normal_bias: f32,
    pub _padding: [f32; 3],
}

impl DirectionalLight {
    pub fn new(direction: [f32; 3], color: [f32; 3], intensity: f32, bias: f32, normal_bias: f32) -> Self {
        Self {
            direction,
            shadow_bias: bias,
            color,
            intensity,
            shadow_normal_bias: normal_bias,
            _padding: [0.0; 3],
        }
    }
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            direction: [0.0, -1.0, 0.0],
            shadow_bias: 0.005,
            color: [1.0, 1.0, 1.0],
            intensity: 0.0,
            shadow_normal_bias: 0.01,
            _padding: [0.0; 3],
        }
    }
}

/// 光源缓冲区
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightBuffer {
    pub dir_light: DirectionalLight,
    pub lights: [Light; 4],
    pub light_count: u32,
    pub _padding: [u32; 3], // 保持 16 字节对齐
}

impl LightBuffer {
    pub fn new() -> Self {
        Self {
            dir_light: DirectionalLight::default(),
            lights: [
                Light::point([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 0.0, 0.0),
                Light::point([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 0.0, 0.0),
                Light::point([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 0.0, 0.0),
                Light::point([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 0.0, 0.0),
            ],
            light_count: 0,
            _padding: [0, 0, 0],
        }
    }

    pub fn add_light(&mut self, light: Light) -> bool {
        if self.light_count < 4 {
            self.lights[self.light_count as usize] = light;
            self.light_count += 1;
            true
        } else {
            false
        }
    }

    pub fn clear_lights(&mut self) {
        self.light_count = 0;
    }

    pub fn set_light(&mut self, index: usize, light: Light) -> bool {
        if index < 4 {
            self.lights[index] = light;
            true
        } else {
            false
        }
    }
}

/// 材质参数缓冲区
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialBuffer {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub has_normal_texture: u32,
    pub has_metallic_roughness_texture: u32,
    pub emissive: [f32; 4],
}

impl Default for MaterialBuffer {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            has_normal_texture: 0,
            has_metallic_roughness_texture: 0,
            emissive: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

/// 模型缓冲区
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelBuffer {
    pub model: [[f32; 4]; 4],
    pub has_skinning: u32,
    pub _padding: [u32; 3],
}

impl ModelBuffer {
    pub fn new(model: cgmath::Matrix4<f32>) -> Self {
        Self {
            model: model.into(),
            has_skinning: 0,
            _padding: [0; 3],
        }
    }

    pub fn with_skinning(model: cgmath::Matrix4<f32>, has_skinning: bool) -> Self {
        Self {
            model: model.into(),
            has_skinning: if has_skinning { 1 } else { 0 },
            _padding: [0; 3],
        }
    }
}

/// 骨骼矩阵 Buffer (最大支持 128 根骨骼)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BoneBuffer {
    pub matrices: [[[f32; 4]; 4]; 128],
}

/// 光空间缓冲区 (用于阴影映射 - 支持 CSM)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightSpaceBuffer {
    pub view_projs: [[[f32; 4]; 4]; 4],
    pub split_distances: [f32; 4],
}

impl LightSpaceBuffer {
    pub fn new(view_projs: [cgmath::Matrix4<f32>; 4], splits: [f32; 4]) -> Self {
        let mut vp = [[[0.0; 4]; 4]; 4];
        for i in 0..4 {
            vp[i] = (OPENGL_TO_WGPU_MATRIX * view_projs[i]).into();
        }
        Self {
            view_projs: vp,
            split_distances: splits,
        }
    }
}
