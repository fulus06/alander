use bevy_ecs::prelude::*;
use crate::scene_manager::Scene;
use alander_render::renderer::Renderer;

/// 编辑器命令接口
pub trait EditorCommand: Send + Sync {
    /// 执行/重做命令
    fn execute(&mut self, scene: &mut Scene, renderer: &mut Renderer);
    /// 撤销命令
    fn undo(&mut self, scene: &mut Scene, renderer: &mut Renderer);
    /// 命令的显示名称 (用于 UI)
    fn name(&self) -> &str;
}

/// 命令管理器，管理撤销与重做栈
pub struct CommandManager {
    /// 撤销栈
    undo_stack: Vec<Box<dyn EditorCommand>>,
    /// 重做栈
    redo_stack: Vec<Box<dyn EditorCommand>>,
    /// 最大撤销深度
    max_depth: usize,
}

impl CommandManager {
    pub fn new(max_depth: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_depth,
        }
    }

    /// 执行新命令并存入撤销栈
    pub fn execute(&mut self, mut command: Box<dyn EditorCommand>, scene: &mut Scene, renderer: &mut Renderer) {
        command.execute(scene, renderer);
        self.undo_stack.push(command);
        self.redo_stack.clear(); // 执行新操作后清空重做栈

        if self.undo_stack.len() > self.max_depth {
            self.undo_stack.remove(0);
        }
    }

    /// 撤销
    pub fn undo(&mut self, scene: &mut Scene, renderer: &mut Renderer) {
        if let Some(mut command) = self.undo_stack.pop() {
            command.undo(scene, renderer);
            self.redo_stack.push(command);
        }
    }

    /// 重做
    pub fn redo(&mut self, scene: &mut Scene, renderer: &mut Renderer) {
        if let Some(mut command) = self.redo_stack.pop() {
            command.execute(scene, renderer);
            self.undo_stack.push(command);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn last_undo_name(&self) -> Option<&str> {
        self.undo_stack.last().map(|c| c.name())
    }

    pub fn last_redo_name(&self) -> Option<&str> {
        self.redo_stack.last().map(|c| c.name())
    }
}

/// 变换命令: 记录实体从旧变换到新变换的变更
pub struct TransformCommand {
    entity: Entity,
    old_transform: alander_core::scene::Transform,
    new_transform: alander_core::scene::Transform,
}

impl TransformCommand {
    pub fn new(entity: Entity, old_transform: alander_core::scene::Transform, new_transform: alander_core::scene::Transform) -> Self {
        Self { entity, old_transform, new_transform }
    }
}

impl EditorCommand for TransformCommand {
    fn execute(&mut self, scene: &mut Scene, _renderer: &mut Renderer) {
        scene.update_entity_transform(self.entity, self.new_transform);
    }

    fn undo(&mut self, scene: &mut Scene, _renderer: &mut Renderer) {
        scene.update_entity_transform(self.entity, self.old_transform);
    }

    fn name(&self) -> &str { "修改变换" }
}

/// 层级变更命令: 记录实体的父节点变更
pub struct ReparentCommand {
    entity: Entity,
    old_parent: Option<Entity>,
    new_parent: Option<Entity>,
}

impl ReparentCommand {
    pub fn new(entity: Entity, old_parent: Option<Entity>, new_parent: Option<Entity>) -> Self {
        Self { entity, old_parent, new_parent }
    }
}

impl EditorCommand for ReparentCommand {
    fn execute(&mut self, scene: &mut Scene, _renderer: &mut Renderer) {
        scene.set_parent(self.entity, self.new_parent);
    }

    fn undo(&mut self, scene: &mut Scene, _renderer: &mut Renderer) {
        scene.set_parent(self.entity, self.old_parent);
    }

    fn name(&self) -> &str { "更改父节点" }
}

/// 删除实体命令
pub struct DeleteEntityCommand {
    entity: Entity,
    serialized_subtree: Vec<crate::scene_manager::EntityData>,
    /// 记录被删除前的父节点，以便恢复时放回原位 (如果是根节点则为 None)
    parent: Option<Entity>,
}

impl DeleteEntityCommand {
    pub fn new(entity: Entity, scene: &Scene) -> Self {
        let serialized_subtree = scene.serialize_entity_subtree(entity);
        let parent = scene.world.get::<alander_core::scene::Parent>(entity).map(|p| p.0);
        Self { entity, serialized_subtree, parent }
    }
}

impl EditorCommand for DeleteEntityCommand {
    fn execute(&mut self, scene: &mut Scene, _renderer: &mut Renderer) {
        scene.remove_entity(self.entity);
    }

    fn undo(&mut self, scene: &mut Scene, renderer: &mut Renderer) {
        let created = scene.spawn_entity_subtree(self.serialized_subtree.clone(), renderer);
        if let Some(new_root) = created.first() {
            // 如果原来有父节点，恢复父子关系
            if let Some(p) = self.parent {
                scene.set_parent(*new_root, Some(p));
            }
            // 更新命令内部引用的 Entity (尽管 undo 后 entity ID 可能变了，但 UUID 保持不变)
            // 建议后续命令系统改用 UUID 引用实体
            self.entity = *new_root;
        }
    }

    fn name(&self) -> &str { "删除实体" }
}

/// 创建实体命令
pub struct CreateEntityCommand {
    entities: Vec<Entity>,
    /// 首次创建时的完整数据，用于 redo
    serialized_data: Option<Vec<crate::scene_manager::EntityData>>,
}

impl CreateEntityCommand {
    pub fn new(entities: Vec<Entity>) -> Self {
        Self { entities, serialized_data: None }
    }
}

impl EditorCommand for CreateEntityCommand {
    fn execute(&mut self, scene: &mut Scene, renderer: &mut Renderer) {
        if let Some(ref data) = self.serialized_data {
            self.entities = scene.spawn_entity_subtree(data.clone(), renderer);
        } else {
            // 第一次执行已经创建好了，我们记录数据以便下次 redo
            if let Some(&first) = self.entities.first() {
                self.serialized_data = Some(scene.serialize_entity_subtree(first));
            }
        }
    }

    fn undo(&mut self, scene: &mut Scene, _renderer: &mut Renderer) {
        for &entity in &self.entities {
            scene.remove_entity(entity);
        }
    }

    fn name(&self) -> &str { "创建实体" }
}

/// 复制实体命令 (本质上是带数据的创建命令)
pub struct DuplicateEntityCommand {
    source_entity: Entity,
    cloned_entities: Vec<Entity>,
    serialized_data: Vec<crate::scene_manager::EntityData>,
}

impl DuplicateEntityCommand {
    pub fn new(source_entity: Entity, scene: &Scene) -> Self {
        let serialized_data = scene.serialize_entity_subtree(source_entity);
        Self { source_entity, cloned_entities: Vec::new(), serialized_data }
    }
}

impl EditorCommand for DuplicateEntityCommand {
    fn execute(&mut self, scene: &mut Scene, renderer: &mut Renderer) {
        // 创建副本
        self.cloned_entities = scene.spawn_entity_subtree(self.serialized_data.clone(), renderer);
    }

    fn undo(&mut self, scene: &mut Scene, _renderer: &mut Renderer) {
        for &entity in &self.cloned_entities {
            scene.remove_entity(entity);
        }
    }

    fn name(&self) -> &str { "复制实体" }
}
