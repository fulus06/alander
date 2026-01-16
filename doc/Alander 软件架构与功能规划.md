这是一个极具野心的目标！要做一个 **Blender 的竞品 (Alander)**，你不能仅仅是“模仿”Blender。Blender 已经开发了 30 年，拥有数百万行 C/C++ 代码。

如果要在 2026 年用 Rust 挑战 Blender，你的策略必须是：**弯道超车**。利用 Rust 的特性（并发、安全、Wasm）和现代开发理念（云协作、AI 辅助、实时渲染）来解决 Blender 的历史包袱。

以下是为你设计的 **Alander** 软件架构与功能规划：

---

### 1. 核心差异化理念 (Philosophy)

不要做“另一个 Blender”，要做 **“3D 时代的 Figma”** 或 **“面向未来的程序化 3D 引擎”**。

*   **云原生与协作 (Collaborative First):** Blender 是单机软件。Alander 应该像 Figma 或 Google Docs 一样，支持多人同时在一个场景里编辑。这是 Rust (WebAssembly + WebSocket) 的强项。
*   **非破坏性 (Non-Destructive Only):** Blender 的很多建模操作是破坏性的。Alander 应全面采用“节点式”或“堆栈式”逻辑，任何操作随时可撤销、可修改参数。
*   **ECS 架构:** 使用 Entity-Component-System 架构管理场景，意味着你可以轻松处理百万级的物体，且天然支持并行计算，性能优于 Blender 的旧式 OOP 架构。

---

### 2. 功能模块设计

#### A. 视口与渲染 (The Viewport)
**目标**: 消除“编辑模式”与“渲染结果”的差异。
*   **基于 WGPU 的混合渲染器**:
    *   **默认模式**: 实时光线追踪 (Ray Tracing) 或高质量光栅化（类似 Unreal Engine 5 的 Lumen）。
    *   **Rust 优势**: 利用 `wgpu` 的 Compute Shader 进行剔除和加速，确保在拥有数百万多边形的场景中也能跑满 60fps。
*   **无限细节 (Nanite-like)**:
    *   尝试实现基于网格着色器 (Mesh Shaders) 的虚拟几何体技术，用户导入高模不需要手动拓扑（Retopology），软件自动处理 LOD。

#### B. 建模系统 (Modeling)
**目标**: 极简交互，底层程序化。
*   **SDF (有向距离场) 建模**:
    *   除了传统的多边形建模，提供基于 SDF 的“黏土”建模。这种方式布尔运算（挖洞、融合）极其快速且不出错，非常适合概念设计。
*   **智能笔刷 (AI Brushes)**:
    *   集成 AI 模型（OnnxRuntime in Rust）。比如画一条线，AI 自动生成植被、栏杆或线缆，而不是手动摆放。

#### C. 动画系统 (Animation)
**目标**: 让动画不再是“K帧地狱”。
*   **基于物理的 puppetry (操纵)**:
    *   集成 `Rapier` (物理引擎)。允许用户像玩木偶一样拖动角色的手，身体自动根据 IK 和物理碰撞跟随，而不是纯粹调曲线。
*   **性能回放**:
    *   Blender 在播放复杂动画时 FPS 会骤降。Rust 的多线程优势可以让 Alander 在播放动画时，自动利用所有 CPU 核心计算骨骼形变。

#### D. 节点系统 (The Brain)
**目标**: 一切皆节点 (Everything Nodes)。
*   Blender 有 Geometry Nodes，但 Alander 应该**底层就是节点**。
*   材质、建模、动画逻辑、甚至 UI 布局，都通过节点图控制。
*   **可视化调试**: 当数据流过节点线时，实时显示数据的缩略图或数值（类似编程 IDE 的 Debug 模式）。

---

### 3. 技术栈建议 (Rust Crates)

这是你构建 Alander 的武器库：

*   **GUI 框架**: **`egui`** (做原型) 或 **`Xilem` / `Vello`** (高性能矢量 UI，未来感更强)。支持 Docking（停靠）和多窗口是必须的。
*   **图形后端**: **`wgpu`**。
*   **核心架构**: **`bevy_ecs`** 或 **`hecs`** (作为独立的数据层，不一定要引入整个 Bevy 引擎，保持轻量)。
*   **物理**: **`rapier3d`**。
*   **脚本语言**: **`Rhai`** 或 **`Lua (mlua)`**。虽然 Rust 是核心，但你需要给用户一个脚本接口（替代 Blender 的 Python）。`Rhai` 是纯 Rust 写的脚本语言，集成极容易。
*   **文件格式**: **`usdo`** (USD 支持) 和 **`gltf`**。
*   **网络协作**: **`matchbox_socket`** (用于 WebRTC P2P 连接，实现多人协作)。

---

### 4. 开发路线图 (MVP 阶段)

不要一开始就想做全能软件，先做一个**“极速场景搭建工具”**：

1.  **Stage 1 (基础架构)**:
    *   搭建 WGPU 渲染窗口。
    *   实现 ECS 场景树（Scene Graph）。
    *   实现摄像机漫游。
2.  **Stage 2 (网格编辑)**:
    *   加载 .obj/.gltf。
    *   实现 Gizmo（移动/旋转/缩放）。
    *   实现基础的顶点/面编辑（这是最难的，涉及大量半边结构 Half-edge 数据结构算法）。
3.  **Stage 3 (节点化)**:
    *   引入 `egui_node_graph`。
    *   实现一个简单的修改器：Array（阵列）或 Mirror（镜像），通过节点连接实现。

### 5. 商业/开源策略

*   **名称**: Alander (很好，A 开头排在 B 前面)。
*   **Slogan**: "The Rust-based, Collaborative 3D Engine for the Web Era."
*   **痛点打击**: 
    *   "Blender 打开大场景太卡？试式 Alander。"
    *   "需要把模型发给客户看？Alander 直接发个链接，他在浏览器里就能和你一起改。"

用 Rust 重写 3D 软件是一个漫长的过程，建议先从**“Blender 做的不好的一小块功能”**（例如：处理超大点云数据、或者极速的概念雕刻）做起，做成一个独立小工具，站稳脚跟后再慢慢扩张成完整的 Alander。