use std::collections::HashMap;

use tomolib::formats::bntx::{Bntx, image::decode_texture_rgba};
use wgpu::util::DeviceExt;

pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

pub struct TextureCache {
    pub textures: HashMap<String, GpuTexture>,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl TextureCache {
    pub fn new(device: &wgpu::Device) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        Self {
            textures: HashMap::new(),
            bind_group_layout,
        }
    }

    pub fn load_from_bntx_bytes(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bntx_bytes: &[u8],
    ) {
        let bntx = match Bntx::parse(&bntx_bytes) {
            Ok(b) => b,
            Err(e) => {
                log::error!("TextureCache: failed to parse BNTX: {e}");
                return;
            }
        };

        log::info!("TextureCache: loading {} textures", bntx.textures.len());

        for tex in &bntx.textures {
            match decode_texture_rgba(tex, 0) {
                Ok(mut rgba) => {
                    let needs_swizzle = tex.info.channel_r != 2
                        || tex.info.channel_g != 3
                        || tex.info.channel_b != 4
                        || tex.info.channel_a != 5;

                    if needs_swizzle {
                        for pixel in &mut rgba.data.chunks_exact_mut(4) {
                            let src = [pixel[0], pixel[1], pixel[2], pixel[3]];

                            let resolve = |ch: u8| match ch {
                                0 => 0x00,
                                1 => 0xFF,
                                2 => src[0], // red
                                3 => src[1], // green
                                4 => src[2], // blue
                                5 => src[3], // alpha
                                _ => 0x00,
                            };

                            pixel[0] = resolve(tex.info.channel_r);
                            pixel[1] = resolve(tex.info.channel_g);
                            pixel[2] = resolve(tex.info.channel_b);
                            pixel[3] = resolve(tex.info.channel_a);
                        }
                    };

                    let gpu_tex = upload_rgba(
                        device,
                        queue,
                        &rgba.data,
                        rgba.width,
                        rgba.height,
                        &tex.name,
                    );

                    log::debug!(
                        "TextureCache: uploaded '{}' ({}x{})",
                        tex.name,
                        rgba.width,
                        rgba.height
                    );

                    self.textures.insert(tex.name.clone(), gpu_tex);
                }
                Err(e) => {
                    log::warn!("TextureCache: failed to decode '{}': {e}", tex.name);
                }
            }
        }
    }

    pub fn get(&self, name: &str) -> Option<&GpuTexture> {
        self.textures.get(name)
    }
}

fn upload_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &[u8],
    width: u32,
    height: u32,
    label: &str,
) -> GpuTexture {
    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        data,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    GpuTexture {
        texture,
        view,
        width,
        height,
    }
}
