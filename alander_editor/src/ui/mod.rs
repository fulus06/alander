pub mod hierarchy;
pub mod inspector;
pub mod menu_bar;
pub mod simulation_bar;

pub use menu_bar::MenuAction;

use egui;
use crate::scene_manager::SceneManager;
use crate::physics_manager::PhysicsManager;
use crate::gizmo_manager::GizmoManager;
use crate::app::EditorState;

pub struct EditorUI {
    // 目前没有需要持久化的内部状态，但保留结构以便后续扩展（如 DockState）
}

impl EditorUI {
    pub fn new() -> Self {
        Self {}
    }

    /// 渲染整个编辑器 UI 并返回执行的菜单操作
    pub fn draw(
        &mut self,
        ctx: &egui::Context,
        scene_manager: &mut SceneManager,
        physics_manager: &mut PhysicsManager,
        gizmo_manager: &mut GizmoManager,
        editor_state: &mut EditorState,
        frame_time: f32,
    ) -> MenuAction {
        let mut menu_action = MenuAction::None;

        // 1. 顶部菜单栏
        egui::TopBottomPanel::top("top_menu").show(ctx, |ui| {
            menu_action = menu_bar::show_menu_bar(ui);
        });

        // 2. 底部模拟控制栏
        egui::TopBottomPanel::bottom("simulation_bar").show(ctx, |ui| {
            simulation_bar::show_simulation_bar(
                ui,
                physics_manager,
                gizmo_manager,
                &mut editor_state.show_colliders,
                frame_time,
            );
        });

        // 3. 左侧场景面板
        egui::SidePanel::left("scene_panel")
            .resizable(true)
            .default_width(200.0)
            .show(ctx, |ui| {
                if let Some(scene) = scene_manager.active_scene() {
                    hierarchy::show_hierarchy(ui, scene, editor_state);
                }
            });

        // 4. 右侧属性面板
        egui::SidePanel::right("properties_panel")
            .resizable(true)
            .default_width(250.0)
            .show(ctx, |ui| {
                if let Some(scene) = scene_manager.active_scene_mut() {
                    inspector::show_inspector(ui, scene, editor_state.selected_entity);
                }
            });

        // 5. 中央面板（透明，显示 3D 视图）
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
            .show(ctx, |_ui| {
                // 中央区域留空
            });

        menu_action
    }
}
