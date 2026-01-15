

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
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
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
    pub _padding1: f32,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub _padding2: [f32; 3],
}

impl Light {
    pub fn new(position: [f32; 3], color: [f32; 3], intensity: f32, range: f32) -> Self {
        Self {
            position,
            _padding1: 0.0,
            color,
            intensity,
            range,
            _padding2: [0.0; 3],
        }
    }
}

/// 光源缓冲区
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightBuffer {
    pub lights: [Light; 4],
    pub light_count: u32,
    pub _padding: [u32; 3],
}

impl LightBuffer {
    pub fn new() -> Self {
        Self {
            lights: [
                Light::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 0.0, 0.0),
                Light::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 0.0, 0.0),
                Light::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 0.0, 0.0),
                Light::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 0.0, 0.0),
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
}

impl ModelBuffer {
    pub fn new(model: cgmath::Matrix4<f32>) -> Self {
        Self {
            model: model.into(),
        }
    }
}
