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
    // 排序以保证 UI 显示顺序稳定 (按 Entity 索引排序)
    roots.sort();
    
    let mut reparent_req = None;
    let mut any_node_hovered = false;

    egui::ScrollArea::vertical().show(ui, |ui| {
        for root in roots {
            if let Some(req) = render_entity_node(ui, root, scene, editor_state, &mut any_node_hovered) {
                reparent_req = Some(req);
            }
        }

        // 处理背景拖放 (移动回根目录)
        // 只有在没有节点被悬停的情况下，背景才响应拖放
        if !any_node_hovered {
            let space_rect = ui.available_rect_before_wrap();
            let bg_response = ui.interact(space_rect, ui.id().with("bg"), egui::Sense::hover());
            
            if bg_response.hovered() {
                if let Some(dragged_entity) = editor_state.dragged_entity {
                    ui.ctx().debug_painter().debug_rect(
                        bg_response.rect, 
                        egui::Color32::from_rgba_unmultiplied(255, 255, 0, 20), 
                        "Drop to Root"
                    );
                    
                    if ui.input(|i| i.pointer.any_released()) {
                        reparent_req = Some((dragged_entity, None));
                    }
                }
            }
        }
    });

    // 释放拖拽状态
    if ui.input(|i| i.pointer.any_released()) {
        editor_state.dragged_entity = None;
    }

    // 延迟执行层级变更，避免在遍历时修改 ECS
    if let Some((child, parent)) = reparent_req {
        scene.set_parent(child, parent);
        scene.update_hierarchy();
    }
}

fn render_entity_node(
    ui: &mut egui::Ui,
    entity: Entity,
    scene: &mut Scene,
    editor_state: &mut EditorState,
    any_node_hovered: &mut bool,
) -> Option<(Entity, Option<Entity>)> {
    let name = scene.world.get::<Name>(entity)
        .map(|n| n.0.clone())
        .unwrap_or_else(|| format!("未命名实体 {:?}", entity));
    
    let children = scene.world.get::<Children>(entity).map(|c| c.0.clone());
    let is_selected = Some(entity) == editor_state.selected_entity;
    let is_dragged = editor_state.dragged_entity == Some(entity);
    
    let has_children = children.as_ref().map_or(false, |c: &Vec<Entity>| !c.is_empty());
    let mut req = None;

    let label_text = if is_selected {
        egui::RichText::new(format!("{} (E)", name)).strong().color(egui::Color32::from_rgb(255, 255, 0))
    } else {
        egui::RichText::new(&name)
    };

    let response = if has_children {
        let header_res = egui::CollapsingHeader::new(label_text)
            .id_source(entity)
            .default_open(true)
            .selectable(true)
            .selected(is_selected)
            .show(ui, |ui| {
                if let Some(mut child_list) = children {
                    child_list.sort();
                    for child in child_list {
                        if let Some(child_req) = render_entity_node(ui, child, scene, editor_state, any_node_hovered) {
                            req = Some(child_req);
                        }
                    }
                }
            });
        
        // 使用 union(drag) 注入拖拽感知
        ui.interact(header_res.header_response.rect, header_res.header_response.id, egui::Sense::click().union(egui::Sense::drag()))
    } else {
        let res = ui.selectable_label(is_selected, label_text);
        ui.interact(res.rect, res.id, egui::Sense::click().union(egui::Sense::drag()))
    };

    // --- 拖拽源逻辑 ---
    if response.drag_started() {
        editor_state.dragged_entity = Some(entity);
    }

    // --- 放置目标逻辑 ---
    if let Some(dragged) = editor_state.dragged_entity {
        // 使用 rect_contains_pointer 获得更稳定的悬停检测
        if dragged != entity && ui.rect_contains_pointer(response.rect) {
            *any_node_hovered = true;
            
            // 绘制黄色边框提示
            ui.painter().rect_stroke(response.rect, 0.0, (2.0, egui::Color32::YELLOW));

            if ui.input(|i| i.pointer.any_released()) {
                req = Some((dragged, Some(entity)));
            }
        }
    }

    // --- 点击选中逻辑 ---
    if response.clicked() {
        editor_state.selected_entity = Some(entity);
    }

    // --- 拖拽游标反馈 ---
    if is_dragged {
        if let Some(pos) = ui.ctx().pointer_interact_pos() {
            ui.ctx().debug_painter().text(
                pos + egui::vec2(15.0, 15.0),
                egui::Align2::LEFT_TOP,
                format!("Dragging: {}", name),
                egui::FontId::proportional(14.0),
                egui::Color32::WHITE,
            );
        }
    }

    req
}
