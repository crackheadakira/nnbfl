use crate::config::OutputGamma;

pub struct Canvas {
    texture: wgpu::Texture,
    target: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    surface_srgb: bool,

    pub gamma: OutputGamma,
}

impl Canvas {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        (width, height): (u32, u32),
        gamma: OutputGamma,
    ) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("canvas_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("canvas"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/canvas.wgsl").into()),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("canvas_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("canvas_copy"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let (texture, target, bind_group) = Self::create_texture(
            device,
            &bind_group_layout,
            (width, height),
            gamma,
            surface_format.is_srgb(),
        );

        Self {
            texture,
            target,
            bind_group,
            bind_group_layout,
            pipeline,
            surface_srgb: surface_format.is_srgb(),
            gamma,
        }
    }

    pub fn format(gamma: OutputGamma) -> wgpu::TextureFormat {
        match gamma {
            OutputGamma::Unorm => wgpu::TextureFormat::Rgba8Unorm,
            OutputGamma::Srgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        }
    }

    pub fn clear_color(&self, color: wgpu::Color) -> wgpu::Color {
        match self.gamma {
            OutputGamma::Srgb => color,
            OutputGamma::Unorm => wgpu::Color {
                r: encode_srgb(color.r),
                g: encode_srgb(color.g),
                b: encode_srgb(color.b),
                a: color.a,
            },
        }
    }

    pub fn target(&self) -> &wgpu::TextureView {
        &self.target
    }

    pub fn resize(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        self.rebuild(device, size, self.gamma);
    }

    pub fn set_gamma(&mut self, device: &wgpu::Device, gamma: OutputGamma) {
        let size = (self.texture.width(), self.texture.height());
        self.rebuild(device, size, gamma);
    }

    pub fn copy_to_surface<'pass>(&'pass self, rpass: &mut wgpu::RenderPass<'pass>) {
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }

    fn rebuild(&mut self, device: &wgpu::Device, size: (u32, u32), gamma: OutputGamma) {
        let (texture, target, bind_group) = Self::create_texture(
            device,
            &self.bind_group_layout,
            size,
            gamma,
            self.surface_srgb,
        );

        self.texture = texture;
        self.target = target;
        self.bind_group = bind_group;
        self.gamma = gamma;
    }

    fn create_texture(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        (width, height): (u32, u32),
        gamma: OutputGamma,
        surface_srgb: bool,
    ) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("canvas"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
        });

        let view = |format: wgpu::TextureFormat| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(format),
                ..Default::default()
            })
        };

        let source = view(if surface_srgb {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("canvas_bg"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&source),
            }],
        });

        let target = view(Self::format(gamma));
        (texture, target, bind_group)
    }
}

fn encode_srgb(c: f64) -> f64 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}
