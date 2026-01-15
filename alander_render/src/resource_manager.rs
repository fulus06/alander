use std::collections::HashMap;
use uuid::Uuid;
use crate::texture::Texture;
use crate::pipelines::SceneObject;

/// 采样器缓存，解耦纹理与采样器
pub struct SamplerCache {
    /// 线性过滤 + 边缘拉伸 (适用于 IBL/天空盒)
    pub linear_clamp: wgpu::Sampler,
    /// 线性过滤 + 重复 (适用于普通材质)
    pub linear_repeat: wgpu::Sampler,
    /// 最近邻过滤 + 边缘拉伸 (兼容性回退)
    pub nearest_clamp: wgpu::Sampler,
}

impl SamplerCache {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            linear_clamp: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("线性采样器 (Clamp)"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                ..Default::default()
            }),
            linear_repeat: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("线性采样器 (Repeat)"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                ..Default::default()
            }),
            nearest_clamp: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("最近邻采样器 (Clamp)"),
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                ..Default::default()
            }),
        }
    }
}

/// 资源管理器，处理纹理、模型及采样器
pub struct ResourceManager {
    /// 全局采样器
    pub samplers: SamplerCache,
    /// 场景对象池
    pub objects: HashMap<Uuid, SceneObject>,
    /// 纹理池 (按 ID 索引)
    pub textures: HashMap<usize, Texture>,
    /// 默认白纹理
    pub default_texture: Texture,
}

impl ResourceManager {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let samplers = SamplerCache::new(device);
        
        let default_img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 255])));
        let default_texture = Texture::from_image(
            device,
            queue,
            &default_img,
            Some("默认白纹理")
        ).expect("无法创建默认纹理");

        Self {
            samplers,
            objects: HashMap::new(),
            textures: HashMap::new(),
            default_texture,
        }
    }

    /// 添加场景对象
    pub fn add_object(&mut self, id: Uuid, object: SceneObject) {
        self.objects.insert(id, object);
    }

    /// 移除场景对象
    pub fn remove_object(&mut self, id: &Uuid) -> Option<SceneObject> {
        self.objects.remove(id)
    }

    /// 获取场景对象
    pub fn get_object(&self, id: &Uuid) -> Option<&SceneObject> {
        self.objects.get(id)
    }

    /// 加载 glTF 模型中的所有纹理
    pub fn load_gltf_textures(
        &mut self, 
        device: &wgpu::Device, 
        queue: &wgpu::Queue, 
        model: &alander_core::assets::GltfModel
    ) -> HashMap<usize, usize> {
        let mut image_to_texture = HashMap::new();
        for (i, img) in model.images.iter().enumerate() {
            if let Ok(texture) = Texture::from_image(device, queue, img, Some(&format!("GltfImage_{}", i))) {
                let texture_idx = self.textures.len();
                self.textures.insert(texture_idx, texture);
                image_to_texture.insert(i, texture_idx);
            }
        }
        image_to_texture
    }

    /// 根据 glTF 模型及网格获取对应的纹理
    pub fn get_texture_from_index<'a>(
        &'a self, 
        model: &alander_core::assets::GltfModel, 
        mesh: &alander_core::assets::GltfMesh,
        image_to_texture: &HashMap<usize, usize>,
        texture_type: u32, // 0: Diffuse, 1: Normal, 2: Metallic-Roughness
    ) -> &'a Texture {
        if let Some(mat_idx) = mesh.material_index {
            if let Some(material) = model.materials.get(mat_idx) {
                let img_idx_opt = match texture_type {
                    0 => material.base_color_texture.as_ref(),
                    1 => material.normal_texture.as_ref(),
                    2 => material.metallic_roughness_texture.as_ref(),
                    _ => None,
                };

                if let Some(img_idx_str) = img_idx_opt {
                    if let Ok(img_idx) = img_idx_str.parse::<usize>() {
                        if let Some(&texture_idx) = image_to_texture.get(&img_idx) {
                            if let Some(texture) = self.textures.get(&texture_idx) {
                                return texture;
                            }
                        }
                    }
                }
            }
        }
        &self.default_texture
    }
}
