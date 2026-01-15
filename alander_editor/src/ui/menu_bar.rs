use egui;

/// UI 操作响应
pub enum MenuAction {
    None,
    OpenScene,
    SaveScene,
    ImportModel,
    ImportHdr,
    ResetCamera,
    Exit,
}

/// 渲染顶部菜单栏
pub fn show_menu_bar(ui: &mut egui::Ui) -> MenuAction {
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
        
        ui.menu_button("视图", |ui| {
            if ui.button("重置相机").clicked() {
                action = MenuAction::ResetCamera;
                ui.close_menu();
            }
        });
    });
    
    action
}
