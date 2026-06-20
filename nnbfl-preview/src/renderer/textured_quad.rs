use std::collections::HashSet;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use super::texture::TextureCache;
use crate::renderer::quad::Uniforms;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TexturedVertex {
    pub position: [f32; 2],
    pub uv0: [f32; 2],
    pub uv1: [f32; 2],
    pub uv2: [f32; 2],
    pub tint: [f32; 4],
}

impl TexturedVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x2, // position
        1 => Float32x2, // uv0
        2 => Float32x2, // uv1
        3 => Float32x2, // uv2
        4 => Float32x4, // tint
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct StandardMaterial {
    pub interpolate_width: [f32; 4],
    pub interpolate_offset: [f32; 4],
    pub combine_mode: u32,
    pub combine_mode2: u32,
    pub texture_count: u32,
    pub alpha_select: u32,
    pub tex_gen_mode: u32,
    pub _pad0: [u32; 3],

    pub indirect_mtx0: [f32; 4],
    pub indirect_mtx1: [f32; 4],
}

impl Default for StandardMaterial {
    fn default() -> Self {
        Self {
            interpolate_width: [1.0, 1.0, 1.0, 1.0],
            interpolate_offset: [0.0, 0.0, 0.0, 0.0],
            combine_mode: 0,
            combine_mode2: 0,
            texture_count: 1,
            alpha_select: 0,
            tex_gen_mode: 0,
            _pad0: [0; 3],
            indirect_mtx0: [0.0; 4],
            indirect_mtx1: [0.0; 4],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct DetailedCombinerMaterial {
    pub constant_colors: [[f32; 4]; 7],

    pub stage_count: u32,
    pub _pad0: [u32; 3],

    pub stage_bits: [[i32; 4]; 6],

    pub texture_count: u32,
    pub _pad1: [u32; 3],
}

impl Default for DetailedCombinerMaterial {
    fn default() -> Self {
        Self {
            constant_colors: [[0.0; 4]; 7],
            stage_count: 0,
            _pad0: [0; 3],
            stage_bits: [[0; 4]; 6],
            texture_count: 1,
            _pad1: [0; 3],
        }
    }
}

#[derive(Clone)]
pub struct TexturedQuad {
    pub pane_idx: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,

    pub uvs: [[[f32; 2]; 3]; 4],
    pub tint: [f32; 4],
    pub texture_name: String,

    pub texture_name1: Option<String>,
    pub texture_name2: Option<String>,

    pub address_mode_u: wgpu::AddressMode,
    pub address_mode_v: wgpu::AddressMode,
    pub min_filter: wgpu::FilterMode,
    pub mag_filter: wgpu::FilterMode,

    pub is_detailed: bool,
    pub standard_material: StandardMaterial,
    pub detailed_combiner_material: DetailedCombinerMaterial,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BatchKey {
    pub texture_name: String,
    pub address_mode_u: wgpu::AddressMode,
    pub address_mode_v: wgpu::AddressMode,
    pub min_filter: wgpu::FilterMode,
    pub mag_filter: wgpu::FilterMode,
    pub combine_mode: u32,
    pub combine_mode2: u32,
    pub is_detailed: bool,
    pub detailed_combiner_hash: [i32; 6],
}

struct TextureBatch {
    vertices: Vec<TexturedVertex>,
    indices: Vec<u32>,
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    bind_group: Option<wgpu::BindGroup>,
    num_indices: u32,

    key: BatchKey,
    pane_indices: Vec<usize>,
}

pub struct TexturedQuadRenderer {
    pipeline_standard: wgpu::RenderPipeline,
    pipeline_detailed: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_bgl: wgpu::BindGroupLayout,
    batches: Vec<TextureBatch>,
}

impl TexturedQuadRenderer {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("textured_quad_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/textured_quad.wgsl").into()),
        });

        let proj_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tq_proj_bgl"),
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
            label: Some("tq_proj_buffer"),
            contents: bytemuck::bytes_of(&Uniforms::from_matrix(identity)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tq_proj_bg"),
            layout: &proj_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tq_texture_bgl"),
            entries: &[
                Self::tex_entry(0), // t_texture0
                Self::smp_entry(1), // s_sampler0
                Self::tex_entry(2), // t_texture1
                Self::smp_entry(3), // s_sampler1
                Self::tex_entry(4), // t_texture2
                Self::smp_entry(5), // s_sampler2
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tq_pipeline_layout"),
            bind_group_layouts: &[Some(&proj_bgl), Some(&texture_bgl)],
            immediate_size: 0,
        });

        let create_pipeline = |entry: &str| -> wgpu::RenderPipeline {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("tq_pipeline_{}", entry)),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[TexturedVertex::desc()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
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
            })
        };

        let pipeline_standard = create_pipeline("fs_standard");
        let pipeline_detailed = create_pipeline("fs_detailed");

        Self {
            pipeline_standard,
            pipeline_detailed,
            uniform_buffer,
            uniform_bind_group,
            texture_bgl,
            batches: Vec::new(),
        }
    }

    fn tex_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }
    }

    fn smp_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
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
            let Some(_gpu_tex0) = texture_cache.get(&q.texture_name) else {
                continue;
            };

            let mut detailed_combiner_hash = [0i32; 6];
            if q.is_detailed {
                for i in 0..6 {
                    detailed_combiner_hash[i] = q.detailed_combiner_material.stage_bits[i][0]
                        ^ q.detailed_combiner_material.stage_bits[i][1]
                        ^ q.detailed_combiner_material.stage_bits[i][2];
                }
            }

            let key = BatchKey {
                texture_name: q.texture_name.clone(),
                address_mode_u: q.address_mode_u,
                address_mode_v: q.address_mode_v,
                min_filter: q.min_filter,
                mag_filter: q.mag_filter,
                combine_mode: q.standard_material.combine_mode,
                combine_mode2: q.standard_material.combine_mode2,
                is_detailed: q.is_detailed,
                detailed_combiner_hash,
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
            if let Some(last) = self.batches.last_mut() {
                if last.key == key {
                    let base = last.vertices.len() as u32;
                    for (i, pos) in positions.iter().enumerate() {
                        last.vertices.push(TexturedVertex {
                            position: *pos,
                            uv0: q.uvs[i][0],
                            uv1: q.uvs[i][1],
                            uv2: q.uvs[i][2],
                            tint: q.tint,
                        });
                    }

                    last.indices.extend_from_slice(&[
                        base,
                        base + 1,
                        base + 2,
                        base + 1,
                        base + 3,
                        base + 2,
                    ]);
                    last.pane_indices.push(q.pane_idx);
                    match_found = true;
                }
            }

            if !match_found {
                let mut batch = TextureBatch {
                    vertices: Vec::new(),
                    indices: Vec::new(),
                    vertex_buffer: None,
                    index_buffer: None,
                    bind_group: None,
                    num_indices: 0,
                    key,
                    pane_indices: vec![q.pane_idx],
                };

                for (i, pos) in positions.iter().enumerate() {
                    batch.vertices.push(TexturedVertex {
                        position: *pos,
                        uv0: q.uvs[i][0],
                        uv1: q.uvs[i][1],
                        uv2: q.uvs[i][2],
                        tint: q.tint,
                    });
                }

                batch.indices.extend_from_slice(&[0, 1, 2, 1, 3, 2]);
                self.batches.push(batch);
            }
        }

        for batch in &mut self.batches {
            if batch.vertices.is_empty() {
                continue;
            }

            let rep_quad = quads
                .iter()
                .find(|q| q.pane_idx == batch.pane_indices[0])
                .unwrap();

            let gpu_tex0 = texture_cache.get(&batch.key.texture_name).unwrap();

            let mat_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tq_standard_mat_buf"),
                contents: bytemuck::bytes_of(&rep_quad.standard_material),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            let detailed_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tq_detailed_mat_buf"),
                contents: bytemuck::bytes_of(&rep_quad.detailed_combiner_material),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            batch.vertex_buffer = Some(device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("tq_vb"),
                    contents: bytemuck::cast_slice(&batch.vertices),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                },
            ));

            batch.index_buffer = Some(device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("tq_ib"),
                    contents: bytemuck::cast_slice(&batch.indices),
                    usage: wgpu::BufferUsages::INDEX,
                },
            ));

            batch.num_indices = batch.indices.len() as u32;

            let gpu_tex1 = rep_quad
                .texture_name1
                .as_ref()
                .and_then(|n| texture_cache.get(n))
                .unwrap_or(gpu_tex0);

            let gpu_tex2 = rep_quad
                .texture_name2
                .as_ref()
                .and_then(|n| texture_cache.get(n))
                .unwrap_or(gpu_tex0);

            let make_sampler = |am_u: wgpu::AddressMode,
                                am_v: wgpu::AddressMode,
                                min: wgpu::FilterMode,
                                mag: wgpu::FilterMode|
             -> wgpu::Sampler {
                device.create_sampler(&wgpu::SamplerDescriptor {
                    address_mode_u: am_u,
                    address_mode_v: am_v,
                    address_mode_w: wgpu::AddressMode::ClampToEdge,
                    min_filter: min,
                    mag_filter: mag,
                    mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                    ..Default::default()
                })
            };

            let sampler0 = make_sampler(
                batch.key.address_mode_u,
                batch.key.address_mode_v,
                batch.key.min_filter,
                batch.key.mag_filter,
            );

            let sampler1 = make_sampler(
                wgpu::AddressMode::ClampToEdge,
                wgpu::AddressMode::ClampToEdge,
                wgpu::FilterMode::Linear,
                wgpu::FilterMode::Linear,
            );

            let sampler2 = make_sampler(
                wgpu::AddressMode::ClampToEdge,
                wgpu::AddressMode::ClampToEdge,
                wgpu::FilterMode::Linear,
                wgpu::FilterMode::Linear,
            );

            batch.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("tq_bg"),
                layout: &self.texture_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&gpu_tex0.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler0),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&gpu_tex1.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&sampler1),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&gpu_tex2.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(&sampler2),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: mat_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: detailed_buf.as_entire_binding(),
                    },
                ],
            }));
        }
    }

    pub fn update_selection(
        &mut self,
        queue: &wgpu::Queue,
        quads: &[TexturedQuad],
        selected_idx: Option<usize>,
        hidden_panes: &HashSet<usize>,
    ) {
        for batch in &mut self.batches {
            for (batch_quad_idx, &pane_idx) in batch.pane_indices.iter().enumerate() {
                let base_vertex_idx = batch_quad_idx * 4;

                if base_vertex_idx + 3 >= batch.vertices.len() {
                    break;
                }

                if let Some(original_q) = quads.iter().find(|q| q.pane_idx == pane_idx) {
                    if hidden_panes.contains(&pane_idx) {
                        for v_offset in 0..4 {
                            batch.vertices[base_vertex_idx + v_offset].tint = [0.0, 0.0, 0.0, 0.0];
                        }
                    } else if Some(pane_idx) == selected_idx {
                        for v_offset in 0..4 {
                            let base_tint = original_q.tint;
                            batch.vertices[base_vertex_idx + v_offset].tint = [
                                (base_tint[0] + 0.4).min(1.0),
                                (base_tint[1] + 0.4).min(1.0),
                                (base_tint[2] + 0.4).min(1.0),
                                0.95,
                            ];
                        }
                    } else {
                        for v_offset in 0..4 {
                            batch.vertices[base_vertex_idx + v_offset].tint = original_q.tint;
                        }
                    }
                }
            }

            if let Some(ref vb) = batch.vertex_buffer {
                queue.write_buffer(vb, 0, bytemuck::cast_slice(&batch.vertices));
            }
        }
    }

    pub fn render<'rpass>(&'rpass self, rpass: &mut wgpu::RenderPass<'rpass>) {
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

            if batch.key.is_detailed {
                rpass.set_pipeline(&self.pipeline_detailed);
            } else {
                rpass.set_pipeline(&self.pipeline_standard);
            }

            rpass.set_bind_group(1, bg, &[]);
            rpass.set_vertex_buffer(0, vb.slice(..));
            rpass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..batch.num_indices, 0, 0..1);
        }
    }
}
