use egui;
use crate::physics_manager::PhysicsManager;
use crate::gizmo_manager::{GizmoManager, GizmoMode};

/// 渲染模拟控制栏
pub fn show_simulation_bar(
    ui: &mut egui::Ui,
    physics_manager: &mut PhysicsManager,
    gizmo_manager: &mut GizmoManager,
    show_colliders: &mut bool,
    frame_time: f32,
) {
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(egui::RichText::new(format!("帧时间: {:>5.2}ms", frame_time * 1000.0)).monospace()));
        ui.separator();
        
        let play_text = if physics_manager.is_running { "⏸ 暂停物理模拟" } else { "▶ 开始物理模拟" };
        if ui.button(play_text).clicked() {
            physics_manager.is_running = !physics_manager.is_running;
        }
        
        ui.separator();
        ui.label("重力:");
        ui.add(egui::DragValue::new(&mut physics_manager.gravity.y).speed(0.1));
        
        ui.separator();
        ui.checkbox(show_colliders, "显示碰撞体");
        
        ui.separator();
        ui.label("变换工具:");
        ui.selectable_value(&mut gizmo_manager.mode, GizmoMode::Translate, "位移");
        ui.selectable_value(&mut gizmo_manager.mode, GizmoMode::Rotate, "旋转");
        ui.selectable_value(&mut gizmo_manager.mode, GizmoMode::Scale, "缩放");
    });
}
