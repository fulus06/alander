//! 渲染器核心实现
//!
//! 此模块包含渲染器的主要功能和渲染循环。

use super::RenderError;
use crate::pipelines::{CameraBuffer, Pipelines, SceneObject, Vertex};
use alander_core::scene::{Camera as CoreCamera, Transform};
use cgmath::SquareMatrix;
use cgmath::{Matrix4, Point3, Vector3};
use std::collections::HashMap;
use wgpu::util::DeviceExt;
use winit::window::Window;
use image;

/// 渲染器
pub struct Renderer {
    /// WGPU 渲染器实例
    renderer: super::Renderer,
    /// 深度纹理视图
    depth_view: wgpu::TextureView,
    /// 相机缓冲区
    camera_buffer: wgpu::Buffer,
    /// 相机与光源绑定组
    camera_bind_group: wgpu::BindGroup,
    /// 光源缓冲区
    light_buffer: wgpu::Buffer,
    /// 渲染管线
    pipelines: Pipelines,
    /// 场景对象
    objects: HashMap<uuid::Uuid, SceneObject>,
    // 默认纹理
    default_texture: crate::texture::Texture,
    // 纹理集
    textures: Vec<crate::texture::Texture>,
}

impl Renderer {
    /// 创建新的渲染器
    pub async fn new(window: &Window) -> Result<Self, RenderError> {
        let renderer = super::Renderer::new(window, super::RendererConfig::default()).await?;

        // 创建白色的 1x1 默认纹理
        let default_img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 255])));
        let default_texture = crate::texture::Texture::from_image(
            renderer.device(),
            renderer.queue(),
            &default_img,
            Some("默认白纹理")
        ).map_err(|_| RenderError::RequestDevice)?; // Reuse error for simplicity

        // 创建深度纹理
        let depth_view =
            Self::create_depth_texture(renderer.device(), renderer.config(), "深度纹理");

        // 创建相机缓冲区
        let camera_buffer =
            renderer
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("相机缓冲区"),
                    contents: bytemuck::bytes_of(&CameraBuffer::new(
                        Matrix4::identity(),
                        Matrix4::identity(),
                        [0.0, 0.0, 0.0],
                    )),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

        // 创建相机绑定组
        // 创建光源缓冲区
        let light_buffer =
            renderer
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("光源缓冲区"),
                    contents: bytemuck::bytes_of(&crate::pipelines::LightBuffer::new()),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

        // 创建相机绑定组
        let camera_bind_group = renderer
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("相机及光源绑定组"),
                layout: &renderer.pipelines.mesh.camera_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: camera_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: light_buffer.as_entire_binding(),
                    },
                ],
            });

        // 创建渲染管线
        let pipelines = Pipelines::new(renderer.device(), renderer.config());

        Ok(Self {
            renderer,
            depth_view,
            camera_buffer,
            camera_bind_group,
            pipelines,
            objects: HashMap::new(),
            light_buffer,
            default_texture,
            textures: Vec::new(),
        })
    }

    /// 调整大小
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.renderer.resize(new_size);

        // 重新创建深度纹理
        let depth_view =
            Self::create_depth_texture(self.renderer.device(), self.renderer.config(), "深度纹理");
        self.depth_view = depth_view;
    }

    /// 更新相机
    pub fn update_camera(&mut self, camera: &CoreCamera, transform: &Transform) {
        let view = Self::calc_view_matrix(transform);
        let proj = Self::calc_proj_matrix(camera);
        let camera_buffer = CameraBuffer::new(view, proj, transform.position.into());

        // 调试信息：相机更新
        // tracing::debug!("更新相机: 位置 {:?}, 视图矩阵: {:?}, 投影矩阵: {:?}", transform.position, view, proj);

        self.renderer.queue().write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&camera_buffer),
        );
    }

    /// 获取渲染管线
    pub fn pipelines(&self) -> &Pipelines {
        &self.pipelines
    }

    /// 更新光源
    pub fn update_lights(&mut self, lights: &crate::pipelines::LightBuffer) {
        self.renderer.queue().write_buffer(
            &self.light_buffer,
            0,
            bytemuck::bytes_of(lights),
        );
    }

    /// 获取 WGPU 设备
    pub fn device(&self) -> &wgpu::Device {
        self.renderer.device()
    }

    /// 获取 WGPU 队列
    pub fn queue(&self) -> &wgpu::Queue {
        self.renderer.queue()
    }

    /// 获取默认纹理
    pub fn default_texture(&self) -> &crate::texture::Texture {
        &self.default_texture
    }

    /// 渲染场景
    pub fn render_scene(&self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        // 创建渲染通道
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("渲染通道"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        // 设置渲染管线和绑定组
        render_pass.set_pipeline(&self.pipelines.mesh.pipeline);
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

        // 渲染场景对象
        for object in self.objects.values() {
            object.render(&mut render_pass);
        }
    }

    /// 添加 glTF 模型
    /// 添加 glTF 模型
    pub fn add_gltf_model(&mut self, model: alander_core::assets::GltfModel) -> Vec<uuid::Uuid> {
        let image_to_texture = self.load_gltf_textures(&model);
        let mut object_ids = Vec::new();

        // 2. 将 glTF 网格转换为场景对象
        for gltf_mesh in &model.meshes {
            let diffuse_texture = self.get_diffuse_texture_for_gltf(&model, gltf_mesh, &image_to_texture);

            let scene_object = SceneObject::new(
                self.renderer.device(),
                &gltf_mesh.data.vertices.iter().map(|v| crate::pipelines::Vertex {
                    position: v.position.into(),
                    normal: v.normal.into(),
                    uv: v.uv.into(),
                }).collect::<Vec<_>>(),
                &gltf_mesh.data.indices,
                self.pipelines.mesh.model_bind_group_layout(),
                &self.pipelines.mesh.texture_bind_group_layout,
                &self.pipelines.mesh.material_bind_group_layout,
                diffuse_texture,
                gltf_mesh.transform,
                crate::pipelines::MaterialBuffer::default(),
            );

            let id = uuid::Uuid::new_v4();
            self.objects.insert(id, scene_object);
            object_ids.push(id);
        }

        object_ids
    }

    /// 加载 glTF 模型中的所有纹理并返回索引映射
    pub fn load_gltf_textures(&mut self, model: &alander_core::assets::GltfModel) -> HashMap<usize, usize> {
        let mut image_to_texture = HashMap::new();
        
        for (i, img) in model.images.iter().enumerate() {
            if let Ok(texture) = crate::texture::Texture::from_image(
                self.renderer.device(),
                self.renderer.queue(),
                &img,
                Some(&format!("glTF 纹理 {}", i)),
            ) {
                image_to_texture.insert(i, self.textures.len());
                self.textures.push(texture);
            }
        }
        image_to_texture
    }

    /// 根据 glTF 模型及网格获取对应的漫反射贴图
    pub fn get_diffuse_texture_for_gltf<'a>(
        &'a self, 
        model: &alander_core::assets::GltfModel, 
        mesh: &alander_core::assets::GltfMesh,
        image_to_texture: &HashMap<usize, usize>
    ) -> &'a crate::texture::Texture {
        if let Some(mat_idx) = mesh.material_index {
            if let Some(material) = model.materials.get(mat_idx) {
                if let Some(img_idx_str) = &material.base_color_texture {
                    if let Ok(img_idx) = img_idx_str.parse::<usize>() {
                        if let Some(&texture_idx) = image_to_texture.get(&img_idx) {
                            return &self.textures[texture_idx];
                        }
                    }
                }
            }
        }
        &self.default_texture
    }

    /// 添加场景对象
    pub fn add_object(&mut self, id: uuid::Uuid, object: SceneObject) {
        // 调试信息
        // tracing::debug!("添加场景对象: {:?}", id);
        self.objects.insert(id, object);
    }

    /// 移除场景对象
    pub fn remove_object(&mut self, id: &uuid::Uuid) -> Option<SceneObject> {
        self.objects.remove(id)
    }

    /// 获取表面
    pub fn surface(&self) -> &wgpu::Surface {
        self.renderer.surface()
    }

    /// 获取配置
    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        self.renderer.config()
    }

    /// 获取表面格式
    pub fn format(&self) -> wgpu::TextureFormat {
        self.renderer.format()
    }

    /// 获取窗口大小
    pub fn size(&self) -> winit::dpi::PhysicalSize<u32> {
        self.renderer.size()
    }

    /// 更新对象模型矩阵与材质
    pub fn update_object_model_material(
        &mut self, 
        object_id: &uuid::Uuid, 
        model: cgmath::Matrix4<f32>,
        material: Option<crate::pipelines::MaterialBuffer>,
    ) {
        if let Some(object) = self.objects.get_mut(object_id) {
            object.update_model(self.renderer.queue(), model);
            if let Some(mat) = material {
                self.renderer.queue().write_buffer(&object.material_buffer, 0, bytemuck::bytes_of(&mat));
            }
        }
    }

    /// 创建深度纹理
    fn create_depth_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        label: &str,
    ) -> wgpu::TextureView {
        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        let texture = device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        view
    }

    /// 计算视图矩阵
    fn calc_view_matrix(transform: &Transform) -> Matrix4<f32> {
        let position = Point3::new(
            transform.position.x,
            transform.position.y,
            transform.position.z,
        );
        let rotation = cgmath::Quaternion::new(
            transform.rotation.w,
            transform.rotation.x,
            transform.rotation.y,
            transform.rotation.z,
        );

        // 在右手坐标系中，相机默认沿着负Z轴看
        // 旋转四元数将标准前向向量(-Z)旋转到相机的实际前向
        let forward = rotation * -Vector3::unit_z();
        let up = rotation * Vector3::unit_y();

        Matrix4::look_at_rh(position, position + forward, up)
    }

    /// 计算投影矩阵
    fn calc_proj_matrix(camera: &CoreCamera) -> Matrix4<f32> {
        match &camera.projection {
            alander_core::scene::Projection::Perspective(p) => cgmath::perspective(
                cgmath::Deg(p.fov_y * 180.0 / std::f32::consts::PI),
                p.aspect_ratio,
                p.near,
                p.far,
            ),
        }
    }
}

pub fn create_cube(
    device: &wgpu::Device,
    model_bind_group_layout: &wgpu::BindGroupLayout,
    texture_bind_group_layout: &wgpu::BindGroupLayout,
    material_bind_group_layout: &wgpu::BindGroupLayout,
    diffuse_texture: &crate::texture::Texture,
) -> SceneObject {
    // 立方体顶点数据
    let vertices = &[
        // 前面
        Vertex {
            position: [-0.5, -0.5, 0.5],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 0.0],
        },
        Vertex {
            position: [0.5, -0.5, 0.5],
            normal: [0.0, 0.0, 1.0],
            uv: [1.0, 0.0],
        },
        Vertex {
            position: [0.5, 0.5, 0.5],
            normal: [0.0, 0.0, 1.0],
            uv: [1.0, 1.0],
        },
        Vertex {
            position: [-0.5, 0.5, 0.5],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 1.0],
        },
        // 后面
        Vertex {
            position: [-0.5, -0.5, -0.5],
            normal: [0.0, 0.0, -1.0],
            uv: [0.0, 0.0],
        },
        Vertex {
            position: [0.5, -0.5, -0.5],
            normal: [0.0, 0.0, -1.0],
            uv: [1.0, 0.0],
        },
        Vertex {
            position: [0.5, 0.5, -0.5],
            normal: [0.0, 0.0, -1.0],
            uv: [1.0, 1.0],
        },
        Vertex {
            position: [-0.5, 0.5, -0.5],
            normal: [0.0, 0.0, -1.0],
            uv: [0.0, 1.0],
        },
        // 左面
        Vertex {
            position: [-0.5, -0.5, -0.5],
            normal: [-1.0, 0.0, 0.0],
            uv: [0.0, 0.0],
        },
        Vertex {
            position: [-0.5, -0.5, 0.5],
            normal: [-1.0, 0.0, 0.0],
            uv: [1.0, 0.0],
        },
        Vertex {
            position: [-0.5, 0.5, 0.5],
            normal: [-1.0, 0.0, 0.0],
            uv: [1.0, 1.0],
        },
        Vertex {
            position: [-0.5, 0.5, -0.5],
            normal: [-1.0, 0.0, 0.0],
            uv: [0.0, 1.0],
        },
        // 右面
        Vertex {
            position: [0.5, -0.5, 0.5],
            normal: [1.0, 0.0, 0.0],
            uv: [0.0, 0.0],
        },
        Vertex {
            position: [0.5, -0.5, -0.5],
            normal: [1.0, 0.0, 0.0],
            uv: [1.0, 0.0],
        },
        Vertex {
            position: [0.5, 0.5, -0.5],
            normal: [1.0, 0.0, 0.0],
            uv: [1.0, 1.0],
        },
        Vertex {
            position: [0.5, 0.5, 0.5],
            normal: [1.0, 0.0, 0.0],
            uv: [0.0, 1.0],
        },
        // 上面
        Vertex {
            position: [-0.5, 0.5, 0.5],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        },
        Vertex {
            position: [0.5, 0.5, 0.5],
            normal: [0.0, 1.0, 0.0],
            uv: [1.0, 0.0],
        },
        Vertex {
            position: [0.5, 0.5, -0.5],
            normal: [0.0, 1.0, 0.0],
            uv: [1.0, 1.0],
        },
        Vertex {
            position: [-0.5, 0.5, -0.5],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 1.0],
        },
        // 下面
        Vertex {
            position: [-0.5, -0.5, -0.5],
            normal: [0.0, -1.0, 0.0],
            uv: [0.0, 0.0],
        },
        Vertex {
            position: [0.5, -0.5, -0.5],
            normal: [0.0, -1.0, 0.0],
            uv: [1.0, 0.0],
        },
        Vertex {
            position: [0.5, -0.5, 0.5],
            normal: [0.0, -1.0, 0.0],
            uv: [1.0, 1.0],
        },
        Vertex {
            position: [-0.5, -0.5, 0.5],
            normal: [0.0, -1.0, 0.0],
            uv: [0.0, 1.0],
        },
    ];

    // 立方体索引数据
    let indices = &[
        // 前面 - 逆时针顺序
        0, 1, 2, 2, 3, 0, // 后面 - 逆时针顺序
        4, 5, 6, 6, 7, 4, // 左面 - 逆时针顺序
        8, 9, 10, 10, 11, 8, // 右面 - 逆时针顺序
        12, 13, 14, 14, 15, 12, // 上面 - 逆时针顺序
        16, 17, 18, 18, 19, 16, // 下面 - 逆时针顺序
        20, 21, 22, 22, 23, 20,
    ];

    // 创建场景对象
    SceneObject::new(
        device,
        vertices,
        indices,
        model_bind_group_layout,
        texture_bind_group_layout,
        material_bind_group_layout,
        diffuse_texture,
        glam::Mat4::IDENTITY,
        crate::pipelines::MaterialBuffer::default(),
    )
}

