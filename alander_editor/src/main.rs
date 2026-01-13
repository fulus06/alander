use alander_core::{
    scene::{Camera, Transform},
    InputState, RenderState, Time,
};
use alander_render::renderer::create_cube;
use alander_render::Renderer;
use egui::Context;
// use egui_dock::Tab; // 暂时移除，后续需要时再添加正确的导入
use std::sync::Arc;
use tracing::{info, Level};
use winit::event::{Event, WindowEvent};
use winit::event_loop::ControlFlow;

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

    /// 运行标志
    running: bool,
}

/// 编辑器状态
struct EditorState {
    /// 选中的实体
    selected_entity: Option<uuid::Uuid>,
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
    async fn new(window: &winit::window::Window) -> anyhow::Result<Self> {
        // 初始化日志
        tracing_subscriber::fmt().with_max_level(Level::INFO).init();

        info!("正在初始化 Alander 编辑器...");

        // 创建渲染器
        let renderer = Renderer::new(window, Default::default()).await?;

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
        );
        // renderer.add_object(cube_id, cube); // 暂时注释，后续需要正确实现

        let _cube_id = cube_id; // 保留变量以避免未使用警告
        let _cube = cube; // 保留变量以避免未使用警告

        Ok(Self {
            renderer,
            egui_rpass,
            editor_state: EditorState {
                selected_entity: None,
                orbit_controller: OrbitController::default(),
            },
            camera,
            camera_transform,
            input: InputState::default(),
            time: Time::default(),
            render_state,
            running: true,
        })
    }

    /// 处理输入事件
    fn handle_input(&mut self, event: &winit::event::WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                self.renderer.resize(*size);
                self.render_state.surface_size = (size.width, size.height);
            }
            WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                self.render_state.scale_factor = 1.0; // 暂时硬编码，后续需要正确实现
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

                // 处理轨道控制
                if *button == winit::event::MouseButton::Left {
                    self.editor_state.orbit_controller.is_dragging =
                        *state == winit::event::ElementState::Pressed;
                    if *state == winit::event::ElementState::Pressed {
                        self.editor_state.orbit_controller.last_mouse_pos =
                            (self.input.mouse_position.x, self.input.mouse_position.y);
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

                    self.editor_state.orbit_controller.rotation.0 += delta_x * 0.01;
                    self.editor_state.orbit_controller.rotation.1 += delta_y * 0.01;

                    self.editor_state.orbit_controller.last_mouse_pos =
                        (self.input.mouse_position.x, self.input.mouse_position.y);

                    self.update_camera_transform();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.input.mouse_scroll_delta = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => glam::Vec2::new(*x, *y),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        glam::Vec2::new(pos.x as f32, pos.y as f32)
                    }
                };

                // 更新轨道控制器距离
                self.editor_state.orbit_controller.distance -=
                    self.input.mouse_scroll_delta.y * 0.1;
                self.editor_state.orbit_controller.distance = self
                    .editor_state
                    .orbit_controller
                    .distance
                    .clamp(0.5, 100.0);

                self.update_camera_transform();
            }
            _ => {}
        }
    }

    /// 更新相机变换
    fn update_camera_transform(&mut self) {
        use cgmath::prelude::*;

        let (yaw, pitch) = self.editor_state.orbit_controller.rotation;
        let distance = self.editor_state.orbit_controller.distance;

        // 计算相机位置
        let x = distance * yaw.cos() * pitch.cos();
        let y = distance * pitch.sin();
        let z = distance * yaw.sin() * pitch.cos();

        self.camera_transform.position = glam::Vec3::new(x, y, z);

        // 计算相机旋转（看向目标点）
        let direction = -self.camera_transform.position.normalize();
        let right = glam::Vec3::new(0.0, 1.0, 0.0).cross(direction).normalize();
        let up = direction.cross(right).normalize();

        // 构造旋转矩阵
        let rotation = glam::Mat4::from_cols(
            glam::Vec4::new(right.x, up.x, direction.x, 0.0),
            glam::Vec4::new(right.y, up.y, direction.y, 0.0),
            glam::Vec4::new(right.z, up.z, direction.z, 0.0),
            glam::Vec4::new(0.0, 0.0, 0.0, 1.0),
        );

        self.camera_transform.rotation = glam::Quat::from_mat4(&rotation);
    }

    /// 更新
    fn update(&mut self, delta_time: f32) {
        // 更新时间
        self.time.delta = delta_time;
        self.time.elapsed += delta_time;

        // 更新相机
        self.renderer
            .update_camera(&self.camera, &self.camera_transform);
    }

    /// 渲染
    fn render(&mut self) -> anyhow::Result<()> {
        // 渲染3D场景
        self.renderer.render()?;

        // 渲染EGUI
        self.render_ui()?;

        Ok(())
    }

    /// 渲染EGUI
    fn render_ui(&mut self) -> anyhow::Result<()> {
        // 暂时跳过EGUI渲染，后续需要正确实现
        Ok(())
    }

    /// UI渲染
    fn ui(&mut self, ctx: &Context) {
        // 创建简单布局
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
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

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Alander 3D 编辑器");
            ui.separator();
            ui.label("按 ESC 退出");
            ui.label("鼠标左键拖动旋转相机");
            ui.label("鼠标滚轮缩放");

            ui.separator();
            ui.label(format!("Fps: {:.2}", 1.0 / self.time.delta));
            ui.label(format!("相机位置: {:?}", self.camera_transform.position));
        });
    }

    /// 文件打开回调
    fn on_file_open(&mut self) {
        // TODO: 实现文件打开
    }

    /// 文件保存回调
    fn on_file_save(&mut self) {
        // TODO: 实现文件保存
    }

    /// 重置相机
    fn reset_camera(&mut self) {
        self.camera_transform = Transform::from_translation(glam::Vec3::new(0.0, 1.0, 5.0));
        self.editor_state.orbit_controller = OrbitController::default();
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

    // 创建应用程序
    let mut app = AlanderApp::new(&window).await?;

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
