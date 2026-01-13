# Alander 技术实施指南

## 项目结构建议

```
alander/
├── alander_core/           # 核心数据结构和算法
│   ├── ecs/               # ECS系统相关代码
│   ├── math/              # 数学库和工具
│   ├── scene/             # 场景管理系统
│   └── assets/            # 资源加载和管理
├── alander_render/        # 渲染引擎
│   ├── wgpu_backend/      # WGPU渲染后端
│   ├── shaders/           # WGSL着色器
│   ├── pipeline/          # 渲染管线
│   └── materials/         # 材质系统
├── alander_editor/        # 编辑器界面
│   ├── ui/                # EGUI界面组件
│   ├── commands/          # 命令系统
│   ├── tools/             # 编辑工具
│   └── panels/            # 编辑器面板
├── alander_nodes/         # 节点系统
│   ├── graph/             # 节点图编辑器
│   ├── dataflow/          # 数据流引擎
│   └── library/           # 节点库
├── alander_physics/       # 物理集成
├── alander_animation/     # 动画系统
├── alander_network/       # 网络和协作
└── alander_wasm/          # WebAssembly绑定
```

## 核心依赖与版本

```toml
[workspace.dependencies]
# 核心框架
winit = "0.28"
wgpu = "0.17"
egui = "0.23"
egui_dock = "0.11"
egui_node_graph = "0.17"

# ECS架构
bevy_ecs = "0.12" # 或 hecs = "0.10"

# 数学库
glam = "0.24" # 快速 SIMD 优化的数学库

# 图形相关
gltf = "1.4" # glTF加载
image = "0.24"

# 物理引擎
rapier3d = "0.17"

# 序列化
serde = { version = "1.0", features = ["derive"] }
ron = "0.8" # 人类可读的数据格式

# 日志
tracing = "0.1"
tracing-subscriber = "0.3"

# 网络协作
matchbox_socket = "0.7"

# WebAssembly
wasm-bindgen = "0.2"
web-sys = "0.3"
```

## 关键实现细节

### 1. ECS系统设计

```rust
// alander_core/src/lib.rs
pub use bevy_ecs::prelude::*;

// 核心组件
#[derive(Component)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

#[derive(Component)]
pub struct Mesh {
    pub handle: Handle<MeshData>,
}

#[derive(Component)]
pub struct Material {
    pub handle: Handle<MaterialData>,
}

#[derive(Component)]
pub struct Name(pub String);

// 资源管理系统
pub struct Assets<T> {
    assets: HashMap<Handle<T>, T>,
    loader: Box<dyn AssetLoader<T>>,
}

// 全局资源
#[derive(Resource)]
pub struct Time {
    pub delta: f32,
    pub elapsed: f32,
}

#[derive(Resource)]
pub struct Input {
    pub mouse_position: Vec2,
    pub mouse_buttons: [bool; 3],
    pub keys: HashSet<VirtualKeyCode>,
}
```

### 2. 渲染器架构

```rust
// alander_render/src/renderer.rs
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    // 渲染管线
    pipelines: HashMap<String, wgpu::RenderPipeline>,
    // 着色器
    shaders: HashMap<String, wgpu::ShaderModule>,
}

impl Renderer {
    pub fn new(window: &Window) -> Self {
        // 初始化WGPU
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        
        // 设备和队列请求
        let (device, queue) = instance
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await;
            
        // 配置交换链等...
        Self {
            device,
            queue,
            config,
            pipelines: HashMap::new(),
            shaders: HashMap::new(),
        }
    }
    
    pub fn render(&mut self, world: &World) -> Result<(), wgpu::SurfaceError> {
        // 1. 更新相机缓冲区
        self.update_camera_buffers(world);
        
        // 2. 获取下一帧
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        // 3. 创建命令编码器
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });
        
        // 4. 渲染场景
        self.render_scene(&mut encoder, &view, world);
        
        // 5. 提交命令
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        
        Ok(())
    }
}
```

### 3. WGSL着色器示例

```wgsl
// alander_render/src/shaders/mesh.wgsl
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct Camera {
    view_proj: mat4x4<f32>,
    position: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var<uniform> model: mat4x4<f32>;

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    
    // 世界空间位置
    out.world_position = (model * vec4<f32>(position, 1.0)).xyz;
    
    // 世界空间法线
    out.world_normal = (model * vec4<f32>(normal, 0.0)).xyz;
    
    // 裁剪空间位置
    out.clip_position = camera.view_proj * vec4<f32>(out.world_position, 1.0);
    
    // 传递UV
    out.uv = uv;
    
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 简单PBR光照
    let albedo = vec3<f32>(0.8, 0.3, 0.2);
    let metallic = 0.0;
    let roughness = 0.7;
    
    // 光照计算...
    let light_dir = normalize(vec3<f32>(1.0, 1.0, 1.0));
    let n = normalize(in.world_normal);
    
    let ndotl = max(dot(n, light_dir), 0.0);
    let color = albedo * ndotl + vec3<f32>(0.1); // 环境光
    
    return vec4<f32>(color, 1.0);
}
```

### 4. 命令系统设计

```rust
// alander_editor/src/commands/mod.rs
pub trait Command: Send + Sync {
    fn execute(&mut self, world: &mut World);
    fn undo(&mut self, world: &mut World);
    fn redo(&mut self, world: &mut World) { self.execute(world); }
}

pub struct MoveEntityCommand {
    entity: Entity,
    old_position: Vec3,
    new_position: Vec3,
}

impl Command for MoveEntityCommand {
    fn execute(&mut self, world: &mut World) {
        if let Some(mut transform) = world.get_mut::<Transform>(self.entity) {
            transform.position = self.new_position;
        }
    }
    
    fn undo(&mut self, world: &mut World) {
        if let Some(mut transform) = world.get_mut::<Transform>(self.entity) {
            transform.position = self.old_position;
        }
    }
}

// 命令历史记录
#[derive(Resource)]
pub struct CommandHistory {
    commands: Vec<Box<dyn Command>>,
    current_index: usize,
}

impl CommandHistory {
    pub fn execute(&mut self, command: Box<dyn Command>, world: &mut World) {
        command.execute(world);
        
        // 如果我们在历史中间执行新命令，删除前方的历史记录
        self.commands.truncate(self.current_index);
        self.commands.push(command);
        self.current_index += 1;
    }
    
    pub fn undo(&mut self, world: &mut World) -> bool {
        if self.current_index > 0 {
            self.current_index -= 1;
            if let Some(command) = &mut self.commands[self.current_index] {
                command.undo(world);
                return true;
            }
        }
        false
    }
    
    pub fn redo(&mut self, world: &mut World) -> bool {
        if self.current_index < self.commands.len() {
            if let Some(command) = &mut self.commands[self.current_index] {
                command.redo(world);
                self.current_index += 1;
                return true;
            }
        }
        false
    }
}
```

### 5. 节点系统基础

```rust
// alander_nodes/src/graph.rs
pub struct NodeGraph {
    pub nodes: HashMap<NodeId, Node>,
    pub connections: Vec<(NodeId, OutputId, NodeId, InputId)>,
}

pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub position: Vec2,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    pub data: Box<dyn NodeData>,
}

pub trait NodeData: Send + Sync {
    fn evaluate(&self, context: &mut EvaluationContext) -> NodeResult;
    fn category(&self) -> &str;
    fn name(&self) -> &str;
}

// 输入类型
pub enum InputValue {
    Float(f32),
    Vec2(Vec2),
    Vec3(Vec3),
    Mesh(Handle<MeshData>),
    Material(Handle<MaterialData>),
    // 更多类型...
}

// 评估上下文
pub struct EvaluationContext<'a> {
    pub world: &'a mut World,
    pub node_graph: &'a NodeGraph,
    pub evaluated_nodes: HashMap<NodeId, NodeResult>,
}

// 示例节点：添加变换
pub struct TransformNode {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl NodeData for TransformNode {
    fn evaluate(&self, context: &mut EvaluationContext) -> NodeResult {
        // 获取输入的网格
        let input_mesh = self.get_input::<Handle<MeshData>>(context, 0)?;
        
        // 创建变换后的网格副本
        let transform = Mat4::from_translation(self.translation) 
            * Mat4::from_quat(self.rotation) 
            * Mat4::from_scale(self.scale);
            
        // 应用变换到网格顶点
        // TODO: 实现网格变换逻辑
        
        NodeResult::Mesh(new_mesh_handle)
    }
    
    fn category(&self) -> &str { "Geometry" }
    fn name(&self) -> &str { "Transform" }
}
```

### 6. WebASsembly集成

```rust
// alander_wasm/src/lib.rs
use wasm_bindgen::prelude::*;

// 当`wasm-pack`构建`crate`时。
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// 从JavaScript导出的函数
#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<(), JsValue> {
    // 获取canvas元素
    let document = web_sys::window()
        .ok_or_else(|| JsValue::from_str("No window"))?
        .document()
        .ok_or_else(|| JsValue::from_str("No document"))?;
        
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| JsValue::from_str("No canvas"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    
    // 启动应用
    // 这里需要调整应用启动方式，使其与浏览器兼容
    alander_editor::start_in_browser(canvas)?;
    
    Ok(())
}

// 导出函数以加载模型
#[wasm_bindgen]
pub async fn load_gltf(url: &str) -> Result<u32, JsValue> {
    // 下载文件
    let req = web_sys::Request::new_with_str(url)?;
    resp = JsFuture::from(window.fetch_with_request(&req)).await?.dyn_into()?;
    let array_buffer = JsFuture::from(resp.array_buffer()?).await?.dyn_into()?;
    let bytes = js_sys::Uint8Array::new(&array_buffer).to_vec();
    
    // 解析gltf
    let gltf = alander_assets::load_gltf_from_bytes(&bytes);
    let entity_id = alander_core::add_mesh_to_scene(gltf);
    
    Ok(entity_id)
}
```

## 开发工具和工作流程

### 1. 代码格式化和静态分析

在项目根目录添加配置文件：

`.rustfmt.toml`:
```toml
edition = "2021"
imports_granularity = "Crate"
group_imports = "StdExternalCrate"
```

`.clippy.toml`:
```toml
msrv = "1.70"
```

### 2. CI/CD Workflow

`.github/workflows/rust.yml`:
```yaml
name: Rust

on:
  push:
    branches: [ "main" ]
  pull_request:
    branches: [ "main" ]

env:
  CARGO_TERM_COLOR: always

jobs:
  check:
    name: Check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo check --workspace

  test:
    name: Test Suite
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --workspace

  fmt:
    name: Rustfmt
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo fmt --all -- --check

  clippy:
    name: Clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo clippy --workspace -- -D warnings

  build-wasm:
    name: Build WASM
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
      - run: wasm-pack build --target web --out-dir pkg
```

### 3. 调试建议

- 使用 `tracing` 和 `tracing-subscriber` 进行结构化日志记录
- 使用 `env_logger` 和 `RUST_LOG` 环境变量控制日志级别
- 对于性能关键代码，使用 `criterion` 进行基准测试
- 使用 `console_error_panic_hook` 在 WASM 中更有意义的错误信息

## 性能考虑

### 1. 内存布局优化

- 使用 `#[repr(C)]` 确保 GPU 资源的结构对齐
- 对于大量使用的结构，考虑使用 `#[repr(transparent)]` 和手动内联

### 2. 批处理渲染

- 按材质和网格批处理渲染调用
- 对于大型场景考虑使用间接渲染

### 3. 多线程架构

- 使用 `rayon` 进行 CPU 并行计算
- 利用 `bevy_ecs` 的并行查询和系统调度
- 对于 WASM，考虑使用 Web Workers

## 下一步行动

1. 立即开始第一阶段的工作：设置工作区、创建窗口、集成 ECS
2. 建立代码仓库和 CI/CD 流水线
3. 创建示例项目验证渲染管线
4. 设计节点系统的 MVP 版本

随着开发进展，本指南应定期更新以反映最新的架构决策和最佳实践。