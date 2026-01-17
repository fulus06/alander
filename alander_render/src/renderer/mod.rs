//! 渲染器核心实现
//!
//! 此模块包含渲染器的主要功能和渲染循环。

use super::RenderError;
use crate::pipelines::{CameraBuffer, Pipelines, SceneObject, Vertex, LightBuffer};
use alander_core::scene::{Camera as CoreCamera, Transform};
use cgmath::{Matrix4, Point3, Vector3, SquareMatrix};
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
    /// Bloom 设置缓冲区
    bloom_settings_buffer: wgpu::Buffer,
    /// Bloom 提取绑定组
    bloom_extract_bind_group: wgpu::BindGroup,
    /// Bloom 模糊绑定组 (0: H, 1: V)
    bloom_blur_bind_groups: [wgpu::BindGroup; 2],
    /// Bloom 纹理视图 (0: 提取/中间, 1: 模糊结果)
    bloom_texture_views: [wgpu::TextureView; 2],
    /// Bloom 原始纹理 (需保持生命周期)
    bloom_textures: [wgpu::Texture; 2],
    /// 阴影纹理
    shadow_texture: wgpu::Texture,
    /// 阴影纹理视图
    shadow_view: wgpu::TextureView,
    /// 阴影采样器
    shadow_sampler: wgpu::Sampler,
    /// 光空间投影缓冲区
    light_space_buffer: wgpu::Buffer,
    /// 光空间绑定组
    light_space_bind_group: wgpu::BindGroup,
    /// 阴影视图投影矩阵 (旧的，保持兼容)
    pub shadow_view_proj: cgmath::Matrix4<f32>,
    /// CSM 级联数量
    pub csm_cascades: usize,
    /// CSM 各级联的视图投影矩阵
    pub csm_view_projs: Vec<cgmath::Matrix4<f32>>,
    /// CSM 各级联的分割距离
    pub csm_split_distances: Vec<f32>,
    /// 点光源阴影立方体纹理
    shadow_cube_texture: wgpu::Texture,
    /// 点光源阴影立方体视图
    shadow_cube_view: wgpu::TextureView,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct BloomSettings {
    pub threshold: f32,
    pub intensity: f32,
}

unsafe impl bytemuck::Pod for BloomSettings {}
unsafe impl bytemuck::Zeroable for BloomSettings {}

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
                    contents: bytemuck::bytes_of(&LightBuffer::new()),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

        // 4. 阴影系统初始化
        let shadow_size = 2048;
        let shadow_texture = ctx.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("阴影纹理"),
            size: wgpu::Extent3d {
                width: shadow_size,
                height: shadow_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 4b. 点光源阴影立方体映射初始化
        let shadow_cube_size = 512;
        let shadow_cube_texture = ctx.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("点光源阴影立方体纹理"),
            size: wgpu::Extent3d {
                width: shadow_cube_size,
                height: shadow_cube_size,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_cube_view = shadow_cube_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("点光源阴影立方体视图"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        let shadow_sampler = ctx.device().create_sampler(&wgpu::SamplerDescriptor {
            label: Some("阴影采样器"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let light_space_buffer = ctx.device().create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("光空间缓冲区"),
            contents: bytemuck::bytes_of(&crate::pipelines::LightSpaceBuffer::new(
                [Matrix4::identity(); 4],
                [0.0; 4]
            )),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let hdr_filterable = if caps.preferred_hdr_format == wgpu::TextureFormat::Rgba16Float {
            caps.hdr_16_filterable
        } else {
            caps.hdr_32_filterable
        };

        let pipelines = Pipelines::new(ctx.device(), caps.preferred_hdr_format, ctx.config().format, hdr_filterable);

        let light_space_bind_group = ctx.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("光空间绑定组"),
            layout: &pipelines.shadow.light_space_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: light_space_buffer.as_entire_binding(),
                },
            ],
        });

        let hdr_view = Self::create_hdr_texture(ctx.device(), ctx.config(), caps.preferred_hdr_format);
        // 后期处理采样器
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
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(&shadow_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: light_space_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: wgpu::BindingResource::TextureView(&shadow_cube_view),
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

        // Bloom 初始化
        let bloom_settings = BloomSettings {
            threshold: 1.0,
            intensity: 0.5,
        };
        let bloom_settings_buffer = ctx.device().create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bloom 设置缓冲区"),
            contents: bytemuck::cast_slice(&[bloom_settings]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let (bloom_textures, bloom_texture_views) = Self::create_bloom_textures(ctx.device(), ctx.config(), caps.preferred_hdr_format);
        
        let bloom_extract_bind_group = ctx.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bloom 提取绑定组"),
            layout: &pipelines.bloom.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&hdr_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&post_proc_sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: bloom_settings_buffer.as_entire_binding() },
            ],
        });

        let bloom_blur_bind_groups = [
            ctx.device().create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Bloom 模糊绑定组 H"),
                layout: &pipelines.bloom.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&bloom_texture_views[0]) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&post_proc_sampler) },
                    wgpu::BindGroupEntry { binding: 2, resource: bloom_settings_buffer.as_entire_binding() },
                ],
            }),
            ctx.device().create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Bloom 模糊绑定组 V"),
                layout: &pipelines.bloom.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&bloom_texture_views[1]) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&post_proc_sampler) },
                    wgpu::BindGroupEntry { binding: 2, resource: bloom_settings_buffer.as_entire_binding() },
                ],
            }),
        ];

        // 重新创建后期处理绑定组以包含 Bloom
        let post_proc_bind_group = ctx.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("后期处理绑定组 (含 Bloom)"),
            layout: &pipelines.post_process.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&hdr_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&post_proc_sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&bloom_texture_views[0]) }, // 最终模糊结果
                wgpu::BindGroupEntry { binding: 3, resource: bloom_settings_buffer.as_entire_binding() },
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
            bloom_settings_buffer,
            bloom_extract_bind_group,
            bloom_blur_bind_groups,
            bloom_texture_views,
            bloom_textures,
            shadow_texture,
            shadow_view,
            shadow_sampler,
            light_space_buffer,
            light_space_bind_group,
            shadow_view_proj: Matrix4::identity(),
            csm_cascades: 4,
            csm_view_projs: vec![Matrix4::identity(); 4],
            csm_split_distances: vec![0.0; 4],
            shadow_cube_texture,
            shadow_cube_view,
        })
    }

    /// 阴影 Pass
    pub fn render_shadow_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        objects: &[&SceneObject],
        light_view_proj: cgmath::Matrix4<f32>,
    ) {
        // 更新光空间矩阵
        self.ctx.queue().write_buffer(
            &self.light_space_buffer,
            0,
            bytemuck::bytes_of(&crate::pipelines::LightSpaceBuffer::new(
                [light_view_proj, Matrix4::identity(), Matrix4::identity(), Matrix4::identity()],
                [0.0; 4]
            )),
        );

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("阴影 Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.shadow_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        render_pass.set_pipeline(&self.pipelines.shadow.pipeline);
        render_pass.set_bind_group(0, &self.light_space_bind_group, &[]);

        for object in objects {
            render_pass.set_vertex_buffer(0, object.vertex_buffer.slice(..));
            render_pass.set_index_buffer(object.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.set_bind_group(1, &object.model_bind_group, &[]);
            render_pass.draw_indexed(0..object.num_elements, 0, 0..1);
        }
    }

    /// 点光源阴影 Pass (6面)
    pub fn render_point_shadow_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        objects: &[&SceneObject],
        light_pos: [f32; 3],
        range: f32,
    ) {
        let light_point = cgmath::Point3::new(light_pos[0], light_pos[1], light_pos[2]);
        let proj = cgmath::perspective(cgmath::Deg(90.0), 1.0, 0.1, range);
        
        let directions = [
            (cgmath::Vector3::unit_x(), cgmath::Vector3::unit_y()),   // +X
            (cgmath::Vector3::new(-1.0, 0.0, 0.0), cgmath::Vector3::unit_y()),  // -X
            (cgmath::Vector3::unit_y(), cgmath::Vector3::new(0.0, 0.0, -1.0)),  // +Y
            (cgmath::Vector3::new(0.0, -1.0, 0.0), cgmath::Vector3::new(0.0, 0.0, 1.0)),   // -Y
            (cgmath::Vector3::unit_z(), cgmath::Vector3::unit_y()),   // +Z
            (cgmath::Vector3::new(0.0, 0.0, -1.0), cgmath::Vector3::unit_y()),  // -Z
        ];

        for i in 0..6 {
            let (dir, up) = directions[i];
            let view = cgmath::Matrix4::look_to_rh(light_point, dir, up);
            let view_proj = proj * view;

            self.ctx.queue().write_buffer(
                &self.light_space_buffer,
                0,
                bytemuck::bytes_of(&crate::pipelines::LightSpaceBuffer::new([view_proj, Matrix4::identity(), Matrix4::identity(), Matrix4::identity()], [0.0; 4])),
            );

            let face_view = self.shadow_cube_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("点光源阴影面 {}", i)),
                format: Some(wgpu::TextureFormat::Depth32Float),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: i as u32,
                array_layer_count: Some(1),
                ..Default::default()
            });

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&format!("点光源阴影 Pass 面 {}", i)),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &face_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            render_pass.set_pipeline(&self.pipelines.shadow.pipeline);
            render_pass.set_bind_group(0, &self.light_space_bind_group, &[]);

            for object in objects {
                render_pass.set_vertex_buffer(0, object.vertex_buffer.slice(..));
                render_pass.set_index_buffer(object.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.set_bind_group(1, &object.model_bind_group, &[]);
                render_pass.draw_indexed(0..object.num_elements, 0, 0..1);
            }
        }
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

        // 重新创建 Bloom 资源
        let (bloom_textures, bloom_views) = Self::create_bloom_textures(self.ctx.device(), self.ctx.config(), self.caps.preferred_hdr_format);
        self.bloom_textures = bloom_textures;
        self.bloom_texture_views = bloom_views;

        // 刷新所有后期处理相关的绑定组
        self.bloom_extract_bind_group = self.ctx.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bloom 提取绑定组"),
            layout: &self.pipelines.bloom.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.hdr_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.post_proc_sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: self.bloom_settings_buffer.as_entire_binding() },
            ],
        });

        self.bloom_blur_bind_groups = [
            self.ctx.device().create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Bloom 模糊绑定组 H"),
                layout: &self.pipelines.bloom.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.bloom_texture_views[0]) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.post_proc_sampler) },
                    wgpu::BindGroupEntry { binding: 2, resource: self.bloom_settings_buffer.as_entire_binding() },
                ],
            }),
            self.ctx.device().create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Bloom 模糊绑定组 V"),
                layout: &self.pipelines.bloom.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.bloom_texture_views[1]) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.post_proc_sampler) },
                    wgpu::BindGroupEntry { binding: 2, resource: self.bloom_settings_buffer.as_entire_binding() },
                ],
            }),
        ];

        self.post_proc_bind_group = self.ctx.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("后期处理绑定组"),
            layout: &self.pipelines.post_process.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.hdr_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.post_proc_sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.bloom_texture_views[0]) },
                wgpu::BindGroupEntry { binding: 3, resource: self.bloom_settings_buffer.as_entire_binding() },
            ],
        });
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

        // 如果有平行光且开启了阴影，计算并更新阴影矩阵
        // 为简单起见，目前只要 intensity > 0 就更新
        if lights.dir_light.intensity > 0.0 {
            let dir = Vector3::new(
                lights.dir_light.direction[0],
                lights.dir_light.direction[1],
                lights.dir_light.direction[2],
            );
            
            // let _light_pos = Point3::new(-dir.x * 20.0, -dir.y * 20.0, -dir.z * 20.0);
            // let light_view = Matrix4::look_to_rh(
            //     light_pos,
            //     dir,
            //     Vector3::unit_y(),
            // );
            
            // 正交投影 (覆盖 40x40 的范围，深度 0.1-100)
            // let light_proj = ortho(-20.0, 20.0, -20.0, 20.0, 0.1, 100.0);
            
            // CSM 实现
            self.update_csm_matrices(&lights.dir_light, dir);
        }
    }

    /// 更新 CSM 矩阵
    fn update_csm_matrices(&mut self, _dir_light: &crate::pipelines::DirectionalLight, light_dir: Vector3<f32>) {
        let cascade_count = self.csm_cascades;
        let near = 0.1;
        let far = 100.0; // 这里的 far 应该取自相机的 far
        
        // 1. 计算分割距离 (Logarithmic Split)
        let lambda = 0.5f32;
        let mut splits = vec![0.0; cascade_count + 1];
        for i in 0..=cascade_count {
            let p = i as f32 / cascade_count as f32;
            let log_split = near * (far / near as f32).powf(p);
            let uniform_split = near + (far - near) * p;
            splits[i] = lambda * log_split + (1.0 - lambda) * uniform_split;
        }
        self.csm_split_distances = splits[1..].to_vec();

        // 2. 为每个级联计算矩阵
        let inv_view_proj = (self.proj_matrix * self.view_matrix).invert().unwrap();
        
        for i in 0..cascade_count {
            let _prev_split = splits[i];
            let _next_split = splits[i+1];
            
            // 计算级联视锥体的 8 个顶点
            let mut frustum_corners = Vec::new();
            for z in 0..2 {
                for y in 0..2 {
                    for x in 0..2 {
                        let corner = cgmath::Vector4::new(
                            x as f32 * 2.0 - 1.0,
                            y as f32 * 2.0 - 1.0,
                            if z == 0 { -1.0 } else { 1.0 }, // NDC Z 范围依后端而定，这里假设映射到了 -1..1 以反旋回世界
                            1.0,
                        );
                        let mut world_corner = inv_view_proj * corner;
                        world_corner /= world_corner.w;
                        frustum_corners.push(world_corner);
                    }
                }
            }
            
            // 简单起见，这里按比例缩放顶点以匹配 split 距离
            // 正确做法是根据 near/far 重新计算视锥体
            let _center = cgmath::Point3::new(0.0, 0.0, 0.0); // 占位
            
            // 计算光空间包围盒
            let light_view = Matrix4::look_to_rh(
                Point3::new(0.0, 0.0, 0.0), // 临时位置
                light_dir,
                Vector3::unit_y(),
            );
            
            let mut min_x = f32::MAX; let mut max_x = f32::MIN;
            let mut min_y = f32::MAX; let mut max_y = f32::MIN;
            let mut min_z = f32::MAX; let mut max_z = f32::MIN;
            
            for corner in frustum_corners {
                let light_space_corner = light_view * corner;
                min_x = min_x.min(light_space_corner.x); max_x = max_x.max(light_space_corner.x);
                min_y = min_y.min(light_space_corner.y); max_y = max_y.max(light_space_corner.y);
                min_z = min_z.min(light_space_corner.z); max_z = max_z.max(light_space_corner.z);
            }

            // 扩展 Z 轴以包含视锥体外的遮挡物
            let z_mult = 10.0;
            if min_z < 0.0 { min_z *= z_mult; } else { min_z /= z_mult; }
            if max_z < 0.0 { max_z /= z_mult; } else { max_z *= z_mult; }

            let light_proj = cgmath::ortho(min_x, max_x, min_y, max_y, min_z, max_z);
            self.csm_view_projs[i] = light_proj * light_view;
        }

        // 保持 shadow_view_proj 为第一个级联以兼容旧逻辑
        self.shadow_view_proj = self.csm_view_projs[0];
        
        let mut projs = [Matrix4::identity(); 4];
        for j in 0..4 {
            projs[j] = self.csm_view_projs[j];
        }
        let mut splits = [0.0; 4];
        for j in 0..cascade_count {
            splits[j] = self.csm_split_distances[j];
        }

        self.ctx.queue().write_buffer(
            &self.light_space_buffer,
            0,
            bytemuck::bytes_of(&crate::pipelines::LightSpaceBuffer::new(projs, splits)),
        );
    }

    /// 更新 Bloom 设置
    pub fn update_bloom_settings(&self, settings: BloomSettings) {
        self.ctx.queue().write_buffer(
            &self.bloom_settings_buffer,
            0,
            bytemuck::bytes_of(&settings),
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
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&self.shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&self.shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.light_space_buffer.as_entire_binding(),
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

        // 2. Bloom 阶段
        self.render_bloom(encoder);

        // 3. 后期处理阶段 (HDR + Bloom -> ToneMap -> Gamma -> target_view)
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

    /// Bloom 渲染阶段
    fn render_bloom(&self, encoder: &mut wgpu::CommandEncoder) {
        // 1. 提取亮度
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom 提取通道"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom_texture_views[0],
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: true },
                })],
                depth_stencil_attachment: None,
            });
            pass.set_pipeline(&self.pipelines.bloom.extract_pipeline);
            pass.set_bind_group(0, &self.bloom_extract_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        // 2. 水平模糊 (0 -> 1)
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom 水平模糊通道"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom_texture_views[1],
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: true },
                })],
                depth_stencil_attachment: None,
            });
            pass.set_pipeline(&self.pipelines.bloom.blur_h_pipeline);
            pass.set_bind_group(0, &self.bloom_blur_bind_groups[0], &[]);
            pass.draw(0..3, 0..1);
        }

        // 3. 垂直模糊 (1 -> 0)
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom 垂直模糊通道"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom_texture_views[0],
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: true },
                })],
                depth_stencil_attachment: None,
            });
            pass.set_pipeline(&self.pipelines.bloom.blur_v_pipeline);
            pass.set_bind_group(0, &self.bloom_blur_bind_groups[1], &[]);
            pass.draw(0..3, 0..1);
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
                    joint_indices: v.joint_indices,
                    joint_weights: v.joint_weights,
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
                gltf_mesh.skin_index.is_some(),
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

    /// 获取场景对象
    pub fn get_object(&self, id: uuid::Uuid) -> Option<&SceneObject> {
        self.resources.objects.get(&id)
    }

    /// 获取场景对象 (可变)
    pub fn get_object_mut(&mut self, id: uuid::Uuid) -> Option<&mut SceneObject> {
        self.resources.objects.get_mut(&id)
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
            object.update_model(self.ctx.queue(), model, object.bone_buffer.is_some());
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

    fn create_bloom_textures(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        format: wgpu::TextureFormat,
    ) -> ([wgpu::Texture; 2], [wgpu::TextureView; 2]) {
        let width = config.width / 4;
        let height = config.height / 4;
        
        let create_tex = || device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Bloom 渲染目标"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let t1 = create_tex();
        let t2 = create_tex();
        let v1 = t1.create_view(&wgpu::TextureViewDescriptor::default());
        let v2 = t2.create_view(&wgpu::TextureViewDescriptor::default());

        ([t1, t2], [v1, v2])
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
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [0.5, -0.5, 0.5],
            normal: [0.0, 0.0, 1.0],
            uv: [1.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [0.5, 0.5, 0.5],
            normal: [0.0, 0.0, 1.0],
            uv: [1.0, 1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [-0.5, 0.5, 0.5],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        // 后面
        Vertex {
            position: [-0.5, -0.5, -0.5],
            normal: [0.0, 0.0, -1.0],
            uv: [0.0, 0.0],
            tangent: [-1.0, 0.0, 0.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [0.5, -0.5, -0.5],
            normal: [0.0, 0.0, -1.0],
            uv: [1.0, 0.0],
            tangent: [-1.0, 0.0, 0.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [0.5, 0.5, -0.5],
            normal: [0.0, 0.0, -1.0],
            uv: [1.0, 1.0],
            tangent: [-1.0, 0.0, 0.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [-0.5, 0.5, -0.5],
            normal: [0.0, 0.0, -1.0],
            uv: [0.0, 1.0],
            tangent: [-1.0, 0.0, 0.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        // 左面
        Vertex {
            position: [-0.5, -0.5, -0.5],
            normal: [-1.0, 0.0, 0.0],
            uv: [0.0, 0.0],
            tangent: [0.0, 0.0, 1.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [-0.5, -0.5, 0.5],
            normal: [-1.0, 0.0, 0.0],
            uv: [1.0, 0.0],
            tangent: [0.0, 0.0, 1.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [-0.5, 0.5, 0.5],
            normal: [-1.0, 0.0, 0.0],
            uv: [1.0, 1.0],
            tangent: [0.0, 0.0, 1.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [-0.5, 0.5, -0.5],
            normal: [-1.0, 0.0, 0.0],
            uv: [0.0, 1.0],
            tangent: [0.0, 0.0, 1.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        // 右面
        Vertex {
            position: [0.5, -0.5, 0.5],
            normal: [1.0, 0.0, 0.0],
            uv: [0.0, 0.0],
            tangent: [0.0, 0.0, -1.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [0.5, -0.5, -0.5],
            normal: [1.0, 0.0, 0.0],
            uv: [1.0, 0.0],
            tangent: [0.0, 0.0, -1.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [0.5, 0.5, -0.5],
            normal: [1.0, 0.0, 0.0],
            uv: [1.0, 1.0],
            tangent: [0.0, 0.0, -1.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [0.5, 0.5, 0.5],
            normal: [1.0, 0.0, 0.0],
            uv: [0.0, 1.0],
            tangent: [0.0, 0.0, -1.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        // 上面
        Vertex {
            position: [-0.5, 0.5, 0.5],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [0.5, 0.5, 0.5],
            normal: [0.0, 1.0, 0.0],
            uv: [1.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [0.5, 0.5, -0.5],
            normal: [0.0, 1.0, 0.0],
            uv: [1.0, 1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [-0.5, 0.5, -0.5],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        // 下面
        Vertex {
            position: [-0.5, -0.5, -0.5],
            normal: [0.0, -1.0, 0.0],
            uv: [0.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [0.5, -0.5, -0.5],
            normal: [0.0, -1.0, 0.0],
            uv: [1.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [0.5, -0.5, 0.5],
            normal: [0.0, -1.0, 0.0],
            uv: [1.0, 1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        },
        Vertex {
            position: [-0.5, -0.5, 0.5],
            normal: [0.0, -1.0, 0.0],
            uv: [0.0, 1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
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
        false,
    )
}

