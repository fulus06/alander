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
pub struct GltfModel {
    pub meshes: Vec<GltfMesh>,
    pub materials: Vec<MaterialData>,
    pub images: Vec<image::DynamicImage>,
}

/// glTF 网格及其绑定的材质索引和变换
pub struct GltfMesh {
    pub data: MeshData,
    pub material_index: Option<usize>,
    pub transform: glam::Mat4,
}

/// glTF 资源加载器
pub struct GltfLoader;

impl GltfLoader {
    /// 加载 glTF 文件并返回模型数据
    pub fn load_scene(&self, path: &str) -> Result<GltfModel, AssetError> {
        tracing::info!("正在从路径加载 glTF: {}", path);
        let (document, buffers, images) = gltf::import(path)
            .map_err(|e| AssetError::Parse(format!("glTF 导入失败: {}", e)))?;

        tracing::info!("glTF 导入成功: {} 个网格, {} 个图像", document.meshes().count(), images.len());

        let mut converted_images = Vec::new();
        for (i, image) in images.into_iter().enumerate() {
            let width = image.width;
            let height = image.height;
            tracing::debug!("转换图像 {}: {}x{}, 格式: {:?}", i, width, height, image.format);
            let dynamic_image = match image.format {
                gltf::image::Format::R8G8B8 => {
                    let buffer = image::RgbImage::from_raw(width, height, image.pixels)
                        .ok_or_else(|| AssetError::Parse(format!("图像 {} RGB 数据不匹配", i)))?;
                    image::DynamicImage::ImageRgb8(buffer)
                }
                gltf::image::Format::R8G8B8A8 => {
                    let buffer = image::RgbaImage::from_raw(width, height, image.pixels)
                        .ok_or_else(|| AssetError::Parse(format!("图像 {} RGBA 数据不匹配", i)))?;
                    image::DynamicImage::ImageRgba8(buffer)
                }
                _ => {
                    tracing::warn!("不支持的图像 {} 格式: {:?}", i, image.format);
                    return Err(AssetError::UnsupportedFormat(format!("不支持的图像格式: {:?}", image.format)));
                }
            };
            converted_images.push(dynamic_image);
        }

        let mut materials = Vec::new();
        for (i, material) in document.materials().enumerate() {
            let pbr = material.pbr_metallic_roughness();
            let base_color = pbr.base_color_factor();
            
            tracing::debug!("处理材质 {}: {}", i, material.name().unwrap_or("未命名"));

            materials.push(MaterialData {
                name: material.name().unwrap_or(&format!("材质_{}", i)).to_string(),
                base_color: base_color.into(),
                metallic: pbr.metallic_factor(),
                roughness: pbr.roughness_factor(),
                base_color_texture: pbr.base_color_texture().map(|t| t.texture().source().index().to_string()),
                normal_texture: material.normal_texture().map(|t| t.texture().source().index().to_string()),
                metallic_roughness_texture: pbr.metallic_roughness_texture().map(|t| t.texture().source().index().to_string()),
            });
        }

        let mut all_meshes = Vec::new();

        // 3. 递归遍历节点并提取网格实例
        for scene in document.scenes() {
            for node in scene.nodes() {
                Self::process_node_internal(&node, glam::Mat4::IDENTITY, &buffers, &mut all_meshes);
            }
        }

        // 如果场景中没有节点（或者没有引用网格），尝试直接加载所有定义的网格（回退方案）
        if all_meshes.is_empty() {
            tracing::info!("场景中未发现网格实例，尝试回退到加载网格定义");
            for mesh in document.meshes() {
                for primitive in mesh.primitives() {
                    let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                    
                    if let Some(pos_iter) = reader.read_positions() {
                        let positions: Vec<[f32; 3]> = pos_iter.collect();
                        let normals: Vec<[f32; 3]> = reader.read_normals()
                            .map(|n| n.collect())
                            .unwrap_or_else(|| vec![[0.0, 0.0, 0.0]; positions.len()]);
                        let uvs: Vec<[f32; 2]> = reader.read_tex_coords(0)
                            .map(|uv| uv.into_f32().collect())
                            .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

                        let tangents: Vec<[f32; 4]> = reader.read_tangents()
                            .map(|t| t.collect())
                            .unwrap_or_else(|| vec![[1.0, 0.0, 0.0, 1.0]; positions.len()]);

                        let mut mesh_vertices = Vec::new();
                        for i in 0..positions.len() {
                            mesh_vertices.push(Vertex::with_tangent(
                                positions[i].into(),
                                normals[i].into(),
                                uvs[i].into(),
                                tangents[i].into(),
                            ));
                        }

                        let mut mesh_indices = Vec::new();
                        if let Some(indices) = reader.read_indices() {
                            mesh_indices.extend(indices.into_u32());
                        } else {
                            for i in 0..positions.len() as u32 {
                                mesh_indices.push(i);
                            }
                        }

                        all_meshes.push(GltfMesh {
                            data: MeshData {
                                name: mesh.name().unwrap_or("Unnamed Mesh").to_string(),
                                vertices: mesh_vertices,
                                indices: mesh_indices,
                            },
                            material_index: primitive.material().index(),
                            transform: glam::Mat4::IDENTITY,
                        });
                    }
                }
            }
        }

        Ok(GltfModel {
            meshes: all_meshes,
            materials,
            images: converted_images,
        })
    }


    fn process_node_internal(node: &gltf::Node, parent_transform: glam::Mat4, buffers: &[gltf::buffer::Data], meshes: &mut Vec<GltfMesh>) {
        let (translation, rotation, scale) = node.transform().decomposed();
        let local_transform = glam::Mat4::from_scale_rotation_translation(
            glam::Vec3::from(scale),
            glam::Quat::from_array(rotation),
            glam::Vec3::from(translation),
        );
        let world_transform = parent_transform * local_transform;

        if let Some(mesh) = node.mesh() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                
                if let Some(pos_iter) = reader.read_positions() {
                    let positions: Vec<[f32; 3]> = pos_iter.collect();
                    let normals: Vec<[f32; 3]> = reader.read_normals()
                        .map(|n| n.collect())
                        .unwrap_or_else(|| vec![[0.0, 0.0, 0.0]; positions.len()]);
                    let uvs: Vec<[f32; 2]> = reader.read_tex_coords(0)
                        .map(|uv| uv.into_f32().collect())
                        .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);
                    let tangents: Vec<[f32; 4]> = reader.read_tangents()
                        .map(|t| t.collect())
                        .unwrap_or_else(|| vec![[1.0, 0.0, 0.0, 1.0]; positions.len()]);

                    let mut mesh_vertices = Vec::new();
                    for i in 0..positions.len() {
                        mesh_vertices.push(Vertex::with_tangent(
                            positions[i].into(),
                            normals[i].into(),
                            uvs[i].into(),
                            tangents[i].into(),
                        ));
                    }

                    let mut mesh_indices = Vec::new();
                    if let Some(indices) = reader.read_indices() {
                        mesh_indices.extend(indices.into_u32());
                    } else {
                        for i in 0..positions.len() as u32 {
                            mesh_indices.push(i);
                        }
                    }

                    meshes.push(GltfMesh {
                        data: MeshData {
                            name: mesh.name().unwrap_or("Unnamed Mesh").to_string(),
                            vertices: mesh_vertices,
                            indices: mesh_indices,
                        },
                        material_index: primitive.material().index(),
                        transform: world_transform,
                    });
                }
            }
        }

        for child in node.children() {
            Self::process_node_internal(&child, world_transform, buffers, meshes);
        }
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