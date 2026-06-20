mod bflyt_view;
mod camera;
mod renderer;
mod ui;

use std::{path::PathBuf, sync::Arc};

use camera::Camera;
use egui_chinese_font::{FontError, setup_chinese_fonts};
use egui_wgpu::{RendererOptions, ScreenDescriptor};
use nnbfl::{bflyt::file::Bflyt, core::ReadWriteable, sarc::file::Sarc};
use pollster::FutureExt;
use renderer::quad::QuadRenderer;
use renderer::texture::TextureCache;
use renderer::textured_quad::TexturedQuadRenderer;
use wgpu::CurrentSurfaceTexture;
use winit::{
    application::ApplicationHandler,
    event::{MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

use bflyt_view::{BflytView, build_view};
use ui::{UiState, draw_ui};

use crate::{
    bflyt_view::ResolvedBlarc,
    ui::{SUPPORTED_SARC_EXTENSIONS, UiAction},
};

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    quad_renderer: QuadRenderer,
    textured_quad_renderer: Option<TexturedQuadRenderer>,
    texture_cache: TextureCache,
    egui_renderer: egui_wgpu::Renderer,
}

impl GpuState {
    fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });

        let surface = instance.create_surface(window).expect("create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .block_on()
            .expect("find adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            })
            .block_on()
            .expect("create device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let mut quad_renderer = QuadRenderer::new(&device, surface_format);
        quad_renderer.upload_quads(&device, &[]);

        let texture_cache = TextureCache::new(&device);

        let egui_renderer =
            egui_wgpu::Renderer::new(&device, surface_format, RendererOptions::default());
        let textured_quad_renderer = None; // created after textures load

        Self {
            surface,
            device,
            queue,
            config,
            quad_renderer,
            textured_quad_renderer,
            texture_cache,
            egui_renderer,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    fn render(
        &mut self,
        window: &Window,
        egui_ctx: &egui::Context,
        egui_state: &mut egui_winit::State,
        bflyt_view: &Option<BflytView>,
        ui_state: &mut UiState,
        camera: &Camera,
    ) {
        self.quad_renderer
            .update_projection(&self.queue, camera, &self.config);
        let matrix = camera.build_matrix(self.config.width as f32, self.config.height as f32);
        if let Some(tqr) = &self.textured_quad_renderer {
            tqr.update_projection(&self.queue, matrix);
        }

        let mut scissor_rect = None;
        if let Some(view) = bflyt_view
            && ui_state.clip_to_root
        {
            let screen_w = self.config.width as f32;
            let screen_h = self.config.height as f32;

            let scale_x = matrix[0][0];
            let scale_y = matrix[1][1];
            let trans_x = matrix[3][0];
            let trans_y = matrix[3][1];

            let ndc_x0 = trans_x;
            let ndc_y0 = trans_y;
            let ndc_x1 = view.layout_width * scale_x + trans_x;
            let ndc_y1 = view.layout_height * scale_y + trans_y;

            let x0 = ((ndc_x0 + 1.0) * 0.5 * screen_w).clamp(0.0, screen_w);
            let y0 = ((1.0 - ndc_y0) * 0.5 * screen_h).clamp(0.0, screen_h);
            let x1 = ((ndc_x1 + 1.0) * 0.5 * screen_w).clamp(0.0, screen_w);
            let y1 = ((1.0 - ndc_y1) * 0.5 * screen_h).clamp(0.0, screen_h);

            let sx = x0.min(x1) as u32;
            let sy = y0.min(y1) as u32;
            let sw = (x0 - x1).abs() as u32;
            let sh = (y0 - y1).abs() as u32;

            if sw > 0 && sh > 0 {
                scissor_rect = Some((sx, sy, sw, sh));
            }
        }

        let output = match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(o) => o,
            CurrentSurfaceTexture::Lost | CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            _ => {
                log::error!("Unknown surface error");
                return;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let raw_input = egui_state.take_egui_input(window);
        let full_output = egui_ctx.run_ui(raw_input, |ui| {
            draw_ui(
                ui,
                bflyt_view,
                ui_state,
                camera,
                self.config.width as f32,
                self.config.height as f32,
            );
        });

        egui_state.handle_platform_output(window, full_output.platform_output.clone());

        if let Some(bflyt_view) = bflyt_view {
            self.quad_renderer.update_selection(
                &self.queue,
                &bflyt_view.quads,
                ui_state.selected_pane,
                &ui_state.hidden_panes,
            );

            if let Some(tqr) = &mut self.textured_quad_renderer {
                tqr.update_selection(
                    &self.queue,
                    &bflyt_view.textured_quads,
                    ui_state.selected_pane,
                    &ui_state.hidden_panes,
                );
            }
        }

        let paint_jobs = egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: full_output.pixels_per_point,
        };

        for (id, delta) in &full_output.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, delta);
        }
        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        let mut render_encoder = self.device.create_command_encoder(&Default::default());
        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut render_encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        {
            let mut rpass = render_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.10,
                            g: 0.10,
                            b: 0.12,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            self.quad_renderer.render_grid(&mut rpass);

            if let Some((sx, sy, sw, sh)) = scissor_rect {
                rpass.set_scissor_rect(sx, sy, sw, sh);
            }

            if !ui_state.only_textured {
                self.quad_renderer.render(&mut rpass);
            }

            if let Some(tqr) = &self.textured_quad_renderer {
                tqr.render(&mut rpass);
            }

            if scissor_rect.is_some() {
                rpass.set_scissor_rect(0, 0, self.config.width, self.config.height);
            }

            let mut rpass = rpass.forget_lifetime();
            self.egui_renderer
                .render(&mut rpass, &paint_jobs, &screen_descriptor);
        }

        self.queue.submit(std::iter::once(render_encoder.finish()));
        output.present();
    }
}

struct App {
    bflyt_path: Option<PathBuf>,
    bflyt_view: Option<BflytView>,
    blarc_dir: Option<PathBuf>,
    blarc_textures_loaded: bool,
    ui_state: UiState,
    camera: Camera,
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    gpu: Option<GpuState>,
    window: Option<Arc<Window>>,
}

impl App {
    fn new() -> Self {
        Self {
            bflyt_path: None,
            bflyt_view: None,
            blarc_dir: None,
            blarc_textures_loaded: false,
            ui_state: UiState::default(),
            camera: Camera::new(),
            egui_ctx: egui::Context::default(),
            egui_state: None,
            gpu: None,
            window: None,
        }
    }

    fn load_file(&mut self) {
        let Some(bflyt_path) = &self.bflyt_path else {
            return;
        };

        let bytes = std::fs::read(bflyt_path).expect("read bflyt file");
        let title_path = bflyt_path.file_name().unwrap().to_string_lossy();

        let res = ResolvedBlarc {
            bflyt_bytes: bytes,
            bntx_bytes: None,
        };

        self.load_file_from_buffer(res, title_path.to_string());
    }

    fn load_file_from_buffer(&mut self, res: ResolvedBlarc, file_name: String) {
        let file = Bflyt::parse(&res.bflyt_bytes).expect("parse bflyt file");
        let mut view = build_view(&file, self.blarc_dir.as_deref());

        if let Some(root_bntx) = res.bntx_bytes {
            view.discovered_bntx_buffers.push(root_bntx);
        }

        self.camera.zoom = 1.0;
        self.camera.offset = [0.0, 0.0];
        self.bflyt_view = Some(view);

        if let Some(gpu) = &mut self.gpu {
            let view = self.bflyt_view.as_ref().unwrap();
            gpu.quad_renderer.upload_quads(&gpu.device, &view.quads);

            for bntx_bytes in &view.discovered_bntx_buffers {
                gpu.texture_cache
                    .load_from_bntx_bytes(&gpu.device, &gpu.queue, bntx_bytes);
            }

            let mut tgr = TexturedQuadRenderer::new(&gpu.device, gpu.config.format);

            tgr.upload_quads(&gpu.device, &view.textured_quads, &gpu.texture_cache);
            gpu.textured_quad_renderer = Some(tgr);

            if let Some(window) = &self.window {
                let size = window.inner_size();
                self.camera.fit(
                    view.layout_width,
                    view.layout_height,
                    size.width as f32,
                    size.height as f32,
                );

                window.set_title(&format!("nnbfl-preview - {file_name}"));
            }
        }

        log::info!(
            "Loaded {} panes from {file_name:?}",
            self.bflyt_view.as_ref().unwrap().panes.len(),
        );
    }

    fn extract_buffer_from_sarc(&self, path: &PathBuf) -> Option<(String, ResolvedBlarc)> {
        let mut file_bytes = std::fs::read(path).ok()?;
        let filename = path.file_name()?.to_string_lossy();

        file_bytes = decompress_if_needed(file_bytes, &filename);

        let mut bflyt_name = "unnamed.bflyt".to_string();
        let mut resolved = ResolvedBlarc {
            bflyt_bytes: Vec::new(),
            bntx_bytes: None,
        };

        unpack_sarc_recursive(&file_bytes, &mut bflyt_name, &mut resolved);

        if resolved.bflyt_bytes.is_empty() {
            return None;
        }

        Some((bflyt_name, resolved))
    }
}

pub fn unpack_sarc_recursive(data: &[u8], bflyt_name: &mut String, resolved: &mut ResolvedBlarc) {
    let Ok(sarc) = Sarc::parse(data) else {
        return;
    };

    for file in sarc.files {
        let Some(name) = &file.name else {
            continue;
        };

        let file_data = decompress_if_needed(file.data.clone(), name);
        let name_lower = name.to_lowercase();

        let clean_name = if name_lower.ends_with(".zs") {
            &name_lower[..name_lower.len() - 3]
        } else {
            &name_lower
        };

        if clean_name.ends_with(".bflyt") {
            if resolved.bflyt_bytes.is_empty() {
                *bflyt_name = name.clone();
                resolved.bflyt_bytes = file_data;
            }
        } else if clean_name.ends_with(".bntx") || clean_name.contains("__combined") {
            if resolved.bntx_bytes.is_none() {
                resolved.bntx_bytes = Some(file_data);
            }
        } else {
            let is_nested_sarc = clean_name.ends_with(".arc")
                || SUPPORTED_SARC_EXTENSIONS
                    .iter()
                    .any(|ext| clean_name.ends_with(&format!(".{}", ext.to_lowercase())));

            if is_nested_sarc {
                unpack_sarc_recursive(&file_data, bflyt_name, resolved);
            }
        }
    }
}

fn decompress_if_needed(data: Vec<u8>, filename: &str) -> Vec<u8> {
    if filename.to_lowercase().ends_with(".zs") {
        let mut decompressed = Vec::new();

        if tomolib::formats::zs::decompress(&data[..], &mut decompressed).is_ok() {
            return decompressed;
        } else {
            log::error!("Failed to decompress Zstd file: {filename}");
        }
    }

    data
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let title_path = if let Some(bflyt_path) = &self.bflyt_path {
            bflyt_path.file_name().unwrap().to_string_lossy()
        } else {
            "No file loaded".into()
        };

        let window = Arc::new(
            event_loop
                .create_window(
                    winit::window::WindowAttributes::default()
                        .with_title(format!("nnbfl-preview - {title_path}"))
                        .with_inner_size(winit::dpi::LogicalSize::new(1280u32, 720u32)),
                )
                .expect("create window"),
        );

        let egui_state = egui_winit::State::new(
            self.egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            None,
            None,
            None,
        );

        if let Err(err) = setup_chinese_fonts(&self.egui_ctx) {
            match err {
                FontError::NotFound(e) => log::warn!("CJK font not found: {e}"),
                FontError::ReadError(e) => log::warn!("CJK font read error: {e}"),
                FontError::UnsupportedPlatform => log::warn!("CJK font platform unsupported"),
            }
        };

        self.load_file();

        let size = window.inner_size();
        self.camera
            .fit(1280.0, 720.0, size.width as f32, size.height as f32);

        let gpu = GpuState::new(window.clone());

        self.egui_state = Some(egui_state);
        self.gpu = Some(gpu);
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if let (Some(state), Some(window)) = (&mut self.egui_state, &self.window) {
            let _ = state.on_window_event(window, &event);
        }

        match &event {
            WindowEvent::CursorMoved { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::KeyboardInput { .. } => {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }

        let egui_wants_pointer = self.egui_ctx.egui_wants_pointer_input();
        let egui_wants_scroll = self.egui_ctx.egui_wants_pointer_input();

        if let Some(action) = self.ui_state.pending_action.take() {
            match action {
                UiAction::SetBlarcDir(dir) => {
                    self.blarc_dir = Some(dir);
                    self.blarc_textures_loaded = false;
                    if let Some(path) = self.bflyt_path.clone() {
                        let bytes = std::fs::read(&path).ok();
                        if let Some(bytes) = bytes {
                            let name = path.file_name().unwrap().to_string_lossy().to_string();

                            let res = ResolvedBlarc {
                                bflyt_bytes: bytes,
                                bntx_bytes: None,
                            };

                            self.load_file_from_buffer(res, name);
                        }
                    }
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }

                UiAction::LoadFile(path) => {
                    let path_str = path.to_string_lossy().to_lowercase();

                    let is_sarc = SUPPORTED_SARC_EXTENSIONS
                        .iter()
                        .any(|ext| path_str.ends_with(&format!(".{}", ext.to_lowercase())));

                    if is_sarc {
                        if let Some((name, data)) = self.extract_buffer_from_sarc(&path) {
                            self.load_file_from_buffer(data, name);
                        }
                    } else {
                        self.bflyt_path = Some(path);
                        self.load_file();
                    }

                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::CursorMoved { position, .. } => {
                let pos = [position.x as f32, position.y as f32];
                self.camera.cursor_screen = pos;

                if self.camera.is_panning && !egui_wants_pointer {
                    self.camera.pan(pos);
                }

                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::MouseInput {
                state,
                button: MouseButton::Middle,
                ..
            } => match state {
                winit::event::ElementState::Pressed => {
                    if !egui_wants_pointer {
                        self.camera.start_pan(self.camera.cursor_screen);
                    }
                }
                winit::event::ElementState::Released => self.camera.end_pan(),
            },

            WindowEvent::MouseWheel { delta, .. } if !egui_wants_scroll => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.01,
                };

                self.camera.zoom_around_cursor(lines);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(size);
                }

                if let Some(view) = &self.bflyt_view {
                    self.camera.fit(
                        view.layout_width,
                        view.layout_height,
                        size.width as f32,
                        size.height as f32,
                    );
                }
            }

            WindowEvent::DroppedFile(path) => {
                if path.extension().and_then(|s| s.to_str()) == Some("bflyt") {
                    self.bflyt_path = Some(path);
                    self.load_file();
                } else {
                    self.ui_state.error_message =
                        Some("Invalid file type. Please drop a .bflyt file".to_string());
                }
            }

            WindowEvent::RedrawRequested => {
                if let (Some(gpu), Some(window), Some(egui_state)) =
                    (&mut self.gpu, &self.window, &mut self.egui_state)
                {
                    gpu.render(
                        window,
                        &self.egui_ctx,
                        egui_state,
                        &self.bflyt_view,
                        &mut self.ui_state,
                        &self.camera,
                    );
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.egui_ctx.has_requested_repaint() {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().expect("create event loop");
    let mut app = App::new();

    event_loop.run_app(&mut app).expect("run event loop");
}
