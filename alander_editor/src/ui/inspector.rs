use egui;
use bevy_ecs::prelude::*;
use crate::scene_manager::Scene;
use alander_core::scene::{Name, Transform, PointLight, PBRMaterial, RigidBody, Collider, RigidBodyType};
use glam::{EulerRot, Vec3, Vec4, Quat};

/// 渲染属性面板
pub fn show_inspector(
    ui: &mut egui::Ui,
    scene: &mut Scene,
    selected_entity: Option<Entity>,
) {
    ui.heading("属性面板");
    ui.separator();
    
    let entity = match selected_entity {
        Some(e) => e,
        None => {
            ui.vertical_centered(|ui| {
                ui.label("未选中任何实体");
            });
            return;
        }
    };

    // 1. 基础信息 (名称和 ID)
    let mut name_query = scene.world.query::<&Name>();
    if let Ok(name) = name_query.get(&scene.world, entity) {
        ui.horizontal(|ui| {
            ui.label("名称:");
            ui.strong(&name.0);
        });
    }
    ui.label(format!("实体 ID: {:?}", entity));
    ui.separator();
    
    // 2. Transform 编辑
    let mut transform_query = scene.world.query::<&mut Transform>();
    if let Ok(mut transform) = transform_query.get_mut(&mut scene.world, entity) {
        ui.collapsing("变换 (Transform)", |ui| {
            ui.label("位置");
            ui.horizontal(|ui| {
                ui.label("X"); ui.add(egui::DragValue::new(&mut transform.position.x).speed(0.1));
                ui.label("Y"); ui.add(egui::DragValue::new(&mut transform.position.y).speed(0.1));
                ui.label("Z"); ui.add(egui::DragValue::new(&mut transform.position.z).speed(0.1));
            });
            
            ui.label("缩放");
            ui.horizontal(|ui| {
                ui.label("X"); ui.add(egui::DragValue::new(&mut transform.scale.x).speed(0.01));
                ui.label("Y"); ui.add(egui::DragValue::new(&mut transform.scale.y).speed(0.01));
                ui.label("Z"); ui.add(egui::DragValue::new(&mut transform.scale.z).speed(0.01));
            });
            
            ui.label("旋转 (Euler)");
            let (mut yaw, mut pitch, mut roll) = transform.rotation.to_euler(EulerRot::YXZ);
            yaw = yaw.to_degrees();
            pitch = pitch.to_degrees();
            roll = roll.to_degrees();
            
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label("Y"); if ui.add(egui::DragValue::new(&mut yaw).speed(1.0)).changed() { changed = true; }
                ui.label("X"); if ui.add(egui::DragValue::new(&mut pitch).speed(1.0)).changed() { changed = true; }
                ui.label("Z"); if ui.add(egui::DragValue::new(&mut roll).speed(1.0)).changed() { changed = true; }
            });
            
            if changed {
                transform.rotation = Quat::from_euler(
                    EulerRot::YXZ,
                    yaw.to_radians(),
                    pitch.to_radians(),
                    roll.to_radians()
                );
            }

            if ui.button("重置变换").clicked() {
                *transform = Transform::default();
            }
        });
    }

    // 3. 点光源编辑
    let mut light_query = scene.world.query::<&mut PointLight>();
    if let Ok(mut light) = light_query.get_mut(&mut scene.world, entity) {
        ui.collapsing("点光源 (Point Light)", |ui| {
            ui.horizontal(|ui| {
                ui.label("颜色");
                let mut color_arr = [light.color.x, light.color.y, light.color.z];
                if ui.color_edit_button_rgb(&mut color_arr).changed() {
                    light.color = Vec3::from_slice(&color_arr);
                }
            });
            ui.horizontal(|ui| {
                ui.label("强度");
                ui.add(egui::DragValue::new(&mut light.intensity).speed(0.1).clamp_range(0.0..=100.0));
            });
            ui.horizontal(|ui| {
                ui.label("范围");
                ui.add(egui::DragValue::new(&mut light.range).speed(0.5).clamp_range(0.1..=1000.0));
            });
        });
    }

    // 4. PBR 材质编辑
    let mut material_query = scene.world.query::<&mut PBRMaterial>();
    if let Ok(mut mat) = material_query.get_mut(&mut scene.world, entity) {
        ui.collapsing("PBR 材质", |ui| {
            ui.horizontal(|ui| {
                ui.label("基色");
                let mut color_arr = [mat.base_color.x, mat.base_color.y, mat.base_color.z, mat.base_color.w];
                if ui.color_edit_button_rgba_unmultiplied(&mut color_arr).changed() {
                    mat.base_color = Vec4::from_slice(&color_arr);
                }
            });
            ui.horizontal(|ui| {
                ui.label("金属度");
                ui.add(egui::Slider::new(&mut mat.metallic, 0.0..=1.0));
            });
            ui.horizontal(|ui| {
                ui.label("粗糙度");
                ui.add(egui::Slider::new(&mut mat.roughness, 0.0..=1.0));
            });
            ui.collapsing("自发光 (Emissive)", |ui| {
                let mut color_arr = [mat.emissive.x, mat.emissive.y, mat.emissive.z];
                if ui.color_edit_button_rgb(&mut color_arr).changed() {
                    mat.emissive = Vec3::from_slice(&color_arr);
                }
            });
        });
    }

    // 5. 刚体 (RigidBody) 编辑
    let mut rb_query = scene.world.query::<&mut RigidBody>();
    if let Ok(mut rb) = rb_query.get_mut(&mut scene.world, entity) {
        ui.collapsing("刚体 (RigidBody)", |ui| {
            ui.horizontal(|ui| {
                ui.label("类型");
                let mut rb_type_idx = match rb.body_type {
                    RigidBodyType::Static => 0,
                    RigidBodyType::Dynamic => 1,
                    RigidBodyType::KinematicVelocityBased => 2,
                    RigidBodyType::KinematicPositionBased => 3,
                };
                
                let preview = match rb_type_idx {
                    0 => "静态 (Static)",
                    1 => "动态 (Dynamic)",
                    2 => "运动学 (速度)",
                    3 => "运动学 (位置)",
                    _ => "未知",
                };

                egui::ComboBox::from_id_source("rb_type_inspector")
                    .selected_text(preview)
                    .show_ui(ui, |ui| {
                        if ui.selectable_value(&mut rb_type_idx, 0, "静态 (Static)").clicked() { rb.body_type = RigidBodyType::Static; }
                        if ui.selectable_value(&mut rb_type_idx, 1, "动态 (Dynamic)").clicked() { rb.body_type = RigidBodyType::Dynamic; }
                        if ui.selectable_value(&mut rb_type_idx, 2, "运动学 (速度)").clicked() { rb.body_type = RigidBodyType::KinematicVelocityBased; }
                        if ui.selectable_value(&mut rb_type_idx, 3, "运动学 (位置)").clicked() { rb.body_type = RigidBodyType::KinematicPositionBased; }
                    });
            });
        });
    }

    // 6. 碰撞体 (Collider) 编辑
    let mut col_query = scene.world.query::<&mut Collider>();
    if let Ok(mut col) = col_query.get_mut(&mut scene.world, entity) {
        ui.collapsing("碰撞体 (Collider)", |ui| {
            ui.horizontal(|ui| {
                ui.label("摩擦力");
                ui.add(egui::DragValue::new(&mut col.friction).speed(0.01).clamp_range(0.0..=2.0));
            });
            ui.horizontal(|ui| {
                ui.label("弹性");
                ui.add(egui::DragValue::new(&mut col.restitution).speed(0.01).clamp_range(0.0..=1.0));
            });
        });
    }
}
