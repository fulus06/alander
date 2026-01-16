# Alander
[English Document](./README.md)

Alander 是一个基于 Rust 开发的现代化 3D 创作套件（DCC, Digital Content Creation），旨在成为 Blender 的强有力竞争者。

[当前状态](./doc/alander003%20-%20物理引擎+层级.mp4)

## 项目愿景

Alander 不仅仅是一个 3D 软件，而是**"面向未来的程序化 3D 引擎"**，具有以下核心差异化特点：

- **云原生与协作**：像 Figma 或 Google Docs 一样支持多人实时协作
- **非破坏性工作流**：全面采用节点式和堆栈式逻辑，所有操作均可随时调整
- **ECS 架构**：高性能并行计算，可轻松处理百万级物体


## 支持平台
* macos


## 示例

```bash
cargo run --bin alander
```

# 下面的内容不要看，是以后的文档，暂时仅供参考：

## 架构概述

Alander 采用模块化架构，主要包含以下核心模块：

```
alander/
├── alander_core/      # 核心数据结构和ECS系统
├── alander_render/    # 基于WGPU的渲染管线
├── alander_editor/    # 编辑器界面和交互
└── alander_wasm/      # WebAssembly 集成
```

## 开发路线

项目分为5个主要阶段：

1. **第一阶段：基石构建** - 构建3D查看器（当前阶段）
2. **第二阶段：交互与编辑器框架** - 从查看器转变为编辑器
3. **第三阶段：节点系统与程序化生成** - 实现差异化竞争力
4. **第四阶段：动画与物理** - 让场景动起来
5. **第五阶段：Web 协作与云端化** - 实现杀手级功能

## 快速开始

### 前提条件

- Rust 1.70+
- 适用于您平台的 Vulkan、Metal 或 DirectX 12 驱动
- 对于WebAssembly功能：现代浏览器支持WebGPU

### 构建与运行

#### 桌面版本

```bash
# 克隆仓库
git clone https://github.com/your-username/alander.git
cd alander

# 构建并运行代码示例
cargo run --example basic_window

# 构建并运行编辑器
cargo run --package alander_editor
```

#### WebAssembly版本

```bash
# 安装wasm-pack
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# 构建WebAssembly包
wasm-pack build --target web --out-dir pkg

# 运行本地Web服务器（假设您已安装Python）
python -m http.server
```

然后在浏览器中打开 `index.html`（您需要创建一个简单的HTML文件来加载Wasm模块）。

### 项目结构说明

- `alander_core` - 定义核心组件（Transform、Mesh、Material等）和ECS系统
- `alander_render` - 实现基于WGPU的渲染管线、着色器和资源管理
- `alander_editor` - 提供编辑器界面、工具栏和交互逻辑
- `alander_wasm` - 提供WebAssembly绑定和浏览器集成



这将创建一个800x600的窗口，显示一个旋转的立方体。按ESC键退出。

## 开发计划

我们采用敏捷开发方法，每个迭代（2-3周）都有明确的目标和交付物。当前我们专注于第一阶段：创建一个能够加载和显示3D模型的查看器。

详细计划请参阅：
- [开发计划](开发计划.md) - 高层次5阶段开发计划
- [详细开发计划](Alander详细开发计划.md) - 每周任务分解
- [技术路线](技术路线.md) - 技术栈和实施指南
- [敏捷开发实施指南](Alander敏捷开发实施指南.md) - 团队协作指南

## 贡献

我们欢迎社区贡献！请阅读"贡献指南"了解如何参与项目。

当前最需要帮助的领域：

- 渲染管线优化
- 节点系统架构设计
- 文档和示例
- 用户界面设计

## 许可证

本项目采用Apache 2.0许可证（见LICENSE文件）。

## 致谢

- Rust生态系统，特别是[wgpu]和[egui]项目
- Bevy引擎的ECS系统设计灵感
- Blender社区的3D软件开发经验

---

*Alander - The Rust-based, Collaborative 3D Engine for the Web Era.*