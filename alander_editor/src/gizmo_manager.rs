use glam::{Vec3, Quat, Vec2, Mat4};
use alander_core::math::Ray;
use alander_core::scene::Transform;
use alander_render::pipelines::DebugVertex;
use bevy_ecs::prelude::*;

/// Gizmo 模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoMode {
    Translate,
    Rotate,
    Scale,
}

/// Gizmo 轴
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoAxis {
    X,
    Y,
    Z,
    // TODO: PlaneXY, PlaneYZ, PlaneXZ
}

/// Gizmo 管理器，负责变换句柄的状态与逻辑
pub struct GizmoManager {
    /// 当前模式
    pub mode: GizmoMode,
    /// 选中的轴
    pub hovered_axis: Option<GizmoAxis>,
    /// 正在拖拽的轴
    pub active_axis: Option<GizmoAxis>,
    
    /// 开始拖拽时的射线起始数据
    drag_start_ray: Option<Ray>,
    /// 开始拖拽时的实体变换副本
    initial_transform: Option<Transform>,
    /// 开始拖拽时，物体在轴上的初始投影位置或平面交点
    drag_start_value: f32,
    drag_start_point: Vec3,
}

impl GizmoManager {
    pub fn new() -> Self {
        Self {
            mode: GizmoMode::Translate,
            hovered_axis: None,
            active_axis: None,
            drag_start_ray: None,
            initial_transform: None,
            drag_start_value: 0.0,
            drag_start_point: Vec3::ZERO,
        }
    }

    /// 更新 Gizmo 逻辑
    pub fn update(
        &mut self,
        ray: &Ray,
        mouse_pos: Vec2,
        window_size: Vec2,
        view_proj: Mat4,
        is_mouse_pressed: bool,
        selected_entity: Option<Entity>,
        world: &mut World,
        camera_pos: Vec3,
    ) {
        let selected_entity = match selected_entity {
            Some(e) => e,
            None => {
                self.hovered_axis = None;
                self.active_axis = None;
                return;
            }
        };

        // 获取实体当前的变换
        let mut transform = match world.get::<Transform>(selected_entity) {
            Some(t) => *t,
            None => return,
        };

        // Gizmo 的视觉缩放：随着距离增加而变大，保持屏幕尺寸恒定
        let dist = (transform.position - camera_pos).length();
        let gizmo_scale = dist * 0.15; // 经验系数

        if let Some(active) = self.active_axis {
            // 正在拖拽中
            if !is_mouse_pressed {
                self.active_axis = None;
                return;
            }

            // 执行拖拽逻辑
            self.handle_drag(active, ray, &mut transform);
            
            // 将更新后的变换写回 World
            if let Some(mut t) = world.get_mut::<Transform>(selected_entity) {
                *t = transform;
            }
        } else {
            // 未拖拽，进行拾取检测
            self.hovered_axis = self.pick_gizmo(ray, &transform, gizmo_scale, mouse_pos, window_size, view_proj);

            if is_mouse_pressed && self.hovered_axis.is_some() {
                self.active_axis = self.hovered_axis;
                self.initial_transform = Some(transform);
                self.drag_start_ray = Some(*ray);
                
                // 初始化拖拽起始数据
                if let Some(axis) = self.active_axis {
                    self.init_drag_data(axis, ray, &transform);
                }
            }
        }
    }

    /// 拾取 Gizmo 句柄
    fn pick_gizmo(
        &self, 
        ray: &Ray, 
        transform: &Transform, 
        scale: f32, 
        mouse_pos: Vec2, 
        window_size: Vec2, 
        view_proj: Mat4
    ) -> Option<GizmoAxis> {
        let pos = transform.position;
        let axes = [
            (GizmoAxis::X, Vec3::X),
            (GizmoAxis::Y, Vec3::Y),
            (GizmoAxis::Z, Vec3::Z),
        ];

        let mut best_axis = None;
        let mut min_dist = 15.0; // 屏幕空间拾取阈值 (像素)

        match self.mode {
            GizmoMode::Translate | GizmoMode::Scale => {
                for (axis, dir) in axes {
                    let axis_start_2d = self.world_to_screen(pos, view_proj, window_size);
                    let axis_end_2d = self.world_to_screen(pos + dir * scale, view_proj, window_size);
                    
                    let dist = distance_to_segment_2d(mouse_pos, axis_start_2d, axis_end_2d);
                    if dist < min_dist {
                        min_dist = dist;
                        best_axis = Some(axis);
                    }
                }
            }
            GizmoMode::Rotate => {
                for (axis, normal) in axes {
                    // 简化圆环拾取：采样圆环上的多个点投影到 2D
                    let segments = 32;
                    let (t1, t2) = find_orthonormal_basis(normal);
                    
                    for i in 0..segments {
                        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
                        let p = pos + (t1 * angle.cos() + t2 * angle.sin()) * scale;
                        let p_2d = self.world_to_screen(p, view_proj, window_size);
                        
                        let dist = mouse_pos.distance(p_2d);
                        if dist < min_dist {
                            min_dist = dist;
                            best_axis = Some(axis);
                        }
                    }
                }
            }
        }

        best_axis
    }

    /// 世界坐标转屏幕像素坐标
    fn world_to_screen(&self, point: Vec3, view_proj: Mat4, window_size: Vec2) -> Vec2 {
        let clip = view_proj.project_point3(point);
        let screen_x = (clip.x + 1.0) * 0.5 * window_size.x;
        let screen_y = (1.0 - clip.y) * 0.5 * window_size.y;
        Vec2::new(screen_x, screen_y)
    }

    /// 初始化拖拽数据
    fn init_drag_data(&mut self, axis: GizmoAxis, ray: &Ray, transform: &Transform) {
        let dir = match axis {
            GizmoAxis::X => Vec3::X,
            GizmoAxis::Y => Vec3::Y,
            GizmoAxis::Z => Vec3::Z,
        };

        match self.mode {
            GizmoMode::Translate => {
                if let Some((_t_ray, t_axis)) = ray_to_line_closest_points(
                    ray.origin, ray.direction,
                    transform.position, dir
                ) {
                    self.drag_start_value = t_axis;
                    self.drag_start_point = transform.position + dir * t_axis;
                }
            }
            GizmoMode::Scale => {
                if let Some((_t_ray, t_axis)) = ray_to_line_closest_points(
                    ray.origin, ray.direction,
                    transform.position, dir
                ) {
                    self.drag_start_value = t_axis;
                    self.drag_start_point = transform.position + dir * t_axis;
                }
            }
            GizmoMode::Rotate => {
                // 记录初始点击位置在旋转平面的投影
                if let Some(hit_point) = ray_plane_intersection(ray.origin, ray.direction, transform.position, dir) {
                    self.drag_start_point = hit_point;
                    let _to_point = (hit_point - transform.position).normalize();
                    // 记录初始角度（通过反正切）
                    // 我们需要一个局部的 2D 坐标系在平面上
                    // 这里简化处理：直接记录初始方向向量即可
                }
            }
        }
    }

    /// 处理拖拽
    fn handle_drag(&mut self, axis: GizmoAxis, ray: &Ray, transform: &mut Transform) {
        let dir = match axis {
            GizmoAxis::X => Vec3::X,
            GizmoAxis::Y => Vec3::Y,
            GizmoAxis::Z => Vec3::Z,
        };

        match self.mode {
            GizmoMode::Translate => {
                if let Some((_t_ray, t_axis)) = ray_to_line_closest_points(
                    ray.origin, ray.direction,
                    self.initial_transform.unwrap().position, dir
                ) {
                    let delta = t_axis - self.drag_start_value;
                    transform.position = self.initial_transform.unwrap().position + dir * delta;
                }
            }
            GizmoMode::Scale => {
                if let Some((_t_ray, t_axis)) = ray_to_line_closest_points(
                    ray.origin, ray.direction,
                    self.initial_transform.unwrap().position, dir
                ) {
                    let delta = t_axis - self.drag_start_value;
                    let initial_scale = self.initial_transform.unwrap().scale;
                    let scale_factor = 1.0 + delta / (self.initial_transform.unwrap().position.distance(self.drag_start_point).max(0.1));
                    
                    match axis {
                        GizmoAxis::X => transform.scale.x = initial_scale.x * scale_factor,
                        GizmoAxis::Y => transform.scale.y = initial_scale.y * scale_factor,
                        GizmoAxis::Z => transform.scale.z = initial_scale.z * scale_factor,
                    }
                }
            }
            GizmoMode::Rotate => {
                if let Some(hit_point) = ray_plane_intersection(ray.origin, ray.direction, transform.position, dir) {
                    let start_dir = (self.drag_start_point - transform.position).normalize();
                    let current_dir = (hit_point - transform.position).normalize();
                    
                    // 计算夹角
                    let dot = start_dir.dot(current_dir).clamp(-1.0, 1.0);
                    let angle = dot.acos();
                    
                    // 确定旋转方向
                    let cross = start_dir.cross(current_dir);
                    let sign = if cross.dot(dir) >= 0.0 { 1.0 } else { -1.0 };
                    
                    let rotation_delta = Quat::from_axis_angle(dir, angle * sign);
                    transform.rotation = rotation_delta * self.initial_transform.unwrap().rotation;
                }
            }
        }
    }

    /// 生成渲染线段
    pub fn render(&self, transform: &Transform, camera_pos: Vec3) -> Vec<DebugVertex> {
        let mut vertices = Vec::new();
        let pos = transform.position;
        let dist = (pos - camera_pos).length();
        let scale = dist * 0.15;

        let axes = [
            (GizmoAxis::X, Vec3::X, [1.0, 0.0, 0.0, 1.0]), // Red
            (GizmoAxis::Y, Vec3::Y, [0.0, 1.0, 0.0, 1.0]), // Green
            (GizmoAxis::Z, Vec3::Z, [0.0, 0.0, 1.0, 1.0]), // Blue
        ];

        for (axis, dir, color) in axes {
            let mut final_color = color;
            if Some(axis) == self.hovered_axis || Some(axis) == self.active_axis {
                final_color = [1.0, 1.0, 0.0, 1.0]; // Yellow
            }

            match self.mode {
                GizmoMode::Translate | GizmoMode::Scale => {
                    let end = pos + dir * scale;
                    vertices.push(DebugVertex { position: pos.into(), color: final_color });
                    vertices.push(DebugVertex { position: end.into(), color: final_color });

                    if self.mode == GizmoMode::Scale {
                        // 画个小方块在末端
                        draw_box_at(&mut vertices, end, scale * 0.05, final_color);
                    }
                }
                GizmoMode::Rotate => {
                    // 画圆环
                    draw_circle(&mut vertices, pos, dir, scale, final_color);
                }
            }
        }

        vertices
    }
}

// --- 渲染辅助函数 ---

fn draw_box_at(vertices: &mut Vec<DebugVertex>, pos: Vec3, size: f32, color: [f32; 4]) {
    let half = size * 0.5;
    let corners = [
        pos + Vec3::new(-half, -half, -half),
        pos + Vec3::new( half, -half, -half),
        pos + Vec3::new( half,  half, -half),
        pos + Vec3::new(-half,  half, -half),
        pos + Vec3::new(-half, -half,  half),
        pos + Vec3::new( half, -half,  half),
        pos + Vec3::new( half,  half,  half),
        pos + Vec3::new(-half,  half,  half),
    ];

    let edges = [
        (0, 1), (1, 2), (2, 3), (3, 0), // Bottom
        (4, 5), (5, 6), (6, 7), (7, 4), // Top
        (0, 4), (1, 5), (2, 6), (3, 7), // Sides
    ];

    for (start, end) in edges {
        vertices.push(DebugVertex { position: corners[start].into(), color });
        vertices.push(DebugVertex { position: corners[end].into(), color });
    }
}

fn draw_circle(vertices: &mut Vec<DebugVertex>, center: Vec3, normal: Vec3, radius: f32, color: [f32; 4]) {
    let segments = 32;
    // 找到平面的两个切向量
    let (t1, t2) = find_orthonormal_basis(normal);

    let mut prev_p = center + t1 * radius;
    for i in 1..=segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let p = center + (t1 * angle.cos() + t2 * angle.sin()) * radius;
        vertices.push(DebugVertex { position: prev_p.into(), color });
        vertices.push(DebugVertex { position: p.into(), color });
        prev_p = p;
    }
}

fn find_orthonormal_basis(normal: Vec3) -> (Vec3, Vec3) {
    let t1 = if normal.x.abs() < 0.9 {
        Vec3::X.cross(normal).normalize()
    } else {
        Vec3::Y.cross(normal).normalize()
    };
    let t2 = normal.cross(t1).normalize();
    (t1, t2)
}

// --- 数学工具函数 ---

/// 射线与平面的交点
fn ray_plane_intersection(ray_origin: Vec3, ray_dir: Vec3, plane_pos: Vec3, plane_normal: Vec3) -> Option<Vec3> {
    let denom = plane_normal.dot(ray_dir);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (plane_pos - ray_origin).dot(plane_normal) / denom;
    if t < 0.0 {
        return None;
    }
    Some(ray_origin + ray_dir * t)
}


/// 计算两条直线（射线 vs 无限直线）最接近的两个点的参数 t1, t2
fn ray_to_line_closest_points(o1: Vec3, d1: Vec3, o2: Vec3, d2: Vec3) -> Option<(f32, f32)> {
    let r = o1 - o2;
    let a = d1.dot(d1);
    let b = d1.dot(d2);
    let c = d1.dot(r);
    let e = d2.dot(d2);
    let f = d2.dot(r);
    
    let det = a * e - b * b;
    if det.abs() < 1e-6 {
        return None;
    }
    
    let t1 = (b * f - c * e) / det;
    let t2 = (a * f - b * c) / det;
    
    Some((t1, t2))
}

/// 计算点到线段的像素距离 (2D)
fn distance_to_segment_2d(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let l2 = a.distance_squared(b);
    if l2 == 0.0 { return p.distance(a); }
    let t = ((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / l2;
    let t = t.clamp(0.0, 1.0);
    p.distance(a + t * (b - a))
}
