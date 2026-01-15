//! Alander 渲染模块
//!
//! 此模块包含基于WGPU的渲染管线、材质系统和着色器管理。

use alander_core::scene::Transform;
use wgpu::util::DeviceExt;
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

pub mod pipelines;
pub mod renderer;
// pub mod shaders; // 暂时移除，后续添加
pub mod texture;
pub mod utils;

/// 渲染错误类型
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("WGPU错误: {0}")]
    Wgpu(#[from] wgpu::SurfaceError),
    #[error("请求适配器错误")]
    RequestAdapter,
    #[error("请求设备错误")]
    RequestDevice,
    #[error("窗口错误")]
    Window,
    #[error("创建表面错误")]
    CreateSurface(#[from] wgpu::CreateSurfaceError),
}

/// 渲染器配置
#[derive(Debug, Clone)]
pub struct RendererConfig {
    pub vsync: bool,
    pub power_preference: wgpu::PowerPreference,
    pub backends: wgpu::Backends,
    pub features: wgpu::Features,
    pub limits: wgpu::Limits,
}

impl Default for RendererConfig {
    fn default() -> Self {
        // 使用wgpu推荐的后端选择：
        // - macOS: Metal
        // - Windows: DX12
        // - Linux/其他: Vulkan
        let backends = wgpu::Backends::PRIMARY;

        Self {
            vsync: true,
            power_preference: wgpu::PowerPreference::HighPerformance,
            backends,
            features: wgpu::Features::default(),
            limits: wgpu::Limits::default(),
        }
    }
}

/// 渲染器资源
pub struct Renderer {
    adapter: wgpu::Adapter,
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    // 管线和着色器
    pipelines: pipelines::Pipelines,
}

impl Renderer {
    /// 创建新的渲染器
    pub async fn new(window: &Window, config: RendererConfig) -> Result<Self, RenderError> {
        let size = window.inner_size();

        // WGPU实例
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: config.backends,
            dx12_shader_compiler: Default::default(),
        });

        // 表面
        let surface = unsafe { instance.create_surface(window)? };

        // 适配器
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: config.power_preference,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or(RenderError::RequestAdapter)?;

        // 设备和队列
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("设备"),
                    features: config.features,
                    limits: config.limits,
                },
                None,
            )
            .await
            .map_err(|_| RenderError::RequestDevice)?;

        // 表面配置
        let capabilities = surface.get_capabilities(&adapter);
        let surface_format = capabilities
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(capabilities.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: if config.vsync {
                wgpu::PresentMode::Fifo
            } else {
                wgpu::PresentMode::AutoVsync
            },
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: vec![surface_format],
        };
        surface.configure(&device, &config);

        // 检测 HDR 过滤支持情况以配置管线布局
        let hdr_16_filterable = adapter.get_texture_format_features(wgpu::TextureFormat::Rgba16Float)
            .flags.contains(wgpu::TextureFormatFeatureFlags::FILTERABLE);
        let hdr_32_filterable = adapter.get_texture_format_features(wgpu::TextureFormat::Rgba32Float)
            .flags.contains(wgpu::TextureFormatFeatureFlags::FILTERABLE);
        // 如果 16 位支持过滤，或者 16 位不支持但 32 位支持，管线将支持过滤
        let hdr_filterable = hdr_16_filterable || hdr_32_filterable;

        // 创建渲染管线
        let pipelines = pipelines::Pipelines::new(&device, &config, hdr_filterable);

        Ok(Self {
            adapter,
            surface,
            device,
            queue,
            config,
            size,
            pipelines,
        })
    }

    /// 调整大小
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// 获取设备
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// 获取队列
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// 获取适配器
    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    /// 更新相机
    pub fn update_camera(
        &self,
        _camera: &alander_core::scene::Camera,
        _transform: &alander_core::scene::Transform,
    ) {
        // 这里需要实现相机更新逻辑
        // 暂时留空，后续实现
    }

    /// 渲染
    pub fn render(&self) -> Result<(), RenderError> {
        // 这里需要实现渲染逻辑
        // 暂时留空，后续实现
        Ok(())
    }

    /// 获取表面
    pub fn surface(&self) -> &wgpu::Surface {
        &self.surface
    }

    /// 获取配置
    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
    }

    /// 获取表面格式
    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// 获取大小
    pub fn size(&self) -> winit::dpi::PhysicalSize<u32> {
        self.size
    }

    /// 获取渲染管线
    pub fn pipelines(&self) -> &pipelines::Pipelines {
        &self.pipelines
    }
}

/// 渲染对象特征
pub trait Renderable {
    /// 准备渲染资源
    fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        transform: &Transform,
    ) -> Result<(), RenderError>;

    /// 渲染对象
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, transform: &'a Transform);
}

/// 缓冲区工具特征
pub trait BufferUtil<T>
where
    T: bytemuck::Pod,
{
    /// 从数据创建缓冲区
    fn from_data(device: &wgpu::Device, data: &[T], usage: wgpu::BufferUsages) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(data),
            usage,
        })
    }

    /// 从数据创建顶点缓冲区
    fn vertex_buffer(device: &wgpu::Device, data: &[T]) -> wgpu::Buffer {
        Self::from_data(device, data, wgpu::BufferUsages::VERTEX)
    }

    /// 从数据创建索引缓冲区
    fn index_buffer(device: &wgpu::Device, data: &[T]) -> wgpu::Buffer {
        Self::from_data(device, data, wgpu::BufferUsages::INDEX)
    }

    /// 从数据创建统一缓冲区
    fn uniform_buffer(device: &wgpu::Device, data: &T) -> wgpu::Buffer {
        Self::from_data(
            device,
            std::slice::from_ref(data),
            wgpu::BufferUsages::UNIFORM,
        )
    }

    /// 创建空缓冲区
    fn empty(device: &wgpu::Device, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage,
            mapped_at_creation: false,
        })
    }
}

impl<T> BufferUtil<T> for T where T: bytemuck::Pod {}

/// 创建窗口和事件循环
pub fn create_window(
    title: &str,
    size: winit::dpi::PhysicalSize<u32>,
) -> Result<(EventLoop<()>, Window), RenderError> {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(size)
        .build(&event_loop)
        .map_err(|_| RenderError::Window)?;

    Ok((event_loop, window))
}
