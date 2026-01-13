//! 渲染器工具模块
//!
//! 提供渲染器相关的辅助功能和工具函数。

/// 缓冲区工具特征
pub trait BufferUtil<T>
where
    T: bytemuck::Pod,
{
    /// 从数据创建缓冲区
    fn from_data(device: &wgpu::Device, data: &[T], usage: wgpu::BufferUsages) -> wgpu::Buffer {
        wgpu::util::DeviceExt::create_buffer_init(device, &wgpu::util::BufferInitDescriptor {
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

    /// 从Pod数据创建统一缓冲区
    fn uniform_buffer_from_pod(device: &wgpu::Device, data: &T) -> wgpu::Buffer {
        Self::from_data(
            device,
            std::slice::from_ref(data),
            wgpu::BufferUsages::UNIFORM,
        )
    }

    /// 从字节数据创建统一缓冲区
    fn uniform_buffer_from_bytes(device: &wgpu::Device, data: &[u8]) -> wgpu::Buffer {
        wgpu::util::DeviceExt::create_buffer_init(device, &wgpu::util::BufferInitDescriptor {
            label: None,
            contents: data,
            usage: wgpu::BufferUsages::UNIFORM,
        })
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
