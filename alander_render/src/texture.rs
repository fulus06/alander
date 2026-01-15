use anyhow::Result;
use image::GenericImageView;
use half::f16;

pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

impl Texture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: &str,
    ) -> Result<Self> {
        let img = image::load_from_memory(bytes)?;
        Self::from_image(device, queue, &img, Some(label))
    }

    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        label: Option<&str>,
    ) -> Result<Self> {
        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Ok(Self {
            texture,
            view,
        })
    }

    /// 创建立方体贴图 (用于天空盒/IBL)
    pub fn create_cubemap(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        images: &[image::DynamicImage; 6],
        label: Option<&str>,
    ) -> Result<Self> {
        let dimensions = images[0].dimensions();
        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 6,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        for (i, img) in images.iter().enumerate() {
            let rgba = img.to_rgba8();
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    aspect: wgpu::TextureAspect::All,
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: i as u32,
                    },
                },
                &rgba,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * dimensions.0),
                    rows_per_image: Some(dimensions.1),
                },
                wgpu::Extent3d {
                    width: dimensions.0,
                    height: dimensions.1,
                    depth_or_array_layers: 1,
                },
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label,
            format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: Some(6),
        });

        Ok(Self {
            texture,
            view,
        })
    }

    /// 创建一个 1x1 的黑色立方体贴图作为默认值
    pub fn create_dummy_cubemap(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Self> {
        let size = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 6,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("默认立方体贴图"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let gray_pixel = [51u8, 51, 51, 255]; // 约 0.2 亮度，更深沉
        for i in 0..6 {
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    aspect: wgpu::TextureAspect::All,
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: i },
                },
                &gray_pixel,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: Some(1),
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        Ok(Self { texture, view })
    }

    /// 从 HDR 文件创建纹理 (.hdr)
    pub fn from_hdr(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &std::path::Path,
        format: wgpu::TextureFormat,
        _sampler: &wgpu::Sampler,
    ) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let decoder = image::codecs::hdr::HdrDecoder::new(std::io::BufReader::new(file))?;
        let metadata = decoder.metadata();
        
        let mut width = metadata.width;
        let mut height = metadata.height;

        // 检查硬件限制
        let max_size = device.limits().max_texture_dimension_2d;
        if width > max_size || height > max_size {
            let ratio = width as f32 / height as f32;
            if width > height {
                width = max_size;
                height = (max_size as f32 / ratio) as u32;
            } else {
                height = max_size;
                width = (max_size as f32 * ratio) as u32;
            }
            tracing::warn!("HDR 图像尺寸 {}x{} 超过硬件限制 {}，已自动缩放至 {}x{}", 
                metadata.width, metadata.height, max_size, width, height);
        }

        let image_data = decoder.read_image_native()?;
        
        // 将 RGBE 转换为 [f32; 4]
        let mut pixels_f32 = Vec::with_capacity((metadata.width * metadata.height * 4) as usize);
        for pixel in image_data {
            let rgbe = [pixel.c[0], pixel.c[1], pixel.c[2], pixel.e];
            if rgbe[3] == 0 {
                pixels_f32.extend_from_slice(&[0.0, 0.0, 0.0, 1.0]);
            } else {
                let exponent = f32::from(rgbe[3]) - 128.0;
                let factor = exponent.exp2() / 256.0;
                pixels_f32.push(f32::from(rgbe[0]) * factor);
                pixels_f32.push(f32::from(rgbe[1]) * factor);
                pixels_f32.push(f32::from(rgbe[2]) * factor);
                pixels_f32.push(1.0);
            }
        }

        // 如果需要缩放
        let final_pixels_f32 = if width != metadata.width || height != metadata.height {
            let img_buffer = image::ImageBuffer::<image::Rgba<f32>, Vec<f32>>::from_raw(
                metadata.width, metadata.height, pixels_f32
            ).ok_or_else(|| anyhow::anyhow!("创建缩放缓冲区失败"))?;
            
            let dynamic_img = image::DynamicImage::ImageRgba32F(img_buffer);
            let resized_img = dynamic_img.resize(width, height, image::imageops::FilterType::Lanczos3);
            resized_img.to_rgba32f().into_raw()
        } else {
            pixels_f32
        };

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HDR 全景图"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // 根据目标格式准备字节数据
        let bytes = if format == wgpu::TextureFormat::Rgba16Float {
            let pixels_f16: Vec<f16> = final_pixels_f32.iter().map(|&f| f16::from_f32(f)).collect();
            bytemuck::cast_slice(&pixels_f16).to_vec()
        } else {
            bytemuck::cast_slice(&final_pixels_f32).to_vec()
        };

        queue.write_texture(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &bytes,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some((if format == wgpu::TextureFormat::Rgba16Float { 8 } else { 16 }) * width),
                rows_per_image: Some(height),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Ok(Self { texture, view })
    }

    /// 将全景图转换为立方体贴图
    pub fn equirectangular_to_cubemap(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        equirect_texture: &Texture,
        cube_size: u32,
        format: wgpu::TextureFormat,
        sampler: &wgpu::Sampler,
        filterable: bool,
    ) -> Result<Self> {
        // 创建立方体贴图纹理
        let size = wgpu::Extent3d {
            width: cube_size,
            height: cube_size,
            depth_or_array_layers: 6,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HDR 立方体贴图"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        
        // 存储视图用于计算着色器写入
        let storage_view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        // 创建计算管线
        let shader_src = include_str!("shaders/equirect_to_cube.wgsl");
        let processed_src = if format == wgpu::TextureFormat::Rgba16Float {
            shader_src.replace("rgba32float", "rgba16float")
        } else {
            shader_src.to_string()
        };

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("HDR 转换着色器"),
            source: wgpu::ShaderSource::Wgsl(processed_src.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HDR 转换布局"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(if filterable { 
                        wgpu::SamplerBindingType::Filtering 
                    } else { 
                        wgpu::SamplerBindingType::NonFiltering 
                    }),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: format,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HDR 转换布局"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("HDR 转换管线"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HDR 转换组"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&equirect_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&storage_view),
                },
            ],
        });

        // 执行计算
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("HDR 转换编码器") });
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("HDR 转换过程") });
            compute_pass.set_pipeline(&compute_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.dispatch_workgroups((cube_size + 7) / 8, (cube_size + 7) / 8, 6);
        }
        queue.submit(Some(encoder.finish()));

        Ok(Self { texture, view })
    }
}
