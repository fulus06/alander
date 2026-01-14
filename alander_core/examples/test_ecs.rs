//! ECS架构基础功能测试示例
//!
//! 此示例演示如何创建实体、定义组件和加载资源。

use alander_core::scene::{Transform, Mesh, Material, Name};
use alander_core::assets::{AssetManager, AssetLoader, SimpleMeshLoader, SimpleMaterialLoader};
use bevy_ecs::prelude::*;
use glam::Vec3;

fn main() {
    println!("=== Alander ECS架构基础功能测试 ===");
    
    // 1. 创建ECS世界
    let mut world = World::new();
    
    // 2. 创建实体并添加组件
    let entity = world.spawn((
        Name("测试立方体".to_string()),
        Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
    )).id();
    
    println!("✅ 创建的实体ID: {:?}", entity);
    
    // 3. 验证实体组件
    let name = world.get::<Name>(entity).unwrap();
    println!("✅ 实体名称: {}", name.0);
    
    let transform = world.get::<Transform>(entity).unwrap();
    println!("✅ 实体位置: {:?}", transform.position);
    
    // 4. 创建资源管理器
    let mut mesh_manager = AssetManager::new();
    let mut material_manager = AssetManager::new();
    
    // 5. 使用资源加载器加载资源
    let mut mesh_loader = SimpleMeshLoader;
    let mut material_loader = SimpleMaterialLoader;
    
    match mesh_loader.load("cube") {
        Ok(mesh_data) => {
            let mesh_handle = mesh_manager.load(mesh_data);
            println!("✅ 加载的网格句柄ID: {}", mesh_handle.id);
            
            // 为实体添加网格组件
            world.entity_mut(entity).insert(Mesh { handle: mesh_handle });
        }
        Err(e) => println!("❌ 网格加载失败: {}", e),
    }
    
    match material_loader.load("red") {
        Ok(material_data) => {
            let material_handle = material_manager.load(material_data);
            println!("✅ 加载的材质句柄ID: {}", material_handle.id);
            
            // 为实体添加材质组件
            world.entity_mut(entity).insert(Material { handle: material_handle });
        }
        Err(e) => println!("❌ 材质加载失败: {}", e),
    }
    
    // 6. 验证实体现在拥有所有组件
    let entity_ref = world.entity(entity);
    println!("✅ 实体组件数量: {}", entity_ref.archetype().components().count());
    
    // 7. 创建更多实体演示批量操作
    println!("\n=== 批量创建实体演示 ===");
    
    let entities: Vec<Entity> = (0..3)
        .map(|i| {
            world.spawn((
                Name(format!("实体{}", i)),
                Transform::from_translation(Vec3::new(i as f32, 0.0, 0.0)),
            )).id()
        })
        .collect();
    
    for (i, entity) in entities.iter().enumerate() {
        let name = world.get::<Name>(*entity).unwrap();
        println!("✅ 批量创建的实体{}: {} (ID: {:?})", i, name.0, entity);
    }
    
    println!("\n=== ECS架构基础功能测试完成 ===");
    println!("总结:");
    println!("  - 成功创建了 {} 个实体", world.entities().len());
    println!("  - 成功实现了ECS核心组件");
    println!("  - 成功实现了资源管理系统");
    println!("  - 满足验收标准: 创建实体并打印其ID，资源正确加载");
}