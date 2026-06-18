use std::collections::HashSet;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32x4,
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

pub struct Quad {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,

    pub color: [f32; 4],
    pub label: String,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Uniforms {
    pub proj: [[f32; 4]; 4],
}

impl Uniforms {
    pub fn from_matrix(proj: [[f32; 4]; 4]) -> Self {
        Self { proj }
    }
}

pub struct QuadRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,

    cached_vertices: Vec<Vertex>,
}

impl QuadRenderer {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("quad_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/quad.wgsl").into()),
        });

        let uniforms = Uniforms::from_matrix([
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ]);

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniform_buffer"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniform_bgl"),
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

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform_bg"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("quad_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quad_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vertex_buffer"),
            size: 0,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("index_buffer"),
            size: 0,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            uniform_buffer,
            uniform_bind_group,
            vertex_buffer,
            index_buffer,
            num_indices: 0,
            cached_vertices: Vec::new(),
        }
    }

    pub fn upload_quads(&mut self, device: &wgpu::Device, quads: &[Quad]) {
        let mut vertices: Vec<Vertex> = Vec::with_capacity(quads.len() * 4);
        let mut indices: Vec<u32> = Vec::with_capacity(quads.len() * 6);

        for (i, q) in quads.iter().enumerate() {
            let base = (i * 4) as u32;
            let x0 = q.x;
            let y0 = q.y;
            let x1 = q.x + q.width;
            let y1 = q.y + q.height;
            let c = q.color;

            for pos in &[[x0, y0], [x1, y0], [x0, y1], [x1, y1]] {
                vertices.push(Vertex {
                    position: *pos,
                    color: c,
                });
            }

            indices.extend_from_slice(&[base, base + 1, base + 2, base + 1, base + 3, base + 2]);
        }

        self.vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex_buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        self.index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("index_buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        self.num_indices = indices.len() as u32;
        self.cached_vertices = vertices;
    }

    pub fn update_projection(&self, queue: &wgpu::Queue, matrix: [[f32; 4]; 4]) {
        let uniforms = Uniforms::from_matrix(matrix);
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    pub fn update_selection(
        &mut self,
        queue: &wgpu::Queue,
        quads: &[Quad],
        selected_idx: Option<usize>,
        hidden_panes: &HashSet<usize>,
    ) {
        if self.cached_vertices.is_empty() {
            return;
        }

        for (i, q) in quads.iter().enumerate() {
            let base_vertex_idx = i * 4;
            if base_vertex_idx + 3 >= self.cached_vertices.len() {
                continue;
            }

            if hidden_panes.contains(&i) {
                for v_offset in 0..4 {
                    self.cached_vertices[base_vertex_idx + v_offset].color = [0.0, 0.0, 0.0, 0.0];
                }
            } else if Some(i) == selected_idx {
                for v_offset in 0..4 {
                    let base_color = q.color;
                    self.cached_vertices[base_vertex_idx + v_offset].color = [
                        (base_color[0] + 0.4).min(1.0),
                        (base_color[1] + 0.4).min(1.0),
                        (base_color[2] + 0.4).min(1.0),
                        0.95,
                    ];
                }
            } else {
                for v_offset in 0..4 {
                    self.cached_vertices[base_vertex_idx + v_offset].color = q.color;
                }
            }
        }

        queue.write_buffer(
            &self.vertex_buffer,
            0,
            bytemuck::cast_slice(&self.cached_vertices),
        );
    }

    pub fn render<'rpass>(&'rpass self, rpass: &mut wgpu::RenderPass<'rpass>) {
        if self.num_indices == 0 {
            return;
        }
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        rpass.draw_indexed(0..self.num_indices, 0, 0..1);
    }
}
