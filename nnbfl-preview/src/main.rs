mod bflyt_view;
mod camera;
mod renderer;
mod ui;

use std::{path::PathBuf, sync::Arc};

use camera::Camera;
use egui_wgpu::ScreenDescriptor;
use nnbfl::{bflyt::file::Bflyt, core::ReadWriteable};
use pollster::FutureExt;
use renderer::quad::QuadRenderer;
use winit::{
    application::ApplicationHandler,
    event::{MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

use bflyt_view::{BflytView, build_view};
use ui::{UiState, draw_ui};

fn parse_args() -> PathBuf {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: nnbfl-preview <file.bflyt>");
        std::process::exit(1);
    }
    PathBuf::from(&args[1])
}

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    quad_renderer: QuadRenderer,
    egui_renderer: egui_wgpu::Renderer,
}

impl GpuState {
    fn new(window: Arc<Window>, view: &BflytView) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
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
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: Default::default(),
                },
                None,
            )
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
        quad_renderer.upload_quads(&device, &view.quads);

        let egui_renderer = egui_wgpu::Renderer::new(&device, surface_format, None, 1, false);

        Self {
            surface,
            device,
            queue,
            config,
            quad_renderer,
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
        bflyt_view: &BflytView,
        ui_state: &mut UiState,
        camera: &Camera,
    ) {
        let matrix = camera.build_matrix(self.config.width as f32, self.config.height as f32);
        self.quad_renderer.update_projection(&self.queue, matrix);

        let output = match self.surface.get_current_texture() {
            Ok(o) => o,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Err(e) => {
                log::error!("Surface error: {:?}", e);
                return;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let raw_input = egui_state.take_egui_input(window);
        let full_output = egui_ctx.run(raw_input, |ctx| {
            draw_ui(ctx, bflyt_view, ui_state);
        });
        egui_state.handle_platform_output(window, full_output.platform_output.clone());

        self.quad_renderer.update_selection(
            &self.queue,
            &bflyt_view.quads,
            ui_state.selected_pane,
            &ui_state.hidden_panes,
        );

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
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.quad_renderer.render(&mut rpass);

            let mut rpass = rpass.forget_lifetime();
            self.egui_renderer
                .render(&mut rpass, &paint_jobs, &screen_descriptor);
        }

        self.queue.submit(std::iter::once(render_encoder.finish()));
        output.present();
    }
}

struct App {
    bflyt_path: PathBuf,
    bflyt_view: Option<BflytView>,
    ui_state: UiState,
    camera: Camera,
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    gpu: Option<GpuState>,
    window: Option<Arc<Window>>,
}

impl App {
    fn new(path: PathBuf) -> Self {
        Self {
            bflyt_path: path,
            bflyt_view: None,
            ui_state: UiState::default(),
            camera: Camera::new(),
            egui_ctx: egui::Context::default(),
            egui_state: None,
            gpu: None,
            window: None,
        }
    }

    fn load_file(&mut self) {
        let bytes = std::fs::read(&self.bflyt_path).expect("read bflyt file");
        let file = Bflyt::parse(&bytes).expect("parse bflyt file");
        let view = build_view(&file);
        self.camera.zoom = 1.0;
        self.camera.offset = [0.0, 0.0];
        self.bflyt_view = Some(view);

        log::info!(
            "Loaded {} panes from {:?}",
            self.bflyt_view.as_ref().unwrap().panes.len(),
            self.bflyt_path
        );
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        self.load_file();
        let bflyt_view = self.bflyt_view.as_ref().unwrap();

        let window = Arc::new(
            event_loop
                .create_window(
                    winit::window::WindowAttributes::default()
                        .with_title(format!(
                            "nnbfl-preview - {}",
                            self.bflyt_path.file_name().unwrap().to_string_lossy()
                        ))
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

        let size = window.inner_size();
        self.camera.fit(
            bflyt_view.layout_width,
            bflyt_view.layout_height,
            size.width as f32,
            size.height as f32,
        );

        let gpu = GpuState::new(window.clone(), bflyt_view);

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

        let egui_wants_pointer = self.egui_ctx.wants_pointer_input();
        let egui_wants_scroll = self.egui_ctx.wants_pointer_input();

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::CursorMoved { position, .. } => {
                let pos = [position.x as f32, position.y as f32];
                self.camera.cursor_screen = pos;

                if self.camera.is_panning && !egui_wants_pointer {
                    self.camera.pan(pos);

                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Middle {
                    match state {
                        winit::event::ElementState::Pressed => {
                            if !egui_wants_pointer {
                                self.camera.start_pan(self.camera.cursor_screen);
                            }
                        }
                        winit::event::ElementState::Released => self.camera.end_pan(),
                    }
                }
            }

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
                if let (Some(gpu), Some(view)) = (&mut self.gpu, &self.bflyt_view) {
                    gpu.resize(size);
                    self.camera.fit(
                        view.layout_width,
                        view.layout_height,
                        size.width as f32,
                        size.height as f32,
                    );
                }
            }

            WindowEvent::RedrawRequested => {
                if let (Some(gpu), Some(window), Some(egui_state), Some(view)) = (
                    &mut self.gpu,
                    &self.window,
                    &mut self.egui_state,
                    &self.bflyt_view,
                ) {
                    gpu.render(
                        window,
                        &self.egui_ctx,
                        egui_state,
                        view,
                        &mut self.ui_state,
                        &self.camera,
                    );
                    window.request_redraw();
                }
            }

            _ => {}
        }
    }
}

fn main() {
    env_logger::init();
    let path = parse_args();
    let event_loop = EventLoop::new().expect("create event loop");
    let mut app = App::new(path);
    event_loop.run_app(&mut app).expect("run event loop");
}
