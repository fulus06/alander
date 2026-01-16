use egui;
use bevy_ecs::prelude::*;
use crate::scene_manager::Scene;
use alander_core::scene::{Name, Transform, PointLight, PBRMaterial, RigidBody, Collider, RigidBodyType, Camera, Projection, AnimationPlayer};
use glam::{EulerRot, Vec3, Vec4, Quat};
use crate::app::EditorState;

/// 渲染属性面板
pub fn show_inspector(
    ui: &mut egui::Ui,
    scene: &mut Scene,
    editor_state: &mut EditorState,
) {
    let selected_entity = editor_state.selected_entity;
    ui.heading("属性面板");
    ui.separator();
    
    let entity = match selected_entity {
        Some(e) => e,
        None => {
            ui.collapsing("渲染预览设置 (Rendering)", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Bloom 阈值");
                    ui.add(egui::Slider::new(&mut editor_state.bloom_threshold, 0.0..=5.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Bloom 强度");
                    ui.add(egui::Slider::new(&mut editor_state.bloom_intensity, 0.0..=2.0));
                });
            });

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

    // 7. 相机 (Camera) 编辑
    let mut camera_query = scene.world.query::<&mut Camera>();
    if let Ok(mut camera) = camera_query.get_mut(&mut scene.world, entity) {
        ui.collapsing("相机 (Camera)", |ui| {
            let mut is_active = Some(entity) == editor_state.active_camera_entity;
            if ui.checkbox(&mut is_active, "激活此相机").changed() {
                if is_active {
                    editor_state.active_camera_entity = Some(entity);
                } else {
                    editor_state.active_camera_entity = None;
                }
            }

            match &mut camera.projection {
                Projection::Perspective(ref mut p) => {
                    ui.label("透视投影 (Perspective)");
                    ui.horizontal(|ui| {
                        ui.label("Fov (Y)");
                        let mut fov_deg = p.fov_y.to_degrees();
                        if ui.add(egui::DragValue::new(&mut fov_deg).speed(0.1).clamp_range(1.0..=179.0)).changed() {
                            p.fov_y = fov_deg.to_radians();
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("近平面");
                        ui.add(egui::DragValue::new(&mut p.near).speed(0.01).clamp_range(0.001..=10.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("远平面");
                        ui.add(egui::DragValue::new(&mut p.far).speed(1.0).clamp_range(0.1..=10000.0));
                    });
                }
            }
        });
    }

    // 8. 动画播放器 (AnimationPlayer) 编辑
    let current_transform = scene.world.get::<Transform>(entity).cloned();
    
    if let Some(mut player) = scene.world.get_mut::<AnimationPlayer>(entity) {
        ui.collapsing("动画 (Animation)", |ui| {
            ui.horizontal(|ui| {
                if ui.button("添加新剪辑").clicked() {
                    player.clips.push(alander_core::scene::AnimationClip::new("New Clip".to_string()));
                }
            });

            for i in 0..player.clips.len() {
                // 我们在循环内部处理 active_clip_index 的设置，避免同时借用 clip 和 player
                let mut name = player.clips[i].name.clone();
                let duration = player.clips[i].duration;
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        if ui.text_edit_singleline(&mut name).changed() {
                            player.clips[i].name = name;
                        }
                        if ui.button("选择").clicked() {
                            player.active_clip_index = Some(i);
                        }
                    });
                    ui.label(format!("时长: {:.2}s", duration));
                });
            }

            if let Some(clip_idx) = player.active_clip_index {
                ui.separator();
                ui.label("当前选中剪辑控制");
                if let Some(transform) = current_transform {
                    if ui.button("捕捉当前 Transform 为关键帧").clicked() {
                        let time = player.current_time;
                        let clip = &mut player.clips[clip_idx];
                        
                        // 捕捉位置
                        let pos_track = clip.position_track.get_or_insert(alander_core::scene::AnimationTrack::new(Vec::new()));
                        pos_track.keyframes.push(alander_core::scene::Keyframe { time, value: transform.position });
                        pos_track.keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
                        
                        // 捕捉旋转
                        let rot_track = clip.rotation_track.get_or_insert(alander_core::scene::AnimationTrack::new(Vec::new()));
                        rot_track.keyframes.push(alander_core::scene::Keyframe { time, value: transform.rotation });
                        rot_track.keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());

                        // 捕捉缩放
                        let sca_track = clip.scale_track.get_or_insert(alander_core::scene::AnimationTrack::new(Vec::new()));
                        sca_track.keyframes.push(alander_core::scene::Keyframe { time, value: transform.scale });
                        sca_track.keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());

                        clip.update_duration();
                    }
                }
            }
        });
    } else {
         if ui.button("➕ 添加动画组件").clicked() {
             scene.world.entity_mut(entity).insert(AnimationPlayer::default());
         }
    }
}
