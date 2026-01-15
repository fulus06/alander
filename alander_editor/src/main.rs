mod scene_manager;
mod physics_manager;
mod gizmo_manager;
mod camera_controller;
mod ui;
mod app;

use app::AlanderApp;
use winit::{
    event::{Event, WindowEvent},
    event_loop::ControlFlow,
};
use tracing::Level;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

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
            Event::MainEventsCleared => {
                let now = std::time::Instant::now();
                let delta_time = now.duration_since(last_update).as_secs_f32();
                last_update = now;

                // 更新
                app.update(delta_time);

                // 请求重绘
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                // 渲染
                if let Err(e) = app.render() {
                    tracing::error!("渲染失败: {}", e);
                }
            }
            _ => {}
        }

        // 检查运行标志
        if !app.running {
            *control_flow = ControlFlow::Exit;
        }
    });
}
