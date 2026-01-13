//! 基础窗口示例
//!
//! 此示例展示如何创建一个基本的Alander窗口，显示一个旋转的立方体。

use alander_core::scene::{Camera, Transform};
use alander_render::renderer::{create_cube, Renderer};
use anyhow::Result;
use std::time::{Duration, Instant};
use tracing::{info, Level};
use winit::event::{Event, WindowEvent};
use winit::event_loop::ControlFlow;

/// 性能统计信息
struct PerformanceStats {
    frame_count: u32,
    fps: f32,
    frame_time_ms: f32,
    last_update: Instant,
    last_frame_time: Instant,
}

fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("启动基础窗口示例");

    // 创建窗口和事件循环
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("Alander - 基础窗口示例")
        .with_inner_size(winit::dpi::PhysicalSize::new(800, 600))
        .build(&event_loop)?;

    // 创建渲染器
    pollster::block_on(async {
        let mut renderer = Renderer::new(&window).await?;

        // 创建相机
        let camera = Camera::perspective(std::f32::consts::PI / 4.0, 800.0 / 600.0, 0.1, 100.0);

        // 创建立方体
        let cube_id = uuid::Uuid::new_v4();
        let cube = create_cube(
            renderer.device(),
            &renderer.pipelines().mesh.model_bind_group_layout,
        );
        renderer.add_object(cube_id, cube);

        // 调试信息
        tracing::debug!("立方体已创建并添加到场景，ID: {:?}", cube_id);

        // 相机放在立方体前面，沿着负Z轴观看
        let camera_transform =
            Transform::from_translation(alander_core::math::Vec3::new(0.0, 0.0, 4.0));

        // 初始旋转值
        let mut rotation = 0.0f32;
        let mut last_update = Instant::now();

        // 性能统计
        let mut perf_stats = PerformanceStats {
            frame_count: 0,
            fps: 0.0,
            frame_time_ms: 0.0,
            last_update: Instant::now(),
            last_frame_time: Instant::now(),
        };

        // 运行循环
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            match event {
                Event::WindowEvent {
                    ref event,
                    window_id,
                } if window_id == window.id() => match event {
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    WindowEvent::Resized(size) => {
                        renderer.resize(*size);
                    }
                    WindowEvent::KeyboardInput { input, .. } => {
                        if let Some(key) = input.virtual_keycode {
                            if key == winit::event::VirtualKeyCode::Escape
                                && input.state == winit::event::ElementState::Pressed
                            {
                                *control_flow = ControlFlow::Exit;
                            }
                        }
                    }
                    _ => {}
                },
                Event::MainEventsCleared => {
                    window.request_redraw();
                }
                Event::RedrawRequested(_) => {
                    // 更新旋转
                    let now = Instant::now();
                    let delta_time = now.duration_since(last_update).as_secs_f32();
                    last_update = now;

                    // 更新性能统计
                    let current_frame_time = Instant::now();
                    let frame_duration =
                        current_frame_time.duration_since(perf_stats.last_frame_time);
                    perf_stats.frame_time_ms = frame_duration.as_secs_f32() * 1000.0;
                    perf_stats.last_frame_time = current_frame_time;
                    perf_stats.frame_count += 1;

                    // 每秒更新FPS
                    if current_frame_time.duration_since(perf_stats.last_update)
                        >= Duration::from_secs(1)
                    {
                        perf_stats.fps = perf_stats.frame_count as f32;
                        perf_stats.frame_count = 0;
                        perf_stats.last_update = current_frame_time;

                        // 输出性能信息到控制台
                        println!(
                            "🎮 性能统计: FPS={:.1}, 帧时间={:.2}ms",
                            perf_stats.fps, perf_stats.frame_time_ms
                        );
                    }

                    rotation += delta_time * 1.0; // 每秒旋转1.0弧度

                    // 立方体变换
                    let translation = cgmath::Vector3::new(0.0, 0.0, 0.0);
                    let scale_factor = 1.5;

                    let rotation_matrix = cgmath::Matrix4::from_angle_y(cgmath::Rad(rotation));
                    let translation_matrix = cgmath::Matrix4::from_translation(translation);
                    let scale_matrix = cgmath::Matrix4::from_scale(scale_factor);

                    let model_matrix = translation_matrix * rotation_matrix * scale_matrix;
                    renderer.update_object_model(&cube_id, model_matrix);

                    // 更新相机
                    renderer.update_camera(&camera, &camera_transform);

                    // 渲染
                    if let Err(e) = renderer.render() {
                        eprintln!("渲染错误: {}", e);
                        *control_flow = ControlFlow::Exit;
                    } else {
                        // 每10帧输出一次详细的渲染信息
                        if perf_stats.frame_count % 10 == 0 {
                            println!("📊 渲染状态: 立方体旋转角度={:.2}rad", rotation);
                        }
                    }
                }
                _ => {}
            }
        });
    })
}
