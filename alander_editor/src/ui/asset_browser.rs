use egui;
use std::path::{Path, PathBuf};
use crate::app::EditorState;

/// 渲染资源浏览器面板
pub fn show_asset_browser(
    ui: &mut egui::Ui,
    editor_state: &mut EditorState,
    asset_root: &Path,
) {
    ui.heading("资源浏览器");
    ui.separator();

    // 1. 文件列表区域
    egui::ScrollArea::vertical().show(ui, |ui| {
        if let Ok(entries) = std::fs::read_dir(asset_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("Unknown");
                
                let is_selected = Some(&path) == editor_state.selected_asset_path.as_ref();
                
                if ui.selectable_label(is_selected, format!("📄 {}", file_name)).clicked() {
                    editor_state.selected_asset_path = Some(path.clone());
                    // 清除旧的预览，让后续逻辑重新加载
                    editor_state.asset_preview_texture = None;
                }
            }
        } else {
            ui.label("无法读取资源目录 (assets/)");
        }
    });

    ui.separator();

    // 2. 预览区域
    ui.label("预览:");
    if let Some(path) = editor_state.selected_asset_path.clone() {
        ui.label(format!("路径: {}", path.display()));
        
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match extension.to_lowercase().as_str() {
            "png" | "jpg" | "jpeg" | "hdr" => {
                show_image_preview(ui, editor_state, &path);
            }
            "glb" | "gltf" => {
                ui.label("📦 模型文件 (暂不支持实时预览)");
            }
            "json" => {
                ui.label("📝 场景/数据文件");
            }
            _ => {
                ui.label("❓ 未知类型");
            }
        }
    } else {
        ui.label("请选择一个资源以查看预览");
    }
}

fn show_image_preview(ui: &mut egui::Ui, editor_state: &mut EditorState, path: &Path) {
    // 如果还没加载预览纹理，尝试加载
    if editor_state.asset_preview_texture.is_none() {
        if let Ok(image_data) = image::open(path) {
            let image_data = image_data.to_rgba8();
            let (width, height) = image_data.dimensions();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [width as usize, height as usize],
                &image_data,
            );
            
            let handle = ui.ctx().load_texture(
                path.to_string_lossy(),
                color_image,
                Default::default()
            );
            editor_state.asset_preview_texture = Some(handle);
        }
    }

    if let Some(texture) = &editor_state.asset_preview_texture {
        let size = texture.size_vec2();
        let max_size = egui::vec2(200.0, 200.0);
        let aspect_ratio = size.x / size.y;
        
        let display_size = if aspect_ratio > 1.0 {
            egui::vec2(max_size.x, max_size.x / aspect_ratio)
        } else {
            egui::vec2(max_size.y * aspect_ratio, max_size.y)
        };

        ui.add(egui::Image::new(texture).max_size(display_size));
        ui.label(format!("尺寸: {}x{}", size.x as u32, size.y as u32));
    } else {
        ui.label("无法加载图片预览");
    }
}
