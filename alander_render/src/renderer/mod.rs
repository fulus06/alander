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

/// 硬件功能支持情况
#[derive(Debug, Clone, Copy)]
pub struct HardwareCaps {
    /// Rgba16Float 是否支持卷积/过滤
    pub hdr_16_filterable: bool,
    /// Rgba32Float 是否支持卷积/过滤
    pub hdr_32_filterable: bool,
    /// 建议的 HDR 格式
    pub preferred_hdr_format: wgpu::TextureFormat,
}

/// 采样器缓存，解耦纹理与采样器
pub struct SamplerCache {
    /// 线性过滤 + 边缘拉伸 (适用于 IBL/天空盒)
    pub linear_clamp: wgpu::Sampler,
    /// 线性过滤 + 重复 (适用于普通材质)
    pub linear_repeat: wgpu::Sampler,
    /// 最近邻过滤 + 边缘拉伸 (兼容性回退)
    pub nearest_clamp: wgpu::Sampler,
}

/// 渲染器
pub struct Renderer {
    /// WGPU 渲染器实例
    renderer: super::Renderer,
    /// 硬件信息
    pub caps: HardwareCaps,
    /// 全局采样器
    pub samplers: SamplerCache,
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
    // 天空盒
    skybox_texture: crate::texture::Texture,
    skybox_bind_group: wgpu::BindGroup,
    skybox_camera_bind_group: wgpu::BindGroup,
    /// 调试顶点缓冲区
    debug_vertex_buffer: Option<wgpu::Buffer>,
    /// 调试顶点数量
    debug_vertex_count: u32,
}

impl Renderer {
    /// 创建新的渲染器
    pub async fn new(window: &Window) -> Result<Self, RenderError> {
        let renderer = super::Renderer::new(window, super::RendererConfig::default()).await?;

        // 1. 检测硬件能力
        let device = renderer.device();
        let adapter = renderer.adapter();
        
        // 检查 16位浮点过滤支持 (大多数硬件支持)
        let hdr_16_features = adapter.get_texture_format_features(wgpu::TextureFormat::Rgba16Float);
        let hdr_16_filterable = hdr_16_features.allowed_usages.contains(wgpu::TextureUsages::TEXTURE_BINDING) 
            && hdr_16_features.flags.contains(wgpu::TextureFormatFeatureFlags::FILTERABLE);

        // 检查 32位浮点过滤支持 (某些旧硬件或移动端不支持)
        let hdr_32_features = adapter.get_texture_format_features(wgpu::TextureFormat::Rgba32Float);
        let hdr_32_filterable = hdr_32_features.allowed_usages.contains(wgpu::TextureUsages::TEXTURE_BINDING) 
            && hdr_32_features.flags.contains(wgpu::TextureFormatFeatureFlags::FILTERABLE);

        let preferred_hdr_format = if hdr_16_filterable {
            wgpu::TextureFormat::Rgba16Float
        } else {
            wgpu::TextureFormat::Rgba32Float
        };

        let caps = HardwareCaps {
            hdr_16_filterable,
            hdr_32_filterable,
            preferred_hdr_format,
        };

        tracing::info!("检测到硬件能力: {:?}", caps);
        tracing::info!("选择 HDR 格式: {:?}, 线性过滤支持: {}", 
            preferred_hdr_format, 
            if preferred_hdr_format == wgpu::TextureFormat::Rgba16Float { hdr_16_filterable } else { hdr_32_filterable }
        );

        // 2. 创建全局采样器
        let samplers = SamplerCache {
            linear_clamp: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("线性采样器 (Clamp)"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                ..Default::default()
            }),
            linear_repeat: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("线性采样器 (Repeat)"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                ..Default::default()
            }),
            nearest_clamp: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("最近邻采样器 (Clamp)"),
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                ..Default::default()
            }),
        };


        // 3. 继续初始化
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

        // 计算 HDR 过滤支持情况
        let hdr_filterable = if caps.preferred_hdr_format == wgpu::TextureFormat::Rgba16Float {
            caps.hdr_16_filterable
        } else {
            caps.hdr_32_filterable
        };

        // 创建渲染管线
        let pipelines = Pipelines::new(renderer.device(), renderer.config(), hdr_filterable);

        // 创建默认立方体贴图
        let skybox_texture = crate::texture::Texture::create_dummy_cubemap(renderer.device(), renderer.queue())
            .map_err(|_| RenderError::RequestDevice)?;

        // 创建相机绑定组 (包含 IBL)
        let camera_bind_group = renderer
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("相机及 IBL 绑定组"),
                layout: &pipelines.mesh.camera_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: camera_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: light_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&skybox_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&skybox_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&samplers.linear_clamp),
                    },
                ],
            });

        // 天空盒相机绑定组
        let skybox_camera_bind_group = renderer.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("天空盒相机绑定组"),
            layout: &pipelines.skybox.camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
            ],
        });

        // 天空盒纹理绑定组 (默认使用 dummy)
        let skybox_bind_group = renderer.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("天空盒纹理绑定组"),
            layout: &pipelines.skybox.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&skybox_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&samplers.linear_clamp),
                },
            ],
        });

        let skybox_texture = crate::texture::Texture::create_dummy_cubemap(renderer.device(), renderer.queue())
            .map_err(|_| RenderError::RequestDevice)?;

        Ok(Self {
            renderer,
            caps,
            samplers,
            depth_view,
            camera_buffer,
            camera_bind_group,
            pipelines,
            objects: HashMap::new(),
            light_buffer,
            default_texture,
            textures: Vec::new(),
            skybox_texture,
            skybox_bind_group,
            skybox_camera_bind_group,
            debug_vertex_buffer: None,
            debug_vertex_count: 0,
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

    /// 加载外部 HDR 环境图
    pub fn load_hdr_environment(&mut self, path: &std::path::Path) -> Result<(), anyhow::Error> {
        let device = self.renderer.device();
        let queue = self.renderer.queue();

        let hdr_format = self.caps.preferred_hdr_format;
        // 是否能使用线性过滤取决于硬件能力
        let sampler = if hdr_format == wgpu::TextureFormat::Rgba16Float {
            if self.caps.hdr_16_filterable { &self.samplers.linear_clamp } else { &self.samplers.nearest_clamp }
        } else {
            if self.caps.hdr_32_filterable { &self.samplers.linear_clamp } else { &self.samplers.nearest_clamp }
        };

        tracing::info!("正在加载 HDR，目标格式: {:?}", hdr_format);

        // 1. 加载 HDR 2D 纹理 (作为转换源，通常不需要过滤，或用 nearest 即可)
        let equirect = crate::texture::Texture::from_hdr(
            device, 
            queue, 
            path, 
            hdr_format, 
            &self.samplers.nearest_clamp
        )?;
        
        // 2. 转换为立方体贴图 (1024x1024)
        let filterable = if hdr_format == wgpu::TextureFormat::Rgba16Float {
            self.caps.hdr_16_filterable
        } else {
            self.caps.hdr_32_filterable
        };

        let skybox_texture = crate::texture::Texture::equirectangular_to_cubemap(
            device, 
            queue, 
            &equirect, 
            1024,
            hdr_format,
            sampler,
            filterable
        )?;
        
        // 3. 更新渲染器状态
        self.skybox_texture = skybox_texture;
        
        // 4. 重建天空盒绑定组
        self.skybox_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("天空盒纹理绑定组 (HDR)"),
            layout: &self.pipelines.skybox.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.skybox_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        // 5. 重建相机/IBL 绑定组
        self.camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("相机及 IBL 绑定组 (HDR)"),
            layout: &self.pipelines.mesh.camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.light_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.skybox_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&self.skybox_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        Ok(())
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
        // 1. 天空盒渲染过程
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("天空盒渲染通道"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
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

            render_pass.set_pipeline(&self.pipelines.skybox.pipeline);
            render_pass.set_bind_group(0, &self.skybox_camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.skybox_bind_group, &[]);
            render_pass.draw(0..36, 0..1);
        }

        // 2. 网格渲染过程
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("网格渲染通道"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            render_pass.set_pipeline(&self.pipelines.mesh.pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

            for object in self.objects.values() {
                object.render(&mut render_pass);
            }
        }

        // 3. 调试线条渲染过程
        if let Some(ref buffer) = self.debug_vertex_buffer {
            if self.debug_vertex_count > 0 {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("调试线条渲染通道"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        }),
                        stencil_ops: None,
                    }),
                });

                render_pass.set_pipeline(&self.pipelines.debug.pipeline);
                render_pass.set_bind_group(0, &self.skybox_camera_bind_group, &[]); // 复用相机绑定组
                render_pass.set_vertex_buffer(0, buffer.slice(..));
                render_pass.draw(0..self.debug_vertex_count, 0..1);
            }
        }
    }

    /// 添加 glTF 模型
    /// 添加 glTF 模型
    pub fn add_gltf_model(&mut self, model: alander_core::assets::GltfModel) -> Vec<uuid::Uuid> {
        let image_to_texture = self.load_gltf_textures(&model);
        let mut object_ids = Vec::new();

        // 2. 将 glTF 网格转换为场景对象
        for gltf_mesh in &model.meshes {
            let diffuse_texture = self.get_texture_from_index(&model, gltf_mesh, &image_to_texture, 0); // Diffuse
            let normal_texture = self.get_texture_from_index(&model, gltf_mesh, &image_to_texture, 1);  // Normal
            let mr_texture = self.get_texture_from_index(&model, gltf_mesh, &image_to_texture, 2);      // Metallic-Roughness

            // 构造材质标志
            let mut material_buffer = crate::pipelines::MaterialBuffer::default();
            if normal_texture as *const _ != &self.default_texture as *const _ {
                material_buffer.has_normal_texture = 1;
            }
            if mr_texture as *const _ != &self.default_texture as *const _ {
                material_buffer.has_metallic_roughness_texture = 1;
            }

            let scene_object = SceneObject::new(
                self.renderer.device(),
                &gltf_mesh.data.vertices.iter().map(|v| crate::pipelines::Vertex {
                    position: v.position.into(),
                    normal: v.normal.into(),
                    uv: v.uv.into(),
                    tangent: v.tangent.into(),
                }).collect::<Vec<_>>(),
                &gltf_mesh.data.indices,
                self.pipelines.mesh.model_bind_group_layout(),
                &self.pipelines.mesh.texture_bind_group_layout,
                &self.pipelines.mesh.material_bind_group_layout,
                diffuse_texture,
                normal_texture,
                mr_texture,
                gltf_mesh.transform,
                material_buffer,
                &self.samplers.linear_clamp,
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
    pub fn get_texture_from_index<'a>(
        &'a self, 
        model: &alander_core::assets::GltfModel, 
        mesh: &alander_core::assets::GltfMesh,
        image_to_texture: &HashMap<usize, usize>,
        texture_type: u32, // 0: Diffuse, 1: Normal, 2: Metallic-Roughness
    ) -> &'a crate::texture::Texture {
        if let Some(mat_idx) = mesh.material_index {
            if let Some(material) = model.materials.get(mat_idx) {
                let img_idx_opt = match texture_type {
                    0 => material.base_color_texture.as_ref(),
                    1 => material.normal_texture.as_ref(),
                    2 => material.metallic_roughness_texture.as_ref(),
                    _ => None,
                };

                if let Some(img_idx_str) = img_idx_opt {
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

    /// 更新调试线条
    pub fn update_debug_lines(&mut self, vertices: &[crate::pipelines::DebugVertex]) {
        if vertices.is_empty() {
            self.debug_vertex_count = 0;
            return;
        }

        let device = self.renderer.device();
        let size = (vertices.len() * std::mem::size_of::<crate::pipelines::DebugVertex>()) as u64;

        // 如果缓冲区不存在或太小，重新创建
        let needs_recreate = match &self.debug_vertex_buffer {
            Some(buffer) => buffer.size() < size,
            None => true,
        };

        if needs_recreate {
            self.debug_vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("调试顶点缓冲区"),
                size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        if let Some(ref buffer) = self.debug_vertex_buffer {
            self.renderer.queue().write_buffer(buffer, 0, bytemuck::cast_slice(vertices));
            self.debug_vertex_count = vertices.len() as u32;
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
            format: crate::texture::Texture::DEPTH_FORMAT,
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
    sampler: &wgpu::Sampler,
) -> SceneObject {
    // 立方体顶点数据
    let vertices = &[
        // 前面
        Vertex {
            position: [-0.5, -0.5, 0.5],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [0.5, -0.5, 0.5],
            normal: [0.0, 0.0, 1.0],
            uv: [1.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [0.5, 0.5, 0.5],
            normal: [0.0, 0.0, 1.0],
            uv: [1.0, 1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [-0.5, 0.5, 0.5],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        // 后面
        Vertex {
            position: [-0.5, -0.5, -0.5],
            normal: [0.0, 0.0, -1.0],
            uv: [0.0, 0.0],
            tangent: [-1.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [0.5, -0.5, -0.5],
            normal: [0.0, 0.0, -1.0],
            uv: [1.0, 0.0],
            tangent: [-1.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [0.5, 0.5, -0.5],
            normal: [0.0, 0.0, -1.0],
            uv: [1.0, 1.0],
            tangent: [-1.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [-0.5, 0.5, -0.5],
            normal: [0.0, 0.0, -1.0],
            uv: [0.0, 1.0],
            tangent: [-1.0, 0.0, 0.0, 1.0],
        },
        // 左面
        Vertex {
            position: [-0.5, -0.5, -0.5],
            normal: [-1.0, 0.0, 0.0],
            uv: [0.0, 0.0],
            tangent: [0.0, 0.0, 1.0, 1.0],
        },
        Vertex {
            position: [-0.5, -0.5, 0.5],
            normal: [-1.0, 0.0, 0.0],
            uv: [1.0, 0.0],
            tangent: [0.0, 0.0, 1.0, 1.0],
        },
        Vertex {
            position: [-0.5, 0.5, 0.5],
            normal: [-1.0, 0.0, 0.0],
            uv: [1.0, 1.0],
            tangent: [0.0, 0.0, 1.0, 1.0],
        },
        Vertex {
            position: [-0.5, 0.5, -0.5],
            normal: [-1.0, 0.0, 0.0],
            uv: [0.0, 1.0],
            tangent: [0.0, 0.0, 1.0, 1.0],
        },
        // 右面
        Vertex {
            position: [0.5, -0.5, 0.5],
            normal: [1.0, 0.0, 0.0],
            uv: [0.0, 0.0],
            tangent: [0.0, 0.0, -1.0, 1.0],
        },
        Vertex {
            position: [0.5, -0.5, -0.5],
            normal: [1.0, 0.0, 0.0],
            uv: [1.0, 0.0],
            tangent: [0.0, 0.0, -1.0, 1.0],
        },
        Vertex {
            position: [0.5, 0.5, -0.5],
            normal: [1.0, 0.0, 0.0],
            uv: [1.0, 1.0],
            tangent: [0.0, 0.0, -1.0, 1.0],
        },
        Vertex {
            position: [0.5, 0.5, 0.5],
            normal: [1.0, 0.0, 0.0],
            uv: [0.0, 1.0],
            tangent: [0.0, 0.0, -1.0, 1.0],
        },
        // 上面
        Vertex {
            position: [-0.5, 0.5, 0.5],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [0.5, 0.5, 0.5],
            normal: [0.0, 1.0, 0.0],
            uv: [1.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [0.5, 0.5, -0.5],
            normal: [0.0, 1.0, 0.0],
            uv: [1.0, 1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [-0.5, 0.5, -0.5],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        // 下面
        Vertex {
            position: [-0.5, -0.5, -0.5],
            normal: [0.0, -1.0, 0.0],
            uv: [0.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [0.5, -0.5, -0.5],
            normal: [0.0, -1.0, 0.0],
            uv: [1.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [0.5, -0.5, 0.5],
            normal: [0.0, -1.0, 0.0],
            uv: [1.0, 1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [-0.5, -0.5, 0.5],
            normal: [0.0, -1.0, 0.0],
            uv: [0.0, 1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
        },
    ];

    // 立方体索引数据
    let indices = &[
        0, 1, 2, 2, 3, 0,       // 前面
        4, 7, 6, 6, 5, 4,       // 后面
        8, 9, 10, 10, 11, 8,    // 左面
        12, 13, 14, 14, 15, 12, // 右面
        16, 17, 18, 18, 19, 16, // 上面
        20, 21, 22, 22, 23, 20, // 下面
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
        diffuse_texture, // Normal dummy
        diffuse_texture, // MR dummy
        glam::Mat4::IDENTITY,
        crate::pipelines::MaterialBuffer::default(),
        sampler,
    )
}

