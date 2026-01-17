use anyhow::Result;
use glam::{Vec3, Vec4, Mat4};
use winit::event::{WindowEvent, ElementState, MouseButton};
use tracing::info;
use std::sync::Arc;
use alander_core::{
    scene::{Camera, Transform, PointLight, SpotLight, Name, RenderId, AssetPath, BoundingBox, PBRMaterial, GlobalTransform},
    InputState, RenderState, Time,
};
use alander_render::renderer::Renderer;
use bevy_ecs::prelude::*;

use crate::scene_manager::{SceneManager, Scene};
use crate::physics_manager::PhysicsManager;
use crate::gizmo_manager::{GizmoManager, GizmoMode};
use crate::camera_controller::OrbitController;
use crate::ui::{EditorUI, MenuAction};
use crate::editor_command::CommandManager;
use sysinfo::{System, SystemExt, ProcessExt};
use crate::script_manager::ScriptManager;

/// 编辑器状态
pub struct EditorState {
    /// 选中的实体
    pub selected_entity: Option<bevy_ecs::entity::Entity>,
    /// 轨道相机控制器
    pub orbit_controller: OrbitController,
    /// 是否显示碰撞体
    pub show_colliders: bool,
    /// 此时正在 UI 中拖拽的实体
    pub dragged_entity: Option<bevy_ecs::entity::Entity>,
    /// 当前激活的场景相机实体
    pub active_camera_entity: Option<bevy_ecs::entity::Entity>,
    /// 当前 FPS
    pub fps: f32,
    /// 内存占用 (MB)
    pub memory_usage: f64,
    /// Bloom 阈值
    pub bloom_threshold: f32,
    /// Bloom 强度
    pub bloom_intensity: f32,
    /// 选中的资源路径
    pub selected_asset_path: Option<std::path::PathBuf>,
    /// 资源预览纹理 ID (egui)
    pub asset_preview_texture: Option<egui::TextureHandle>,
}

/// 应用程序状态
pub struct AlanderApp {
    /// 渲染器
    pub renderer: Renderer,

    /// EGUI 渲染通道 (暂时不直接使用，通过 egui_renderer)
    pub egui_rpass: Option<()>,

    /// 编辑器状态
    pub editor_state: EditorState,

    /// 撤销/重做管理器
    pub command_manager: CommandManager,

    /// 相机
    pub camera: Camera,

    /// 相机变换
    pub camera_transform: Transform,

    /// 输入状态
    pub input: InputState,

    /// 时间
    pub time: Time,

    /// 渲染状态
    pub render_state: RenderState,

    /// 场景管理器
    pub scene_manager: SceneManager,

    /// 运行标志
    pub running: bool,

    /// 窗口
    pub window: Arc<winit::window::Window>,

    /// EGUI 上下文
    pub egui_context: egui::Context,

    /// 物理管理器
    pub physics_manager: PhysicsManager,

    /// Gizmo 管理器
    pub gizmo_manager: GizmoManager,

    /// EGUI 状态
    pub egui_state: egui_winit::State,

    /// EGUI 渲染器
    pub egui_renderer: egui_wgpu::Renderer,

    /// UI 组件
    pub editor_ui: EditorUI,

    /// FPS 更新定时器
    pub fps_update_timer: f32,

    /// 显示的帧时间
    pub displayed_delta_time: f32,

    /// 系统信息 (供统计使用)
    pub system_info: System,

    /// 脚本管理器
    pub script_manager: ScriptManager,
}

impl AlanderApp {
    /// 创建新的应用程序
    pub async fn new(window: Arc<winit::window::Window>) -> Result<Self> {
        // 初始化日志 (如果还没初始化)
        // tracing_subscriber::fmt().with_max_level(Level::INFO).try_init().ok();

        info!("正在初始化 Alander 编辑器...");

        // 创建渲染器
        let renderer = Renderer::new(&window).await?;

        // 创建相机
        let camera = Camera::perspective(
            std::f32::consts::PI / 4.0,
            renderer.size().width as f32 / renderer.size().height as f32,
            0.1,
            100.0,
        );

        // 初始相机位置
        let camera_transform = Transform::from_translation(Vec3::new(0.0, 1.0, 5.0));

        // 渲染状态
        let render_state = RenderState {
            surface_size: (renderer.size().width, renderer.size().height),
            scale_factor: window.scale_factor(),
        };

        // 创建场景管理器并添加测试场景
        let scene_manager = SceneManager::new();
        // 这里需要传递 mut 引用到 renderer，但我们在 app 初始化时还没创建 self
        // 暂时在外部初始化
        // scene_manager.create_test_scene(&mut renderer);

        // 初始化 EGUI
        let egui_context = egui::Context::default();
        let egui_state = egui_winit::State::new(&*window);
        
        // 初始化 EGUI 渲染器
        let egui_renderer = egui_wgpu::Renderer::new(
            renderer.device(),
            renderer.format(),
            None,
            1,
        );

        let mut app = Self {
            renderer,
            egui_rpass: None,
            editor_state: EditorState {
                selected_entity: None,
                orbit_controller: OrbitController::default(),
                show_colliders: false,
                dragged_entity: None,
                active_camera_entity: None,
                fps: 0.0,
                memory_usage: 0.0,
                bloom_threshold: 1.0,
                bloom_intensity: 0.5,
                selected_asset_path: None,
                asset_preview_texture: None,
            },
            command_manager: CommandManager::new(50),
            camera,
            camera_transform,
            input: InputState::default(),
            time: Time::default(),
            render_state,
            scene_manager,
            running: true,
            window,
            egui_context,
            egui_state,
            egui_renderer,
            physics_manager: PhysicsManager::new(),
            gizmo_manager: GizmoManager::new(),
            editor_ui: EditorUI::new(),
            fps_update_timer: 0.0,
            displayed_delta_time: 0.0,
            system_info: System::new_all(),
            script_manager: ScriptManager::new(),
        };

        // 完成后续初始化
        app.scene_manager.create_test_scene(&mut app.renderer);
        app.update_camera_transform();
        app.setup_fonts();

        Ok(app)
    }

    /// 设置字体以支持中文
    fn setup_fonts(&self) {
        let mut fonts = egui::FontDefinitions::default();
        let font_paths = [
            "/System/Library/Fonts/STHeiti Light.ttc",
            "/System/Library/Fonts/Supplemental/Songti.ttc",
        ];

        let mut font_loaded = false;
        for path in font_paths {
            if let Ok(bytes) = std::fs::read(path) {
                fonts.font_data.insert(
                    "chinese_font".to_owned(),
                    egui::FontData::from_owned(bytes),
                );
                fonts.families.get_mut(&egui::FontFamily::Proportional)
                    .unwrap()
                    .insert(0, "chinese_font".to_owned());
                fonts.families.get_mut(&egui::FontFamily::Monospace)
                    .unwrap()
                    .push("chinese_font".to_owned());
                font_loaded = true;
                info!("成功加载中文字体: {}", path);
                break;
            }
        }

        if !font_loaded {
            tracing::warn!("未能加载中文字体，中文显示可能异常");
        }
        self.egui_context.set_fonts(fonts);
    }

    /// 处理输入事件
    pub fn handle_input(&mut self, event: &WindowEvent) {
        let _egui_res = self.egui_state.on_event(&self.egui_context, event);

        let window_size = self.window.inner_size();
        let scale_factor = self.window.scale_factor() as f32;
        let mouse_pos = self.input.mouse_position;
        
        let left_px = 200.0 * scale_factor;
        let right_px = window_size.width as f32 - 250.0 * scale_factor;
        let top_px = 30.0 * scale_factor;
        let bottom_px = window_size.height as f32 - 150.0 * scale_factor; // 各种底部面板的高度
        
        let is_in_viewport = if right_px > left_px && bottom_px > top_px {
            mouse_pos.x > left_px && mouse_pos.x < right_px && mouse_pos.y > top_px && mouse_pos.y < bottom_px
        } else {
            false
        };

        match event {
            WindowEvent::Resized(size) => {
                self.renderer.resize(*size);
                self.render_state.surface_size = (size.width, size.height);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.render_state.scale_factor = *scale_factor;
            }
            WindowEvent::KeyboardInput { input, .. } => {
                if let Some(key) = input.virtual_keycode {
                    let old_state = self.input.keyboard.insert(key, input.state);
                    
                    if input.state == ElementState::Pressed {
                        if old_state.map_or(true, |s| s == ElementState::Released) {
                            self.input.just_pressed.insert(key);
                        }

                        if key == winit::event::VirtualKeyCode::Escape {
                            self.running = false;
                        }

                        match key {
                            winit::event::VirtualKeyCode::W => self.gizmo_manager.mode = GizmoMode::Translate,
                            winit::event::VirtualKeyCode::E => self.gizmo_manager.mode = GizmoMode::Rotate,
                            winit::event::VirtualKeyCode::R => self.gizmo_manager.mode = GizmoMode::Scale,
                            _ => {}
                        }
                    } else {
                        self.input.just_released.insert(key);
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let egui_wants_input = self.egui_context.wants_pointer_input();
                let is_pressed = *state == ElementState::Pressed;
                
                // 左键处理：拾取 (仅当不在 UI 上时)
                if *button == MouseButton::Left && is_pressed && is_in_viewport && !egui_wants_input {
                    if self.gizmo_manager.hovered_axis.is_none() {
                        self.pick_entity();
                    }
                }

                // 中键处理：旋转 (即使在 CentralPanel 上也允许，因为它是透明视口)
                if *button == MouseButton::Middle {
                    if is_pressed && is_in_viewport {
                        self.editor_state.orbit_controller.is_dragging = true;
                        self.editor_state.orbit_controller.last_mouse_pos = (mouse_pos.x, mouse_pos.y);
                    } else if !is_pressed {
                        self.editor_state.orbit_controller.is_dragging = false;
                    }
                }

                self.input.mouse_buttons.insert(*button, *state);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.input.mouse_position = Vec2::new(position.x as f32, position.y as f32);

                if self.editor_state.orbit_controller.is_dragging {
                    let delta_x = self.input.mouse_position.x - self.editor_state.orbit_controller.last_mouse_pos.0;
                    let delta_y = self.input.mouse_position.y - self.editor_state.orbit_controller.last_mouse_pos.1;

                    let sensitivity = 0.005;
                    self.editor_state.orbit_controller.rotation.0 -= delta_x * sensitivity;
                    self.editor_state.orbit_controller.rotation.1 -= delta_y * sensitivity;
                    self.editor_state.orbit_controller.rotation.1 = self.editor_state.orbit_controller.rotation.1
                        .clamp(-std::f32::consts::PI / 2.1, std::f32::consts::PI / 2.1);

                    self.editor_state.orbit_controller.last_mouse_pos = (self.input.mouse_position.x, self.input.mouse_position.y);
                    self.update_camera_transform();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if is_in_viewport && !self.egui_context.is_using_pointer() && !self.editor_state.orbit_controller.is_dragging {
                    let zoom_speed = (self.editor_state.orbit_controller.distance * 0.1).max(0.5);
                    let scroll_y = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => *y * 2.0,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.05,
                    };

                    self.editor_state.orbit_controller.distance -= scroll_y * zoom_speed;
                    self.editor_state.orbit_controller.distance = self.editor_state.orbit_controller.distance.clamp(0.1, 2000.0);
                    self.update_camera_transform();
                }
            }
            _ => {}
        }
    }

    fn pick_entity(&mut self) {
        let mouse_pos = self.input.mouse_position;
        let window_size = self.window.inner_size();
        let x = mouse_pos.x / window_size.width as f32;
        let y = mouse_pos.y / window_size.height as f32;
        let ray = self.renderer.screen_to_world_ray(glam::Vec2::new(x, y));

        if let Some(hit_entity) = self.physics_manager.ray_cast(&ray) {
            self.editor_state.selected_entity = Some(hit_entity);
            info!("拾取到实体: {:?}", hit_entity);
        } else {
            self.editor_state.selected_entity = None;
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        // 更新统计数据
        self.fps_update_timer += delta_time;
        if self.fps_update_timer >= 0.5 {
            self.editor_state.fps = 1.0 / delta_time;
            
            // 更新内存统计
            self.system_info.refresh_process(sysinfo::get_current_pid().unwrap());
            if let Some(process) = self.system_info.process(sysinfo::get_current_pid().unwrap()) {
                self.editor_state.memory_usage = process.memory() as f64 / 1024.0 / 1024.0;
            }
            
            self.displayed_delta_time = delta_time;
            self.fps_update_timer = 0.0;
        }

        // 运行脚本
        if let Some(mut scene) = self.scene_manager.active_scene_mut() {
            self.script_manager.update_scripts(&mut scene, delta_time);
        }

        if let Some(scene) = self.scene_manager.active_scene_mut() {
            // 0. 首先更新层级变换，确保逻辑和 Gizmo 使用的是最新的世界位姿
            scene.update_hierarchy();

            // 1. 逻辑更新 (如 Gizmo)
            let mouse_pos = self.input.mouse_position;
            let window_size = self.window.inner_size();
            let x = mouse_pos.x / window_size.width as f32;
            let y = mouse_pos.y / window_size.height as f32;
            let ray = self.renderer.screen_to_world_ray(glam::Vec2::new(x, y));
            let is_left_pressed = self.input.mouse_button_pressed(MouseButton::Left);
            
            if let Some(initial_transform) = self.gizmo_manager.update(
                &ray,
                glam::Vec2::new(mouse_pos.x, mouse_pos.y),
                glam::Vec2::new(window_size.width as f32, window_size.height as f32),
                self.renderer.view_proj_glam(),
                is_left_pressed, 
                self.editor_state.selected_entity, 
                &mut scene.world, 
                self.camera_transform.position
            ) {
                // 拖拽结束，记录命令
                if let Some(entity) = self.editor_state.selected_entity {
                    if let Some(new_transform) = scene.world.get::<Transform>(entity) {
                        let cmd = crate::editor_command::TransformCommand::new(
                            entity,
                            initial_transform,
                            *new_transform,
                        );
                        self.command_manager.execute(Box::new(cmd), scene, &mut self.renderer);
                    }
                }
            }

            if let Some(entity) = self.editor_state.selected_entity {
                let gizmo_lines = self.gizmo_manager.render(&scene.world, entity, self.camera_transform.position);
                self.renderer.update_debug_overlay(&gizmo_lines);
            } else {
                self.renderer.update_debug_overlay(&[]);
            }

            // 处理撤销/重做快捷键
            let is_ctrl = self.input.key_pressed(winit::event::VirtualKeyCode::LControl) || 
                         self.input.key_pressed(winit::event::VirtualKeyCode::RControl) ||
                         self.input.key_pressed(winit::event::VirtualKeyCode::LWin) || 
                         self.input.key_pressed(winit::event::VirtualKeyCode::RWin); // Mac CMD

            if is_ctrl && self.input.key_just_pressed(winit::event::VirtualKeyCode::Z) {
                if self.input.key_pressed(winit::event::VirtualKeyCode::LShift) || 
                   self.input.key_pressed(winit::event::VirtualKeyCode::RShift) {
                    self.command_manager.redo(scene, &mut self.renderer);
                } else {
                    self.command_manager.undo(scene, &mut self.renderer);
                }
            }

            // 快捷键: 删除 (Delete/Backspace)
            if self.input.key_just_pressed(winit::event::VirtualKeyCode::Delete) ||
               self.input.key_just_pressed(winit::event::VirtualKeyCode::Back) {
                if let Some(entity) = self.editor_state.selected_entity {
                    tracing::info!("删除实体: {:?}", entity);
                    let cmd = crate::editor_command::DeleteEntityCommand::new(entity, scene);
                    self.command_manager.execute(Box::new(cmd), scene, &mut self.renderer);
                    self.editor_state.selected_entity = None;
                }
            }

            // 快捷键: 复制 (Ctrl+D)
            if is_ctrl && self.input.key_just_pressed(winit::event::VirtualKeyCode::D) {
                if let Some(entity) = self.editor_state.selected_entity {
                    tracing::info!("复制实体: {:?}", entity);
                    let cmd = crate::editor_command::DuplicateEntityCommand::new(entity, scene);
                    self.command_manager.execute(Box::new(cmd), scene, &mut self.renderer);
                }
            }

            // 1.5 更新动画系统
            update_animations(scene, delta_time);

            // 2. 将逻辑变更同步到物理世界并执行步进
            self.physics_manager.integration_parameters.dt = delta_time;
            self.physics_manager.sync_ecs_to_physics(&mut scene.world);
            self.physics_manager.step();
            self.physics_manager.sync_physics_to_ecs(&mut scene.world);
            self.physics_manager.update_query_pipeline();

            // 3. 收集并更新调试线框 (碰撞体 + 视锥体)
            let mut debug_vertices = Vec::new();

            // 物理碰撞体
            if self.editor_state.show_colliders {
                debug_vertices.extend(self.physics_manager.render_debug_lines());
            }

            // 相机视锥体
            let mut camera_query = scene.world.query::<(Entity, &Camera, &GlobalTransform)>();
            for (entity, camera, gt) in camera_query.iter(&scene.world) {
                // 不为当前工作的视口相机或编辑器相机显示视锥体
                if Some(entity) == self.editor_state.active_camera_entity {
                    continue;
                }

                let camera_proj: glam::Mat4 = camera.compute_projection_matrix();
                let world_matrix: glam::Mat4 = gt.0;
                let inv_view_proj: glam::Mat4 = (camera_proj * world_matrix.inverse()).inverse();
                let ndc_points = [
                    glam::Vec3::new(-1.0, -1.0, -1.0), glam::Vec3::new(1.0, -1.0, -1.0),
                    glam::Vec3::new(1.0, 1.0, -1.0), glam::Vec3::new(-1.0, 1.0, -1.0),
                    glam::Vec3::new(-1.0, -1.0, 1.0), glam::Vec3::new(1.0, -1.0, 1.0),
                    glam::Vec3::new(1.0, 1.0, 1.0), glam::Vec3::new(-1.0, 1.0, 1.0),
                ];

                let mut world_points = Vec::with_capacity(8);
                for p in ndc_points {
                    let p_h = inv_view_proj * glam::Vec4::from((p, 1.0));
                    world_points.push(glam::Vec3::new(p_h.x / p_h.w, p_h.y / p_h.w, p_h.z / p_h.w));
                }

                let color = [1.0, 1.0, 0.0, 1.0]; // 黄色线框
                let push_line = |v: &mut Vec<alander_render::pipelines::DebugVertex>, a: glam::Vec3, b: glam::Vec3| {
                    v.push(alander_render::pipelines::DebugVertex { position: a.into(), color });
                    v.push(alander_render::pipelines::DebugVertex { position: b.into(), color });
                };

                // 近平面
                push_line(&mut debug_vertices, world_points[0], world_points[1]);
                push_line(&mut debug_vertices, world_points[1], world_points[2]);
                push_line(&mut debug_vertices, world_points[2], world_points[3]);
                push_line(&mut debug_vertices, world_points[3], world_points[0]);
                // 远平面
                push_line(&mut debug_vertices, world_points[4], world_points[5]);
                push_line(&mut debug_vertices, world_points[5], world_points[6]);
                push_line(&mut debug_vertices, world_points[6], world_points[7]);
                push_line(&mut debug_vertices, world_points[7], world_points[4]);
                // 连接
                push_line(&mut debug_vertices, world_points[0], world_points[4]);
                push_line(&mut debug_vertices, world_points[1], world_points[5]);
                push_line(&mut debug_vertices, world_points[2], world_points[6]);
                push_line(&mut debug_vertices, world_points[3], world_points[7]);
            }

            self.renderer.update_debug_lines(&debug_vertices);

            // 同步光源与渲染对象
            Self::sync_scene_to_renderer_static(&mut self.renderer, scene);
        }

        self.input.clear_frame_state();

        self.time.delta = delta_time;
        self.time.elapsed += delta_time;
        self.fps_update_timer += delta_time;
        if self.fps_update_timer >= 0.2 {
            self.displayed_delta_time = delta_time;
            self.fps_update_timer = 0.0;
        }

        // 同步相机到渲染器
        let mut camera_synced = false;
        if let Some(scene) = self.scene_manager.active_scene() {
            if let Some(active_entity) = self.editor_state.active_camera_entity {
                if let (Some(camera), Some(gt)) = (
                    scene.world.get::<Camera>(active_entity),
                    scene.world.get::<GlobalTransform>(active_entity)
                ) {
                    let (_, rot, pos) = gt.0.to_scale_rotation_translation();
                    let transform = Transform {
                        position: pos,
                        rotation: rot,
                        scale: glam::Vec3::ONE,
                    };
                    self.renderer.update_camera(camera, &transform);
                    camera_synced = true;
                }
            }
        }

        if !camera_synced {
            self.renderer.update_camera(&self.camera, &self.camera_transform);
        }
    }

    fn sync_scene_to_renderer_static(renderer: &mut Renderer, scene: &mut Scene) {
        // 1. 同步光源
        let mut light_buffer = alander_render::pipelines::LightBuffer::new();
        
        // 同步平行光
        let mut dir_light_query = scene.world.query::<(&Transform, &alander_core::scene::DirectionalLight)>();
        if let Some((transform, light)) = dir_light_query.iter(&scene.world).next() {
            let rotation = transform.rotation;
            let direction = rotation * glam::Vec3::NEG_Z; // 假设 -Z 是主照射方向
            
            light_buffer.dir_light = alander_render::pipelines::DirectionalLight::new(
                direction.into(),
                light.color.into(),
                light.intensity,
                light.shadow_bias,
                light.shadow_normal_bias,
            );
        }

        // 同步点光源
        let mut light_query = scene.world.query::<(&GlobalTransform, &PointLight)>();
        for (global_transform, light) in light_query.iter(&scene.world) {
            // 从全局变换矩阵提取位置
            let pos = global_transform.0.transform_point3(glam::Vec3::ZERO);
            let render_light = alander_render::pipelines::Light::point(
                pos.into(),
                light.color.into(),
                light.intensity,
                light.range,
            );
            light_buffer.add_light(render_light);
        }

        // 同步聚光灯
        let mut spot_light_query = scene.world.query::<(&GlobalTransform, &SpotLight)>();
        for (global_transform, light) in spot_light_query.iter(&scene.world) {
            let pos = global_transform.0.transform_point3(glam::Vec3::ZERO);
            // 假设聚光灯朝向 -Z (局部)
            let direction = global_transform.0.transform_vector3(glam::Vec3::NEG_Z).normalize();
            
            let render_light = alander_render::pipelines::Light::spot(
                pos.into(),
                light.color.into(),
                light.intensity,
                light.range,
                direction.into(),
                light.inner_angle,
                light.outer_angle,
                light.shadow_bias,
            );
            light_buffer.add_light(render_light);
        }

        renderer.update_lights(&light_buffer);

        // 2. 同步渲染对象
        let mut query = scene.world.query::<(
            &GlobalTransform, 
            &RenderId, 
            Option<&mut BoundingBox>,
            Option<&PBRMaterial>
        )>();
        
        for (global_transform, render_id, mut bbox, material) in query.iter_mut(&mut scene.world) {
            let matrix = global_transform.0;
            let m = matrix.to_cols_array_2d();
            let cg_matrix = cgmath::Matrix4::new(
                m[0][0], m[0][1], m[0][2], m[0][3],
                m[1][0], m[1][1], m[1][2], m[1][3],
                m[2][0], m[2][1], m[2][2], m[2][3],
                m[3][0], m[3][1], m[3][2], m[3][3],
            );

            let render_mat = material.map(|m: &PBRMaterial| alander_render::pipelines::MaterialBuffer {
                base_color: m.base_color.into(),
                metallic: m.metallic,
                roughness: m.roughness,
                has_normal_texture: 0,
                has_metallic_roughness_texture: 0,
                emissive: [m.emissive.x, m.emissive.y, m.emissive.z, 1.0],
            });

            renderer.update_object_model_material(&render_id.0, cg_matrix, render_mat);

            if let Some(ref mut bbox) = bbox {
                bbox.world = bbox.local.transform(matrix);
            }
        }
    }

    pub fn render(&mut self) -> Result<()> {
        // 更新 Bloom 设置到渲染器
        self.renderer.update_bloom_settings(alander_render::renderer::BloomSettings {
            threshold: self.editor_state.bloom_threshold,
            intensity: self.editor_state.bloom_intensity,
        });

        let output = self.renderer.surface().get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.renderer.device().create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("渲染编码器"),
        });

        // EGUI 帧构建
        self.egui_state.set_pixels_per_point(self.window.scale_factor() as f32);
        let raw_input = self.egui_state.take_egui_input(&*self.window);
        self.egui_context.begin_frame(raw_input);
        self.ui(&self.egui_context.clone());
        let full_output = self.egui_context.end_frame();
        let paint_jobs = self.egui_context.tessellate(full_output.shapes);

        let screen_descriptor = egui_wgpu::renderer::ScreenDescriptor {
            size_in_pixels: [self.render_state.surface_size.0, self.render_state.surface_size.1],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        for (id, delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(self.renderer.device(), self.renderer.queue(), *id, delta);
        }

        self.egui_renderer.update_buffers(
            self.renderer.device(), self.renderer.queue(), &mut encoder, &paint_jobs, &screen_descriptor,
        );

        // 渲染 3D 场景
        // 获取所有可渲染对象
        let objects: Vec<_> = self.renderer.resources.objects.values().collect();
        self.renderer.render_shadow_pass(&mut encoder, &objects, self.renderer.shadow_view_proj);

        // 简单的演示：如果有第一个点光源，渲染其全向阴影
        // 实际开发中应该动态收集需要阴影的点光源
        if let Some(scene) = self.scene_manager.active_scene_mut() {
            // 更新动画
            update_animations(scene, self.displayed_delta_time);

            // 5. 更新已有对象的变换和骨骼
            let mut query = scene.world.query::<(Entity, &alander_core::scene::GlobalTransform, &alander_core::scene::RenderId, Option<&alander_core::scene::Skin>)>();
            for (entity, gt, rid, skin) in query.iter(&scene.world) {
                if let Some(obj) = self.renderer.get_object(rid.0) {
                    let cgmath_matrix = cgmath::Matrix4::from(gt.0.to_cols_array_2d());
                    obj.update_model(self.renderer.queue(), cgmath_matrix, skin.is_some());

                    // 如果有蒙皮，更新骨骼矩阵
                    if let Some(skin_comp) = skin {
                        let mut bone_buffer = alander_render::pipelines::common::BoneBuffer {
                            matrices: [[[0.0; 4]; 4]; 128],
                        };
                        
                        let mesh_inv_world = gt.0.inverse();

                        for (i, &joint_entity) in skin_comp.joints.iter().enumerate() {
                            if i >= 128 { break; }
                            
                            if let Some(joint_gt) = scene.world.get::<alander_core::scene::GlobalTransform>(joint_entity) {
                                // 计算骨骼空间变换: World(Joint) * InvBind(Joint) * InvWorld(Mesh)
                                let ibm = skin_comp.inverse_bind_matrices[i];
                                let joint_matrix = mesh_inv_world * joint_gt.0 * ibm;
                                bone_buffer.matrices[i] = cgmath::Matrix4::from(joint_matrix.to_cols_array_2d()).into();
                            }
                        }
                        obj.update_bones(self.renderer.queue(), &bone_buffer);
                    }
                }
            }

            let mut point_light_query = scene.world.query::<(&alander_core::scene::GlobalTransform, &alander_core::scene::PointLight)>();
            if let Some((gt, light)) = point_light_query.iter(&scene.world).next() {
                let pos = gt.0.transform_point3(glam::Vec3::ZERO);
                self.renderer.render_point_shadow_pass(&mut encoder, &objects, pos.into(), light.range);
            }
        }

        self.renderer.render_scene(&view, &mut encoder);

        // 渲染 EGUI 叠加
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("EGUI 渲染通道"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
            self.egui_renderer.render(&mut render_pass, &paint_jobs, &screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        self.renderer.queue().submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    fn ui(&mut self, ctx: &egui::Context) {
        let action = self.editor_ui.draw(
            ctx,
            &mut self.scene_manager,
            &mut self.physics_manager,
            &mut self.gizmo_manager,
            &mut self.renderer,
            &mut self.command_manager,
            &mut self.editor_state,
            self.displayed_delta_time
        );

        match action {
            MenuAction::Undo => {
                if let Some(scene) = self.scene_manager.active_scene_mut() {
                    self.command_manager.undo(scene, &mut self.renderer);
                }
            }
            MenuAction::Redo => {
                if let Some(scene) = self.scene_manager.active_scene_mut() {
                    self.command_manager.redo(scene, &mut self.renderer);
                }
            }
            MenuAction::OpenScene => self.on_file_open(),
            MenuAction::SaveScene => self.on_file_save(),
            MenuAction::ImportModel => self.on_import_model(),
            MenuAction::ImportHdr => self.on_import_hdr_environment(),
            MenuAction::ResetCamera => self.reset_camera(),
            MenuAction::Exit => self.running = false,
            MenuAction::None => {}
        }
    }

    // --- 回调方法 ---

    fn on_import_model(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("glTF 模型", &["gltf", "glb"])
            .pick_file()
        {
            let path_str = path.to_string_lossy();
            let loader = alander_core::assets::GltfLoader;
            match loader.load_scene(&path_str) {
                Ok(model) => {
                    if let Some(scene) = self.scene_manager.active_scene_mut() {
                        let _root = scene.spawn_gltf_model(model, &mut self.renderer, &path_str);
                        tracing::info!("导入 glTF 模型完成: {}", path_str);
                    }
                }
                Err(e) => tracing::error!("加载 glTF 失败: {}", e),
            }
        }
    }

    fn on_import_hdr_environment(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("HDR 环境贴图", &["hdr"]).pick_file() {
            if let Err(e) = self.renderer.load_hdr_environment(&path) {
                tracing::error!("加载 HDR 失败: {}", e);
            }
        }
    }

    fn on_file_open(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Alander 场景", &["json"]).pick_file() {
            match std::fs::read_to_string(&path) {
                Ok(json) => {
                    match Scene::from_json(&json, &mut self.renderer) {
                        Ok(new_scene) => {
                            if let Some(scene) = self.scene_manager.active_scene_mut() {
                                for entity in scene.world.iter_entities() {
                                    if let Some(render_id) = scene.world.get::<RenderId>(entity.id()) {
                                        self.renderer.remove_object(&render_id.0);
                                    }
                                }
                            }
                            self.scene_manager.create_scene_from_object(new_scene);
                        }
                        Err(e) => tracing::error!("解析 JSON 失败: {}", e),
                    }
                }
                Err(e) => tracing::error!("读取文件失败: {}", e),
            }
        }
    }

    fn on_file_save(&mut self) {
        if let Some(scene) = self.scene_manager.active_scene_mut() {
            if let Some(path) = rfd::FileDialog::new().add_filter("Alander 场景", &["json"]).set_file_name("scene.json").save_file() {
                match scene.to_json() {
                    Ok(json) => {
                        if let Err(e) = std::fs::write(&path, json) {
                            tracing::error!("写入失败: {}", e);
                        }
                    }
                    Err(e) => tracing::error!("序列化失败: {}", e),
                }
            }
        }
    }

    fn reset_camera(&mut self) {
        self.editor_state.orbit_controller = OrbitController::default();
        self.update_camera_transform();
    }

    fn update_camera_transform(&mut self) {
        self.editor_state.orbit_controller.update_transform(&mut self.camera_transform);
    }

}

/// 更新所有识体的动画系统 (独立于 AlanderApp 以避免借用冲突)
fn update_animations(scene: &mut Scene, dt: f32) {
    use alander_core::scene::{AnimationPlayer, Transform, Name, Children};
    use bevy_ecs::prelude::*;

    let mut animation_updates = Vec::new();

    // 1. 采样所有活跃的播放器
    {
        let mut query = scene.world.query::<(Entity, &mut AnimationPlayer)>();
        for (root_entity, mut player) in query.iter_mut(&mut scene.world) {
            if !player.is_playing { continue; }

            let mut sync_clips = Vec::new(); // (clip_idx, time, weight)

            if let Some(active_idx) = player.active_clip_index {
                // 更新主剪辑时间
                if let Some(clip) = player.clips.get(active_idx) {
                    let duration = clip.duration;
                    let playback_speed = player.playback_speed;
                    let loop_enabled = player.loop_enabled;
                    
                    let mut new_time = player.current_time + dt * playback_speed;
                    let mut is_playing = true;
                    if loop_enabled && duration > 0.0 {
                        new_time %= duration;
                    } else if new_time > duration {
                        new_time = duration;
                        is_playing = false;
                    }

                    player.current_time = new_time;
                    player.is_playing = is_playing;

                    // 处理过渡百分比
                    if let Some(target_idx) = player.transition_target_index {
                        player.transition_time += dt;
                        let alpha = (player.transition_time / player.transition_duration).clamp(0.0, 1.0);
                        
                        sync_clips.push((active_idx, player.current_time, 1.0 - alpha));
                        sync_clips.push((target_idx, player.transition_time, alpha)); // 假设目标剪辑从 0 开始随过渡时间推进

                        if player.transition_time >= player.transition_duration {
                            // 过渡完成
                            player.active_clip_index = Some(target_idx);
                            player.current_time = player.transition_time; // 保持一致
                            player.transition_target_index = None;
                            player.transition_time = 0.0;
                        }
                    } else {
                        sync_clips.push((active_idx, player.current_time, 1.0));
                    }
                }
            }

            // 执行混合采样
            if !sync_clips.is_empty() {
                // 收集所有涉及的通道名称
                let mut channel_names = std::collections::HashSet::new();
                for &(idx, _, _) in &sync_clips {
                    if let Some(clip) = player.clips.get(idx) {
                        for channel in &clip.channels {
                            channel_names.insert(channel.target_name.clone());
                        }
                    }
                }

                for target_name in channel_names {
                    let mut blended_pos: Option<glam::Vec3> = None;
                    let mut blended_rot: Option<glam::Quat> = None;
                    let mut blended_sca: Option<glam::Vec3> = None;
                    
                    let mut total_weight = 0.0;
                    for &(idx, t, weight) in &sync_clips {
                        if let Some(clip) = player.clips.get(idx) {
                            if let Some(channel) = clip.channels.iter().find(|c| c.target_name == target_name) {
                                if let Some(p) = channel.position_track.as_ref().and_then(|tr| tr.sample_vec3(t)) {
                                    blended_pos = Some(blended_pos.map_or(p * weight, |acc| acc + p * weight));
                                }
                                if let Some(r) = channel.rotation_track.as_ref().and_then(|tr| tr.sample_quat(t)) {
                                    blended_rot = Some(match blended_rot {
                                        None => r,
                                        Some(acc) => {
                                            let alpha = weight / (total_weight + weight);
                                            acc.slerp(r, alpha)
                                        }
                                    });
                                }
                                if let Some(s) = channel.scale_track.as_ref().and_then(|tr| tr.sample_vec3(t)) {
                                    blended_sca = Some(blended_sca.map_or(s * weight, |acc| acc + s * weight));
                                }
                                total_weight += weight;
                            }
                        }
                    }
                    animation_updates.push((root_entity, target_name.clone(), blended_pos, blended_rot, blended_sca));
                }
            }
        }
    }

    // 2. 应用到子实体
    for update in animation_updates {
        let (root, target_name, pos, rot, sca) = update;
        if let Some(target_entity) = find_entity_by_name_recursive(&scene.world, root, &target_name) {
            if let Some(mut transform) = scene.world.get_mut::<Transform>(target_entity) {
                if let Some(p) = pos { transform.position = p; }
                if let Some(r) = rot { transform.rotation = r; }
                if let Some(s) = sca { transform.scale = s; }
            }
        }
    }
}

fn find_entity_by_name_recursive(world: &World, entity: Entity, name: &str) -> Option<Entity> {
    use alander_core::scene::{Name, Children};
    if let Some(n) = world.get::<Name>(entity) {
        if n.0 == name { return Some(entity); }
    }
    if let Some(children) = world.get::<Children>(entity) {
        for &child in &children.0 {
            if let Some(found) = find_entity_by_name_recursive(world, child, name) {
                return Some(found);
            }
        }
    }
    None
}

// 辅助类型
type Vec2 = glam::Vec2;

