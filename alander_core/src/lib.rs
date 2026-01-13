//! Alander核心模块
//!
//! 此模块包含Alander的基础数据结构、ECS系统和核心功能。

pub use bevy_ecs::prelude::*;
use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;

/// 数学类型导出
pub mod math {
    pub use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
}

/// 资源系统
pub mod assets {
    use super::*;
    use std::sync::Arc;

    /// 唯一资源标识符
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct Handle<T> {
        pub id: u64,
        _phantom: std::marker::PhantomData<T>,
    }

    impl<T> Handle<T> {
        pub fn new(id: u64) -> Self {
            Self {
                id,
                _phantom: std::marker::PhantomData,
            }
        }
    }

    /// 资源加载器特征
    pub trait AssetLoader<T> {
        fn load(&mut self, source: &str) -> Result<T, AssetError>;
    }

    /// 资源错误
    #[derive(Debug, thiserror::Error)]
    pub enum AssetError {
        #[error("IO错误: {0}")]
        Io(#[from] std::io::Error),
        #[error("解析错误: {0}")]
        Parse(String),
        #[error("资源未找到: {0}")]
        NotFound(String),
    }
}

/// 场景系统
pub mod scene {
    use super::*;

    /// 场景实体组件
    #[derive(Component, Debug, Clone, Serialize, Deserialize)]
    pub struct Name(pub String);

    /// 变换组件
    #[derive(Component, Debug, Clone, Copy, Serialize, Deserialize)]
    pub struct Transform {
        pub position: Vec3,
        pub rotation: Quat,
        pub scale: Vec3,
    }

    impl Default for Transform {
        fn default() -> Self {
            Self {
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            }
        }
    }

    impl Transform {
        /// 创建新的变换
        pub fn from(position: Vec3, rotation: Quat, scale: Vec3) -> Self {
            Self {
                position,
                rotation,
                scale,
            }
        }

        /// 从平移向量创建
        pub fn from_translation(translation: Vec3) -> Self {
            Self {
                position: translation,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            }
        }

        /// 从旋转四元数创建
        pub fn from_rotation(rotation: Quat) -> Self {
            Self {
                position: Vec3::ZERO,
                rotation,
                scale: Vec3::ONE,
            }
        }

        /// 从缩放向量创建
        pub fn from_scale(scale: Vec3) -> Self {
            Self {
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale,
            }
        }

        /// 计算变换矩阵
        pub fn compute_matrix(&self) -> Mat4 {
            Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
        }
    }

    /// 网格数据
    #[derive(Component, Debug, Clone, Serialize, Deserialize)]
    pub struct Mesh {
        pub handle: super::assets::Handle<MeshData>,
    }

    /// 材质数据
    #[derive(Component, Debug, Clone, Serialize, Deserialize)]
    pub struct Material {
        pub handle: super::assets::Handle<MaterialData>,
    }

    /// 网格数据资源
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MeshData {
        pub vertices: Vec<Vertex>,
        pub indices: Vec<u32>,
    }

    /// 顶点数据
    #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
    pub struct Vertex {
        pub position: Vec3,
        pub normal: Vec3,
        pub uv: Vec2,
    }

    impl Vertex {
        /// 创建新顶点
        pub fn new(position: Vec3, normal: Vec3, uv: Vec2) -> Self {
            Self {
                position,
                normal,
                uv,
            }
        }
    }

    /// 材质数据资源
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MaterialData {
        pub name: String,
        pub base_color: Vec4,
        pub metallic: f32,
        pub roughness: f32,
        pub normal_texture: Option<String>,
        pub base_color_texture: Option<String>,
        pub metallic_roughness_texture: Option<String>,
    }

    impl Default for MaterialData {
        fn default() -> Self {
            Self {
                name: "默认材质".to_string(),
                base_color: Vec4::new(0.8, 0.8, 0.8, 1.0),
                metallic: 0.0,
                roughness: 0.5,
                normal_texture: None,
                base_color_texture: None,
                metallic_roughness_texture: None,
            }
        }
    }

    /// 相机组件
    #[derive(Component, Debug, Clone, Serialize, Deserialize)]
    pub struct Camera {
        pub projection: Projection,
        pub viewport: Viewport,
    }

    impl Camera {
        /// 创建透视相机
        pub fn perspective(fov_y: f32, aspect_ratio: f32, near: f32, far: f32) -> Self {
            Self {
                projection: Projection::Perspective(Perspective {
                    fov_y,
                    aspect_ratio,
                    near,
                    far,
                }),
                viewport: Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: 800.0,
                    height: 600.0,
                },
            }
        }

        /// 计算视图矩阵
        pub fn view_matrix(&self, transform: &Transform) -> Mat4 {
            transform.compute_matrix().inverse()
        }

        /// 计算投影矩阵
        pub fn projection_matrix(&self) -> Mat4 {
            match &self.projection {
                Projection::Perspective(p) => {
                    Mat4::perspective_rh_gl(p.fov_y, p.aspect_ratio, p.near, p.far)
                }
            }
        }
    }

    /// 投影类型
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Projection {
        Perspective(Perspective),
    }

    /// 透视投影参数
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Perspective {
        pub fov_y: f32,
        pub aspect_ratio: f32,
        pub near: f32,
        pub far: f32,
    }

    /// 视口参数
    #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
    pub struct Viewport {
        pub x: f32,
        pub y: f32,
        pub width: f32,
        pub height: f32,
    }
}

/// 时间系统
#[derive(Resource, Debug, Clone)]
pub struct Time {
    /// 从应用启动经过总时间（秒）
    pub elapsed: f32,
    /// 上一帧和当前帧之间经过的时间（秒）
    pub delta: f32,
}

impl Default for Time {
    fn default() -> Self {
        Self {
            elapsed: 0.0,
            delta: 0.0,
        }
    }
}

/// 输入系统
#[derive(Resource, Debug, Default)]
pub struct InputState {
    /// 当前帧键盘按键状态
    pub keyboard: HashMap<winit::event::VirtualKeyCode, winit::event::ElementState>,
    /// 当前帧鼠标按键状态
    pub mouse_buttons: HashMap<winit::event::MouseButton, winit::event::ElementState>,
    /// 鼠标位置
    pub mouse_position: Vec2,
    /// 鼠标滚轮增量
    pub mouse_scroll_delta: Vec2,
}

impl InputState {
    /// 检查键是否按下
    pub fn key_pressed(&self, key: winit::event::VirtualKeyCode) -> bool {
        self.keyboard
            .get(&key)
            .map(|&state| state == winit::event::ElementState::Pressed)
            .unwrap_or(false)
    }

    /// 检查鼠标键是否按下
    pub fn mouse_button_pressed(&self, button: winit::event::MouseButton) -> bool {
        self.mouse_buttons
            .get(&button)
            .map(|&state| state == winit::event::ElementState::Pressed)
            .unwrap_or(false)
    }
}

/// 渲染系统提交的系统信息
#[derive(Resource, Debug, Clone)]
pub struct RenderState {
    pub surface_size: (u32, u32),
    pub scale_factor: f64,
}

/// 事件系统
pub mod events {
    use super::*;

    /// 网格加载事件
    #[derive(Debug, Clone)]
    pub struct MeshLoadedEvent {
        pub handle: super::assets::Handle<super::scene::MeshData>,
        pub mesh_data: super::scene::MeshData,
    }

    /// 材质加载事件
    #[derive(Debug, Clone)]
    pub struct MaterialLoadedEvent {
        pub handle: super::assets::Handle<super::scene::MaterialData>,
        pub material_data: super::scene::MaterialData,
    }

    /// 场景变更事件
    #[derive(Debug, Clone)]
    pub struct SceneChangedEvent {
        pub entity: Entity,
        pub change_type: SceneChangeType,
    }

    /// 场景变更类型
    #[derive(Debug, Clone)]
    pub enum SceneChangeType {
        TransformChanged {
            old_transform: super::scene::Transform,
            new_transform: super::scene::Transform,
        },
        MeshChanged {
            old_mesh: super::scene::Mesh,
            new_mesh: super::scene::Mesh,
        },
        MaterialChanged {
            old_material: super::scene::Material,
            new_material: super::scene::Material,
        },
    }
}
