use anyhow::Result;
use glam::{Vec3, Vec4, Mat4};
use winit::event::{WindowEvent, ElementState, MouseButton};
use tracing::info;
use std::sync::Arc;
use alander_core::{
    scene::{Camera, Transform, PointLight, Name, RenderId, AssetPath, BoundingBox, PBRMaterial, GlobalTransform},
    InputState, RenderState, Time,
};
use alander_render::renderer::Renderer;

use crate::scene_manager::{SceneManager, Scene};
use crate::physics_manager::PhysicsManager;
use crate::gizmo_manager::{GizmoManager, GizmoMode};
use crate::camera_controller::OrbitController;
use crate::ui::{EditorUI, MenuAction};

/// 编辑器状态
pub struct EditorState {
    /// 选中的实体
    pub selected_entity: Option<bevy_ecs::entity::Entity>,
    /// 轨道相机控制器
    pub orbit_controller: OrbitController,
    /// 是否显示碰撞体
    pub show_colliders: bool,
}

/// 应用程序状态
pub struct AlanderApp {
    /// 渲染器
    pub renderer: Renderer,

    /// EGUI 渲染通道 (暂时不直接使用，通过 egui_renderer)
    pub egui_rpass: Option<()>,

    /// 编辑器状态
    pub editor_state: EditorState,

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
            },
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
        
        let is_in_viewport = mouse_pos.x > left_px && mouse_pos.x < right_px && mouse_pos.y > top_px;

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
                    self.input.keyboard.insert(key, input.state);

                    if key == winit::event::VirtualKeyCode::Escape
                        && input.state == ElementState::Pressed
                    {
                        self.running = false;
                    }

                    if input.state == ElementState::Pressed {
                        match key {
                            winit::event::VirtualKeyCode::W => self.gizmo_manager.mode = GizmoMode::Translate,
                            winit::event::VirtualKeyCode::E => self.gizmo_manager.mode = GizmoMode::Rotate,
                            winit::event::VirtualKeyCode::R => self.gizmo_manager.mode = GizmoMode::Scale,
                            _ => {}
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *button == MouseButton::Left && *state == ElementState::Pressed && is_in_viewport {
                    // 如果鼠标下有 Gizmo，不进行场景拾取
                    if self.gizmo_manager.hovered_axis.is_none() {
                        self.pick_entity();
                    }
                }
                self.input.mouse_buttons.insert(*button, *state);
                
                if is_in_viewport {
                    if *button == MouseButton::Middle {
                        self.editor_state.orbit_controller.is_dragging = *state == ElementState::Pressed;
                        if *state == ElementState::Pressed {
                            self.editor_state.orbit_controller.last_mouse_pos = (mouse_pos.x, mouse_pos.y);
                        }
                    }
                }
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
        if let Some(scene) = self.scene_manager.active_scene_mut() {
            // 1. 逻辑更新 (如 Gizmo)

            // Gizmo 更新
            let mouse_pos = self.input.mouse_position;
            let window_size = self.window.inner_size();
            let x = mouse_pos.x / window_size.width as f32;
            let y = mouse_pos.y / window_size.height as f32;
            let ray = self.renderer.screen_to_world_ray(glam::Vec2::new(x, y));
            let is_left_pressed = self.input.mouse_button_pressed(MouseButton::Left);
            
            self.gizmo_manager.update(
                &ray,
                glam::Vec2::new(mouse_pos.x, mouse_pos.y),
                glam::Vec2::new(window_size.width as f32, window_size.height as f32),
                self.renderer.view_proj_glam(),
                is_left_pressed, 
                self.editor_state.selected_entity, 
                &mut scene.world, 
                self.camera_transform.position
            );

            if let Some(entity) = self.editor_state.selected_entity {
                if let Some(transform) = scene.world.get::<Transform>(entity) {
                    let gizmo_lines = self.gizmo_manager.render(transform, self.camera_transform.position);
                    self.renderer.update_debug_overlay(&gizmo_lines);
                }
            } else {
                self.renderer.update_debug_overlay(&[]);
            }

            // 1.5 场景层级更新 (计算 GlobalTransform)
            scene.update_hierarchy();

            // 2. 将逻辑变更同步到物理世界并执行步进
            // 这样确保下一帧的拾取 (在 WindowsEvent 中) 使用的是最新的物理世界
            self.physics_manager.integration_parameters.dt = delta_time;
            self.physics_manager.sync_ecs_to_physics(&mut scene.world);
            self.physics_manager.step();
            self.physics_manager.sync_physics_to_ecs(&mut scene.world);
            self.physics_manager.update_query_pipeline();

            if self.editor_state.show_colliders {
                let collider_lines = self.physics_manager.render_debug_lines();
                self.renderer.update_debug_lines(&collider_lines);
            } else {
                self.renderer.update_debug_lines(&[]);
            }

            // 同步光源与渲染对象
            Self::sync_scene_to_renderer_static(&mut self.renderer, scene);
        }

        self.time.delta = delta_time;
        self.time.elapsed += delta_time;
        self.fps_update_timer += delta_time;
        if self.fps_update_timer >= 0.2 {
            self.displayed_delta_time = delta_time;
            self.fps_update_timer = 0.0;
        }

        self.renderer.update_camera(&self.camera, &self.camera_transform);
    }

    fn sync_scene_to_renderer_static(renderer: &mut Renderer, scene: &mut Scene) {
        // 1. 同步光源
        let mut light_buffer = alander_render::pipelines::LightBuffer::new();
        let mut light_query = scene.world.query::<(&GlobalTransform, &PointLight)>();
        for (global_transform, light) in light_query.iter(&scene.world) {
            // 从全局变换矩阵提取位置
            let pos = global_transform.0.transform_point3(glam::Vec3::ZERO);
            let render_light = alander_render::pipelines::Light::new(
                pos.into(),
                light.color.into(),
                light.intensity,
                light.range,
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
            &mut self.editor_state,
            self.displayed_delta_time
        );

        match action {
            MenuAction::OpenScene => self.on_file_open(),
            MenuAction::SaveScene => self.on_file_save(),
            MenuAction::ImportModel => self.on_import_model(),
            MenuAction::ImportHdr => self.on_import_hdr_environment(),
            MenuAction::ResetCamera => self.reset_camera(),
            MenuAction::Exit => self.running = false,
            _ => {}
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
                    let mesh_names: Vec<String> = model.meshes.iter().map(|m| m.data.name.clone()).collect();
                    let mesh_transforms: Vec<Mat4> = model.meshes.iter().map(|m| m.transform).collect();
                    let ids = self.renderer.add_gltf_model(model);
                    
                    if let Some(scene) = self.scene_manager.active_scene_mut() {
                        for (i, render_id) in ids.into_iter().enumerate() {
                            let name = mesh_names.get(i).cloned().unwrap_or_else(|| format!("Mesh_{}", i));
                            let transform_mat = mesh_transforms.get(i).cloned().unwrap_or(Mat4::IDENTITY);
                            let (scale, rotation, translation) = transform_mat.to_scale_rotation_translation();
                            
                            scene.create_entity((
                                Name(name.clone()),
                                Transform { position: translation, rotation, scale },
                                RenderId(render_id),
                                AssetPath { path: path_str.to_string(), sub_asset: Some(name) },
                            ));
                        }
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
        if let Some(scene) = self.scene_manager.active_scene() {
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

// 辅助类型
type Vec2 = glam::Vec2;
