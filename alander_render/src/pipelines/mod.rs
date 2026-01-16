//! 渲染管线模块
//!
//! 此模块包含所有渲染管线的定义和管理。

pub mod common;
pub mod mesh;
pub mod skybox;
pub mod debug;
pub mod post_process;
pub mod shadow;

pub use common::*;
pub use mesh::*;
pub use skybox::*;
pub use debug::*;
pub use post_process::*;
pub use shadow::*;

/// 管线集合
pub struct Pipelines {
    /// 基础网格管线
    pub mesh: MeshPipeline,
    /// 天空盒管线
    pub skybox: SkyboxPipeline,
    /// 调试管线
    pub debug: DebugPipeline,
    /// 后期处理管线
    pub post_process: PostProcessPipeline,
    /// Bloom 管线
    pub bloom: BloomPipeline,
    /// 阴影管线
    pub shadow: ShadowPipeline,
}

impl Pipelines {
    pub fn new(device: &wgpu::Device, hdr_format: wgpu::TextureFormat, sdr_format: wgpu::TextureFormat, hdr_filterable: bool) -> Self {
        let mesh = MeshPipeline::new(device, hdr_format, hdr_filterable);
        let skybox = SkyboxPipeline::new(device, hdr_format, hdr_filterable);
        let debug = DebugPipeline::new(device, hdr_format);
        let post_process = PostProcessPipeline::new(device, sdr_format);
        let bloom = BloomPipeline::new(device, hdr_format);
        let shadow = ShadowPipeline::new(device);

        Self { mesh, skybox, debug, post_process, bloom, shadow }
    }
}
