//! 渲染管线模块
//!
//! 此模块包含所有渲染管线的定义和管理。

pub mod common;
pub mod mesh;
pub mod skybox;
pub mod debug;

pub use common::*;
pub use mesh::*;
pub use skybox::*;
pub use debug::*;

/// 管线集合
pub struct Pipelines {
    /// 基础网格管线
    pub mesh: MeshPipeline,
    /// 天空盒管线
    pub skybox: SkyboxPipeline,
    /// 调试管线
    pub debug: DebugPipeline,
}

impl Pipelines {
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, hdr_filterable: bool) -> Self {
        let mesh = MeshPipeline::new(device, config.format, hdr_filterable);
        let skybox = SkyboxPipeline::new(device, config.format, hdr_filterable);
        let debug = DebugPipeline::new(device, config.format);

        Self { mesh, skybox, debug }
    }
}
