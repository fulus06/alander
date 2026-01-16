use egui;
use crate::scene_manager::Scene;
use alander_core::scene::AnimationPlayer;

/// 渲染时间线面板
pub fn show_timeline(ui: &mut egui::Ui, scene: &mut Scene, selected_entity: Option<bevy_ecs::entity::Entity>) {
    ui.heading("时间线 (Timeline)");
    
    if let Some(entity) = selected_entity {
        if let Some(mut player) = scene.world.get_mut::<AnimationPlayer>(entity) {
            ui.horizontal(|ui| {
                if ui.button(if player.is_playing { "暂停" } else { "播放" }).clicked() {
                    player.is_playing = !player.is_playing;
                }
                
                ui.checkbox(&mut player.loop_enabled, "循环");
                
                ui.label(format!("当前时间: {:.2}s", player.current_time));
            });

            if let Some(clip_idx) = player.active_clip_index {
                // 先获取进度条所需的元数据，避免在 UI 交互时持有对内部 clip 的借用
                let (clip_name, clip_duration) = if let Some(clip) = player.clips.get(clip_idx) {
                    (clip.name.clone(), clip.duration)
                } else {
                    ("Unknown".to_string(), 0.0)
                };

                let mut time = player.current_time;
                ui.horizontal(|ui| {
                    ui.label("时间:");
                    ui.add(egui::DragValue::new(&mut time).speed(0.1).clamp_range(0.0..=1000.0));
                    
                    let slider_max = clip_duration.max(10.0);
                    if ui.add(egui::Slider::new(&mut time, 0.0..=slider_max).text("进度")).changed() {
                        // 联动更新
                    }
                });
                player.current_time = time;
                
                ui.label(format!("当前剪辑: {}", clip_name));
            } else if !player.clips.is_empty() {
                if ui.button("开始播放第一个剪辑").clicked() {
                    player.play(0);
                }
            } else {
                ui.label("该实体没有动画剪辑资源");
            }
        } else {
            ui.label("选中实体不包含 AnimationPlayer 组件");
        }
    } else {
        ui.label("请在层级面板中选择一个实体以控制其动画");
    }
}
