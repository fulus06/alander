//! 渲染管线模块
//!
//! 此模块包含所有渲染管线的定义和管理。

use crate::utils;
use cgmath::SquareMatrix;
use wgpu::util::DeviceExt;

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

/// 管线集合
pub struct Pipelines {
    /// 基础网格管线
    pub mesh: MeshPipeline,
    /// 天空盒管线
    pub skybox: SkyboxPipeline,
}

impl Pipelines {
    /// 创建所有渲染管线
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
        let mesh = MeshPipeline::new(device, config.format);
        let skybox = SkyboxPipeline::new(device, config.format);

        Self { mesh, skybox }
    }
}

/// 基础网格渲染管线
pub struct MeshPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    pub model_bind_group_layout: wgpu::BindGroupLayout,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
    pub material_bind_group_layout: wgpu::BindGroupLayout,
}

impl MeshPipeline {
    /// 创建新的网格管线
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        // 着色器
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("网格着色器"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/mesh.wgsl").into()),
        });

        // 相机绑定组布局
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("相机绑定组布局"),
                entries: &[
                    // 相机绑定
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // 光源绑定
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // IBL 辐照度图 (Irradiance)
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::Cube,
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        },
                        count: None,
                    },
                    // IBL 环境反射图 (Environment)
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::Cube,
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        },
                        count: None,
                    },
                    // IBL 统一采样器
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                ],
            });

        // 模型绑定组布局
        let model_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("模型绑定组布局"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // 纹理绑定组布局
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("纹理绑定组布局"),
                entries: &[
                    // 漫反射贴图
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    // 法线贴图
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    // 金属度/粗糙度贴图
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    // 统一采样器
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // 材质绑定组布局
        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("材质绑定组布局"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // 管线布局
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("网格管线布局"),
            bind_group_layouts: &[
                &camera_bind_group_layout,
                &model_bind_group_layout,
                &texture_bind_group_layout,
                &material_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        // 渲染管线
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("网格渲染管线"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        Self {
            pipeline,
            camera_bind_group_layout,
            model_bind_group_layout,
            texture_bind_group_layout,
            material_bind_group_layout,
        }
    }
}

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
                // 位置
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // 法线
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // UV
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // 切线
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
    /// 从视图和投影矩阵创建相机缓冲区
    pub fn new(view: cgmath::Matrix4<f32>, proj: cgmath::Matrix4<f32>, position: [f32; 3]) -> Self {
        let view_proj = OPENGL_TO_WGPU_MATRIX * proj * view;
        Self {
            view_proj: view_proj.into(),
            position,
            _padding: 0,
        }
    }
}

impl MeshPipeline {
    /// 获取模型绑定组布局
    pub fn model_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.model_bind_group_layout
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
    /// 创建新的光源
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
    /// 创建新的光源缓冲区
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

    /// 添加光源
    pub fn add_light(&mut self, light: Light) -> bool {
        if self.light_count < 4 {
            self.lights[self.light_count as usize] = light;
            self.light_count += 1;
            true
        } else {
            false
        }
    }

    /// 清除所有光源
    pub fn clear_lights(&mut self) {
        self.light_count = 0;
    }

    /// 设置单个光源
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
    model: [[f32; 4]; 4],
}

impl ModelBuffer {
    /// 从模型矩阵创建模型缓冲区
    pub fn new(model: cgmath::Matrix4<f32>) -> Self {
        Self {
            model: model.into(),
        }
    }
}

/// 场景对象
pub struct SceneObject {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub model_buffer: wgpu::Buffer,
    pub material_buffer: wgpu::Buffer,
    pub model_bind_group: wgpu::BindGroup,
    pub texture_bind_group: wgpu::BindGroup,
    pub material_bind_group: wgpu::BindGroup,
}

impl SceneObject {
    /// 创建新场景对象
    pub fn new(
        device: &wgpu::Device,
        vertices: &[Vertex],
        indices: &[u32],
        model_bind_group_layout: &wgpu::BindGroupLayout,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        material_bind_group_layout: &wgpu::BindGroupLayout,
        diffuse_texture: &crate::texture::Texture,
        normal_texture: &crate::texture::Texture,
        mr_texture: &crate::texture::Texture,
        transform: glam::Mat4,
        material: MaterialBuffer,
    ) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("顶点缓冲区"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("索引缓冲区"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let num_elements = indices.len() as u32;

        // 转换 glam::Mat4 到 [f32; 16] 供 bytemuck 使用
        let transform_array: [[f32; 4]; 4] = transform.to_cols_array_2d();
        let matrix = cgmath::Matrix4::from(transform_array);

        // 创建模型缓冲区
        let model_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("模型缓冲区"),
            contents: bytemuck::bytes_of(&ModelBuffer::new(matrix)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // 创建模型绑定组
        let model_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("模型绑定组"),
            layout: model_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: model_buffer.as_entire_binding(),
            }],
        });

        // 创建纹理绑定组
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("纹理绑定组"),
            layout: texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&normal_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&mr_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
        });

        // 创建材质参数缓冲区
        let material_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("材质缓冲区"),
            contents: bytemuck::bytes_of(&material),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // 创建材质绑定组
        let material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("材质绑定组"),
            layout: material_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: material_buffer.as_entire_binding(),
            }],
        });

        Self {
            vertex_buffer,
            index_buffer,
            num_elements,
            model_buffer,
            material_buffer,
            model_bind_group,
            texture_bind_group,
            material_bind_group,
        }
    }

    /// 更新模型矩阵
    pub fn update_model(&self, queue: &wgpu::Queue, model: cgmath::Matrix4<f32>) {
        let model_buffer = ModelBuffer::new(model);
        queue.write_buffer(&self.model_buffer, 0, bytemuck::bytes_of(&model_buffer));
    }

    /// 渲染对象
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.set_bind_group(1, &self.model_bind_group, &[]);
        render_pass.set_bind_group(2, &self.texture_bind_group, &[]);
        render_pass.set_bind_group(3, &self.material_bind_group, &[]);
        render_pass.draw_indexed(0..self.num_elements, 0, 0..1);
    }
}

/// 天空盒渲染管线
pub struct SkyboxPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
}

impl SkyboxPipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        // 着色器
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("天空盒着色器"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/skybox.wgsl").into()),
        });

        // 相机绑定组布局
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("天空盒相机布局"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // 纹理绑定组布局
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("天空盒纹理布局"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::Cube,
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                ],
            });

        // 管线布局
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("天空盒管线布局"),
            bind_group_layouts: &[&camera_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        // 渲染管线
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("天空盒渲染管线"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None, // 天空盒我们从内部看，关闭背面剔除或小心处理
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            pipeline,
            camera_bind_group_layout,
            texture_bind_group_layout,
        }
    }
}
