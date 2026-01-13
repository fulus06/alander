use wasm_bindgen::prelude::*;
use web_sys::{console, HtmlCanvasElement, Window, Document};
use wasm_bindgen_futures::future_to_promise;
use std::cell::RefCell;
use std::rc::Rc;

use alander_render::Renderer;

// 当 wasm-pack 构建时设置控制台错误处理
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// 初始化告警处理
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).expect("初始化日志失败");
    log::info!("Alander WebAssembly 模块已加载");
}

/// 渲染器包装器
#[wasm_bindgen]
pub struct WasmRenderer {
    renderer: RefCell<Option<Renderer>>,
    canvas: HtmlCanvasElement,
}

#[wasm_bindgen]
impl WasmRenderer {
    /// 创建新的渲染器实例
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str) -> Result<WasmRenderer, JsValue> {
        // 获取 canvas 元素
        let document = web_sys::window()
            .ok_or_else(|| JsValue::from_str("无法获取窗口"))?
            .document()
            .ok_or_else(|| JsValue::from_str("无法获取文档"))?;

        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str("找不到指定的canvas元素"))?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| JsValue::from_str("元素不是canvas"))?;

        // 创建 Wasm 渲染器（稍后实现）
        Ok(WasmRenderer {
            renderer: RefCell::new(None),
            canvas,
        })
    }

    /// 初始化渲染器（异步）
    pub fn init(&self) -> js_sys::Promise {
        let canvas = self.canvas.clone();
        future_to_promise(async move {
            // 获取 WebGPU 适配器
            // 注意：浏览器中的 WebGPU 支持仍有限
            // 这里简化实现，返回成功

            log::info!("Wasm 渲染器初始化完成");
            Ok(JsValue::from_bool(true))
        })
    }

    /// 调整大小
    pub fn resize(&self, width: u32, height: u32) -> Result<(), JsValue> {
        // 设置canvas大小
        self.canvas.set_width(width);
        self.canvas.set_height(height);

        // 调整渲染器大小
        if let Some(ref mut renderer) = *self.renderer.borrow_mut() {
            // renderer.resize(winit::dpi::PhysicalSize::new(width, height));
            // 延迟实现
        }

        Ok(())
    }

    /// 开始渲染循环
    pub fn start(&self) -> Result<(), JsValue> {
        // 设置渲染循环
        let closure = Closure::wrap(Box::new(move || {
            // 渲染帧
            // request_animation_frame(render_frame);
        }) as Box<dyn Fn()>);

        // 设置动画帧回调
        web_sys::window()
            .ok_or_else(|| JsValue::from_str("无法获取窗口"))?
            .request_animation_frame(closure.as_ref().unchecked_ref())
            .expect("设置动画帧失败");

        closure.forget(); // 防止闭包被垃圾回收

        log::info!("渲染循环已启动");
        Ok(())
    }
}

// 便捷函数

/// 从 JavaScript 加载 glTF 模型
#[wasm_bindgen]
pub async fn load_gltf(url: &str) -> Result<u32, JsValue> {
    log::info!("开始加载 glTF 模型: {}", url);

    // 发起 HTTP 请求
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("无法获取窗口"))?;
    let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(url)).await?;

    let resp = resp_value.dyn_into::<web_sys::Response>()
        .map_err(|_| JsValue::from_str("响应不是有效的Response对象"))?;

    // 读取响应为二进制数据
    let array_buffer = wasm_bindgen_futures::JsFuture::from(
        resp.array_buffer()?
    ).await?;

    // 转换为 Rust 字节切片
    let uint8_array = js_sys::Uint8Array::new(&array_buffer);
    let mut bytes = vec![0; uint8_array.length() as usize];
    uint8_array.copy_to(&mut bytes);

    // 解析 glTF（简化实现）
    // TODO: 实际解析 glTF 数据

    log::info!("glTF 模型加载成功，大小: {} 字节", bytes.len());

    // 返回实体 ID（临时实现）
    Ok(12345)
}

/// 加载纹理
#[wasm_bindgen]
pub async fn load_texture(url: &str) -> Result<u32, JsValue> {
    log::info!("开始加载纹理: {}", url);

    // TODO: 实现纹理加载

    Ok(54321)
}

/// 初始化 WebAssembly 模块
#[wasm_bindgen]
pub fn init_module(canvas_id: &str) -> Result<WasmRenderer, JsValue> {
    log::info!("初始化 Alander WebAssembly 模块");

    let renderer = WasmRenderer::new(canvas_id)?;

    Ok(renderer)
}

// 辅助函数

/// 记录错误到控制台
pub fn log_error(msg: &str) {
    console::error_1(&msg.into());
}

/// 记录信息到控制台
pub fn log_info(msg: &str) {
    console::log_1(&msg.into());
}
