use std::collections::HashSet;

use bytemuck::{Pod, Zeroable};
use wgpu::{SurfaceConfiguration, util::DeviceExt};

use crate::camera::Camera;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
    pub quad_size: [f32; 2],
    pub uv: [f32; 2],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32x4,
        2 => Float32x2,
        3 => Float32x2,
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
    pub has_textured: bool,
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

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GridUniforms {
    pub proj: [[f32; 4]; 4],
    pub resolution: [f32; 2],
    pub zoom: f32,
    pub _padding: f32,
}

pub struct QuadRenderer {
    pub quad_pipeline: RenderPipelineContainer,
    pub grid_pipeline: RenderPipelineContainer,

    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,

    cached_vertices: Vec<Vertex>,
}

impl QuadRenderer {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let matrix = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ];
        let quad_uniforms = Uniforms::from_matrix(matrix);

        let quad_container = RenderPipelineContainer::new(
            device,
            "quad",
            include_str!("../shaders/quad.wgsl"),
            &[Vertex::desc()],
            surface_format,
            quad_uniforms,
            wgpu::ShaderStages::VERTEX,
        );

        let grid_container = RenderPipelineContainer::new(
            device,
            "grid",
            include_str!("../shaders/grid.wgsl"),
            &[],
            surface_format,
            GridUniforms {
                proj: matrix,
                resolution: [1920.0, 1080.0],
                zoom: 1.0,
                _padding: 0.0,
            },
            wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
        );

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
            quad_pipeline: quad_container,
            grid_pipeline: grid_container,
            vertex_buffer,
            index_buffer,
            num_indices: 0,
            cached_vertices: Vec::new(),
        }
    }

    pub fn upload_quads(&mut self, device: &wgpu::Device, quads: &[Quad]) {
        let mut vertices: Vec<Vertex> = Vec::with_capacity(quads.len() * 4);
        let mut indices: Vec<u32> = Vec::with_capacity(quads.len() * 6);
        let uvs = [[0.0f32, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];

        for (i, q) in quads.iter().enumerate() {
            /*if q.has_textured {
                continue;
            }*/

            let base = (i * 4) as u32;
            let x0 = q.x;
            let y0 = q.y;
            let x1 = q.x + q.width;
            let y1 = q.y + q.height;
            let c = q.color;
            let size = [q.width, q.height];

            for (v_idx, pos) in [[x0, y0], [x1, y0], [x0, y1], [x1, y1]].iter().enumerate() {
                vertices.push(Vertex {
                    position: *pos,
                    color: c,
                    quad_size: size,
                    uv: uvs[v_idx],
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

    pub fn update_projection(
        &self,
        queue: &wgpu::Queue,
        camera: &Camera,
        config: &SurfaceConfiguration,
    ) {
        let width = config.width as f32;
        let height = config.height as f32;
        let matrix = camera.build_matrix(width, height);

        let quad_uniforms = Uniforms::from_matrix(matrix);
        queue.write_buffer(
            &self.quad_pipeline.uniform_buffer,
            0,
            bytemuck::bytes_of(&quad_uniforms),
        );

        let grid_uniforms = GridUniforms {
            proj: matrix,
            resolution: [width, height],
            zoom: camera.zoom,
            _padding: 0.0,
        };

        queue.write_buffer(
            &self.grid_pipeline.uniform_buffer,
            0,
            bytemuck::bytes_of(&grid_uniforms),
        );
    }

    pub fn update_anim(
        &mut self,
        queue: &wgpu::Queue,
        quads: &[Quad],
        hidden_panes: &HashSet<usize>,
        quad_for_tex: bool,
    ) {
        if self.cached_vertices.is_empty() {
            return;
        }

        let uvs = [[0.0f32, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];

        for (i, q) in quads.iter().enumerate() {
            let base = i * 4;
            if base + 3 >= self.cached_vertices.len() {
                continue;
            }

            let color = if hidden_panes.contains(&i) || (q.has_textured && !quad_for_tex) {
                [0.0; 4]
            } else {
                q.color
            };

            let x0 = q.x;
            let y0 = q.y;
            let x1 = q.x + q.width;
            let y1 = q.y + q.height;
            let size = [q.width, q.height];

            let positions = [[x0, y0], [x1, y0], [x0, y1], [x1, y1]];
            for v_offset in 0..4 {
                self.cached_vertices[base + v_offset].position = positions[v_offset];
                self.cached_vertices[base + v_offset].color = color;
                self.cached_vertices[base + v_offset].quad_size = size;
                self.cached_vertices[base + v_offset].uv = uvs[v_offset];
            }
        }

        queue.write_buffer(
            &self.vertex_buffer,
            0,
            bytemuck::cast_slice(&self.cached_vertices),
        );
    }

    pub fn update_selection(
        &mut self,
        queue: &wgpu::Queue,
        quads: &[Quad],
        selected_idx: Option<usize>,
        hidden_panes: &HashSet<usize>,
        quad_for_tex: bool,
    ) {
        if self.cached_vertices.is_empty() {
            return;
        }

        let uvs = [[0.0f32, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];

        for (i, q) in quads.iter().enumerate() {
            let base_vertex_idx = i * 4;
            if base_vertex_idx + 3 >= self.cached_vertices.len() {
                continue;
            }

            let size = [q.width, q.height];

            let final_color = if hidden_panes.contains(&i) || (q.has_textured && !quad_for_tex) {
                [0.0, 0.0, 0.0, 0.0]
            } else if Some(i) == selected_idx {
                [
                    (q.color[0] + 0.4).min(1.0),
                    (q.color[1] + 0.4).min(1.0),
                    (q.color[2] + 0.4).min(1.0),
                    0.95,
                ]
            } else {
                q.color
            };

            for (v_offset, uv) in uvs.iter().enumerate() {
                let vertex = &mut self.cached_vertices[base_vertex_idx + v_offset];
                vertex.color = final_color;
                vertex.quad_size = size;
                vertex.uv = *uv;
            }
        }

        queue.write_buffer(
            &self.vertex_buffer,
            0,
            bytemuck::cast_slice(&self.cached_vertices),
        );
    }

    pub fn render<'rpass>(&'rpass self, rpass: &mut wgpu::RenderPass<'rpass>) {
        if self.num_indices > 0 {
            rpass.set_pipeline(&self.quad_pipeline.pipeline);
            rpass.set_bind_group(0, &self.quad_pipeline.bind_group, &[]);
            rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..self.num_indices, 0, 0..1);
        }
    }

    pub fn render_grid<'rpass>(&'rpass self, rpass: &mut wgpu::RenderPass<'rpass>) {
        rpass.set_pipeline(&self.grid_pipeline.pipeline);
        rpass.set_bind_group(0, &self.grid_pipeline.bind_group, &[]);
        rpass.draw(0..6, 0..1);
    }
}

pub struct RenderPipelineContainer {
    pub pipeline: wgpu::RenderPipeline,
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

impl RenderPipelineContainer {
    pub fn new<U: bytemuck::Pod + bytemuck::Zeroable>(
        device: &wgpu::Device,
        label: &str,
        shader_src: &str,
        vertex_buffers: &[wgpu::VertexBufferLayout],
        surface_format: wgpu::TextureFormat,
        initial_uniforms: U,
        visibility: wgpu::ShaderStages,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{}_ub", label)),
            contents: bytemuck::bytes_of(&initial_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("{}_bgl", label)),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{}_bg", label)),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&format!("{}_layout", label)),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: vertex_buffers,
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
            bind_group,
        }
    }
}
