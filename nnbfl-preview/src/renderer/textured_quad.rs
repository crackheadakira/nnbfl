use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use super::texture::TextureCache;
use crate::renderer::quad::Uniforms;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TexturedVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub tint: [f32; 4],
}

impl TexturedVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x2, // position
        1 => Float32x2, // uv
        2 => Float32x4, // tint
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}
pub struct TexturedQuad {
    /// top-left in layout space
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub uvs: [[f32; 2]; 4],
    pub tint: [f32; 4],
    pub texture_name: String,
    pub secondary_texture_name: Option<String>,

    pub address_mode_u: wgpu::AddressMode,
    pub address_mode_v: wgpu::AddressMode,
    pub min_filter: wgpu::FilterMode,
    pub mag_filter: wgpu::FilterMode,

    pub material_uniforms: MaterialUniforms,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniforms {
    pub tev_mode: u32,
    pub source_a: u32,
    pub source_b: u32,
    pub source_c: u32,

    pub color_op: u32,
    pub alpha_op: u32,
    pub has_indirect: u32,
    pub _padding: u32,

    pub indirect_scale_x: f32,
    pub indirect_scale_y: f32,
    pub _padding2: [f32; 2],

    pub constant_color0: [f32; 4],
    pub constant_color1: [f32; 4],
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BatchKey {
    pub texture_name: String,
    pub address_mode_u: wgpu::AddressMode,
    pub address_mode_v: wgpu::AddressMode,
    pub min_filter: wgpu::FilterMode,
    pub mag_filter: wgpu::FilterMode,

    pub tev_mode: u32,
    pub source_a: u32,
    pub source_b: u32,
    pub source_c: u32,
}

struct TextureBatch {
    vertices: Vec<TexturedVertex>,
    indices: Vec<u32>,
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    bind_group: Option<wgpu::BindGroup>,
    num_indices: u32,

    key: BatchKey,
}

pub struct TexturedQuadRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    batches: Vec<TextureBatch>,
}

impl TexturedQuadRenderer {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        texture_cache: &TextureCache,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("textured_quad_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/textured_quad.wgsl").into()),
        });

        let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tq_uniform_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let identity = [
            [1f32, 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ];
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tq_uniform_buffer"),
            contents: bytemuck::bytes_of(&Uniforms::from_matrix(identity)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tq_uniform_bg"),
            layout: &uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tq_pipeline_layout"),
            bind_group_layouts: &[Some(&uniform_bgl), Some(&texture_cache.bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("textured_quad_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[TexturedVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            uniform_buffer,
            uniform_bind_group,
            batches: Vec::new(),
        }
    }

    pub fn update_projection(&self, queue: &wgpu::Queue, matrix: [[f32; 4]; 4]) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&Uniforms::from_matrix(matrix)),
        );
    }

    pub fn upload_quads(
        &mut self,
        device: &wgpu::Device,
        quads: &[TexturedQuad],
        texture_cache: &TextureCache,
    ) {
        self.batches.clear();

        for q in quads {
            let key = BatchKey {
                texture_name: q.texture_name.clone(),
                address_mode_u: q.address_mode_u,
                address_mode_v: q.address_mode_v,
                min_filter: q.min_filter,
                mag_filter: q.mag_filter,

                tev_mode: q.material_uniforms.tev_mode,
                source_a: q.material_uniforms.source_a,
                source_b: q.material_uniforms.source_b,
                source_c: q.material_uniforms.source_c,
            };

            let mut x0 = q.x;
            let mut y0 = q.y;
            let mut w = q.width;
            let mut h = q.height;

            if let Some(gpu_tex) = texture_cache.get(&q.texture_name) {
                let pane_aspect = w / h;
                let texture_aspect = gpu_tex.width as f32 / gpu_tex.height as f32;

                // it seems to be aspect-ratio locked?
                if texture_aspect > pane_aspect {
                    let target_h = w / texture_aspect;
                    let delta_y = h - target_h;
                    h = target_h;
                    y0 += delta_y * 0.5;
                } else {
                    let target_w = h * texture_aspect;
                    let delta_x = w - target_w;
                    w = target_w;
                    x0 += delta_x * 0.5;
                }
            }

            let x1 = x0 + w;
            let y1 = y0 + h;
            let positions = [[x0, y0], [x1, y0], [x0, y1], [x1, y1]];

            let mut match_found = false;
            if let Some(last_batch) = self.batches.last_mut() {
                if last_batch.key == key {
                    let base = last_batch.vertices.len() as u32;

                    for (pos, uv) in positions.iter().zip(q.uvs.iter()) {
                        last_batch.vertices.push(TexturedVertex {
                            position: *pos,
                            uv: *uv,
                            tint: q.tint,
                        });
                    }

                    last_batch.indices.extend_from_slice(&[
                        base,
                        base + 1,
                        base + 2,
                        base + 1,
                        base + 3,
                        base + 2,
                    ]);
                    match_found = true;
                }
            }

            if !match_found {
                let mut new_batch = TextureBatch {
                    vertices: Vec::new(),
                    indices: Vec::new(),
                    vertex_buffer: None,
                    index_buffer: None,
                    bind_group: None,
                    num_indices: 0,
                    key: key.clone(),
                };

                for (pos, uv) in positions.iter().zip(q.uvs.iter()) {
                    new_batch.vertices.push(TexturedVertex {
                        position: *pos,
                        uv: *uv,
                        tint: q.tint,
                    });
                }

                new_batch.indices.extend_from_slice(&[0, 1, 2, 1, 3, 2]);

                self.batches.push(new_batch);
            }
        }

        for batch in &mut self.batches {
            let key = &batch.key;

            let q = quads
                .iter()
                .find(|quad| quad.texture_name == key.texture_name)
                .unwrap();

            let mat_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("tq_mat_ub_{}", key.texture_name)),
                contents: bytemuck::bytes_of(&q.material_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            batch.vertex_buffer = Some(device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("tq_vb_{}", key.texture_name)),
                    contents: bytemuck::cast_slice(&batch.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));

            batch.index_buffer = Some(device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("tq_ib_{}", key.texture_name)),
                    contents: bytemuck::cast_slice(&batch.indices),
                    usage: wgpu::BufferUsages::INDEX,
                },
            ));

            batch.num_indices = batch.indices.len() as u32;

            if let Some(gpu_tex) = texture_cache.get(&key.texture_name) {
                let dynamic_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some(&format!("tq_sampler_{}", key.texture_name)),
                    address_mode_u: key.address_mode_u,
                    address_mode_v: key.address_mode_v,
                    address_mode_w: wgpu::AddressMode::ClampToEdge,
                    min_filter: key.min_filter,
                    mag_filter: key.mag_filter,
                    mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                    ..Default::default()
                });

                let secondary_tex = q
                    .secondary_texture_name
                    .as_ref()
                    .and_then(|name| texture_cache.get(name))
                    .unwrap_or(gpu_tex);

                batch.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("tq_bg_{}", key.texture_name)),
                    layout: &texture_cache.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&gpu_tex.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&dynamic_sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&secondary_tex.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: mat_buffer.as_entire_binding(),
                        },
                    ],
                }));
            }
        }
    }

    pub fn render<'rpass>(&'rpass self, rpass: &mut wgpu::RenderPass<'rpass>) {
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.uniform_bind_group, &[]);

        for batch in &self.batches {
            if batch.num_indices == 0 {
                continue;
            }

            let (Some(vb), Some(ib), Some(bg)) =
                (&batch.vertex_buffer, &batch.index_buffer, &batch.bind_group)
            else {
                continue;
            };

            rpass.set_bind_group(1, bg, &[]);
            rpass.set_vertex_buffer(0, vb.slice(..));
            rpass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..batch.num_indices, 0, 0..1);
        }
    }
}
