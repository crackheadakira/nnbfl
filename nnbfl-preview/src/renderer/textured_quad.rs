use std::collections::HashSet;

use bytemuck::{Pod, Zeroable};
use nnbfl::bflyt::list::MaterialTextureSrt;
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
    pub tex_aspects: [f32; 3],
}

impl TexturedVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
        0 => Float32x2, // position
        1 => Float32x2, // uv0
        2 => Float32x2, // uv1
        3 => Float32x2, // uv2
        4 => Float32x4, // tint
        5 => Float32x3, // tex_aspects
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
    pub visible: u32,
    pub use_texture_only: u32,
    pub use_thresholding_alpha_interpolation: u32,

    pub debug_stage: u32,
    pub _padding: [f32; 3],

    pub indirect_mtx0: [f32; 4],
    pub indirect_mtx1: [f32; 4],

    pub proj_mtx0: [[f32; 4]; 2],
    pub proj_mtx1: [[f32; 4]; 2],
    pub proj_mtx2: [[f32; 4]; 2],
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
            visible: 1,
            use_texture_only: 0,
            use_thresholding_alpha_interpolation: 0,
            debug_stage: 0,
            _padding: [0.0; 3],
            indirect_mtx0: [0.0; 4],
            indirect_mtx1: [0.0; 4],
            proj_mtx0: [[1.0, 0.0, 0.0, 0.5], [0.0, 1.0, 0.0, 0.5]],
            proj_mtx1: [[1.0, 0.0, 0.0, 0.5], [0.0, 1.0, 0.0, 0.5]],
            proj_mtx2: [[1.0, 0.0, 0.0, 0.5], [0.0, 1.0, 0.0, 0.5]],
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

#[derive(Clone, Debug)]
pub struct TexturedQuad {
    pub pane_idx: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// World-space corner positions [TL, TR, BL, BR] after rotation.
    pub corners: [[f32; 2]; 4],

    pub uvs: [[[f32; 2]; 3]; 4],
    pub base_uvs: [[[f32; 2]; 3]; 4],
    pub tex_srts: Vec<MaterialTextureSrt>,
    pub tint: [f32; 4],
    pub corner_tints: [[f32; 4]; 4],
    pub texture_name: String,
    pub texture_name1: Option<String>,
    pub texture_name2: Option<String>,

    pub address_mode_u: wgpu::AddressMode,
    pub address_mode_v: wgpu::AddressMode,
    pub min_filter: wgpu::FilterMode,
    pub mag_filter: wgpu::FilterMode,

    pub address_mode_u1: wgpu::AddressMode,
    pub address_mode_v1: wgpu::AddressMode,
    pub min_filter1: wgpu::FilterMode,
    pub mag_filter1: wgpu::FilterMode,

    pub address_mode_u2: wgpu::AddressMode,
    pub address_mode_v2: wgpu::AddressMode,
    pub min_filter2: wgpu::FilterMode,
    pub mag_filter2: wgpu::FilterMode,

    pub is_detailed: bool,
    pub standard_material: StandardMaterial,
    pub detailed_combiner_material: DetailedCombinerMaterial,

    pub proj_scales: [[f32; 2]; 3],
    pub proj_translations: [[f32; 2]; 3],
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
    mat_buffer: Option<wgpu::Buffer>,
    detailed_buffer: Option<wgpu::Buffer>,
    num_indices: u32,

    key: BatchKey,
    pane_indices: Vec<usize>,
    adjusted_positions: Vec<[[f32; 2]; 4]>,
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
        quads: &mut [&mut TexturedQuad],
        texture_cache: &TextureCache,
        layout_w: f32,
        layout_h: f32,
    ) {
        self.batches.clear();

        for q in quads.iter_mut() {
            let mut detailed_combiner_hash = [0i32; 6];

            if q.is_detailed {
                for (i, hash) in detailed_combiner_hash.iter_mut().enumerate() {
                    *hash = q.detailed_combiner_material.stage_bits[i][0]
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

            let positions = q.corners;

            let tex_aspects = [
                Self::get_layer_aspect(q, texture_cache, layout_w, layout_h, 0),
                Self::get_layer_aspect(q, texture_cache, layout_w, layout_h, 1),
                Self::get_layer_aspect(q, texture_cache, layout_w, layout_h, 2),
            ];

            let mut match_found = false;
            if let Some(last) = self.batches.last_mut()
                && last.key == key
            {
                let base = last.vertices.len() as u32;
                for (i, pos) in positions.iter().enumerate() {
                    last.vertices.push(TexturedVertex {
                        position: *pos,
                        uv0: q.uvs[i][0],
                        uv1: q.uvs[i][1],
                        uv2: q.uvs[i][2],
                        tint: q.tint,
                        tex_aspects,
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
                last.adjusted_positions.push(positions);
                match_found = true;
            }

            if !match_found {
                let mut batch = TextureBatch {
                    vertices: Vec::new(),
                    indices: Vec::new(),
                    vertex_buffer: None,
                    index_buffer: None,
                    bind_group: None,
                    mat_buffer: None,
                    detailed_buffer: None,
                    num_indices: 0,
                    key,
                    pane_indices: vec![q.pane_idx],
                    adjusted_positions: vec![positions],
                };

                for (i, pos) in positions.iter().enumerate() {
                    batch.vertices.push(TexturedVertex {
                        position: *pos,
                        uv0: q.uvs[i][0],
                        uv1: q.uvs[i][1],
                        uv2: q.uvs[i][2],
                        tint: q.tint,
                        tex_aspects,
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

            let pane_cx = rep_quad.x + rep_quad.width * 0.5;
            let pane_cy = rep_quad.y + rep_quad.height * 0.5;

            let mut final_mat = rep_quad.standard_material;

            final_mat.proj_mtx0 = Self::calculate_projection_matrix(
                rep_quad,
                texture_cache,
                layout_w,
                layout_h,
                pane_cx,
                pane_cy,
                0,
            );

            final_mat.proj_mtx1 = Self::calculate_projection_matrix(
                rep_quad,
                texture_cache,
                layout_w,
                layout_h,
                pane_cx,
                pane_cy,
                1,
            );

            final_mat.proj_mtx2 = Self::calculate_projection_matrix(
                rep_quad,
                texture_cache,
                layout_w,
                layout_h,
                pane_cx,
                pane_cy,
                2,
            );

            let mat_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tq_standard_mat_buf"),
                contents: bytemuck::bytes_of(&final_mat),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            batch.mat_buffer = Some(mat_buf);

            let detailed_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tq_detailed_mat_buf"),
                contents: bytemuck::bytes_of(&rep_quad.detailed_combiner_material),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            batch.detailed_buffer = Some(detailed_buf);

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
                rep_quad.address_mode_u1,
                rep_quad.address_mode_v1,
                rep_quad.min_filter1,
                rep_quad.mag_filter1,
            );

            let sampler2 = make_sampler(
                rep_quad.address_mode_u2,
                rep_quad.address_mode_v2,
                rep_quad.min_filter2,
                rep_quad.mag_filter2,
            );

            let mat_buf_ref = batch.mat_buffer.as_ref().unwrap();
            let detailed_buf_ref = batch.detailed_buffer.as_ref().unwrap();

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
                        resource: mat_buf_ref.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: detailed_buf_ref.as_entire_binding(),
                    },
                ],
            }));
        }
    }

    pub fn update_selection(
        &mut self,
        queue: &wgpu::Queue,
        quads: &mut [&mut TexturedQuad],
        selected_idx: Option<usize>,
        active_debug_stage: u32,
    ) {
        for batch in &mut self.batches {
            for (batch_quad_idx, &pane_idx) in batch.pane_indices.iter().enumerate() {
                let base = batch_quad_idx * 4;
                if base + 3 >= batch.vertices.len() {
                    break;
                }

                let Some(q) = quads.iter_mut().find(|q| q.pane_idx == pane_idx) else {
                    continue;
                };

                q.standard_material.debug_stage = active_debug_stage;

                let tint = if Some(pane_idx) == selected_idx {
                    [
                        (q.tint[0] + 0.4).min(1.0),
                        (q.tint[1] + 0.4).min(1.0),
                        (q.tint[2] + 0.4).min(1.0),
                        0.95,
                    ]
                } else {
                    q.tint
                };

                for v_offset in 0..4 {
                    let ct = q.corner_tints[v_offset];
                    batch.vertices[base + v_offset].tint = [
                        tint[0] * ct[0],
                        tint[1] * ct[1],
                        tint[2] * ct[2],
                        tint[3] * ct[3],
                    ];
                }
            }

            if let Some(ref vb) = batch.vertex_buffer {
                queue.write_buffer(vb, 0, bytemuck::cast_slice(&batch.vertices));
            }
        }
    }

    pub fn update_texture_pattern(
        &mut self,
        device: &wgpu::Device,
        quads: &[&mut TexturedQuad],
        texture_cache: &TextureCache,
    ) {
        for batch in &mut self.batches {
            let Some(&pane_idx) = batch.pane_indices.first() else {
                continue;
            };
            let Some(tq) = quads.iter().find(|q| q.pane_idx == pane_idx) else {
                continue;
            };

            let tex0_name = &tq.texture_name;
            let tex1_name = tq.texture_name1.as_deref().unwrap_or(tex0_name);
            let tex2_name = tq.texture_name2.as_deref().unwrap_or(tex0_name);

            if batch.key.texture_name == *tex0_name {
                continue;
            }

            let Some(gpu_tex0) = texture_cache.get(tex0_name) else {
                continue;
            };
            let gpu_tex1 = texture_cache.get(tex1_name).unwrap_or(gpu_tex0);
            let gpu_tex2 = texture_cache.get(tex2_name).unwrap_or(gpu_tex0);

            let make_sampler = |am_u, am_v, min, mag| {
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
                tq.address_mode_u1,
                tq.address_mode_v1,
                tq.min_filter1,
                tq.mag_filter1,
            );

            let sampler2 = make_sampler(
                tq.address_mode_u2,
                tq.address_mode_v2,
                tq.min_filter2,
                tq.mag_filter2,
            );

            let Some(mat_buf) = &batch.mat_buffer else {
                continue;
            };

            let detailed_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tq_detailed_mat_buf_pat"),
                contents: bytemuck::bytes_of(&tq.detailed_combiner_material),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            batch.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("tq_bg_pattern"),
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

            batch.detailed_buffer = Some(detailed_buf);
            batch.key.texture_name = tex0_name.clone();
        }
    }

    pub fn recompute_proj_mtx(
        &mut self,
        quads: &mut [&mut TexturedQuad],
        texture_cache: &TextureCache,
        layout_w: f32,
        layout_h: f32,
    ) {
        for batch in &mut self.batches {
            let Some(&first_pane) = batch.pane_indices.first() else {
                continue;
            };

            let Some(tq) = quads.iter_mut().find(|q| q.pane_idx == first_pane) else {
                continue;
            };

            let mode0 = tq.standard_material.tex_gen_mode & 0x3;
            let mode1 = (tq.standard_material.tex_gen_mode >> 8) & 0x3;
            let mode2 = (tq.standard_material.tex_gen_mode >> 16) & 0x3;
            if mode0 != 1 && mode1 != 1 && mode2 != 1 {
                continue;
            }

            let pane_cx = tq.x + tq.width * 0.5;
            let pane_cy = tq.y + tq.height * 0.5;

            tq.standard_material.proj_mtx0 = Self::calculate_projection_matrix(
                tq,
                texture_cache,
                layout_w,
                layout_h,
                pane_cx,
                pane_cy,
                0,
            );

            tq.standard_material.proj_mtx1 = Self::calculate_projection_matrix(
                tq,
                texture_cache,
                layout_w,
                layout_h,
                pane_cx,
                pane_cy,
                1,
            );

            tq.standard_material.proj_mtx2 = Self::calculate_projection_matrix(
                tq,
                texture_cache,
                layout_w,
                layout_h,
                pane_cx,
                pane_cy,
                2,
            );
        }
    }

    fn calculate_projection_matrix(
        quad: &TexturedQuad,
        texture_cache: &TextureCache,
        layout_w: f32,
        layout_h: f32,
        pane_cx: f32,
        pane_cy: f32,
        layer_idx: usize,
    ) -> [[f32; 4]; 2] {
        let tex_aspect_ratio =
            Self::get_layer_aspect(quad, texture_cache, layout_w, layout_h, layer_idx);

        let shift = layer_idx * 8;
        let packed = quad.standard_material.tex_gen_mode >> shift;
        let mode = packed & 0x3;

        if mode != 1 {
            return [[1.0, 0.0, 0.0, 0.5], [0.0, 1.0, 0.0, 0.5]];
        }

        let fitting_layout_size = (packed & (1 << 2)) != 0;
        let _fitting_pane_size = (packed & (1 << 3)) != 0;
        let adjust_sr = (packed & (1 << 4)) != 0;
        let orthogonal = (packed & (1 << 5)) != 0;

        let (base_w, base_h, cx, cy) = if orthogonal {
            (layout_w, layout_h, layout_w * 0.5, layout_h * 0.5)
        } else if fitting_layout_size {
            (layout_w, layout_h, pane_cx, pane_cy)
        } else {
            (quad.width, quad.height, pane_cx, pane_cy)
        };

        let srt_tu = quad
            .tex_srts
            .get(layer_idx)
            .map(|s| s.translate_u)
            .unwrap_or(0.0);

        let srt_tv = quad
            .tex_srts
            .get(layer_idx)
            .map(|s| s.translate_v)
            .unwrap_or(0.0);

        if adjust_sr {
            let sx = quad.proj_scales[layer_idx][0];
            let sy = quad.proj_scales[layer_idx][1];

            let tx = quad.proj_translations[layer_idx][0];
            let ty = quad.proj_translations[layer_idx][1];

            let (input_w, input_h) = if orthogonal {
                (layout_w, layout_h)
            } else {
                (quad.width, quad.height)
            };

            let reciprocal_width = 1.0 / input_w;
            let reciprocal_height = 1.0 / input_h;

            let mut scale_s = 0.5 / sx;
            let mut scale_t = 0.5 / sy;

            let mut trans_s = 0.5 - (tx / sx / base_w) + srt_tu;
            let mut trans_t = 0.5 - (ty / sy / base_h) + srt_tv;

            if tex_aspect_ratio > 1.0 {
                scale_t *= tex_aspect_ratio;
                trans_t = trans_t * tex_aspect_ratio + (0.5 - 0.5 * tex_aspect_ratio);
            } else {
                let inv_ratio = 1.0 / tex_aspect_ratio;
                scale_s *= inv_ratio;
                trans_s = trans_s * inv_ratio + (0.5 - 0.5 * inv_ratio);
            }

            [
                [2.0 * reciprocal_width * scale_s, 0.0, 0.0, trans_s],
                [0.0, 2.0 * reciprocal_height * scale_t, 0.0, trans_t],
            ]
        } else {
            let mut m_s = 1.0 / base_w;
            let mut m_t = 1.0 / base_h;

            let (scale_s, scale_t) = if tex_aspect_ratio > 1.0 {
                (1.0, tex_aspect_ratio)
            } else {
                (1.0 / tex_aspect_ratio, 1.0)
            };

            m_s *= scale_s;
            m_t *= scale_t;

            let trans_s = 0.5 - (cx * m_s) + srt_tu;
            let trans_t = 0.5 - (cy * m_t) + srt_tv;

            [[m_s, 0.0, 0.0, trans_s], [0.0, m_t, 0.0, trans_t]]
        }
    }

    fn get_layer_aspect(
        quad: &TexturedQuad,
        texture_cache: &TextureCache,
        layout_w: f32,
        layout_h: f32,
        layer_idx: usize,
    ) -> f32 {
        let shift = layer_idx * 8;
        let packed = quad.standard_material.tex_gen_mode >> shift;
        let orthogonal = (packed & (1 << 5)) != 0;

        let base_aspect = if orthogonal {
            if layout_h > 0.0 {
                layout_w / layout_h
            } else {
                1.0
            }
        } else {
            if quad.height > 0.0 {
                quad.width / quad.height
            } else {
                1.0
            }
        };

        let tex_name = match layer_idx {
            1 => quad.texture_name1.as_deref(),
            2 => quad.texture_name2.as_deref(),
            _ => Some(quad.texture_name.as_str()),
        };

        tex_name
            .and_then(|name| texture_cache.get(name))
            .map(|t| (t.width as f32 / t.height as f32) / base_aspect)
            .unwrap_or(1.0)
    }

    pub fn update_anim(
        &mut self,
        queue: &wgpu::Queue,
        quads: &[&mut TexturedQuad],
        hidden_panes: &HashSet<usize>,
    ) {
        for batch in &mut self.batches {
            let mut dirty = false;

            for (batch_quad_idx, &pane_idx) in batch.pane_indices.iter().enumerate() {
                let base = batch_quad_idx * 4;
                if base + 3 >= batch.vertices.len() {
                    break;
                }
                let Some(tq) = quads.iter().find(|q| q.pane_idx == pane_idx) else {
                    continue;
                };

                let is_hidden = hidden_panes.contains(&pane_idx);

                let Some(base_positions) = batch.adjusted_positions.get(batch_quad_idx) else {
                    continue;
                };

                let dx = tq.corners[0][0] - base_positions[0][0];
                let dy = tq.corners[0][1] - base_positions[0][1];

                let positions = [
                    [base_positions[0][0] + dx, base_positions[0][1] + dy],
                    [base_positions[1][0] + dx, base_positions[1][1] + dy],
                    [base_positions[2][0] + dx, base_positions[2][1] + dy],
                    [base_positions[3][0] + dx, base_positions[3][1] + dy],
                ];

                let tint = if is_hidden { [0.0; 4] } else { tq.tint };

                let corner_tints = if is_hidden {
                    [[0.0f32; 4]; 4]
                } else {
                    tq.corner_tints
                };

                for v_offset in 0..4 {
                    let v = &mut batch.vertices[base + v_offset];
                    v.position = positions[v_offset];
                    v.uv0 = tq.uvs[v_offset][0];
                    v.uv1 = tq.uvs[v_offset][1];
                    v.uv2 = tq.uvs[v_offset][2];

                    let ct = corner_tints[v_offset];
                    v.tint = [
                        tint[0] * ct[0],
                        tint[1] * ct[1],
                        tint[2] * ct[2],
                        tint[3] * ct[3],
                    ];
                }

                dirty = true;
            }

            if dirty && let Some(vb) = &batch.vertex_buffer {
                queue.write_buffer(vb, 0, bytemuck::cast_slice(&batch.vertices));
            }
        }
    }

    pub fn flush_mat_buffers(
        &self,
        queue: &wgpu::Queue,
        quads: &[&mut TexturedQuad],
        hidden_panes: &HashSet<usize>,
    ) {
        for batch in &self.batches {
            let Some(&first_pane_idx) = batch.pane_indices.first() else {
                continue;
            };
            let Some(tq) = quads.iter().find(|q| q.pane_idx == first_pane_idx) else {
                continue;
            };
            let Some(mb) = &batch.mat_buffer else {
                continue;
            };

            let mut mat = tq.standard_material;
            if hidden_panes.contains(&first_pane_idx) {
                mat.visible = 0;
            }

            queue.write_buffer(mb, 0, bytemuck::bytes_of(&mat));
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
