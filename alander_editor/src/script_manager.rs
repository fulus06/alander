use rhai::{Engine, Scope};
use alander_core::scene::{Transform, Script};
use alander_core::math::Vec3;
use crate::scene_manager::Scene;

/// 脚本管理器，负责 Rhai 引擎的生命周期和绑定
pub struct ScriptManager {
    engine: Engine,
}

impl ScriptManager {
    pub fn new() -> Self {
        let mut engine = Engine::new();

        // 1. 注册基础数学类型 Vec3
        engine.register_type_with_name::<Vec3>("Vec3")
            .register_fn("vec3", |x: f32, y: f32, z: f32| Vec3::new(x, y, z))
            .register_get_set("x", |v: &mut Vec3| v.x, |v: &mut Vec3, val: f32| v.x = val)
            .register_get_set("y", |v: &mut Vec3| v.y, |v: &mut Vec3, val: f32| v.y = val)
            .register_get_set("z", |v: &mut Vec3| v.z, |v: &mut Vec3, val: f32| v.z = val);

        // 2. 注册 Transform 组件
        engine.register_type_with_name::<Transform>("Transform")
            .register_get_set("position", |t: &mut Transform| t.position, |t: &mut Transform, v: Vec3| t.position = v)
            .register_get_set("scale", |t: &mut Transform| t.scale, |t: &mut Transform, v: Vec3| t.scale = v);
        
        // 注意：旋转稍微复杂些，暂时暴露简单的 Euler
        engine.register_fn("set_rotation_euler", |t: &mut Transform, x: f32, y: f32, z: f32| {
            t.rotation = glam::Quat::from_euler(glam::EulerRot::YXZ, y.to_radians(), x.to_radians(), z.to_radians());
        });

        Self { engine }
    }

    /// 执行脚本更新
    pub fn update_scripts(&self, scene: &mut Scene, delta_time: f32) {
        let mut query = scene.world.query::<(&mut Script, &mut Transform)>();
        
        for (mut script, mut transform) in query.iter_mut(&mut scene.world) {
            if !script.active || script.code.is_empty() {
                continue;
            }

            // 编译 AST (实际应用中应缓存 AST)
            let ast = match self.engine.compile(&script.code) {
                Ok(ast) => ast,
                Err(e) => {
                    script.last_error = Some(format!("编译错误: {}", e));
                    script.active = false;
                    continue;
                }
            };

            // 创建 Scope 并注入变量
            let mut scope = Scope::new();
            scope.push("dt", delta_time);
            
            // 将 Transform 克隆进脚本环境（Rhai 无法直接操作 Rust 引用，需要这种方式）
            scope.push("transform", transform.clone());

            // 运行脚本
            if let Err(e) = self.engine.run_ast_with_scope(&mut scope, &ast) {
                script.last_error = Some(format!("运行错误: {}", e));
                script.active = false;
                continue;
            }

            // 写回修改后的 Transform
            if let Some(new_transform) = scope.get_value::<Transform>("transform") {
                *transform = new_transform;
            }
            
            script.last_error = None;
        }
    }
}
