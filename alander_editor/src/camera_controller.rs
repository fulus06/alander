use glam::{Vec3, Quat};
use alander_core::scene::Transform;

/// 轨道相机控制器
#[derive(Debug, Clone, Copy)]
pub struct OrbitController {
    /// 旋转 (Yaw, Pitch)
    pub rotation: (f32, f32),
    /// 距离
    pub distance: f32,
    /// 目标点
    pub target: Vec3,
    /// 是否正在拖动
    pub is_dragging: bool,
    /// 上次鼠标位置
    pub last_mouse_pos: (f32, f32),
}

impl OrbitController {
    /// 更新相机变换
    pub fn update_transform(&self, transform: &mut Transform) {
        let (yaw, pitch) = self.rotation;
        let distance = self.distance;

        // 基础旋转：先绕 Y 轴转 (yaw)，再绕 X 轴转 (pitch)
        let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch);
        
        // 计算相机在世界空间的位置
        // 相机默认看向中心，并加上 target 偏移
        transform.position = rotation * Vec3::new(0.0, 0.0, distance) + self.target;
        transform.rotation = rotation;
    }
}

impl Default for OrbitController {
    fn default() -> Self {
        Self {
            rotation: (0.0, -0.2),
            distance: 10.0,
            target: Vec3::ZERO,
            is_dragging: false,
            last_mouse_pos: (0.0, 0.0),
        }
    }
}
