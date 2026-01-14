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
pub mod assets;

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

/// 测试ECS和资源管理功能
#[cfg(feature = "test")]
pub fn test_ecs_and_assets() {
    use bevy_ecs::prelude::*;
    use glam::Vec3;
    
    println!("=== 开始ECS和资源管理功能测试 ===");
    
    // 1. 创建世界和实体
    let mut world = World::new();
    let entity = world.spawn((
        scene::Name("测试实体".to_string()),
        scene::Transform::from_translation(Vec3::new(1.0, 2.0, 3.0)),
    )).id();
    
    println!("✅ 创建的实体ID: {:?}", entity);
    
    // 2. 验证实体组件
    let name = world.get::<scene::Name>(entity).unwrap();
    println!("✅ 实体名称: {}", name.0);
    
    let transform = world.get::<scene::Transform>(entity).unwrap();
    println!("✅ 实体变换: position={:?}", transform.position);
    
    // 3. 创建资源管理器并加载资源
    let mut mesh_manager = assets::AssetManager::<scene::MeshData>::new();
    
    // 创建测试网格数据
    let test_mesh = scene::MeshData {
        vertices: vec![
            scene::Vertex::new(Vec3::ZERO, Vec3::Z, Vec2::ZERO),
            scene::Vertex::new(Vec3::X, Vec3::Z, Vec2::X),
            scene::Vertex::new(Vec3::Y, Vec3::Z, Vec2::Y),
        ],
        indices: vec![0, 1, 2],
    };
    
    let mesh_handle = mesh_manager.load(test_mesh);
    println!("✅ 加载的网格句柄ID: {}", mesh_handle.id);
    
    // 4. 验证资源加载
    assert!(mesh_manager.contains(&mesh_handle));
    let loaded_mesh = mesh_manager.get(&mesh_handle);
    assert!(loaded_mesh.is_some());
    println!("✅ 资源加载验证成功!");
    
    // 5. 测试资源加载器
    let mut mesh_loader = assets::SimpleMeshLoader;
    let mesh_result = mesh_loader.load("cube");
    assert!(mesh_result.is_ok());
    println!("✅ 网格加载器测试成功!");
    
    let mut material_loader = assets::SimpleMaterialLoader;
    let material_result = material_loader.load("default");
    assert!(material_result.is_ok());
    println!("✅ 材质加载器测试成功!");
    
    println!("=== ECS和资源管理功能测试完成 ===");
}
