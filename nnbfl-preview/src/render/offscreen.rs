use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use pollster::FutureExt;
use thiserror::Error;

use super::frame::{FrameView, prepare_frame};
use crate::{
    LayoutData,
    camera::Camera,
    font::glyph_atlas::GlyphData,
    renderer::{texture::TextureCache, textured_quad::PaneRenderer},
    ui::general::PaneVisibilityFlags,
};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

#[derive(Error, Debug)]
pub enum RenderError {
    #[error("No usable GPU adapter: {0}")]
    Adapter(String),
    #[error("Creating the GPU device failed: {0}")]
    Device(String),
    #[error("Reading the rendered image back failed: {0}")]
    Readback(String),
    #[error("Writing {path:?} failed: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DeviceOptions {
    pub software: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Pose {
    pub anim: Option<usize>,
    pub frame: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct RenderSettings {
    pub scale: f32,
    /// sRGB-encoded RGBA
    pub background: [f32; 4],
    pub flags: PaneVisibilityFlags,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            scale: 1.0,
            background: [0.0; 4],
            flags: PaneVisibilityFlags {
                only_textured: true,
                ..Default::default()
            },
        }
    }
}

/// Frames from `start` to `end` (the animation's last frame when `None`) every `step`, the end
/// always included
#[derive(Debug, Clone, Copy)]
pub struct FrameRange {
    pub start: f32,
    pub end: Option<f32>,
    pub step: f32,
}

impl FrameRange {
    pub fn frames(&self, frame_count: f32) -> Vec<f32> {
        let end = self.end.unwrap_or(frame_count).max(self.start);
        let step = if self.step > 0.0 { self.step } else { 1.0 };

        let mut frames: Vec<f32> = (0..)
            .map(|i| self.start + i as f32 * step)
            .take_while(|&f| f <= end + f32::EPSILON)
            .collect();

        if frames
            .last()
            .is_none_or(|&last| (end - last).abs() > f32::EPSILON)
        {
            frames.push(end);
        }

        frames
    }
}

pub struct Image {
    pub width: u32,
    pub height: u32,
    /// sRGB-encoded, alpha not premultiplied
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn write_png(&self, path: &Path) -> Result<(), RenderError> {
        let write_error = |source| RenderError::Write {
            path: path.to_path_buf(),
            source,
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(write_error)?;
        }

        let file = std::fs::File::create(path).map_err(write_error)?;
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);

        let png_error = |e: png::EncodingError| write_error(std::io::Error::other(e));
        let mut writer = encoder.write_header().map_err(png_error)?;
        writer.write_image_data(&self.rgba).map_err(png_error)
    }
}

pub struct OffscreenRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pane_renderer: PaneRenderer,
    texture_cache: TextureCache,
}

impl OffscreenRenderer {
    pub fn new(options: DeviceOptions) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: options.software,
            })
            .block_on()
            .map_err(|e| RenderError::Adapter(e.to_string()))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("offscreen"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            })
            .block_on()
            .map_err(|e| RenderError::Device(e.to_string()))?;

        let pane_renderer = PaneRenderer::new(&device, &queue, FORMAT);

        Ok(Self {
            device,
            queue,
            pane_renderer,
            texture_cache: TextureCache::new(),
        })
    }

    pub fn prepare(&mut self, layout: &mut LayoutData, glyphs: &mut GlyphData) {
        self.texture_cache.clear();

        for bntx in layout.tree.all_bntxs() {
            self.texture_cache
                .load_from_bntx(&self.device, &self.queue, bntx);
        }

        glyphs.atlas.mark_dirty();
        glyphs
            .atlas
            .upload(&self.device, &self.queue, &mut self.texture_cache);

        let layout_size = layout.tree.layout_size;
        let quads = layout.tree.collect_render_quads();

        self.pane_renderer.upload_quads(
            &self.device,
            &quads,
            &self.texture_cache,
            layout_size,
            &RenderSettings::default().flags,
        );

        layout.tree.take_rebatch();
    }

    pub fn render(
        &mut self,
        layout: &mut LayoutData,
        glyphs: &mut GlyphData,
        pose: &Pose,
        settings: &RenderSettings,
    ) -> Result<Image, RenderError> {
        layout.timeline.anim_player.pose(pose.anim, pose.frame);
        glyphs
            .atlas
            .upload(&self.device, &self.queue, &mut self.texture_cache);

        prepare_frame(
            &mut self.pane_renderer,
            &self.device,
            &self.queue,
            &self.texture_cache,
            layout,
            &FrameView {
                selected: None,
                hidden: &HashSet::new(),
                flags: settings.flags,
            },
        );

        let size = layout.tree.layout_size;
        let width = ((size.x * settings.scale).round() as u32).max(1);
        let height = ((size.y * settings.scale).round() as u32).max(1);

        let mut camera = Camera::new();
        camera.zoom = settings.scale;
        camera.offset = [0.0, 0.0];
        self.pane_renderer.update_projection(
            &self.queue,
            camera.build_matrix(width as f32, height as f32),
        );

        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen_target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = target.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("offscreen_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color(settings.background)),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            self.pane_renderer.render(&mut rpass);
        }

        self.read_back(encoder, &target, width, height)
    }

    fn read_back(
        &self,
        mut encoder: wgpu::CommandEncoder,
        target: &wgpu::Texture,
        width: u32,
        height: u32,
    ) -> Result<Image, RenderError> {
        let row = width * 4;
        let padded_row = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);

        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("offscreen_readback"),
            size: u64::from(padded_row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(std::iter::once(encoder.finish()));

        let (sender, receiver) = std::sync::mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = sender.send(result);
        });

        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .map_err(|e| RenderError::Readback(e.to_string()))?;
        receiver
            .recv()
            .map_err(|e| RenderError::Readback(e.to_string()))?
            .map_err(|e| RenderError::Readback(e.to_string()))?;

        let mut rgba = Vec::with_capacity((row * height) as usize);
        {
            let mapped = buffer.get_mapped_range(..);
            for chunk in mapped.chunks(padded_row as usize) {
                rgba.extend_from_slice(&chunk[..row as usize]);
            }
        }

        buffer.unmap();

        Ok(Image {
            width,
            height,
            rgba,
        })
    }
}

fn clear_color([r, g, b, a]: [f32; 4]) -> wgpu::Color {
    let linear = |c: f32| {
        let c = f64::from(c);

        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };

    wgpu::Color {
        r: linear(r),
        g: linear(g),
        b: linear(b),
        a: f64::from(a),
    }
}

pub fn export_animation(
    renderer: &mut OffscreenRenderer,
    layout: &mut LayoutData,
    glyphs: &mut GlyphData,
    anim: usize,
    range: &FrameRange,
    settings: &RenderSettings,
    dir: &Path,
) -> Result<Vec<PathBuf>, RenderError> {
    let frame_count = layout
        .timeline
        .anim_player
        .anims
        .get(anim)
        .map_or(0.0, |a| a.frame_count());

    let frames = range.frames(frame_count);
    let digits = frames
        .last()
        .map_or(1, |last| (last.trunc() as u32).to_string().len());

    let mut written = Vec::with_capacity(frames.len());

    for frame in frames {
        let image = renderer.render(
            layout,
            glyphs,
            &Pose {
                anim: Some(anim),
                frame,
            },
            settings,
        )?;

        let path = dir.join(format!("{}.png", frame_name(frame, digits)));
        image.write_png(&path)?;
        written.push(path);
    }

    Ok(written)
}

fn frame_name(frame: f32, digits: usize) -> String {
    let whole = frame.trunc() as u32;
    let hundredths = (frame.fract() * 100.0).round() as u32;

    match hundredths {
        0 => format!("{whole:0digits$}"),
        _ => format!("{whole:0digits$}_{hundredths:02}"),
    }
}
