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
    
    let mut reparent_req = None;

    // 处理背景拖放 (移动回根目录)
    let panel_id = ui.id().with("hierarchy_panel");
    let bg_response = ui.interact(ui.available_rect_before_wrap(), panel_id, egui::Sense::hover());
    
    if bg_response.hovered() {
        if let Some(dragged_entity) = editor_state.dragged_entity {
            ui.ctx().debug_painter().debug_rect(bg_response.rect, egui::Color32::from_rgba_unmultiplied(255, 255, 0, 30), "Drop to Root");
            
            if ui.input(|i| i.pointer.any_released()) {
                reparent_req = Some((dragged_entity, None));
            }
        }
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for root in roots {
            if let Some(req) = render_entity_node(ui, root, scene, editor_state) {
                reparent_req = Some(req);
            }
        }
    });

    // 释放拖拽状态
    if ui.input(|i| i.pointer.any_released()) {
        editor_state.dragged_entity = None;
    }

    // 延迟执行层级变更，避免在遍历时修改 ECS
    if let Some((child, parent)) = reparent_req {
        // 防止自己拖给自己，或者拖给自己的子节点 (循环引用)
        if Some(child) != parent {
            scene.set_parent(child, parent);
            scene.update_hierarchy();
        }
    }
}

fn render_entity_node(
    ui: &mut egui::Ui,
    entity: Entity,
    scene: &mut Scene,
    editor_state: &mut EditorState,
) -> Option<(Entity, Option<Entity>)> {
    let name = scene.world.get::<Name>(entity)
        .map(|n| n.0.clone())
        .unwrap_or_else(|| format!("未命名实体 {:?}", entity));
    
    let children = scene.world.get::<Children>(entity).map(|c| c.0.clone());
    let is_selected = Some(entity) == editor_state.selected_entity;
    
    let has_children = children.as_ref().map_or(false, |c: &Vec<Entity>| !c.is_empty());
    
    let mut reparent_req = None;

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
                        if let Some(req) = render_entity_node(ui, child, scene, editor_state) {
                            reparent_req = Some(req);
                        }
                    }
                }
            });
            
        // 关键：通过重复 interact 注入 drag sense
        let response = ui.interact(header_res.header_response.rect, header_res.header_response.id, egui::Sense::click().union(egui::Sense::drag()));
        
        // 拖拽源处理
        if response.drag_started() {
            editor_state.dragged_entity = Some(entity);
        }

        // 放置目标处理
        if response.hovered() {
            if let Some(dragged_entity) = editor_state.dragged_entity {
                if dragged_entity != entity {
                    ui.ctx().debug_painter().rect_stroke(response.rect, 0.0, (2.0, egui::Color32::YELLOW));
                    if ui.input(|i| i.pointer.any_released()) {
                        reparent_req = Some((dragged_entity, Some(entity)));
                    }
                }
            }
        }

        if response.clicked() {
            editor_state.selected_entity = Some(entity);
        }
    } else {
        let label = if is_selected {
            egui::RichText::new(format!("{} (E)", name))
                .strong()
                .color(egui::Color32::from_rgb(255, 255, 0))
        } else {
            egui::RichText::new(&name)
        };
        
        // 关键：手动构造带拖拽的 SelectableLabel
        let res = ui.selectable_label(is_selected, label);
        let response = ui.interact(res.rect, res.id, egui::Sense::click().union(egui::Sense::drag()));
        
        // 拖拽源处理
        if response.drag_started() {
            editor_state.dragged_entity = Some(entity);
        }

        // 放置目标处理
        if response.hovered() {
            if let Some(dragged_entity) = editor_state.dragged_entity {
                if dragged_entity != entity {
                    ui.ctx().debug_painter().rect_stroke(response.rect, 0.0, (2.0, egui::Color32::YELLOW));
                    if ui.input(|i| i.pointer.any_released()) {
                        reparent_req = Some((dragged_entity, Some(entity)));
                    }
                }
            }
        }

        if response.clicked() {
            editor_state.selected_entity = Some(entity);
        }
    }

    // 拖拽中的视觉反馈 (跟随鼠标)
    if editor_state.dragged_entity == Some(entity) {
        if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
            ui.ctx().debug_painter().text(
                pointer_pos + egui::vec2(15.0, 15.0),
                egui::Align2::LEFT_TOP,
                format!("Dragging: {}", name),
                egui::FontId::proportional(14.0),
                egui::Color32::WHITE,
            );
        }
    }

    reparent_req
}
