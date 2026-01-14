use alander_core::{
    scene::{Camera, Transform},
    InputState, RenderState, Time,
};
use alander_render::renderer::Renderer;
use alander_render::renderer::create_cube;
use egui::{self, Color32, Context, FontId, RichText, Ui};
use egui_dock::{DockArea, NodeIndex, Style};
use glam::{Mat4, Quat, Vec2, Vec3};
use std::collections::HashMap;
use tracing::{info, Level};
use uuid::Uuid;
use winit::{
    event::{ElementState, Event, KeyboardInput, MouseButton, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod scene_manager;
use scene_manager::{SceneManager, SceneHandle};

/// 应用程序状态
struct AlanderApp {
    /// 渲染器
    renderer: Renderer,

    /// EGUI 渲染通道
    egui_rpass: Option<()>, // 暂时移除，后续需要正确实现

    /// 编辑器状态
    editor_state: EditorState,

    /// 相机
    camera: Camera,

    /// 相机变换
    camera_transform: Transform,

    /// 输入状态
    input: InputState,

    /// 时间
    time: Time,

    /// 渲染状态
    render_state: RenderState,

    /// 场景管理器
    scene_manager: SceneManager,

    /// 运行标志
    running: bool,

    /// 窗口
    window: std::sync::Arc<winit::window::Window>,

    /// EGUI 上下文
    egui_context: egui::Context,

    /// EGUI 状态
    egui_state: egui_winit::State,

    /// Dock 状态
    dock_state: egui_dock::DockState<String>,

    /// EGUI 渲染器
    egui_renderer: egui_wgpu::Renderer,
}

/// 编辑器状态
struct EditorState {
    /// 选中的实体
    selected_entity: Option<bevy_ecs::entity::Entity>,
    /// 轨道相机控制器
    orbit_controller: OrbitController,
}

/// 轨道相机控制器
struct OrbitController {
    /// 旋转 (X轴和Y轴)
    rotation: (f32, f32),
    /// 距离
    distance: f32,
    /// 目标点
    target: glam::Vec3,
    /// 是否正在拖动
    is_dragging: bool,
    /// 上次鼠标位置
    last_mouse_pos: (f32, f32),
}

impl AlanderApp {
    /// 创建新的应用程序
    /// 创建新的应用程序
    async fn new(window: std::sync::Arc<winit::window::Window>) -> anyhow::Result<Self> {
        // 初始化日志
        tracing_subscriber::fmt().with_max_level(Level::INFO).init();

        info!("正在初始化 Alander 编辑器...");

        // 创建渲染器
        let mut renderer = Renderer::new(&window).await?;

        // 创建 EGUI 渲染通道（暂时移除）
        let egui_rpass = None;

        // 创建相机
        let camera = Camera::perspective(
            std::f32::consts::PI / 4.0,
            renderer.size().width as f32 / renderer.size().height as f32,
            0.1,
            100.0,
        );

        // 初始相机位置
        let camera_transform = Transform::from_translation(glam::Vec3::new(0.0, 1.0, 5.0));

        // 渲染状态
        let render_state = RenderState {
            surface_size: (renderer.size().width, renderer.size().height),
            scale_factor: window.scale_factor(),
        };

        // 添加示例立方体
        let cube_id = uuid::Uuid::new_v4();

        // 获取渲染器的管线信息，以便传递model_bind_group_layout
        let cube = create_cube(
            renderer.device(),
            &renderer.pipelines().mesh.model_bind_group_layout,
            &renderer.pipelines().mesh.texture_bind_group_layout,
            renderer.default_texture(),
        );
        // renderer.add_object(cube_id, cube); // 暂时注释，后续需要正确实现

        let _cube_id = cube_id; // 保留变量以避免未使用警告
        let _cube = cube; // 保留变量以避免未使用警告

        // 创建场景管理器并添加测试场景
        let mut scene_manager = SceneManager::new();
        scene_manager.create_test_scene(&mut renderer);

        // 初始化EGUI
        let egui_context = egui::Context::default();
        let egui_state = egui_winit::State::new(&*window);
        let dock_state = egui_dock::DockState::new(vec!["场景".to_string(), "属性".to_string()]);
        
        // 初始化EGUI渲染器
        let egui_renderer = egui_wgpu::Renderer::new(
            renderer.device(),
            renderer.format(),
            None,
            1,
        );

        let mut app = Self {
            renderer,
            egui_rpass,
            editor_state: EditorState {
                selected_entity: None,
                orbit_controller: OrbitController {
                    rotation: (0.0, -0.2), // Yaw: 0 (from +Z), Pitch: -0.2 (looking slightly down)
                    distance: 10.0,
                    target: glam::Vec3::ZERO,
                    is_dragging: false,
                    last_mouse_pos: (0.0, 0.0),
                },
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
            dock_state,
            egui_renderer,
        };

        app.update_camera_transform();
        app.setup_fonts();

        Ok(app)
    }

    /// 设置字体以支持中文
    fn setup_fonts(&self) {
        let mut fonts = egui::FontDefinitions::default();

        // 加载中文字体
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

                // 将中文字体添加到等宽和比例字体家族中
                fonts.families.get_mut(&egui::FontFamily::Proportional)
                    .unwrap()
                    .insert(0, "chinese_font".to_owned());

                fonts.families.get_mut(&egui::FontFamily::Monospace)
                    .unwrap()
                    .push("chinese_font".to_owned());
                
                font_loaded = true;
                tracing::info!("成功加载中文字体: {}", path);
                break;
            }
        }

        if !font_loaded {
            tracing::warn!("未能加载中文字体，中文显示可能异常");
        }

        self.egui_context.set_fonts(fonts);
    }

    /// 处理输入事件
    fn handle_input(&mut self, event: &winit::event::WindowEvent) {
        // 先让 EGUI 处理事件
        let _egui_res = self.egui_state.on_event(&self.egui_context, event);

        // 计算鼠标是否在 3D 视口内（物理像素坐标）
        let window_size = self.window.inner_size();
        let scale_factor = self.window.scale_factor() as f32;
        let mouse_pos = self.input.mouse_position;
        
        let left_px = 200.0 * scale_factor;
        let right_px = window_size.width as f32 - 250.0 * scale_factor;
        let top_px = 30.0 * scale_factor; // 假设菜单栏高度稍多一点
        
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

                    // ESC 退出
                    if key == winit::event::VirtualKeyCode::Escape
                        && input.state == winit::event::ElementState::Pressed
                    {
                        self.running = false;
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.input.mouse_buttons.insert(*button, *state);
                
                // 仿 Blender: 按住中键拖拽进行旋转
                // 只要鼠标在视口内，我们就允许旋转，无视 EGUI 是否消耗（因为 CentralPanel 总是会拦截）
                if is_in_viewport {
                    if *button == winit::event::MouseButton::Middle {
                        self.editor_state.orbit_controller.is_dragging =
                            *state == winit::event::ElementState::Pressed;
                        if *state == winit::event::ElementState::Pressed {
                            self.editor_state.orbit_controller.last_mouse_pos =
                                (mouse_pos.x, mouse_pos.y);
                            tracing::info!("Blender 轨道旋转开始 (中键)");
                        } else {
                            tracing::info!("Blender 轨道旋转停止");
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.input.mouse_position = glam::Vec2::new(position.x as f32, position.y as f32);

                // 更新轨道控制器
                if self.editor_state.orbit_controller.is_dragging {
                    let delta_x = self.input.mouse_position.x
                        - self.editor_state.orbit_controller.last_mouse_pos.0;
                    let delta_y = self.input.mouse_position.y
                        - self.editor_state.orbit_controller.last_mouse_pos.1;

                    // 灵敏度系数
                    let sensitivity = 0.005;
                    self.editor_state.orbit_controller.rotation.0 -= delta_x * sensitivity;
                    self.editor_state.orbit_controller.rotation.1 -= delta_y * sensitivity;

                    // 限制俯仰角
                    self.editor_state.orbit_controller.rotation.1 = self
                        .editor_state
                        .orbit_controller
                        .rotation
                        .1
                        .clamp(-std::f32::consts::PI / 2.1, std::f32::consts::PI / 2.1);

                    self.editor_state.orbit_controller.last_mouse_pos =
                        (self.input.mouse_position.x, self.input.mouse_position.y);

                    self.update_camera_transform();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // 如果鼠标在中心视口区域，且 EGUI 没在处理复杂的交互（如滑块），就允许缩放
                if is_in_viewport && !self.egui_context.is_using_pointer() {
                    // 更新轨道控制器距离
                    let zoom_speed = (self.editor_state.orbit_controller.distance * 0.1).max(0.5);
                    let scroll_y = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => *y * 2.0,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.05,
                    };

                    self.editor_state.orbit_controller.distance -= scroll_y * zoom_speed;
                    self.editor_state.orbit_controller.distance = self
                        .editor_state
                        .orbit_controller
                        .distance
                        .clamp(0.1, 2000.0);

                    self.update_camera_transform();
                }
            }
            _ => {}
        }
    }

    /// 更新相机变换
    fn update_camera_transform(&mut self) {
        let (yaw, pitch) = self.editor_state.orbit_controller.rotation;
        let distance = self.editor_state.orbit_controller.distance;

        // 基础旋转：先绕 Y 轴转 (yaw)，再绕 X 轴转 (pitch)
        let rotation = glam::Quat::from_rotation_y(yaw) * glam::Quat::from_rotation_x(pitch);
        
        // 计算相机在世界空间的位置（将默认的 Z 轴方向旋转并平移）
        // 在该控制器的逻辑下，相机默认看向 -Z 方向（模型中心）
        self.camera_transform.position = rotation * glam::Vec3::new(0.0, 0.0, distance);
        self.camera_transform.rotation = rotation;
    }

    /// 更新
    fn update(&mut self, delta_time: f32) {
        // 同步 ECS 中的 Transform 到渲染器中的模型矩阵
        if let Some(scene) = self.scene_manager.active_scene_mut() {
            let mut query = scene.world.query::<(&alander_core::scene::Transform, &alander_core::scene::RenderId)>();
            for (transform, render_id) in query.iter(&scene.world) {
                let matrix = transform.compute_matrix();
                // 将 glam::Mat4 转换为 cgmath::Matrix4
                let m = matrix.to_cols_array_2d();
                let cg_matrix = cgmath::Matrix4::new(
                    m[0][0], m[0][1], m[0][2], m[0][3],
                    m[1][0], m[1][1], m[1][2], m[1][3],
                    m[2][0], m[2][1], m[2][2], m[2][3],
                    m[3][0], m[3][1], m[3][2], m[3][3],
                );
                self.renderer.update_object_model(&render_id.0, cg_matrix);
            }
        }
        
        // 更新时间
        self.time.delta = delta_time;
        self.time.elapsed += delta_time;

        // 更新相机
        self.renderer
            .update_camera(&self.camera, &self.camera_transform);
    }

    /// 渲染
    fn render(&mut self) -> anyhow::Result<()> {
        let output = self.renderer.surface().get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            self.renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("渲染编码器"),
                });

        // 1. 开始 EGUI 帧并构建 UI
        self.egui_state.set_pixels_per_point(self.window.scale_factor() as f32);
        let raw_input = self.egui_state.take_egui_input(&*self.window);
        self.egui_context.begin_frame(raw_input);
        self.ui(&self.egui_context.clone());
        let full_output = self.egui_context.end_frame();
        let paint_jobs = self.egui_context.tessellate(full_output.shapes);

        // 2. 更新 EGUI 资源 (准备阶段)
        // 2. 更新 EGUI 资源 (准备阶段)
        let screen_descriptor = egui_wgpu::renderer::ScreenDescriptor {
            size_in_pixels: [self.render_state.surface_size.0, self.render_state.surface_size.1],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        for (id, delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(
                self.renderer.device(),
                self.renderer.queue(),
                *id,
                delta,
            );
        }

        self.egui_renderer.update_buffers(
            self.renderer.device(),
            self.renderer.queue(),
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        // 3. 渲染 3D 场景
        self.renderer.render_scene(&view, &mut encoder);

        // 4. 渲染 EGUI (叠加阶段)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("EGUI 渲染通道"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // 加载场景渲染结果
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

        // 5. 提交并呈现
        self.renderer.queue().submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    /// UI渲染
    fn ui(&mut self, ctx: &egui::Context) {
        // 1. 顶部菜单栏
        egui::TopBottomPanel::top("top_menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("文件", |ui| {
                    if ui.button("打开").clicked() {
                        self.on_file_open();
                        ui.close_menu();
                    }
                    if ui.button("保存").clicked() {
                        self.on_file_save();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("退出").clicked() {
                        self.running = false;
                        ui.close_menu();
                    }
                });
                ui.menu_button("视图", |ui| {
                    if ui.button("重置相机").clicked() {
                        self.reset_camera();
                        ui.close_menu();
                    }
                });
            });
        });

        // 2. 左侧场景面板
        egui::SidePanel::left("scene_panel")
            .resizable(true)
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.heading("场景管理器");
                ui.separator();
                
                if let Some(scene) = self.scene_manager.active_scene() {
                    let entities = scene.get_entities_with_names();
                    for (entity, name) in entities {
                        let is_selected = Some(entity) == self.editor_state.selected_entity;
                        
                        let label = if is_selected {
                            egui::RichText::new(format!("{} (E)", name)).strong().color(egui::Color32::from_rgb(255, 255, 0))
                        } else {
                            egui::RichText::new(name)
                        };
                        
                        if ui.selectable_label(is_selected, label).clicked() {
                            self.editor_state.selected_entity = Some(entity);
                            tracing::info!("选中实体: {:?}", entity);
                        }
                    }
                }
            });

        // 3. 右侧属性面板
        egui::SidePanel::right("properties_panel")
            .resizable(true)
            .default_width(250.0)
            .show(ctx, |ui| {
                ui.heading("实体属性");
                ui.separator();
                if let Some(id) = self.editor_state.selected_entity {
                    ui.label(format!("实体 ID: {:?}", id));
    // 更多属性编辑...
                } else {
                    ui.label("未选中实体");
                }
            });

        // 4. 中央透明区域（用于查看 3D 场景）
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
            .show(ctx, |_ui| {
                // 中央区域留空，背景将显示 3D 场景
            });
    }


    /// 文件打开回调
    fn on_file_open(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("glTF 模型", &["gltf", "glb"])
            .pick_file()
        {
            let path_str = path.to_string_lossy();
            tracing::info!("正在加载 glTF: {}", path_str);
            
            let loader = alander_core::assets::GltfLoader;
            match loader.load_scene(&path_str) {
                Ok(model) => {
                    let _mesh_count = model.meshes.len();
                    let mesh_names: Vec<String> = model.meshes.iter().map(|m| m.data.name.clone()).collect();
                    let mesh_transforms: Vec<glam::Mat4> = model.meshes.iter().map(|m| m.transform).collect();
                    
                    let ids = self.renderer.add_gltf_model(model);
                    tracing::info!("成功加载 glTF，创建了 {} 个渲染对象", ids.len());
                    
                    // 在 ECS 中创建对应的实体
                    if let Some(scene) = self.scene_manager.active_scene_mut() {
                        for i in 0..ids.len() {
                            let render_id = ids[i];
                            let name = mesh_names.get(i).cloned().unwrap_or_else(|| format!("Mesh_{}", i));
                            let transform_mat = mesh_transforms.get(i).cloned().unwrap_or(glam::Mat4::IDENTITY);
                            
                            let (scale, rotation, translation) = transform_mat.to_scale_rotation_translation();
                            
                            scene.create_entity((
                                alander_core::scene::Name(name),
                                alander_core::scene::Transform {
                                    position: translation,
                                    rotation,
                                    scale,
                                },
                                alander_core::scene::RenderId(render_id),
                            ));
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("加载 glTF 失败: {}", e);
                }
            }
        }
    }

    /// 文件保存回调
    fn on_file_save(&mut self) {
        // TODO: 实现文件保存
    }

    /// 重置相机
    fn reset_camera(&mut self) {
        self.editor_state.orbit_controller = OrbitController {
            rotation: (0.0, -0.2), // Yaw: 0, Pitch: -0.2 (看向斜下方)
            distance: 10.0,
            target: glam::Vec3::ZERO,
            is_dragging: false,
            last_mouse_pos: (0.0, 0.0),
        };
        self.update_camera_transform();
    }
}

/// Tab 查看器实现
struct TabViewer<'a> {
    app: &'a mut AlanderApp,
}

impl<'a> egui_dock::TabViewer for TabViewer<'a> {
    type Tab = String;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.as_str().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab.as_str() {
            "场景" => {
                ui.heading("场景管理器");
                ui.separator();
                for (handle, name) in self.app.scene_manager.get_scenes() {
                    let label = if Some(handle) == self.app.scene_manager.active_scene().map(|s| &s.handle) {
                        egui::RichText::new(name).strong().color(egui::Color32::from_rgb(255, 255, 0))
                    } else {
                        egui::RichText::new(name)
                    };
                    
                    if ui.selectable_label(false, label).clicked() {
                        // 切换场景逻辑
                    }
                }
            }
            "属性" => {
                ui.heading("实体属性");
                ui.separator();
                if let Some(id) = self.app.editor_state.selected_entity {
                    ui.label(format!("UUID: {:?}", id));
                    // 更多属性编辑...
                } else {
                    ui.label("未选中实体");
                }
            }
            _ => {
                ui.label(format!("未知面板: {}", tab));
            }
        }
    }
}

impl Default for OrbitController {
    fn default() -> Self {
        Self {
            rotation: (0.0, 0.0),
            distance: 5.0,
            target: glam::Vec3::new(0.0, 0.0, 0.0),
            is_dragging: false,
            last_mouse_pos: (0.0, 0.0),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 创建窗口和事件循环
    let (event_loop, window) = alander_render::create_window(
        "Alander 3D 编辑器",
        winit::dpi::PhysicalSize::new(1280, 720),
    )?;

    // 包装窗口在 Arc 中
    let window = std::sync::Arc::new(window);

    // 创建应用程序
    let mut app = AlanderApp::new(window.clone()).await?;

    // 记录最后更新时间
    let mut last_update = std::time::Instant::now();

    // 运行循环
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                // 处理输入
                app.handle_input(event);

                match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(_) => {
                        window.request_redraw();
                    }
                    _ => {}
                }
            }
            Event::RedrawRequested(_) => {
                // 更新应用程序
                let now = std::time::Instant::now();
                let delta_time = now.duration_since(last_update).as_secs_f32();
                last_update = now;
                app.update(delta_time);

                // 渲染
                if let Err(e) = app.render() {
                    eprintln!("渲染错误: {}", e);
                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            _ => {}
        }

        if !app.running {
            *control_flow = ControlFlow::Exit;
        }
    });
}


/// 用于EGUI的配置
#[allow(dead_code)]
struct RendererConfig {
    samples: u32,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self { samples: 1 }
    }
}
