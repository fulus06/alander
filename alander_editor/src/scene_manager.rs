//! 场景管理器
//!
//! 此模块负责管理ECS世界、场景和实体。

use alander_core::scene::{Transform, Mesh, Material, Name, RenderId};
use alander_render::renderer::{Renderer, create_cube};
use alander_core::assets::{AssetManager, AssetLoader, SimpleMeshLoader, SimpleMaterialLoader};
use bevy_ecs::prelude::*;
use glam::Vec3;
use std::collections::HashMap;
use uuid::Uuid;

/// 场景句柄
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneHandle(Uuid);

impl SceneHandle {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// 场景数据
pub struct Scene {
    pub handle: SceneHandle,
    pub name: String,
    pub world: World,
    pub mesh_manager: AssetManager<alander_core::scene::MeshData>,
    pub material_manager: AssetManager<alander_core::scene::MaterialData>,
}

impl Scene {
    pub fn new(name: &str) -> Self {
        let world = World::new();
        
        Self {
            handle: SceneHandle::new(),
            name: name.to_string(),
            world,
            mesh_manager: AssetManager::new(),
            material_manager: AssetManager::new(),
        }
    }
    
    /// 创建新实体
    pub fn create_entity(&mut self, components: impl Bundle) -> Entity {
        self.world.spawn(components).id()
    }
    
    /// 删除实体
    pub fn remove_entity(&mut self, entity: Entity) -> bool {
        self.world.despawn(entity)
    }
    
    /// 获取实体数量
    pub fn entity_count(&self) -> usize {
        self.world.entities().len() as usize
    }
    
    /// 加载网格资源并添加到渲染器
    pub fn load_mesh(&mut self, renderer: &mut Renderer, source: &str) -> Result<(alander_core::assets::Handle<alander_core::scene::MeshData>, RenderId), String> {
        let mut loader = SimpleMeshLoader;
        match loader.load(source) {
            Ok(mesh_data) => {
                let handle = self.mesh_manager.load(mesh_data.clone());
                
                // 将网格直接添加到渲染器
                let scene_object = create_cube(
                    renderer.device(),
                    &renderer.pipelines().mesh.model_bind_group_layout,
                    &renderer.pipelines().mesh.texture_bind_group_layout,
                    renderer.default_texture(),
                );
                let render_uuid = uuid::Uuid::new_v4();
                renderer.add_object(render_uuid, scene_object);
                
                Ok((handle, RenderId(render_uuid)))
            },
            Err(e) => Err(format!("网格加载失败: {}", e)),
        }
    }
    
    /// 加载材质资源
    pub fn load_material(&mut self, source: &str) -> Result<alander_core::assets::Handle<alander_core::scene::MaterialData>, String> {
        let mut loader = SimpleMaterialLoader;
        match loader.load(source) {
            Ok(material_data) => Ok(self.material_manager.load(material_data)),
            Err(e) => Err(format!("材质加载失败: {}", e)),
        }
    }
    
    /// 获取所有实体及其名称
    pub fn get_entities_with_names(&self) -> Vec<(Entity, String)> {
        let mut entities = Vec::new();
        
        for entity in self.world.iter_entities() {
            let entity_id = entity.id();
            if let Some(name) = self.world.get::<Name>(entity_id) {
                entities.push((entity_id, name.0.clone()));
            } else {
                entities.push((entity_id, format!("未命名实体 {:?}", entity_id)));
            }
        }
        
        entities
    }
    
    /// 获取实体的变换组件
    pub fn get_entity_transform(&self, entity: Entity) -> Option<Transform> {
        self.world.get::<Transform>(entity).cloned()
    }
    
    /// 更新实体的变换组件
    pub fn update_entity_transform(&mut self, entity: Entity, transform: Transform) -> bool {
        if let Some(mut entity_mut) = self.world.get_entity_mut(entity) {
            entity_mut.insert(transform);
            true
        } else {
            false
        }
    }
}

/// 场景管理器
pub struct SceneManager {
    scenes: HashMap<SceneHandle, Scene>,
    active_scene: Option<SceneHandle>,
}

impl SceneManager {
    pub fn new() -> Self {
        Self {
            scenes: HashMap::new(),
            active_scene: None,
        }
    }
    
    /// 创建新场景
    pub fn create_scene(&mut self, name: &str) -> SceneHandle {
        let scene = Scene::new(name);
        let handle = scene.handle;
        self.scenes.insert(handle, scene);
        
        // 如果没有激活场景，设置此场景为激活场景
        if self.active_scene.is_none() {
            self.active_scene = Some(handle);
        }
        
        handle
    }
    
    /// 获取激活场景
    pub fn active_scene(&self) -> Option<&Scene> {
        self.active_scene.and_then(|handle| self.scenes.get(&handle))
    }
    
    /// 获取激活场景的可变引用
    pub fn active_scene_mut(&mut self) -> Option<&mut Scene> {
        self.active_scene.and_then(|handle| self.scenes.get_mut(&handle))
    }
    
    /// 设置激活场景
    pub fn set_active_scene(&mut self, handle: SceneHandle) -> bool {
        if self.scenes.contains_key(&handle) {
            self.active_scene = Some(handle);
            true
        } else {
            false
        }
    }
    
    /// 获取所有场景
    pub fn get_scenes(&self) -> Vec<(&SceneHandle, &str)> {
        self.scenes.iter()
            .map(|(handle, scene)| (handle, scene.name.as_str()))
            .collect()
    }
    
    /// 删除场景
    pub fn remove_scene(&mut self, handle: SceneHandle) -> bool {
        if Some(handle) == self.active_scene {
            self.active_scene = None;
        }
        self.scenes.remove(&handle).is_some()
    }
    
    /// 创建测试场景
    pub fn create_test_scene(&mut self, renderer: &mut Renderer) -> SceneHandle {
        let handle = self.create_scene("测试场景");
        
        if let Some(scene) = self.active_scene_mut() {
            // 创建地面实体
            let ground_object = create_cube(
                renderer.device(),
                &renderer.pipelines().mesh.model_bind_group_layout,
                &renderer.pipelines().mesh.texture_bind_group_layout,
                renderer.default_texture(),
            );
            let ground_uuid = uuid::Uuid::new_v4();
            renderer.add_object(ground_uuid, ground_object);

            scene.create_entity((
                Name("地面".to_string()),
                Transform {
                    position: glam::Vec3::new(0.0, -1.0, 0.0),
                    rotation: glam::Quat::IDENTITY,
                    scale: glam::Vec3::new(10.0, 0.1, 10.0), // 扁平的地面
                },
                RenderId(ground_uuid),
            ));
            
            // 创建立方体实体
            if let Ok((mesh_handle, render_id)) = scene.load_mesh(renderer, "cube") {
                scene.create_entity((
                    Name("立方体".to_string()),
                    Transform::from_translation(glam::Vec3::new(0.0, 0.5, 0.0)),
                    Mesh { handle: mesh_handle },
                    render_id,
                ));
            }
            
            // 创建更多测试实体
            for i in 0..3 {
                let cube_object = create_cube(
                    renderer.device(),
                    &renderer.pipelines().mesh.model_bind_group_layout,
                    &renderer.pipelines().mesh.texture_bind_group_layout,
                    renderer.default_texture(),
                );
                let cube_uuid = uuid::Uuid::new_v4();
                renderer.add_object(cube_uuid, cube_object);

                scene.create_entity((
                    Name(format!("测试实体{}", i)),
                    Transform::from_translation(glam::Vec3::new((i as f32) * 2.0 + 3.0, 0.5, 0.0)),
                    RenderId(cube_uuid),
                ));
            }
        }
        
        handle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_scene_creation() {
        let mut manager = SceneManager::new();
        let handle = manager.create_scene("测试场景");
        
        assert!(manager.active_scene().is_some());
        assert_eq!(manager.active_scene().unwrap().name, "测试场景");
        
        let scene = manager.active_scene().unwrap();
        assert_eq!(scene.entity_count(), 0);
    }
    
    #[test]
    fn test_entity_creation() {
        let mut manager = SceneManager::new();
        manager.create_scene("测试场景");
        
        if let Some(scene) = manager.active_scene_mut() {
            let entity = scene.create_entity((
                Name("测试实体".to_string()),
                Transform::default(),
            ));
            
            assert_eq!(scene.entity_count(), 1);
            
            let name = scene.world.get::<Name>(entity).unwrap();
            assert_eq!(name.0, "测试实体");
        }
    }
    
    #[test]
    fn test_test_scene() {
        let mut manager = SceneManager::new();
        manager.create_test_scene();
        
        let scene = manager.active_scene().unwrap();
        assert!(scene.entity_count() >= 4); // 地面 + 立方体 + 3个测试实体
        
        let entities = scene.get_entities_with_names();
        assert!(entities.iter().any(|(_, name)| name == "立方体"));
        assert!(entities.iter().any(|(_, name)| name == "地面"));
    }
}