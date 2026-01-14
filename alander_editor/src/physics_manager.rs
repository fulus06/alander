use rapier3d::prelude::*;
use rapier3d::na::{Vector3, UnitQuaternion, Isometry3, Quaternion};
use alander_core::scene::{Transform, RigidBody, Collider, RigidBodyType, ColliderShape};
use glam::{Vec3, Quat};
use bevy_ecs::prelude::*;

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
            is_running: false,
        }
    }

    /// 执行物理步进
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
        
        for (_entity, transform, mut rb, mut collider) in query.iter_mut(world) {
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
                        .build();
                    
                    let col_handle = self.collider_set.insert_with_parent(collider_obj, handle, &mut self.rigid_body_set);
                    col.handle_index = Some(col_handle.into_raw_parts().0);
                    col.handle_generation = Some(col_handle.into_raw_parts().1);
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
}
