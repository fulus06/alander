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

use crate::DeviceContext;
use crate::resource_manager::ResourceManager;

/// 帧渲染上下文，封装单帧所需的临时资源
pub struct RenderContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub view: &'a wgpu::TextureView,
    pub depth_view: &'a wgpu::TextureView,
}

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

/// 渲染器
pub struct Renderer {
    /// WGPU 渲染设备上下文
    ctx: DeviceContext,
    /// 硬件信息
    pub caps: HardwareCaps,
    /// 资源管理器
    pub resources: ResourceManager,
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
    // 天空盒
    skybox_texture: crate::texture::Texture,
    skybox_bind_group: wgpu::BindGroup,
    skybox_camera_bind_group: wgpu::BindGroup,
    /// 调试顶点缓冲区
    debug_vertex_buffer: Option<wgpu::Buffer>,
    /// 调试顶点数量
    debug_vertex_count: u32,
    /// 调试覆盖层顶点缓冲区 (不进行深度测试)
    debug_overlay_vertex_buffer: Option<wgpu::Buffer>,
    /// 调试覆盖层顶点数量
    debug_overlay_vertex_count: u32,
    /// 当前视图矩阵 (用于 CPU 侧计算)
    pub view_matrix: cgmath::Matrix4<f32>,
    /// 投影矩阵 (用于 CPU 侧计算)
    pub proj_matrix: cgmath::Matrix4<f32>,
    /// HDR 颜色纹理视图 (中间渲染目标)
    hdr_view: wgpu::TextureView,
    /// 后期处理采样器
    post_proc_sampler: wgpu::Sampler,
    /// 后期处理绑定组
    post_proc_bind_group: wgpu::BindGroup,
}

impl Renderer {
    /// 创建新的渲染器
    pub async fn new(window: &Window) -> Result<Self, RenderError> {
        let ctx = DeviceContext::new(window, crate::RendererConfig::default()).await?;

        // 1. 检测硬件能力
        let _device = ctx.device();
        let adapter = ctx.adapter();
        
        let hdr_16_features = adapter.get_texture_format_features(wgpu::TextureFormat::Rgba16Float);
        let hdr_16_filterable = hdr_16_features.allowed_usages.contains(wgpu::TextureUsages::TEXTURE_BINDING) 
            && hdr_16_features.flags.contains(wgpu::TextureFormatFeatureFlags::FILTERABLE);

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

        // 2. 创建资源管理器
        let resources = ResourceManager::new(ctx.device(), ctx.queue());

        // 3. 继续初始化
        let depth_view =
            Self::create_depth_texture(ctx.device(), ctx.config(), "深度纹理");

        let camera_buffer =
            ctx.device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("相机缓冲区"),
                    contents: bytemuck::bytes_of(&CameraBuffer::new(
                        Matrix4::identity(),
                        Matrix4::identity(),
                        [0.0, 0.0, 0.0],
                    )),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

        let light_buffer =
            ctx.device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("光源缓冲区"),
                    contents: bytemuck::bytes_of(&crate::pipelines::LightBuffer::new()),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

        let hdr_filterable = if caps.preferred_hdr_format == wgpu::TextureFormat::Rgba16Float {
            caps.hdr_16_filterable
        } else {
            caps.hdr_32_filterable
        };

        let pipelines = Pipelines::new(ctx.device(), caps.preferred_hdr_format, ctx.config().format, hdr_filterable);

        let hdr_view = Self::create_hdr_texture(ctx.device(), ctx.config(), caps.preferred_hdr_format);
        let post_proc_sampler = ctx.device().create_sampler(&wgpu::SamplerDescriptor {
            label: Some("后期处理采样器"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let post_proc_bind_group = Self::create_post_proc_bind_group(
            ctx.device(),
            &pipelines.post_process.bind_group_layout,
            &hdr_view,
            &post_proc_sampler,
        );

        let skybox_texture = crate::texture::Texture::create_dummy_cubemap(ctx.device(), ctx.queue())
            .map_err(|_| RenderError::RequestDevice)?;

        let camera_bind_group = ctx.device()
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
                        resource: wgpu::BindingResource::Sampler(&resources.samplers.linear_clamp),
                    },
                ],
            });

        let skybox_camera_bind_group = ctx.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("天空盒相机绑定组"),
            layout: &pipelines.skybox.camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
            ],
        });

        let skybox_bind_group = ctx.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("天空盒纹理绑定组"),
            layout: &pipelines.skybox.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&skybox_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&resources.samplers.linear_clamp),
                },
            ],
        });

        Ok(Self {
            ctx,
            caps,
            resources,
            depth_view,
            camera_buffer,
            camera_bind_group,
            pipelines,
            light_buffer,
            skybox_texture,
            skybox_bind_group,
            skybox_camera_bind_group,
            debug_vertex_buffer: None,
            debug_vertex_count: 0,
            debug_overlay_vertex_buffer: None,
            debug_overlay_vertex_count: 0,
            view_matrix: cgmath::Matrix4::identity(),
            proj_matrix: cgmath::Matrix4::identity(),
            hdr_view,
            post_proc_sampler,
            post_proc_bind_group,
        })
    }

    /// 调整大小
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.ctx.resize(new_size);

        // 重新创建深度纹理
        let depth_view =
            Self::create_depth_texture(self.ctx.device(), self.ctx.config(), "深度纹理");
        self.depth_view = depth_view;

        // 重新创建 HDR 纹理与绑定组
        self.hdr_view = Self::create_hdr_texture(self.ctx.device(), self.ctx.config(), self.caps.preferred_hdr_format);
        self.post_proc_bind_group = Self::create_post_proc_bind_group(
            self.ctx.device(),
            &self.pipelines.post_process.bind_group_layout,
            &self.hdr_view,
            &self.post_proc_sampler,
        );
    }

    /// 更新相机
    pub fn update_camera(&mut self, camera: &CoreCamera, transform: &Transform) {
        let view = Self::calc_view_matrix(transform);
        let proj = Self::calc_proj_matrix(camera);
        
        self.view_matrix = view;
        self.proj_matrix = proj;

        let camera_buffer = CameraBuffer::new(view, proj, transform.position.into());

        // 调试信息：相机更新
        // tracing::debug!("更新相机: 位置 {:?}, 视图矩阵: {:?}, 投影矩阵: {:?}", transform.position, view, proj);

        self.ctx.queue().write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&camera_buffer),
        );
    }

    /// 将屏幕坐标转换为世界空间射线
    /// screen_pos: [0, 1] 范围的归一化屏幕坐标 (或像素坐标，取决于调用者)
    /// 这里约定传入的是 [0, 1] 范围
    pub fn screen_to_world_ray(&self, screen_pos: glam::Vec2) -> alander_core::math::Ray {
        use cgmath::SquareMatrix;

        // 1. 将 [0, 1] 转换为 NDC [-1, 1]
        // 注意：Y轴在屏幕空间通常向下，但在 NDC 向上
        let ndc_x = screen_pos.x * 2.0 - 1.0;
        let ndc_y = (1.0 - screen_pos.y) * 2.0 - 1.0;

        // 2. 构造 NDC 近平面和远平面点
        // 注意：cgmath::perspective 产生的是 OpenGL 风格矩阵 (Z 范围 [-1, 1])
        let ndc_near = cgmath::Vector4::new(ndc_x, ndc_y, -1.0, 1.0);
        let ndc_far = cgmath::Vector4::new(ndc_x, ndc_y, 1.0, 1.0);

        // 3. 计算 逆转换矩阵
        // 注意：WGPU 的投影矩阵包含了深度范围的调整，这里我们需要使用原始的 Proj * View
        let inv_view_proj = (self.proj_matrix * self.view_matrix).invert().unwrap();

        // 4. 转换到世界空间
        let world_near_h = inv_view_proj * ndc_near;
        let world_far_h = inv_view_proj * ndc_far;

        // 5. 透视除法
        let world_near = glam::Vec3::new(world_near_h.x / world_near_h.w, world_near_h.y / world_near_h.w, world_near_h.z / world_near_h.w);
        let world_far = glam::Vec3::new(world_far_h.x / world_far_h.w, world_far_h.y / world_far_h.w, world_far_h.z / world_far_h.w);

        // 6. 构造射线
        alander_core::math::Ray::new(world_near, (world_far - world_near).normalize())
    }

    /// 获取渲染管线
    pub fn pipelines(&self) -> &Pipelines {
        &self.pipelines
    }

    /// 更新光源
    pub fn update_lights(&mut self, lights: &crate::pipelines::LightBuffer) {
        self.ctx.queue().write_buffer(
            &self.light_buffer,
            0,
            bytemuck::bytes_of(lights),
        );
    }

    /// 加载外部 HDR 环境图
    pub fn load_hdr_environment(&mut self, path: &std::path::Path) -> Result<(), anyhow::Error> {
        let device = self.ctx.device();
        let queue = self.ctx.queue();

        let hdr_format = self.caps.preferred_hdr_format;
        // 是否能使用线性过滤取决于硬件能力
        let sampler = if hdr_format == wgpu::TextureFormat::Rgba16Float {
            if self.caps.hdr_16_filterable { &self.resources.samplers.linear_clamp } else { &self.resources.samplers.nearest_clamp }
        } else {
            if self.caps.hdr_32_filterable { &self.resources.samplers.linear_clamp } else { &self.resources.samplers.nearest_clamp }
        };

        tracing::info!("正在加载 HDR，目标格式: {:?}", hdr_format);

        // 1. 加载 HDR 2D 纹理 (作为转换源，通常不需要过滤，或用 nearest 即可)
        let equirect = crate::texture::Texture::from_hdr(
            device, 
            queue, 
            path, 
            hdr_format, 
            &self.resources.samplers.nearest_clamp
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
        self.ctx.device()
    }

    /// 获取 WGPU 队列
    pub fn queue(&self) -> &wgpu::Queue {
        self.ctx.queue()
    }

    /// 获取默认纹理
    pub fn default_texture(&self) -> &crate::texture::Texture {
        &self.resources.default_texture
    }

    /// 获取当前投影矩阵 (用于 CPU 侧计算)
    pub fn proj_matrix(&self) -> cgmath::Matrix4<f32> {
        self.proj_matrix
    }

    /// 获取视图投影矩阵 (glam 格式)
    pub fn view_proj_glam(&self) -> glam::Mat4 {
        let vp = self.proj_matrix * self.view_matrix;
        let raw: [[f32; 4]; 4] = vp.into();
        glam::Mat4::from_cols_array_2d(&raw)
    }

    /// 渲染场景
    pub fn render_scene(&self, target_view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        // 1. 主场景 HDR 渲染阶段 (渲染到 hdr_view)
        {
            let mut ctx = RenderContext {
                device: self.ctx.device(),
                queue: self.ctx.queue(),
                encoder,
                view: &self.hdr_view,
                depth_view: &self.depth_view,
            };

            self.render_skybox(&mut ctx);
            self.render_opaque(&mut ctx);
            self.render_debug(&mut ctx);
            self.render_debug_overlay(&mut ctx);
        }

        // 2. 后期处理阶段 (HDR -> ToneMap -> Gamma -> target_view)
        {
            let mut post_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("后期处理通道"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            post_pass.set_pipeline(&self.pipelines.post_process.pipeline);
            post_pass.set_bind_group(0, &self.post_proc_bind_group, &[]);
            post_pass.draw(0..3, 0..1);
        }
    }

    /// 天空盒渲染阶段
    pub fn render_skybox(&self, ctx: &mut RenderContext) {
        let mut render_pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("天空盒渲染通道"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ctx.view,
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
                view: ctx.depth_view,
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

    /// 不透明物体渲染阶段 (PBR)
    pub fn render_opaque(&self, ctx: &mut RenderContext) {
        let mut render_pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("网格渲染通道"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ctx.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: ctx.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        render_pass.set_pipeline(&self.pipelines.mesh.pipeline);
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

        for object in self.resources.objects.values() {
            object.render(&mut render_pass);
        }
    }

    /// 调试信息渲染阶段 (带深度测试，如碰撞体)
    pub fn render_debug(&self, ctx: &mut RenderContext) {
        if let Some(ref buffer) = self.debug_vertex_buffer {
            if self.debug_vertex_count > 0 {
                let mut render_pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("调试线条渲染通道"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: ctx.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: ctx.depth_view,
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

    /// 调试覆盖层渲染阶段 (无深度测试，如 Gizmo)
    pub fn render_debug_overlay(&self, ctx: &mut RenderContext) {
        if let Some(ref buffer) = self.debug_overlay_vertex_buffer {
            if self.debug_overlay_vertex_count > 0 {
                let mut render_pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("调试覆盖层渲染通道"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: ctx.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: ctx.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        }),
                        stencil_ops: None,
                    }),
                });

                render_pass.set_pipeline(&self.pipelines.debug.overlay_pipeline);
                render_pass.set_bind_group(0, &self.skybox_camera_bind_group, &[]); // 复用相机绑定组
                render_pass.set_vertex_buffer(0, buffer.slice(..));
                render_pass.draw(0..self.debug_overlay_vertex_count, 0..1);
            }
        }
    }

    /// 添加 glTF 模型
    pub fn add_gltf_model(&mut self, model: alander_core::assets::GltfModel) -> Vec<uuid::Uuid> {
        let image_to_texture = self.resources.load_gltf_textures(self.ctx.device(), self.ctx.queue(), &model);
        let mut object_ids = Vec::new();

        // 2. 将 glTF 网格转换为场景对象
        for gltf_mesh in &model.meshes {
            let diffuse_texture = self.resources.get_texture_from_index(&model, gltf_mesh, &image_to_texture, 0); // Diffuse
            let normal_texture = self.resources.get_texture_from_index(&model, gltf_mesh, &image_to_texture, 1);  // Normal
            let mr_texture = self.resources.get_texture_from_index(&model, gltf_mesh, &image_to_texture, 2);      // Metallic-Roughness

            // 构造材质标志
            let mut material_buffer = crate::pipelines::MaterialBuffer::default();
            if normal_texture as *const _ != &self.resources.default_texture as *const _ {
                material_buffer.has_normal_texture = 1;
            }
            if mr_texture as *const _ != &self.resources.default_texture as *const _ {
                material_buffer.has_metallic_roughness_texture = 1;
            }

            let scene_object = SceneObject::new(
                self.ctx.device(),
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
                &self.resources.samplers.linear_clamp,
            );

            let id = uuid::Uuid::new_v4();
            self.resources.add_object(id, scene_object);
            object_ids.push(id);
        }

        object_ids
    }

    /// 加载 glTF 模型中的所有纹理
    pub fn load_gltf_textures(&mut self, model: &alander_core::assets::GltfModel) -> HashMap<usize, usize> {
        let (ctx, resources) = (&self.ctx, &mut self.resources);
        resources.load_gltf_textures(ctx.device(), ctx.queue(), model)
    }

    /// 添加场景对象
    pub fn add_object(&mut self, id: uuid::Uuid, object: SceneObject) {
        self.resources.add_object(id, object);
    }

    /// 移除场景对象
    pub fn remove_object(&mut self, id: &uuid::Uuid) -> Option<SceneObject> {
        self.resources.remove_object(id)
    }

    /// 获取表面
    pub fn surface(&self) -> &wgpu::Surface {
        self.ctx.surface()
    }

    /// 获取配置
    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        self.ctx.config()
    }

    /// 获取表面格式
    pub fn format(&self) -> wgpu::TextureFormat {
        self.ctx.format()
    }

    /// 获取窗口大小
    pub fn size(&self) -> winit::dpi::PhysicalSize<u32> {
        self.ctx.size()
    }

    /// 更新对象模型矩阵与材质
    pub fn update_object_model_material(
        &mut self, 
        object_id: &uuid::Uuid, 
        model: cgmath::Matrix4<f32>,
        material: Option<crate::pipelines::MaterialBuffer>,
    ) {
        if let Some(object) = self.resources.objects.get_mut(object_id) {
            object.update_model(self.ctx.queue(), model);
            if let Some(mat) = material {
                self.ctx.queue().write_buffer(&object.material_buffer, 0, bytemuck::bytes_of(&mat));
            }
        }
    }

    /// 更新调试线框 (受深度影响，如碰撞体)
    pub fn update_debug_lines(&mut self, vertices: &[crate::pipelines::DebugVertex]) {
        if vertices.is_empty() {
            self.debug_vertex_count = 0;
            return;
        }

        let device = self.ctx.device();
        let size = (vertices.len() * std::mem::size_of::<crate::pipelines::DebugVertex>()) as u64;

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
            self.ctx.queue().write_buffer(buffer, 0, bytemuck::cast_slice(vertices));
            self.debug_vertex_count = vertices.len() as u32;
        }
    }

    /// 更新调试覆盖层线框 (始终在最上层，如 Gizmo)
    pub fn update_debug_overlay(&mut self, vertices: &[crate::pipelines::DebugVertex]) {
        if vertices.is_empty() {
            self.debug_overlay_vertex_count = 0;
            return;
        }

        let device = self.ctx.device();
        let size = (vertices.len() * std::mem::size_of::<crate::pipelines::DebugVertex>()) as u64;

        let needs_recreate = match &self.debug_overlay_vertex_buffer {
            Some(buffer) => buffer.size() < size,
            None => true,
        };

        if needs_recreate {
            self.debug_overlay_vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("调试覆盖层顶点缓冲区"),
                size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        if let Some(ref buffer) = self.debug_overlay_vertex_buffer {
            self.ctx.queue().write_buffer(buffer, 0, bytemuck::cast_slice(vertices));
            self.debug_overlay_vertex_count = vertices.len() as u32;
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

    fn create_hdr_texture(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, format: wgpu::TextureFormat) -> wgpu::TextureView {
        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HDR 颜色缓冲"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    fn create_post_proc_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        hdr_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("后期处理绑定组"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(hdr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
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

