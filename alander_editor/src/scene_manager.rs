//! 场景管理器
//!
//! 此模块负责管理ECS世界、场景和实体。

use alander_core::scene::{Transform, Mesh, Name, RenderId, BoundingBox, PBRMaterial, PointLight, RigidBody, Collider, RigidBodyType, AssetPath, EntityUuid, Parent, Children, GlobalTransform, Camera, Material};
use serde::{Serialize, Deserialize};
use alander_core::math::AABB;
use alander_render::renderer::{Renderer, create_cube};
use alander_render::pipelines::{SceneObject, Vertex, MaterialBuffer};
use alander_core::assets::{AssetManager, AssetLoader, SimpleMeshLoader, SimpleMaterialLoader};
use bevy_ecs::prelude::*;
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
        let entity = self.world.spawn(components).id();
        // 自动分配 UUID 和初始全局变换
        self.world.entity_mut(entity).insert((
            EntityUuid(Uuid::new_v4()),
            GlobalTransform::default(),
        ));
        entity
    }
    
    /// 复制实体及其所有核心组件
    pub fn duplicate_entity(&mut self, entity: Entity) -> Option<Entity> {
        self.duplicate_entity_recursive(entity, None)
    }

    /// 递归复制实体及其子树
    fn duplicate_entity_recursive(&mut self, entity: Entity, new_parent: Option<Entity>) -> Option<Entity> {
        // 1. 获取原实体的核心组件副本
        let name = self.world.get::<Name>(entity).cloned();
        let transform = self.world.get::<Transform>(entity).cloned();
        let mesh = self.world.get::<Mesh>(entity).cloned();
        let material = self.world.get::<PBRMaterial>(entity).cloned();
        let light = self.world.get::<PointLight>(entity).cloned();
        let camera = self.world.get::<Camera>(entity).cloned();
        let render_id = self.world.get::<RenderId>(entity).cloned();
        let asset_path = self.world.get::<AssetPath>(entity).cloned();
        let rigid_body = self.world.get::<RigidBody>(entity).cloned();
        let collider = self.world.get::<Collider>(entity).cloned();

        // 2. 创建新实体并应用组件
        let mut builder = self.world.spawn_empty();
        if let Some(n) = name { builder.insert(Name(format!("{} (Copy)", n.0))); }
        if let Some(t) = transform { builder.insert(t); }
        if let Some(m) = mesh { builder.insert(m); }
        if let Some(mat) = material { builder.insert(mat); }
        if let Some(l) = light { builder.insert(l); }
        if let Some(c) = camera { builder.insert(c); }
        if let Some(rid) = render_id { builder.insert(rid); }
        if let Some(ap) = asset_path { builder.insert(ap); }
        if let Some(rb) = rigid_body { builder.insert(rb); }
        if let Some(col) = collider { builder.insert(col); }

        let new_entity = builder.id();

        // 自动分配新的 UUID
        self.world.entity_mut(new_entity).insert((
            EntityUuid(Uuid::new_v4()),
            GlobalTransform::default(),
        ));

        // 3. 处理父子关系
        if let Some(p) = new_parent {
            self.set_parent(new_entity, Some(p));
        }

        // 4. 递归处理子节点
        let children = self.world.get::<Children>(entity).map(|c| c.0.clone());
        if let Some(child_list) = children {
            for child in child_list {
                self.duplicate_entity_recursive(child, Some(new_entity));
            }
        }

        Some(new_entity)
    }
    
    /// 删除实体 (连带删除子节点)
    pub fn remove_entity(&mut self, entity: Entity) -> bool {
        // 先收集所有子节点
        let mut to_remove = vec![entity];
        let mut idx = 0;
        while idx < to_remove.len() {
            let curr = to_remove[idx];
            if let Some(children) = self.world.get::<Children>(curr) {
                for &child in &children.0 {
                    to_remove.push(child);
                }
            }
            idx += 1;
        }

        // 从父节点中移除
        if let Some(parent_comp) = self.world.get::<Parent>(entity) {
            let parent = parent_comp.0;
            if let Some(mut children) = self.world.get_mut::<Children>(parent) {
                children.0.retain(|&c| c != entity);
            }
        }

        for e in to_remove {
            self.world.despawn(e);
        }
        true
    }
    
    /// 设置父子关系，并保持世界坐标不变
    pub fn set_parent(&mut self, child: Entity, parent: Option<Entity>) {
        if Some(child) == parent { return; }
        
        let current_parent = self.world.get::<Parent>(child).map(|p| p.0);
        if current_parent == parent { return; }

        let child_global = self.world.get::<GlobalTransform>(child)
            .map(|gt| gt.0)
            .unwrap_or_else(|| {
                self.world.get::<Transform>(child)
                    .map(|t| t.compute_matrix())
                    .unwrap_or(glam::Mat4::IDENTITY)
            });

        if let Some(old_parent_comp) = self.world.get::<Parent>(child) {
            let old_parent = old_parent_comp.0;
            if let Some(mut children) = self.world.get_mut::<Children>(old_parent) {
                children.0.retain(|&c| c != child);
            }
        }

        if let Some(p) = parent {
            self.world.entity_mut(child).insert(Parent(p));
            if let Some(mut children) = self.world.get_mut::<Children>(p) {
                if !children.0.contains(&child) {
                    children.0.push(child);
                }
            } else {
                self.world.entity_mut(p).insert(Children(vec![child]));
            }

            let parent_global_inv = self.world.get::<GlobalTransform>(p)
                .map(|gt| gt.0.inverse())
                .unwrap_or(glam::Mat4::IDENTITY);
            
            let new_local_matrix = parent_global_inv * child_global;
            let new_transform = Transform::from_matrix(new_local_matrix);
            self.world.entity_mut(child).insert(new_transform);
        } else {
            self.world.entity_mut(child).remove::<Parent>();
            let new_transform = Transform::from_matrix(child_global);
            self.world.entity_mut(child).insert(new_transform);
        }
    }

    /// 递归更新层级变换
    pub fn update_hierarchy(&mut self) {
        let mut roots = Vec::new();
        {
            let mut query = self.world.query_filtered::<Entity, Without<Parent>>();
            for entity in query.iter(&self.world) {
                roots.push(entity);
            }
        }
        for root in roots {
            self.update_node_transform(root, glam::Mat4::IDENTITY);
        }
    }

    fn update_node_transform(&mut self, entity: Entity, parent_global: glam::Mat4) {
        let local_matrix = if let Some(transform) = self.world.get::<Transform>(entity) {
            transform.compute_matrix()
        } else {
            glam::Mat4::IDENTITY
        };
        let global_matrix = parent_global * local_matrix;
        if let Some(mut global) = self.world.get_mut::<GlobalTransform>(entity) {
            global.0 = global_matrix;
        } else {
            self.world.entity_mut(entity).insert(GlobalTransform(global_matrix));
        }
        let children = self.world.get::<Children>(entity).map(|c| c.0.clone());
        if let Some(child_list) = children {
            for child in child_list {
                self.update_node_transform(child, global_matrix);
            }
        }
    }

    pub fn load_mesh(&mut self, renderer: &mut Renderer, source: &str) -> Result<(alander_core::assets::Handle<alander_core::scene::MeshData>, RenderId, BoundingBox, AssetPath), String> {
        let mut loader = SimpleMeshLoader;
        match loader.load(source) {
            Ok(mesh_data) => {
                let handle = self.mesh_manager.load(mesh_data.clone());
                let mesh_data_clone = mesh_data.clone();
                let render_vertices: Vec<Vertex> = mesh_data_clone.vertices.iter().map(|v| {
                    Vertex {
                        position: v.position.to_array(),
                        normal: v.normal.to_array(),
                        uv: v.uv.to_array(),
                        tangent: v.tangent.to_array(),
                    }
                }).collect();
                let scene_object = SceneObject::new(
                    renderer.device(),
                    &render_vertices,
                    &mesh_data_clone.indices,
                    &renderer.pipelines().mesh.model_bind_group_layout,
                    &renderer.pipelines().mesh.texture_bind_group_layout,
                    &renderer.pipelines().mesh.material_bind_group_layout,
                    renderer.default_texture(),
                    renderer.default_texture(),
                    renderer.default_texture(),
                    glam::Mat4::IDENTITY,
                    MaterialBuffer::default(),
                    &renderer.resources.samplers.linear_clamp,
                );
                let render_uuid = uuid::Uuid::new_v4();
                renderer.add_object(render_uuid, scene_object);
                let bbox = BoundingBox {
                    local: AABB::new(glam::Vec3::splat(-0.5), glam::Vec3::splat(0.5)),
                    world: AABB::new(glam::Vec3::splat(-0.5), glam::Vec3::splat(0.5)),
                };
                let asset_path = AssetPath { path: source.to_string(), sub_asset: None };
                Ok((handle, RenderId(render_uuid), bbox, asset_path))
            },
            Err(e) => Err(format!("网格加载失败: {}", e)),
        }
    }
    
    pub fn get_entities_with_names(&self) -> Vec<(Entity, String)> {
        let mut entities = Vec::new();
        for entity in self.world.iter_entities() {
            let entity_id = entity.id();
            if self.world.get::<Parent>(entity_id).is_none() {
                let name = self.world.get::<Name>(entity_id).map(|n| n.0.clone()).unwrap_or_else(|| format!("未命名实体 {:?}", entity_id));
                entities.push((entity_id, name));
            }
        }
        entities
    }
    
    pub fn get_entity_transform(&self, entity: Entity) -> Option<Transform> {
        self.world.get::<Transform>(entity).cloned()
    }
    
    pub fn update_entity_transform(&mut self, entity: Entity, transform: Transform) -> bool {
        if let Some(mut entity_mut) = self.world.get_entity_mut(entity) {
            entity_mut.insert(transform);
            true
        } else {
            false
        }
    }

    pub fn serialize_entity_subtree(&self, entity: Entity) -> Vec<EntityData> {
        let mut entities_data = Vec::new();
        let mut to_process = vec![entity];
        let mut idx = 0;
        while idx < to_process.len() {
            let curr = to_process[idx];
            idx += 1;
            if let Some(uuid_comp) = self.world.get::<EntityUuid>(curr) {
                let uuid = uuid_comp.0;
                let name = self.world.get::<Name>(curr).map(|n| n.0.clone()).unwrap_or_default();
                let transform = self.world.get::<Transform>(curr).cloned();
                let pbr_material = self.world.get::<PBRMaterial>(curr).cloned();
                let point_light = self.world.get::<PointLight>(curr).cloned();
                let rigid_body = self.world.get::<RigidBody>(curr).cloned();
                let collider = self.world.get::<Collider>(curr).cloned();
                let asset_path = self.world.get::<AssetPath>(curr).cloned();
                let parent_uuid = if let Some(parent_comp) = self.world.get::<Parent>(curr) {
                    self.world.get::<EntityUuid>(parent_comp.0).map(|id| id.0)
                } else {
                    None
                };
                entities_data.push(EntityData { name, uuid, transform, pbr_material, point_light, rigid_body, collider, asset_path, parent_uuid });
                if let Some(children) = self.world.get::<Children>(curr) {
                    for &child in &children.0 { to_process.push(child); }
                }
            }
        }
        entities_data
    }

    pub fn spawn_entity_subtree(&mut self, entities_data: Vec<EntityData>, renderer: &mut Renderer) -> Vec<Entity> {
        let mut uuid_to_entity = HashMap::new();
        let mut created_entities = Vec::new();
        let mut gltf_cache: HashMap<String, (alander_core::assets::GltfModel, HashMap<usize, usize>)> = HashMap::new();

        for data in &entities_data {
            let mut builder = self.world.spawn_empty();
            builder.insert((Name(data.name.clone()), EntityUuid(data.uuid), GlobalTransform::default()));
            if let Some(ref t) = data.transform { builder.insert(*t); }
            if let Some(ref rb) = data.rigid_body { builder.insert(rb.clone()); }
            if let Some(ref col) = data.collider { builder.insert(col.clone()); }
            if let Some(ref light) = data.point_light { builder.insert(light.clone()); }
            if let Some(ref mat) = data.pbr_material { builder.insert(mat.clone()); }
            if let Some(ref asset_path) = data.asset_path {
                builder.insert(asset_path.clone());
                if asset_path.path.ends_with(".glb") || asset_path.path.ends_with(".gltf") {
                    if !gltf_cache.contains_key(&asset_path.path) {
                        let loader = alander_core::assets::GltfLoader;
                        if let Ok(m) = loader.load_scene(&asset_path.path) {
                            let t_map = renderer.load_gltf_textures(&m);
                            gltf_cache.insert(asset_path.path.clone(), (m, t_map));
                        }
                    }
                    if let Some((model, texture_map)) = gltf_cache.get(&asset_path.path) {
                        let sub_name = asset_path.sub_asset.as_deref().unwrap_or("");
                        if let Some(gltf_mesh) = model.meshes.iter().find(|m| m.data.name == sub_name || sub_name.is_empty()) {
                            let diffuse_texture = renderer.resources.get_texture_from_index(model, gltf_mesh, texture_map, 0);
                            let normal_texture = renderer.resources.get_texture_from_index(model, gltf_mesh, texture_map, 1);
                            let mr_texture = renderer.resources.get_texture_from_index(model, gltf_mesh, texture_map, 2);
                            let mut material_buffer = MaterialBuffer::default();
                            if normal_texture as *const _ != renderer.default_texture() as *const _ { material_buffer.has_normal_texture = 1; }
                            if mr_texture as *const _ != renderer.default_texture() as *const _ { material_buffer.has_metallic_roughness_texture = 1; }
                            let render_vertices: Vec<Vertex> = gltf_mesh.data.vertices.iter().map(|v| {
                                Vertex { position: v.position.to_array(), normal: v.normal.to_array(), uv: v.uv.to_array(), tangent: v.tangent.to_array() }
                            }).collect();
                            let scene_object = SceneObject::new(
                                renderer.device(), &render_vertices, &gltf_mesh.data.indices,
                                &renderer.pipelines().mesh.model_bind_group_layout, &renderer.pipelines().mesh.texture_bind_group_layout,
                                &renderer.pipelines().mesh.material_bind_group_layout,
                                diffuse_texture, normal_texture, mr_texture, glam::Mat4::IDENTITY, material_buffer, &renderer.resources.samplers.linear_clamp,
                            );
                            let render_uuid = Uuid::new_v4();
                            renderer.add_object(render_uuid, scene_object);
                            builder.insert(RenderId(render_uuid));
                        }
                    }
                }
            }
            let entity = builder.id();
            uuid_to_entity.insert(data.uuid, entity);
            created_entities.push(entity);
        }
        for data in &entities_data {
            if let (Some(&child), Some(parent_uuid)) = (uuid_to_entity.get(&data.uuid), data.parent_uuid) {
                if let Some(&parent) = uuid_to_entity.get(&parent_uuid) {
                    self.set_parent(child, Some(parent));
                }
            }
        }
        created_entities
    }

    pub fn to_json(&mut self) -> Result<String, String> {
        let mut entities_data = Vec::new();
        let mut query = self.world.query_filtered::<Entity, Without<Parent>>();
        for entity in query.iter(&self.world) {
            entities_data.extend(self.serialize_entity_subtree(entity));
        }
        let scene_data = SceneData { name: self.name.clone(), entities: entities_data };
        serde_json::to_string_pretty(&scene_data).map_err(|e| e.to_string())
    }

    pub fn from_json(json: &str, renderer: &mut Renderer) -> Result<Self, String> {
        let scene_data: SceneData = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let mut scene = Scene::new(&scene_data.name);
        scene.spawn_entity_subtree(scene_data.entities, renderer);
        scene.update_hierarchy();
        Ok(scene)
    }

    pub fn entity_count(&self) -> usize { self.world.entities().len() as usize }
}

#[derive(Serialize, Deserialize)]
pub struct SceneData {
    pub name: String,
    pub entities: Vec<EntityData>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct EntityData {
    pub name: String,
    pub uuid: Uuid,
    pub transform: Option<Transform>,
    pub pbr_material: Option<PBRMaterial>,
    pub point_light: Option<PointLight>,
    pub rigid_body: Option<RigidBody>,
    pub collider: Option<Collider>,
    pub asset_path: Option<AssetPath>,
    pub parent_uuid: Option<Uuid>,
}

pub struct SceneManager {
    scenes: HashMap<SceneHandle, Scene>,
    active_scene: Option<SceneHandle>,
}

impl SceneManager {
    pub fn new() -> Self {
        Self { scenes: HashMap::new(), active_scene: None }
    }

    pub fn add_scene(&mut self, scene: Scene) -> SceneHandle {
        let handle = scene.handle;
        self.scenes.insert(handle, scene);
        if self.active_scene.is_none() { self.active_scene = Some(handle); }
        handle
    }

    pub fn create_scene(&mut self, name: &str) -> SceneHandle {
        self.add_scene(Scene::new(name))
    }

    pub fn create_scene_from_object(&mut self, scene: Scene) -> SceneHandle {
        let handle = scene.handle;
        self.scenes.insert(handle, scene);
        self.active_scene = Some(handle);
        handle
    }

    pub fn active_scene(&self) -> Option<&Scene> { self.active_scene.and_then(|h| self.scenes.get(&h)) }
    pub fn active_scene_mut(&mut self) -> Option<&mut Scene> { self.active_scene.and_then(|h| self.scenes.get_mut(&h)) }
    pub fn set_active_scene(&mut self, handle: SceneHandle) -> bool {
        if self.scenes.contains_key(&handle) { self.active_scene = Some(handle); true } else { false }
    }
    pub fn get_scenes(&self) -> Vec<(&SceneHandle, &str)> { self.scenes.iter().map(|(h, s)| (h, s.name.as_str())).collect() }
    pub fn remove_scene(&mut self, handle: SceneHandle) -> bool {
        if Some(handle) == self.active_scene { self.active_scene = None; }
        self.scenes.remove(&handle).is_some()
    }

    pub fn create_test_scene(&mut self, renderer: &mut Renderer) -> SceneHandle {
        let handle = self.create_scene("测试场景");
        if let Some(scene) = self.active_scene_mut() {
            // 1. 创建地面
            let ground_object = create_cube(
                renderer.device(), &renderer.pipelines().mesh.model_bind_group_layout, &renderer.pipelines().mesh.texture_bind_group_layout,
                &renderer.pipelines().mesh.material_bind_group_layout, renderer.default_texture(), &renderer.resources.samplers.linear_clamp,
            );
            let ground_uuid = uuid::Uuid::new_v4();
            renderer.add_object(ground_uuid, ground_object);
            scene.create_entity((
                Name("地面".to_string()),
                Transform { position: glam::Vec3::new(0.0, -1.0, 0.0), rotation: glam::Quat::IDENTITY, scale: glam::Vec3::new(10.0, 0.1, 10.0) },
                RenderId(ground_uuid),
                BoundingBox { local: AABB::new(glam::Vec3::splat(-0.5), glam::Vec3::splat(0.5)), world: AABB::new(glam::Vec3::splat(-0.5), glam::Vec3::splat(0.5)) },
                PBRMaterial { base_color: glam::Vec4::new(0.5, 0.5, 0.5, 1.0), metallic: 0.1, roughness: 0.8, emissive: glam::Vec3::ZERO },
                RigidBody::new(RigidBodyType::Static),
                Collider::cuboid(0.5, 0.5, 0.5), // 使用单位大小，缩放由 Transform 控制
            ));

            // 2. 创建主光源
            scene.create_entity((
                Name("主光源".to_string()),
                Transform::from_translation(glam::Vec3::new(4.0, 5.0, 4.0)),
                PointLight {
                    color: glam::Vec3::new(1.0, 1.0, 1.0),
                    intensity: 100.0,
                    range: 20.0,
                },
            ));

            // 3. 创建相机实体
            scene.create_entity((
                Name("主相机".to_string()),
                Transform::from_translation(glam::Vec3::new(0.0, 2.0, 5.0)),
                Camera::perspective(45.0f32.to_radians(), 16.0 / 9.0, 0.1, 1000.0),
            ));

            // 4. 加载并创建两个立方体
            if let Ok((mesh_handle, render_id, bbox, asset_path)) = scene.load_mesh(renderer, "cube") {
                // 立方体 A
                scene.create_entity((
                    Name("立方体 A".to_string()),
                    Transform::from_translation(glam::Vec3::new(-1.5, 2.5, 0.0)),
                    Mesh { handle: mesh_handle.clone() },
                    render_id,
                    bbox.clone(),
                    asset_path.clone(),
                    PBRMaterial {
                        base_color: glam::Vec4::new(1.0, 0.3, 0.3, 1.0), // 红色
                        metallic: 0.8,
                        roughness: 0.2,
                        emissive: glam::Vec3::ZERO,
                    },
                    RigidBody::new(RigidBodyType::Dynamic),
                    Collider::cuboid(0.5, 0.5, 0.5),
                ));

                // 立方体 B
                if let Ok((mesh_handle_b, render_id_b, bbox_b, asset_path_b)) = scene.load_mesh(renderer, "cube") {
                    scene.create_entity((
                        Name("立方体 B".to_string()),
                        Transform::from_translation(glam::Vec3::new(1.5, 5.0, 0.0)),
                        Mesh { handle: mesh_handle_b },
                        render_id_b,
                        bbox_b,
                        asset_path_b,
                        PBRMaterial {
                            base_color: glam::Vec4::new(0.3, 1.0, 0.3, 1.0), // 绿色
                            metallic: 0.1,
                            roughness: 0.9,
                            emissive: glam::Vec3::ZERO,
                        },
                        RigidBody::new(RigidBodyType::Dynamic),
                        Collider::cuboid(0.5, 0.5, 0.5),
                    ));
                }
            }
        }
        handle
    }
}