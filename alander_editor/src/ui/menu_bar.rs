use egui;
use crate::editor_command::CommandManager;

/// UI 操作响应
pub enum MenuAction {
    None,
    OpenScene,
    SaveScene,
    ImportModel,
    ImportHdr,
    Undo,
    Redo,
    ResetCamera,
    Exit,
}

/// 渲染顶部菜单栏
pub fn show_menu_bar(ui: &mut egui::Ui, command_manager: &CommandManager) -> MenuAction {
    let mut action = MenuAction::None;
    
    egui::menu::bar(ui, |ui| {
        ui.menu_button("文件", |ui| {
            if ui.button("打开场景").clicked() {
                action = MenuAction::OpenScene;
                ui.close_menu();
            }
            if ui.button("保存场景").clicked() {
                action = MenuAction::SaveScene;
                ui.close_menu();
            }
            ui.separator();
            if ui.button("导入模型 (glTF)").clicked() {
                action = MenuAction::ImportModel;
                ui.close_menu();
            }
            if ui.button("导入环境贴图 (HDR)").clicked() {
                action = MenuAction::ImportHdr;
                ui.close_menu();
            }
            ui.separator();
            if ui.button("退出").clicked() {
                action = MenuAction::Exit;
                ui.close_menu();
            }
        });

        ui.menu_button("编辑", |ui| {
            let undo_label = if let Some(name) = command_manager.last_undo_name() {
                format!("撤销 ({})", name)
            } else {
                "撤销".to_string()
            };
            if ui.add_enabled(command_manager.can_undo(), egui::Button::new(format!("{} (Ctrl+Z)", undo_label))).clicked() {
                action = MenuAction::Undo;
                ui.close_menu();
            }

            let redo_label = if let Some(name) = command_manager.last_redo_name() {
                format!("重做 ({})", name)
            } else {
                "重做".to_string()
            };
            if ui.add_enabled(command_manager.can_redo(), egui::Button::new(format!("{} (Ctrl+Shift+Z)", redo_label))).clicked() {
                action = MenuAction::Redo;
                ui.close_menu();
            }
        });
        
        ui.menu_button("视图", |ui| {
            if ui.button("重置相机").clicked() {
                action = MenuAction::ResetCamera;
                ui.close_menu();
            }
        });
    });
    
    action
}
