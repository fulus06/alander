use rapier3d::prelude::*;
use rapier3d::na::{Vector3, UnitQuaternion, Isometry3, Quaternion};
use alander_core::scene::{Transform, RigidBody, Collider, RigidBodyType, ColliderShape};
use glam::{Vec3, Quat};
use bevy_ecs::prelude::*;
use rapier3d::pipeline::{DebugRenderPipeline, DebugRenderMode, DebugRenderStyle, DebugRenderBackend, DebugRenderObject};

/// 物理管理器，封装了 Rapier3D 的世界和模拟逻辑
pub struct PhysicsManager {
    /// 刚体集合
    pub rigid_body_set: RigidBodySet,
    /// 碰撞体集合
    pub collider_set: ColliderSet,
    /// 重力向量
    pub gravity: Vector<f32>,
    /// 积分参数
    pub integration_parameters: IntegrationParameters,
    /// 物理流水线
    pub physics_pipeline: PhysicsPipeline,
    /// 岛屿管理器 (用于睡眠和活跃物体管理)
    pub island_manager: IslandManager,
    /// 宽相碰撞检测
    pub broad_phase: BroadPhase,
    /// 窄相碰撞检测
    pub narrow_phase: NarrowPhase,
    /// 冲量关节集合
    pub impulse_joint_set: ImpulseJointSet,
    /// 多体关节集合
    pub multibody_joint_set: MultibodyJointSet,
    /// 连续碰撞检测 (CCD) 解析器
    pub ccd_solver: CCDSolver,
    /// 调试渲染管线
    pub debug_pipeline: DebugRenderPipeline,
    /// 场景查询管线 (用于射线检测等)
    pub query_pipeline: QueryPipeline,
    /// 模拟是否正在运行
    pub is_running: bool,
}

impl PhysicsManager {
    /// 创建新的物理管理器
    pub fn new() -> Self {
        Self {
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            gravity: vector![0.0, -9.81, 0.0],
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: BroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            debug_pipeline: DebugRenderPipeline::new(
                DebugRenderStyle::default(),
                DebugRenderMode::all(),
            ),
            query_pipeline: QueryPipeline::new(),
            is_running: false,
        }
    }

    pub fn step(&mut self) {
        if !self.is_running {
            return;
        }

        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            None,
            &(),
            &(),
        );
    }

    /// 将 ECS 中的实体同步到物理世界
    pub fn sync_ecs_to_physics(&mut self, world: &mut World) {
        let mut query = world.query::<(Entity, &Transform, &mut RigidBody, Option<&mut Collider>)>();
        
        for (entity, transform, mut rb, mut collider) in query.iter_mut(world) {
            // 如果还没有物理句柄，则创建物理对象
            if rb.handle_index.is_none() {
                let rb_type = match rb.body_type {
                    RigidBodyType::Static => rapier3d::prelude::RigidBodyType::Fixed,
                    RigidBodyType::Dynamic => rapier3d::prelude::RigidBodyType::Dynamic,
                    RigidBodyType::KinematicVelocityBased => rapier3d::prelude::RigidBodyType::KinematicVelocityBased,
                    RigidBodyType::KinematicPositionBased => rapier3d::prelude::RigidBodyType::KinematicPositionBased,
                };

                let pos = transform.position;
                let rot = transform.rotation;
                
                let rigid_body = RigidBodyBuilder::new(rb_type)
                    .position(Isometry3::from_parts(
                        Vector3::new(pos.x, pos.y, pos.z).into(),
                        UnitQuaternion::from_quaternion(Quaternion::new(rot.w, rot.x, rot.y, rot.z))
                    ))
                    .build();
                
                let handle = self.rigid_body_set.insert(rigid_body);
                rb.handle_index = Some(handle.into_raw_parts().0);
                rb.handle_generation = Some(handle.into_raw_parts().1);

                // 如果有碰撞体组件，则创建对应的碰撞体
                if let Some(ref mut col) = collider {
                    let shape = match col.shape {
                        ColliderShape::Ball { radius } => SharedShape::ball(radius),
                        ColliderShape::Cuboid { half_extents } => {
                            SharedShape::cuboid(half_extents.x, half_extents.y, half_extents.z)
                        }
                        ColliderShape::Capsule { half_height, radius } => {
                            SharedShape::capsule_y(half_height, radius)
                        }
                    };

                    let collider_obj = ColliderBuilder::new(shape)
                        .friction(col.friction)
                        .restitution(col.restitution)
                        .user_data(entity.to_bits() as u128) // 存储 ECS Entity ID
                        .build();
                    
                    let col_handle = self.collider_set.insert_with_parent(collider_obj, handle, &mut self.rigid_body_set);
                    col.handle_index = Some(col_handle.into_raw_parts().0);
                    col.handle_generation = Some(col_handle.into_raw_parts().1);
                } else {
                    // 如果碰撞体已经存在，确保其 user_data 是最新的 (防止旧碰撞体无法拾取)
                    let col_handle = ColliderHandle::from_raw_parts(
                        collider.as_ref().unwrap().handle_index.unwrap(),
                        collider.as_ref().unwrap().handle_generation.unwrap()
                    );
                    if let Some(col_obj) = self.collider_set.get_mut(col_handle) {
                        col_obj.user_data = entity.to_bits() as u128;
                    }
                }
            } else {
                // 如果模拟没运行，允许从 Transform 同步到物理引擎（手动编辑模式）
                if !self.is_running {
                    let handle = RigidBodyHandle::from_raw_parts(rb.handle_index.unwrap(), rb.handle_generation.unwrap());
                    if let Some(body) = self.rigid_body_set.get_mut(handle) {
                        let pos = transform.position;
                        let rot = transform.rotation;
                        body.set_position(Isometry3::from_parts(
                            Vector3::new(pos.x, pos.y, pos.z).into(),
                            UnitQuaternion::from_quaternion(Quaternion::new(rot.w, rot.x, rot.y, rot.z))
                        ), true);
                    }
                }
            }
        }

        // 重要：同步完成后必须更新查询管线，否则射线检测会滞后一帧或使用旧位置
        self.query_pipeline.update(&self.rigid_body_set, &self.collider_set);
    }

    /// 将物理世界的结果同步回 ECS Transform
    pub fn sync_physics_to_ecs(&self, world: &mut World) {
        if !self.is_running {
            return;
        }

        let mut query = world.query::<(&RigidBody, &mut Transform)>();
        for (rb, mut transform) in query.iter_mut(world) {
            if let (Some(idx), Some(gen)) = (rb.handle_index, rb.handle_generation) {
                let handle = RigidBodyHandle::from_raw_parts(idx, gen);
                if let Some(body) = self.rigid_body_set.get(handle) {
                    let pos = body.translation();
                    let rot = body.rotation();
                    
                    transform.position = Vec3::new(pos.x, pos.y, pos.z);
                    transform.rotation = Quat::from_xyzw(rot.i, rot.j, rot.k, rot.w);
                }
            }
        }
    }

    /// 执行射线投射，检测鼠标选中的物体
    pub fn ray_cast(&self, ray: &alander_core::math::Ray) -> Option<Entity> {
        let rapier_ray = Ray::new(
            Point::new(ray.origin.x, ray.origin.y, ray.origin.z),
            Vector::new(ray.direction.x, ray.direction.y, ray.direction.z),
        );

        // 设置最大检测距离
        let max_toi = 1000.0;
        let solid = true;
        let filter = QueryFilter::default();

        if let Some((handle, toi)) = self.query_pipeline.cast_ray(
            &self.rigid_body_set,
            &self.collider_set,
            &rapier_ray,
            max_toi,
            solid,
            filter,
        ) {
            tracing::info!("射线命中: {:?}, 距离: {}", handle, toi);
            
            // 从 Collider 中取回 Entity ID
            if let Some(collider) = self.collider_set.get(handle) {
                let entity_bits = collider.user_data as u64;
                if entity_bits != 0 {
                    return Some(Entity::from_bits(entity_bits));
                }
            }
        }

        None
    }

    /// 提取物理世界的调试线框数据
    pub fn render_debug_lines(&mut self) -> Vec<alander_render::pipelines::DebugVertex> {
        let mut vertices = Vec::new();

        self.debug_pipeline.render(
            &mut DebugCollector { vertices: &mut vertices },
            &self.rigid_body_set,
            &self.collider_set,
            &self.impulse_joint_set,
            &self.multibody_joint_set,
            &self.narrow_phase,
        );

        vertices
    }
}

/// 内部结构，转换 Rapier3D 的调试线条到渲染器的顶点格式
struct DebugCollector<'a> {
    vertices: &'a mut Vec<alander_render::pipelines::DebugVertex>,
}

impl<'a> DebugRenderBackend for DebugCollector<'a> {
    fn draw_line(&mut self, _object: DebugRenderObject, a: Point<f32>, b: Point<f32>, color: [f32; 4]) {
        self.vertices.push(alander_render::pipelines::DebugVertex {
            position: [a.x, a.y, a.z],
            color,
        });
        self.vertices.push(alander_render::pipelines::DebugVertex {
            position: [b.x, b.y, b.z],
            color,
        });
    }
}
