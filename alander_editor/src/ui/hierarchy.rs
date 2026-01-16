use alander_core::scene::{Name, Children, Parent};
use bevy_ecs::prelude::*;
use crate::scene_manager::Scene;
use crate::app::EditorState;

/// 渲染场景大纲面板
pub fn show_hierarchy(
    ui: &mut egui::Ui,
    scene: &mut Scene,
    editor_state: &mut EditorState,
) {
    ui.heading("场景层级");
    ui.separator();
    
    // 获取所有根节点 (没有 Parent 的节点)
    let mut roots = Vec::new();
    let mut query = scene.world.query_filtered::<Entity, Without<Parent>>();
    for entity in query.iter(&scene.world) {
        roots.push(entity);
    }
    
    egui::ScrollArea::vertical().show(ui, |ui| {
        for root in roots {
            render_entity_node(ui, root, scene, editor_state);
        }
    });
}

fn render_entity_node(
    ui: &mut egui::Ui,
    entity: Entity,
    scene: &mut Scene,
    editor_state: &mut EditorState,
) {
    let name = scene.world.get::<Name>(entity)
        .map(|n| n.0.clone())
        .unwrap_or_else(|| format!("未命名实体 {:?}", entity));
    
    let children = scene.world.get::<Children>(entity).map(|c| c.0.clone());
    let is_selected = Some(entity) == editor_state.selected_entity;
    
    let has_children = children.as_ref().map_or(false, |c: &Vec<Entity>| !c.is_empty());
    
    if has_children {
        let label = if is_selected {
            egui::RichText::new(format!("{} (E)", name)).strong().color(egui::Color32::from_rgb(255, 255, 0))
        } else {
            egui::RichText::new(&name)
        };

        let header_res = egui::CollapsingHeader::new(label)
            .id_source(entity)
            .default_open(true)
            .selectable(true)
            .selected(is_selected)
            .show(ui, |ui| {
                if let Some(child_list) = children {
                    for child in child_list {
                        render_entity_node(ui, child, scene, editor_state);
                    }
                }
            });
            
        if header_res.header_response.clicked() {
            editor_state.selected_entity = Some(entity);
        }
    } else {
        let label = if is_selected {
            egui::RichText::new(format!("{} (E)", name))
                .strong()
                .color(egui::Color32::from_rgb(255, 255, 0))
        } else {
            egui::RichText::new(name)
        };
        
        if ui.selectable_label(is_selected, label).clicked() {
            editor_state.selected_entity = Some(entity);
        }
    }
}
