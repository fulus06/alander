//! 资源管理系统
//!
//! 此模块提供资源加载、管理和生命周期管理功能。

use crate::scene::{MeshData, Vertex, MaterialData};

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

/// 唯一资源标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Handle<T> {
    pub id: u64,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> Handle<T> {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            _phantom: std::marker::PhantomData,
        }
    }
}

/// 资源管理器
pub struct AssetManager<T> {
    assets: HashMap<u64, Arc<T>>,
    next_id: u64,
}

impl<T> AssetManager<T> {
    pub fn new() -> Self {
        Self {
            assets: HashMap::new(),
            next_id: 1,
        }
    }
    
    pub fn load(&mut self, asset: T) -> Handle<T> {
        let handle = Handle::new(self.next_id);
        self.next_id += 1;
        self.assets.insert(handle.id, Arc::new(asset));
        handle
    }
    
    pub fn get(&self, handle: &Handle<T>) -> Option<Arc<T>> {
        self.assets.get(&handle.id).cloned()
    }
    
    pub fn contains(&self, handle: &Handle<T>) -> bool {
        self.assets.contains_key(&handle.id)
    }
    
    pub fn len(&self) -> usize {
        self.assets.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }
}

/// 资源加载器特征
pub trait AssetLoader<T> {
    fn load(&mut self, source: &str) -> Result<T, AssetError>;
}

/// 资源错误
#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("解析错误: {0}")]
    Parse(String),
    #[error("资源未找到: {0}")]
    NotFound(String),
    #[error("不支持的格式: {0}")]
    UnsupportedFormat(String),
}


/// 测试用的简单网格加载器
pub struct SimpleMeshLoader;

impl AssetLoader<crate::scene::MeshData> for SimpleMeshLoader {
    fn load(&mut self, source: &str) -> Result<crate::scene::MeshData, AssetError> {
        // 简化实现：创建默认立方体
        if source == "cube" {
            use crate::scene::Vertex;
            use glam::{Vec3, Vec2, Vec4};

            let vertices = vec![
                // 前面 (Front)
                Vertex::with_tangent(Vec3::new(-0.5, -0.5,  0.5), Vec3::new(0.0, 0.0, 1.0), Vec2::new(0.0, 0.0), Vec4::new(1.0, 0.0, 0.0, 1.0)),
                Vertex::with_tangent(Vec3::new( 0.5, -0.5,  0.5), Vec3::new(0.0, 0.0, 1.0), Vec2::new(1.0, 0.0), Vec4::new(1.0, 0.0, 0.0, 1.0)),
                Vertex::with_tangent(Vec3::new( 0.5,  0.5,  0.5), Vec3::new(0.0, 0.0, 1.0), Vec2::new(1.0, 1.0), Vec4::new(1.0, 0.0, 0.0, 1.0)),
                Vertex::with_tangent(Vec3::new(-0.5,  0.5,  0.5), Vec3::new(0.0, 0.0, 1.0), Vec2::new(0.0, 1.0), Vec4::new(1.0, 0.0, 0.0, 1.0)),
                // 后面 (Back)
                Vertex::with_tangent(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.0, 0.0, -1.0), Vec2::new(0.0, 0.0), Vec4::new(-1.0, 0.0, 0.0, 1.0)),
                Vertex::with_tangent(Vec3::new( 0.5, -0.5, -0.5), Vec3::new(0.0, 0.0, -1.0), Vec2::new(1.0, 0.0), Vec4::new(-1.0, 0.0, 0.0, 1.0)),
                Vertex::with_tangent(Vec3::new( 0.5,  0.5, -0.5), Vec3::new(0.0, 0.0, -1.0), Vec2::new(1.0, 1.0), Vec4::new(-1.0, 0.0, 0.0, 1.0)),
                Vertex::with_tangent(Vec3::new(-0.5,  0.5, -0.5), Vec3::new(0.0, 0.0, -1.0), Vec2::new(0.0, 1.0), Vec4::new(-1.0, 0.0, 0.0, 1.0)),
                // 左面 (Left)
                Vertex::with_tangent(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(-1.0, 0.0, 0.0), Vec2::new(0.0, 0.0), Vec4::new(0.0, 0.0, 1.0, 1.0)),
                Vertex::with_tangent(Vec3::new(-0.5, -0.5,  0.5), Vec3::new(-1.0, 0.0, 0.0), Vec2::new(1.0, 0.0), Vec4::new(0.0, 0.0, 1.0, 1.0)),
                Vertex::with_tangent(Vec3::new(-0.5,  0.5,  0.5), Vec3::new(-1.0, 0.0, 0.0), Vec2::new(1.0, 1.0), Vec4::new(0.0, 0.0, 1.0, 1.0)),
                Vertex::with_tangent(Vec3::new(-0.5,  0.5, -0.5), Vec3::new(-1.0, 0.0, 0.0), Vec2::new(0.0, 1.0), Vec4::new(0.0, 0.0, 1.0, 1.0)),
                // 右面 (Right)
                Vertex::with_tangent(Vec3::new( 0.5, -0.5,  0.5), Vec3::new( 1.0, 0.0, 0.0), Vec2::new(0.0, 0.0), Vec4::new(0.0, 0.0, -1.0, 1.0)),
                Vertex::with_tangent(Vec3::new( 0.5, -0.5, -0.5), Vec3::new( 1.0, 0.0, 0.0), Vec2::new(1.0, 0.0), Vec4::new(0.0, 0.0, -1.0, 1.0)),
                Vertex::with_tangent(Vec3::new( 0.5,  0.5, -0.5), Vec3::new( 1.0, 0.0, 0.0), Vec2::new(1.0, 1.0), Vec4::new(0.0, 0.0, -1.0, 1.0)),
                Vertex::with_tangent(Vec3::new( 0.5,  0.5,  0.5), Vec3::new( 1.0, 0.0, 0.0), Vec2::new(0.0, 1.0), Vec4::new(0.0, 0.0, -1.0, 1.0)),
                // 上面 (Top)
                Vertex::with_tangent(Vec3::new(-0.5,  0.5,  0.5), Vec3::new( 0.0, 1.0, 0.0), Vec2::new(0.0, 0.0), Vec4::new(1.0, 0.0, 0.0, 1.0)),
                Vertex::with_tangent(Vec3::new( 0.5,  0.5,  0.5), Vec3::new( 0.0, 1.0, 0.0), Vec2::new(1.0, 0.0), Vec4::new(1.0, 0.0, 0.0, 1.0)),
                Vertex::with_tangent(Vec3::new( 0.5,  0.5, -0.5), Vec3::new( 0.0, 1.0, 0.0), Vec2::new(1.0, 1.0), Vec4::new(1.0, 0.0, 0.0, 1.0)),
                Vertex::with_tangent(Vec3::new(-0.5,  0.5, -0.5), Vec3::new( 0.0, 1.0, 0.0), Vec2::new(0.0, 1.0), Vec4::new(1.0, 0.0, 0.0, 1.0)),
                // 下面 (Bottom)
                Vertex::with_tangent(Vec3::new(-0.5, -0.5, -0.5), Vec3::new( 0.0, -1.0, 0.0), Vec2::new(0.0, 0.0), Vec4::new(1.0, 0.0, 0.0, 1.0)),
                Vertex::with_tangent(Vec3::new( 0.5, -0.5, -0.5), Vec3::new( 0.0, -1.0, 0.0), Vec2::new(1.0, 0.0), Vec4::new(1.0, 0.0, 0.0, 1.0)),
                Vertex::with_tangent(Vec3::new( 0.5, -0.5,  0.5), Vec3::new( 0.0, -1.0, 0.0), Vec2::new(1.0, 1.0), Vec4::new(1.0, 0.0, 0.0, 1.0)),
                Vertex::with_tangent(Vec3::new(-0.5, -0.5,  0.5), Vec3::new( 0.0, -1.0, 0.0), Vec2::new(0.0, 1.0), Vec4::new(1.0, 0.0, 0.0, 1.0)),
            ];

            let indices = vec![
                0, 1, 2, 2, 3, 0,       // 前面
                4, 7, 6, 6, 5, 4,       // 后面 (CW to be visible from outside)
                8, 9, 10, 10, 11, 8,    // 左面
                12, 13, 14, 14, 15, 12, // 右面
                16, 17, 18, 18, 19, 16, // 上面
                20, 21, 22, 22, 23, 20, // 下面
            ];

            Ok(crate::scene::MeshData {
                name: "Cube".to_string(),
                vertices,
                indices,
            })
        } else {
            Err(AssetError::NotFound(format!("网格 '{}' 未找到", source)))
        }
    }
}

/// 测试用的简单材质加载器
pub struct SimpleMaterialLoader;

impl AssetLoader<crate::scene::MaterialData> for SimpleMaterialLoader {
    fn load(&mut self, source: &str) -> Result<crate::scene::MaterialData, AssetError> {
        // 简化实现：创建默认材质
        if source == "default" {
            Ok(crate::scene::MaterialData::default())
        } else if source == "red" {
            Ok(crate::scene::MaterialData {
                name: "红色材质".to_string(),
                base_color: glam::Vec4::new(1.0, 0.0, 0.0, 1.0),
                metallic: 0.0,
                roughness: 0.5,
                normal_texture: None,
                base_color_texture: None,
                metallic_roughness_texture: None,
            })
        } else {
            Err(AssetError::NotFound(format!("材质 '{}' 未找到", source)))
        }
    }
}

/// glTF 模型数据
/// glTF 节点数据 (用于重建层级结构)
pub struct GltfNode {
    pub name: String,
    pub index: usize,
    pub local_transform: super::scene::Transform,
    pub mesh_indices: Vec<usize>, // 对应 model.meshes 中的索引 (一个节点可能有多个 primitive)
    pub skin_index: Option<usize>,
    pub children: Vec<usize>,
}

/// glTF 模型数据
pub struct GltfModel {
    pub nodes: Vec<GltfNode>,
    pub meshes: Vec<GltfMesh>,
    pub materials: Vec<MaterialData>,
    pub images: Vec<image::DynamicImage>,
    pub skins: Vec<SkinData>,
    pub animations: Vec<super::scene::AnimationClip>,
    pub root_nodes: Vec<usize>,
}

/// glTF 蒙皮数据
pub struct SkinData {
    pub name: String,
    pub inverse_bind_matrices: Vec<glam::Mat4>,
    pub joints: Vec<usize>, // 对应 glTF node 索引
}

/// glTF 网格及其绑定的材质索引和变换
pub struct GltfMesh {
    pub data: MeshData,
    pub material_index: Option<usize>,
    pub transform: glam::Mat4, // 默认的世界变换 (回退用)
    pub skin_index: Option<usize>,
}

/// glTF 资源加载器
pub struct GltfLoader;

impl GltfLoader {
    /// 加载 glTF 文件并返回模型数据
    pub fn load_scene(&self, path: &str) -> Result<GltfModel, AssetError> {
        tracing::info!("正在从路径加载 glTF: {}", path);
        let (document, buffers, images) = gltf::import(path)
            .map_err(|e| AssetError::Parse(format!("glTF 导入失败: {}", e)))?;

        let mut converted_images = Vec::new();
        for image in images {
            let dynamic_image = match image.format {
                gltf::image::Format::R8G8B8 => image::DynamicImage::ImageRgb8(image::RgbImage::from_raw(image.width, image.height, image.pixels).unwrap()),
                gltf::image::Format::R8G8B8A8 => image::DynamicImage::ImageRgba8(image::RgbaImage::from_raw(image.width, image.height, image.pixels).unwrap()),
                _ => return Err(AssetError::UnsupportedFormat(format!("不支持的图像格式: {:?}", image.format))),
            };
            converted_images.push(dynamic_image);
        }

        let mut materials = Vec::new();
        for material in document.materials() {
            let pbr = material.pbr_metallic_roughness();
            materials.push(MaterialData {
                name: material.name().unwrap_or(&format!("Material_{}", material.index().unwrap_or(0))).to_string(),
                base_color: pbr.base_color_factor().into(),
                metallic: pbr.metallic_factor(),
                roughness: pbr.roughness_factor(),
                base_color_texture: pbr.base_color_texture().map(|t| t.texture().source().index().to_string()),
                normal_texture: material.normal_texture().map(|t| t.texture().source().index().to_string()),
                metallic_roughness_texture: pbr.metallic_roughness_texture().map(|t| t.texture().source().index().to_string()),
            });
        }

        let mut all_skins = Vec::new();
        for skin in document.skins() {
            let reader = skin.reader(|buffer| Some(&buffers[buffer.index()]));
            let mut ibms = Vec::new();
            if let Some(ibm_iter) = reader.read_inverse_bind_matrices() {
                for m in ibm_iter { ibms.push(glam::Mat4::from_cols_array_2d(&m)); }
            }
            all_skins.push(SkinData {
                name: skin.name().unwrap_or(&format!("Skin_{}", skin.index())).to_string(),
                inverse_bind_matrices: ibms,
                joints: skin.joints().map(|j| j.index()).collect(),
            });
        }

        let mut all_meshes = Vec::new();
        let mut node_to_mesh_indices = std::collections::HashMap::new();

        // 1. 提取所有节点的网格数据
        for node in document.nodes() {
            if let Some(mesh) = node.mesh() {
                let mut indices = Vec::new();
                for primitive in mesh.primitives() {
                    let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                    if let Some(pos_iter) = reader.read_positions() {
                        let positions: Vec<[f32; 3]> = pos_iter.collect();
                        let normals: Vec<[f32; 3]> = reader.read_normals().map(|n| n.collect()).unwrap_or_else(|| vec![[0.0, 0.0, 0.0]; positions.len()]);
                        let uvs: Vec<[f32; 2]> = reader.read_tex_coords(0).map(|uv| uv.into_f32().collect()).unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);
                        let tangents: Vec<[f32; 4]> = reader.read_tangents().map(|t| t.collect()).unwrap_or_else(|| vec![[1.0, 0.0, 0.0, 1.0]; positions.len()]);
                        let joint_indices: Vec<[u32; 4]> = reader.read_joints(0).map(|j| j.into_u16().map(|v| v.map(|x| x as u32)).collect()).unwrap_or_else(|| vec![[0; 4]; positions.len()]);
                        let joint_weights: Vec<[f32; 4]> = reader.read_weights(0).map(|w| w.into_f32().collect()).unwrap_or_else(|| vec![[0.0; 4]; positions.len()]);

                        let mut mesh_vertices = Vec::new();
                        for i in 0..positions.len() {
                            mesh_vertices.push(Vertex::with_skinning(
                                positions[i].into(), normals[i].into(), uvs[i].into(), tangents[i].into(),
                                joint_indices[i], joint_weights[i]
                            ));
                        }

                        let mesh_indices = reader.read_indices().map(|indices| indices.into_u32().collect()).unwrap_or_else(|| (0..positions.len() as u32).collect());

                        all_meshes.push(GltfMesh {
                            data: MeshData { name: mesh.name().unwrap_or("Mesh").to_string(), vertices: mesh_vertices, indices: mesh_indices },
                            material_index: primitive.material().index(),
                            transform: glam::Mat4::IDENTITY, // 在层级结构中会重新应用
                            skin_index: node.skin().map(|s| s.index()),
                        });
                        indices.push(all_meshes.len() - 1);
                    }
                }
                node_to_mesh_indices.insert(node.index(), indices);
            }
        }

        // 2. 提取层级信息
        let mut all_nodes = Vec::new();
        for node in document.nodes() {
            let (translation, rotation, scale) = node.transform().decomposed();
            all_nodes.push(GltfNode {
                name: node.name().unwrap_or(&format!("Node_{}", node.index())).to_string(),
                index: node.index(),
                local_transform: super::scene::Transform {
                    position: translation.into(),
                    rotation: glam::Quat::from_array(rotation),
                    scale: scale.into(),
                },
                mesh_indices: node_to_mesh_indices.get(&node.index()).cloned().unwrap_or_default(),
                skin_index: node.skin().map(|s| s.index()),
                children: node.children().map(|c| c.index()).collect(),
            });
        }

        let mut root_nodes = Vec::new();
        for scene in document.scenes() {
            for node in scene.nodes() { root_nodes.push(node.index()); }
        }

        // 3. 提取动画
        let mut all_animations = Vec::new();
        for animation in document.animations() {
            let mut clip = super::scene::AnimationClip::new(animation.name().unwrap_or(&format!("Animation_{}", animation.index())).to_string());
            for channel in animation.channels() {
                let target = channel.target();
                let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
                let target_node = target.node();
                let target_name = target_node.name().unwrap_or(&format!("Node_{}", target_node.index())).to_string();
                
                let mut channel_found = false;
                for c in &mut clip.channels {
                    if c.target_name == target_name {
                        let input = reader.read_inputs().unwrap().collect::<Vec<_>>();
                        let output = reader.read_outputs().unwrap();
                        match output {
                            gltf::animation::util::ReadOutputs::Translations(iter) => {
                                let mut kfs = Vec::new();
                                for (t, v) in input.iter().zip(iter) { kfs.push(super::scene::Keyframe { time: *t, value: v.into() }); }
                                c.position_track = Some(super::scene::AnimationTrack::new(kfs));
                            }
                            gltf::animation::util::ReadOutputs::Rotations(iter) => {
                                let mut kfs = Vec::new();
                                for (t, v) in input.iter().zip(iter.into_f32()) { kfs.push(super::scene::Keyframe { time: *t, value: glam::Quat::from_array(v) }); }
                                c.rotation_track = Some(super::scene::AnimationTrack::new(kfs));
                            }
                            gltf::animation::util::ReadOutputs::Scales(iter) => {
                                let mut kfs = Vec::new();
                                for (t, v) in input.iter().zip(iter) { kfs.push(super::scene::Keyframe { time: *t, value: v.into() }); }
                                c.scale_track = Some(super::scene::AnimationTrack::new(kfs));
                            }
                            _ => {}
                        }
                        channel_found = true;
                        break;
                    }
                }

                if !channel_found {
                    let mut anim_channel = super::scene::AnimationChannel {
                        target_name: target_name.clone(),
                        position_track: None, rotation_track: None, scale_track: None,
                    };
                    let input = reader.read_inputs().unwrap().collect::<Vec<_>>();
                    let output = reader.read_outputs().unwrap();
                    match output {
                        gltf::animation::util::ReadOutputs::Translations(iter) => {
                            let mut keyframes = Vec::new();
                            for (t, v) in input.iter().zip(iter) {
                                keyframes.push(super::scene::Keyframe { time: *t, value: v.into() });
                            }
                            anim_channel.position_track = Some(super::scene::AnimationTrack::new(keyframes));
                        }
                        gltf::animation::util::ReadOutputs::Rotations(iter) => {
                            let mut keyframes = Vec::new();
                            for (t, v) in input.iter().zip(iter.into_f32()) {
                                keyframes.push(super::scene::Keyframe { time: *t, value: glam::Quat::from_array(v) });
                            }
                            anim_channel.rotation_track = Some(super::scene::AnimationTrack::new(keyframes));
                        }
                        gltf::animation::util::ReadOutputs::Scales(iter) => {
                            let mut keyframes = Vec::new();
                            for (t, v) in input.iter().zip(iter) {
                                keyframes.push(super::scene::Keyframe { time: *t, value: v.into() });
                            }
                            anim_channel.scale_track = Some(super::scene::AnimationTrack::new(keyframes));
                        }
                        _ => {}
                    }
                    clip.channels.push(anim_channel);
                }
            }
            clip.update_duration();
            all_animations.push(clip);
        }

        Ok(GltfModel {
            nodes: all_nodes,
            meshes: all_meshes,
            materials,
            images: converted_images,
            skins: all_skins,
            animations: all_animations,
            root_nodes,
        })
    }


}




#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{MeshData, MaterialData};

    #[test]
    fn test_asset_manager() {
        let mut manager = AssetManager::<MeshData>::new();
        
        // 测试加载资源
        let mesh = MeshData {
            name: "Test Mesh".to_string(),
            vertices: vec![],
            indices: vec![],
        };
        
        let handle = manager.load(mesh);
        assert!(manager.contains(&handle));
        assert_eq!(manager.len(), 1);
        
        // 测试获取资源
        let loaded_mesh = manager.get(&handle);
        assert!(loaded_mesh.is_some());
    }
    
    #[test]
    fn test_mesh_loader() {
        let mut loader = SimpleMeshLoader;
        
        // 测试加载立方体
        let result = loader.load("cube");
        assert!(result.is_ok());
        
        let mesh_data = result.unwrap();
        assert_eq!(mesh_data.vertices.len(), 4);
        assert_eq!(mesh_data.indices.len(), 6);
        
        // 测试加载不存在的网格
        let result = loader.load("nonexistent");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_material_loader() {
        let mut loader = SimpleMaterialLoader;
        
        // 测试加载默认材质
        let result = loader.load("default");
        assert!(result.is_ok());
        
        let material_data = result.unwrap();
        assert_eq!(material_data.name, "默认材质");
        
        // 测试加载红色材质
        let result = loader.load("red");
        assert!(result.is_ok());
        
        let material_data = result.unwrap();
        assert_eq!(material_data.name, "红色材质");
        assert_eq!(material_data.base_color, glam::Vec4::new(1.0, 0.0, 0.0, 1.0));
    }
}