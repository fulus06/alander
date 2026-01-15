use egui;
use bevy_ecs::entity::Entity;
use crate::scene_manager::Scene;
use crate::app::EditorState;

/// 渲染场景大纲面板
pub fn show_hierarchy(
    ui: &mut egui::Ui,
    scene: &Scene,
    editor_state: &mut EditorState,
) {
    ui.heading("场景管理器");
    ui.separator();
    
    let entities = scene.get_entities_with_names();
    for (entity, name) in entities {
        let is_selected = Some(entity) == editor_state.selected_entity;
        
        let label = if is_selected {
            egui::RichText::new(format!("{} (E)", name))
                .strong()
                .color(egui::Color32::from_rgb(255, 255, 0))
        } else {
            egui::RichText::new(name)
        };
        
        if ui.selectable_label(is_selected, label).clicked() {
            editor_state.selected_entity = Some(entity);
            tracing::info!("选中实体: {:?}", entity);
        }
    }
}
