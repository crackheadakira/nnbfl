mod anim_state;
mod archive;
mod archive_browser;
mod camera;
mod chinese_font;
mod cli;
mod edit_history;
mod error;
mod font;
mod game_config;
mod keybinds;
mod pane_factory;
mod pane_tree;
mod render;
mod renderer;
mod traits;
mod ui;

use std::{collections::HashMap, path::Path, sync::Arc, time::Instant};

use camera::Camera;
use chinese_font::{FontError, setup_chinese_fonts};
use egui_wgpu::{RendererOptions, ScreenDescriptor};
use nnbfl::{
    bflan::file::Bflan,
    bflyt::{file::Bflyt, list::Layout},
    core::{Endianness, FileReadWriteable, VersionFormat},
    sarc::file::{MagicFiles, Sarc, SarcFile},
    ui2d::types::{Vector2f, Vector3f},
};
use pollster::FutureExt;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use renderer::quad::GridRenderer;
use renderer::texture::TextureCache;
use renderer::textured_quad::PaneRenderer;
use tomolib::formats::bntx::Bntx;
use wgpu::CurrentSurfaceTexture;
use winit::{
    application::ApplicationHandler,
    event::{MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

use crate::{
    archive::{DocStore, EntryPath, LayoutSource, anim_owner, file_stem},
    archive_browser::{ArchiveEntry, ArchiveScan},
    edit_history::{AnimSession, EditCtx, EditHistory},
    error::LoadError,
    font::glyph_atlas::{GLYPH_ATLAS_TEXTURE_NAME, GlyphData},
    pane_tree::{Anchor, PaneId, PaneTree},
    renderer::{
        selection::{Handle, SelectionRenderer, point_in_quad},
        texture::{TexturePreviewData, TexturePreviewPipeline},
    },
    traits::Displaying,
    ui::{
        general::{SUPPORTED_SARC_EXTENSIONS, UiAction, UiState, draw_ui},
        timeline::TimelineState,
    },
};

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,

    grid_renderer: GridRenderer,
    pane_renderer: PaneRenderer,
    selection_renderer: SelectionRenderer,
    egui_renderer: egui_wgpu::Renderer,

    texture_cache: TextureCache,
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

        let grid_renderer = GridRenderer::new(&device, surface_format);

        let selection_renderer = SelectionRenderer::new(&device, surface_format);

        let texture_cache = TextureCache::new();
        let pane_renderer = PaneRenderer::new(&device, &queue, surface_format);

        let mut egui_renderer =
            egui_wgpu::Renderer::new(&device, surface_format, RendererOptions::default());

        let preview_pipeline = TexturePreviewPipeline::new(&device, surface_format);
        egui_renderer.callback_resources.insert(TexturePreviewData {
            pipeline: preview_pipeline,
            bind_groups: HashMap::new(),
        });

        Self {
            surface,
            device,
            queue,
            config,
            grid_renderer,
            pane_renderer,
            texture_cache,
            egui_renderer,
            selection_renderer,
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
        mut ctx: RenderContext<'_>,
    ) {
        puffin::GlobalProfiler::lock().new_frame();
        puffin::profile_function!();

        if ctx.layout_tabs.glyphs.atlas.is_dirty() {
            ctx.layout_tabs
                .glyphs
                .atlas
                .upload(&self.device, &self.queue, &mut self.texture_cache);
        };

        self.grid_renderer
            .update_projection(&self.queue, ctx.camera, &self.config);

        let matrix = ctx
            .camera
            .build_matrix(self.config.width as f32, self.config.height as f32);
        self.pane_renderer.update_projection(&self.queue, matrix);
        self.selection_renderer
            .update_projection(&self.queue, matrix);

        let mut scissor_rect = None;
        if let Some(layout) = ctx.layout_tabs.active()
            && ctx.ui_state.visiblity_flags.clip_to_root
        {
            puffin::profile_scope!("render_scissor_calc");
            let screen_w = self.config.width as f32;
            let screen_h = self.config.height as f32;

            let scale_x = matrix[0][0];
            let scale_y = matrix[1][1];
            let trans_x = matrix[3][0];
            let trans_y = matrix[3][1];

            let ndc_x0 = trans_x;
            let ndc_y0 = trans_y;
            let ndc_x1 = layout.tree.layout_size.x * scale_x + trans_x;
            let ndc_y1 = layout.tree.layout_size.y * scale_y + trans_y;

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

        let surface_texture = {
            puffin::profile_scope!("wgpu_surface_wait_for_vsync");
            self.surface.get_current_texture()
        };

        let output = match surface_texture {
            CurrentSurfaceTexture::Success(o) => o,
            CurrentSurfaceTexture::Suboptimal(o) => {
                self.surface.configure(&self.device, &self.config);
                o
            }

            CurrentSurfaceTexture::Lost | CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return;
            }

            CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => return,
            CurrentSurfaceTexture::Validation => {
                log::error!("Surface validation error");
                return;
            }
        };

        let view = {
            puffin::profile_scope!("surface_view");

            output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default())
        };

        let raw_input = egui_state.take_egui_input(window);
        let full_output = egui_ctx.run_ui(raw_input, |ui| {
            draw_ui(
                ui,
                &mut ctx,
                self.config.width as f32,
                self.config.height as f32,
            );
        });

        egui_state.handle_platform_output(window, full_output.platform_output.clone());

        let editing =
            egui_ctx.input(|i| i.pointer.any_down()) || egui_ctx.egui_wants_keyboard_input();

        if !editing && let Some(layout) = ctx.layout_tabs.active_mut() {
            layout.commit_anim_session();
        }

        if let Some(layout) = ctx.layout_tabs.active_mut()
            && let Some(pending_key_edit) = layout.timeline.pending_key_edit.take()
        {
            puffin::profile_scope!("pending_key_edit");
            layout.begin_anim_session();
            layout
                .timeline
                .anim_player
                .apply_key_edit(&pending_key_edit);
        }

        if let Some(layout) = ctx.layout_tabs.active_mut()
            && let Some(pending_slope_edit) = layout.timeline.pending_slope_edit.take()
        {
            puffin::profile_scope!("pending_slope_edit");
            layout.begin_anim_session();
            layout
                .timeline
                .anim_player
                .apply_slope_edit(&pending_slope_edit);
        }

        if let Some(layout) = ctx.layout_tabs.active_mut() {
            puffin::profile_scope!("pane_renderer_logic");

            render::prepare_frame(
                &mut self.pane_renderer,
                &self.device,
                &self.queue,
                &self.texture_cache,
                layout,
                &render::FrameView {
                    selected: &ctx.ui_state.pane_tree_view.selection,
                    hidden: &ctx.ui_state.pane_tree_view.hidden_panes,
                    flags: ctx.ui_state.visiblity_flags,
                },
            );

            match ctx
                .ui_state
                .pane_tree_view
                .selected_pane
                .and_then(|idx| layout.tree.find_by_idx(idx))
            {
                Some(node) => self.selection_renderer.update(
                    &self.device,
                    &node.world_corners,
                    &node.handle_capabilities,
                ),
                None => self.selection_renderer.clear(),
            }
        }

        let paint_jobs = egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: full_output.pixels_per_point,
        };

        for (id, delta) in &full_output.textures_delta.set {
            puffin::profile_scope!("egui_update_texture");
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, delta);
        }

        for id in &full_output.textures_delta.free {
            puffin::profile_scope!("egui_free_texture");
            self.egui_renderer.free_texture(id);
        }

        let mut render_encoder = self.device.create_command_encoder(&Default::default());

        {
            puffin::profile_scope!("egui_buffer_update");
            self.egui_renderer.update_buffers(
                &self.device,
                &self.queue,
                &mut render_encoder,
                &paint_jobs,
                &screen_descriptor,
            );
        }

        if let Some(preview_data) = self
            .egui_renderer
            .callback_resources
            .get_mut::<TexturePreviewData>()
            && let Some(ref name) = ctx.ui_state.texture_editor.selected_texture
            && (!preview_data.bind_groups.contains_key(name) || name == GLYPH_ATLAS_TEXTURE_NAME)
            && let Some(gpu_tex) = self.texture_cache.get(name)
        {
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Preview Bind Group: {}", name)),
                layout: &preview_data.pipeline.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&gpu_tex.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&preview_data.pipeline.sampler),
                    },
                ],
            });

            preview_data.bind_groups.insert(name.clone(), bind_group);
        }

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

            self.grid_renderer.render_grid(&mut rpass);

            if let Some((sx, sy, sw, sh)) = scissor_rect {
                rpass.set_scissor_rect(sx, sy, sw, sh);
            }

            self.pane_renderer.render(&mut rpass);

            self.selection_renderer.render(&mut rpass);

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

pub struct RenderContext<'a> {
    pub camera: &'a Camera,
    pub layout_tabs: &'a mut LayoutTabs,
    pub ui_state: &'a mut UiState,
}

struct DragState {
    pane_idx: PaneId,
    handle: Handle,
    start_screen: [f32; 2],
    moved: bool,
    cycles: bool,
    start_world: [f32; 2],
    start_translation: Vector3f,
    start_size: Vector2f,
    rotate_z: f32,
}

pub struct LayoutTabs {
    pub items: Vec<LayoutData>,
    pub active_index: usize,
    pub glyphs: GlyphData,
}

impl LayoutTabs {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            active_index: 0,
            glyphs: GlyphData::new(),
        }
    }

    pub fn active(&self) -> Option<&LayoutData> {
        self.items.get(self.active_index)
    }

    pub fn active_mut(&mut self) -> Option<&mut LayoutData> {
        self.items.get_mut(self.active_index)
    }

    pub fn active_timeline(&self) -> Option<&TimelineState> {
        self.active().map(|layout| &layout.timeline)
    }

    pub fn active_timeline_mut(&mut self) -> Option<&mut TimelineState> {
        self.active_mut().map(|layout| &mut layout.timeline)
    }

    pub fn push_and_select(&mut self, layout: LayoutData) {
        self.items.push(layout);
        self.active_index = self.items.len().saturating_sub(1);
    }

    pub fn set_single(&mut self, layout: LayoutData) {
        if self.items.is_empty() {
            self.items.push(layout);
            self.active_index = 0;
        } else {
            self.items[self.active_index] = layout;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn collect_all_texture_names(&self) -> std::collections::HashSet<&str> {
        let mut active_textures = std::collections::HashSet::new();

        for layout in &self.items {
            for bntx in layout.tree.all_bntxs() {
                for tex in &bntx.textures {
                    active_textures.insert(tex.name.as_str());
                }
            }
        }

        active_textures
    }

    pub fn active_anim_names(&mut self) -> Vec<String> {
        let Some(layout) = self.active_mut() else {
            return Vec::new();
        };

        layout
            .timeline
            .anim_player
            .anims
            .iter_mut()
            .enumerate()
            .map(|(idx, a)| {
                if a.name.is_empty() {
                    a.name = format!("Animation {}", idx + 1);
                }
                a.name.clone()
            })
            .collect()
    }
}

pub struct LayoutData {
    pub tree: PaneTree,
    pub source: Option<LayoutSource>,
    pub is_centered: bool,
    pub parts_size: Vector2f,
    pub file_name: String,
    pub version: VersionFormat,
    pub history: EditHistory,
    anim_session: Option<AnimSession>,

    pub timeline: TimelineState,
}

impl LayoutData {
    const EDIT_HISTORY_LIMIT: usize = 20;

    pub fn edit_ctx(&mut self) -> (&mut EditHistory, EditCtx<'_>) {
        (
            &mut self.history,
            EditCtx {
                tree: &mut self.tree,
                timeline: &mut self.timeline,
            },
        )
    }

    fn begin_anim_session(&mut self) {
        if self.anim_session.is_some() {
            return;
        }

        let player = &self.timeline.anim_player;

        if let Some(anim) = player.active
            && let Some(instance) = player.anims.get(anim)
        {
            self.anim_session = Some(AnimSession {
                anim,
                baseline: instance.bflan.anim_info.clone(),
            });
        }
    }

    fn commit_anim_session(&mut self) {
        let Some(session) = self.anim_session.take() else {
            return;
        };

        if let Some(instance) = self.timeline.anim_player.anims.get(session.anim)
            && instance.bflan.anim_info != session.baseline
        {
            let after = instance.bflan.anim_info.clone();
            self.history
                .record_anim(session.anim, session.baseline, after);
        }
    }

    pub fn bake_bflyt(&self) -> Bflyt {
        let mut nodes = Vec::new();
        for root_node in &self.tree.roots {
            root_node.flatten_to_bflyt_nodes(&mut nodes);
        }

        let layout_header = Layout {
            is_centered: self.is_centered,
            width: self.tree.layout_size.x,
            height: self.tree.layout_size.y,
            parts_width: self.parts_size.x,
            parts_height: self.parts_size.y,
            name: self.file_name.clone(),
        };

        Bflyt {
            endianness: Endianness::Little,
            version: self.version,
            layout: layout_header,
            user_data: self.tree.user_data.clone(),
            texture_list: self.tree.texture_list.clone(),
            font_list: self.tree.font_list.clone(),
            material_list: self.tree.material_list.clone(),
            capture_texture_list: self.tree.capture_texture_list.clone(),
            nodes,
            root_group: self.tree.group.clone(),
            control_source: self.tree.control_source.clone(),
        }
    }

    pub fn save_problems(&self) -> Vec<String> {
        let mut problems = Vec::new();

        let textures: Vec<&str> = self
            .tree
            .texture_list
            .iter()
            .flat_map(|list| list.textures.iter().map(String::as_str))
            .collect();

        for material in self.tree.material_list.iter().flat_map(|l| &l.materials) {
            if material.material_name.len() > 28 {
                problems.push(format!(
                    "Material name '{}' is longer than 28 bytes",
                    material.material_name
                ));
            }

            for tex_map in &material.tex_maps {
                if !textures.contains(&tex_map.texture_name.as_str()) {
                    problems.push(format!(
                        "Material '{}' uses texture '{}', which isn't in the texture list",
                        material.material_name, tex_map.texture_name
                    ));
                }
            }
        }

        for node in self.tree.iter().filter(|n| n.parts_source.is_none()) {
            if node.label.len() > 24 {
                problems.push(format!(
                    "Pane name '{}' is longer than 24 bytes",
                    node.label
                ));
            }
        }

        problems
    }

    pub fn load_from_buffer(
        all_files: Vec<(EntryPath, MagicFiles)>,
        source: Option<LayoutSource>,
        layout_dir: Option<&Path>,
        archive_scan_entries: Option<&[ArchiveEntry]>,
        glyphs: &mut GlyphData,
    ) -> Result<(Self, Vec<String>), LoadError> {
        let (bflyt_path, bflyt_bytes) = all_files
            .iter()
            .find_map(|(path, file)| match file {
                MagicFiles::Bflyt(_, bytes)
                    if source.as_ref().is_none_or(|source| source.layout == *path) =>
                {
                    Some((path, bytes))
                }
                _ => None,
            })
            .ok_or(LoadError::NoBflytFound)?;

        let bflyt = Bflyt::parse_file(bflyt_bytes).map_err(LoadError::BflytParse)?;

        let mut layout_names: Vec<&str> = all_files
            .iter()
            .filter_map(|(_, file)| match file {
                MagicFiles::Bflyt(Some(name), _) => Some(file_stem(name)),
                _ => None,
            })
            .collect();
        layout_names.push(&bflyt.layout.name);
        let owned: Vec<EntryPath> = all_files
            .iter()
            .filter_map(|(path, file)| match file {
                MagicFiles::Bflan(Some(name), _)
                    if anim_owner(file_stem(name), layout_names.iter().copied())
                        == Some(bflyt.layout.name.as_str()) =>
                {
                    Some(path.clone())
                }
                _ => None,
            })
            .collect();

        // The opened layout is left out, so it can't become a parts layout of itself
        let archive_layouts: Vec<(String, Vec<u8>)> = all_files
            .iter()
            .filter_map(|(path, file)| match file {
                MagicFiles::Bflyt(Some(name), bytes) if path != bflyt_path => {
                    Some((file_stem(name).to_string(), bytes.clone()))
                }
                _ => None,
            })
            .collect();

        let (discovered_bntxs, discovered_bflans) = all_files
            .into_par_iter()
            .fold(
                || (Vec::new(), Vec::new()),
                |(mut bntxs, mut bflans), (path, magic_file)| {
                    match magic_file {
                        MagicFiles::Bntx(_, bytes) => match Bntx::parse(&bytes) {
                            Ok(bntx) => bntxs.push(bntx),
                            Err(e) => log::error!("TextureCache: failed to parse BNTX: {e}"),
                        },
                        MagicFiles::Bflan(file_name, bytes) => {
                            if let Ok(bflan) = Bflan::parse_file(&bytes) {
                                bflans.push((path, file_name, bflan));
                            }
                        }
                        _ => {}
                    }
                    (bntxs, bflans)
                },
            )
            .reduce(
                || (Vec::new(), Vec::new()),
                |(mut bntxs_a, mut bflans_a), (bntxs_b, bflans_b)| {
                    bntxs_a.extend(bntxs_b);
                    bflans_a.extend(bflans_b);
                    (bntxs_a, bflans_a)
                },
            );

        let mut timeline = TimelineState::default();
        for (path, file_name, bflan) in discovered_bflans {
            if file_name
                .as_ref()
                .is_some_and(|f| f.contains(&bflyt.layout.name))
                || file_name.is_none()
            {
                let saved_to = (source.is_some() && owned.contains(&path)).then_some(path);
                timeline.anim_player.load(bflan, saved_to);
            }
        }

        let anim_names = timeline
            .anim_player
            .anims
            .iter_mut()
            .enumerate()
            .map(|(idx, a)| {
                if a.name.is_empty() {
                    a.name = format!("Animation {}", idx + 1)
                };

                a.name.clone()
            })
            .collect();

        let layout_name = bflyt.layout.name.clone();
        log::info!("Preparing to build BflytView for {layout_name}...");

        let version = bflyt.version;
        let is_centered = bflyt.layout.is_centered;
        let parts_size = Vector2f {
            x: bflyt.layout.parts_width,
            y: bflyt.layout.parts_height,
        };

        let tree = PaneTree::from_bflyt(
            bflyt,
            layout_dir,
            layout_name.clone(),
            archive_scan_entries,
            &archive_layouts,
            discovered_bntxs,
            glyphs,
        );

        let layout_data = Self {
            tree,
            source,
            timeline,
            is_centered,
            parts_size,
            file_name: layout_name,
            version,
            history: EditHistory::new(Self::EDIT_HISTORY_LIMIT),
            anim_session: None,
        };

        Ok((layout_data, anim_names))
    }
}

struct App {
    tabs: LayoutTabs,
    docs: DocStore,
    ui_state: UiState,
    camera: Camera,
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    gpu: Option<GpuState>,
    window: Option<Arc<Window>>,
    last_tick: Instant,
    drag_state: Option<DragState>,
}

impl App {
    fn new() -> Self {
        Self {
            tabs: LayoutTabs::new(),
            docs: DocStore::default(),
            ui_state: UiState::default(),
            camera: Camera::new(),
            egui_ctx: egui::Context::default(),
            egui_state: None,
            gpu: None,
            window: None,
            last_tick: Instant::now(),
            drag_state: None,
        }
    }

    fn try_start_drag(&mut self, screen_pos: [f32; 2]) -> bool {
        let Some(gpu) = &self.gpu else { return false };

        let Some(idx) = self.ui_state.pane_tree_view.selected_pane else {
            return false;
        };

        let Some(layout) = self.tabs.active() else {
            return false;
        };

        let Some(node) = layout.tree.find_by_idx(idx) else {
            return false;
        };

        if node.plain_quad.is_parts_root || node.parts_source.is_some() {
            return false;
        };

        let world_pos = self.camera.screen_to_world(screen_pos);

        let radius = 8.0 / self.camera.zoom.max(0.01);

        let handle = match gpu.selection_renderer.hit_test(world_pos, radius) {
            Some(h) => h,
            None if point_in_quad(world_pos, &node.world_corners) => Handle::Body,
            None => return false,
        };

        self.ui_state.pane_tree_view.deselect_from_view();

        let base = node.section.get_base_pane();
        let translation = base.map(|b| b.translation).unwrap_or_default();

        let size = base.map(|b| b.size).unwrap_or(node.world_size);
        let rotate_z = base.map(|b| b.rotation.z).unwrap_or(0.0);

        self.drag_state = Some(DragState {
            pane_idx: idx,
            handle,
            start_screen: screen_pos,
            moved: false,
            cycles: matches!(handle, Handle::Body),
            start_world: world_pos,
            start_translation: translation,
            start_size: size,
            rotate_z,
        });

        true
    }

    fn update_drag(&mut self, screen_pos: [f32; 2]) {
        const DEAD_ZONE: f32 = 4.0;

        if let Some(drag) = &mut self.drag_state
            && !drag.moved
        {
            let dx = screen_pos[0] - drag.start_screen[0];
            let dy = screen_pos[1] - drag.start_screen[1];

            if dx * dx + dy * dy < DEAD_ZONE * DEAD_ZONE {
                return;
            }

            drag.moved = true;
        }

        let Some(drag) = &self.drag_state else { return };

        let Some(layout) = self.tabs.active_mut() else {
            return;
        };

        puffin::profile_function!();

        let world_pos = self.camera.screen_to_world(screen_pos);

        let dx = world_pos[0] - drag.start_world[0];
        let dy = world_pos[1] - drag.start_world[1];

        let Some(node) = layout.tree.find_by_idx_mut(drag.pane_idx) else {
            return;
        };

        if node.plain_quad.is_parts_root {
            return;
        }

        let Some(base) = node.section.get_base_pane_mut() else {
            return;
        };

        match drag.handle {
            Handle::Body => {
                base.translation.x = drag.start_translation.x + dx;
                base.translation.y = drag.start_translation.y - dy;
            }
            Handle::Rotation => {
                let pivot_x = node.world_center.x;
                let pivot_y = node.world_center.y;

                let start_v_x = drag.start_world[0] - pivot_x;
                let start_v_y = drag.start_world[1] - pivot_y;
                let start_angle = start_v_y.atan2(start_v_x).to_degrees();

                let current_v_x = world_pos[0] - pivot_x;
                let current_v_y = world_pos[1] - pivot_y;
                let current_angle = current_v_y.atan2(current_v_x).to_degrees();

                let mut angle_delta = current_angle - start_angle;

                if angle_delta > 180.0 {
                    angle_delta -= 360.0;
                } else if angle_delta < -180.0 {
                    angle_delta += 360.0;
                }

                base.rotation.z = drag.rotate_z - angle_delta;
            }
            _ => {
                let rad = -drag.rotate_z.to_radians();
                let (sin_r, cos_r) = rad.sin_cos();
                let local_dx = dx * cos_r + dy * sin_r;
                let local_dy = -dx * sin_r + dy * cos_r;

                match drag.handle {
                    Handle::TopLeft
                    | Handle::TopRight
                    | Handle::BottomLeft
                    | Handle::BottomRight => {
                        let sx = if matches!(drag.handle, Handle::TopLeft | Handle::BottomLeft) {
                            -1.0
                        } else {
                            1.0
                        };

                        let sy = if matches!(drag.handle, Handle::TopLeft | Handle::TopRight) {
                            -1.0
                        } else {
                            1.0
                        };

                        base.size.x = (drag.start_size.x + local_dx * sx * 2.0).max(1.0);
                        base.size.y = (drag.start_size.y + local_dy * sy * 2.0).max(1.0);
                    }

                    Handle::Left | Handle::Right => {
                        let sx = if drag.handle == Handle::Left {
                            -1.0
                        } else {
                            1.0
                        };
                        base.size.x = (drag.start_size.x + local_dx * sx * 2.0).max(1.0);
                    }

                    Handle::Top | Handle::Bottom => {
                        let sy = if drag.handle == Handle::Top {
                            -1.0
                        } else {
                            1.0
                        };
                        base.size.y = (drag.start_size.y + local_dy * sy * 2.0).max(1.0);
                    }
                    _ => {}
                }
            }
        }

        node.mark_transform_dirty();
        layout.tree.recompute_dirty();
    }

    fn end_drag(&mut self) {
        let Some(drag) = self.drag_state.take() else {
            return;
        };

        if !drag.moved {
            if drag.cycles {
                self.select_below(self.camera.cursor_screen, drag.pane_idx);
            }

            return;
        }

        let Some(layout) = self.tabs.active_mut() else {
            return;
        };

        let Some(node) = layout.tree.find_by_idx_mut(drag.pane_idx) else {
            return;
        };

        let Some(base) = node.section.get_base_pane() else {
            return;
        };

        let before = edit_history::PaneTransform {
            translation: drag.start_translation,
            size: drag.start_size,
            rotation_z: drag.rotate_z,
        };

        let after = edit_history::PaneTransform {
            translation: base.translation,
            size: base.size,
            rotation_z: base.rotation.z,
        };

        layout
            .history
            .record_transform(drag.pane_idx, before, after);
    }

    fn try_select_at(&mut self, screen_pos: [f32; 2]) {
        let pane = self.pane_at(screen_pos);
        self.ui_state.pane_tree_view.select(pane);
    }

    fn select_below(&mut self, screen_pos: [f32; 2], current: PaneId) {
        let stack = self.panes_at(screen_pos);

        let next = match stack.iter().position(|&p| p == current) {
            Some(i) => stack.get((i + 1) % stack.len()).copied(),
            None => stack.first().copied(),
        };

        if next.is_some() {
            self.ui_state.pane_tree_view.select(next);
        }
    }

    fn pane_at(&self, screen_pos: [f32; 2]) -> Option<PaneId> {
        self.panes_at(screen_pos).first().copied()
    }

    fn panes_at(&self, screen_pos: [f32; 2]) -> Vec<PaneId> {
        let Some(layout) = self.tabs.active() else {
            return Vec::new();
        };

        puffin::profile_function!();

        let world_pos = self.camera.screen_to_world(screen_pos);

        let mut stack = Vec::new();

        for node in layout.tree.iter() {
            if !node.visible
                || self
                    .ui_state
                    .pane_tree_view
                    .hidden_panes
                    .contains(&node.pane_idx)
                    | node.plain_quad.is_parts_root
                    | node.parts_source.is_some()
            {
                continue;
            }

            if !point_in_quad(world_pos, &node.world_corners) {
                continue;
            }

            stack.push(node.pane_idx);
        }

        stack.reverse();
        stack
    }

    fn open_document(&mut self, path: &Path, scope: EntryPath, layout: Option<EntryPath>) {
        let doc = match self.docs.open(path) {
            Ok(doc) => doc,
            Err(err) => {
                log::error!("{err}");
                self.ui_state.error_message = Some(err);
                return;
            }
        };

        let files = self.docs.docs[doc].files(&scope);
        let layout = layout.or_else(|| {
            files.iter().find_map(|(path, file)| {
                matches!(file, MagicFiles::Bflyt(..)).then(|| path.clone())
            })
        });

        let Some(layout) = layout else {
            self.ui_state.error_message = Some(LoadError::NoBflytFound.to_string());
            return;
        };

        if let Some(tab) = self.tabs.items.iter().position(|tab| {
            tab.source
                .as_ref()
                .is_some_and(|source| source.doc == doc && source.layout == layout)
        }) {
            self.tabs.active_index = tab;
            self.switch_active_tab();
            return;
        }

        self.open_file_from_buffer(files, Some(LayoutSource { doc, layout }));
    }

    fn open_file_from_buffer(
        &mut self,
        all_files: Vec<(EntryPath, MagicFiles)>,
        source: Option<LayoutSource>,
    ) {
        let is_dir_stale = match (
            &self.ui_state.archive_browser.layout_dir,
            &self.ui_state.archive_browser.archive_scan,
        ) {
            (Some(dir), Some(scan)) => scan.root() != dir,
            _ => false,
        };

        if is_dir_stale {
            self.tabs.items.clear();
            self.tabs.active_index = 0;
            self.clear_active_view();
            log::warn!("Layout directory mismatch detected during load, clearing tabs.");
        }

        let layout_dir = self.ui_state.archive_browser.layout_dir.as_deref();
        let archive_scan_entries = self
            .ui_state
            .archive_browser
            .archive_scan
            .as_ref()
            .map(|s| s.entries.as_slice());

        match LayoutData::load_from_buffer(
            all_files,
            source,
            layout_dir,
            archive_scan_entries,
            &mut self.tabs.glyphs,
        ) {
            Ok((layout, anim_names)) => {
                self.ui_state.error_message = None;
                self.ui_state.anim_names = anim_names;

                self.camera.zoom = 1.0;
                self.camera.offset = [0.0, 0.0];

                self.tabs.push_and_select(layout);

                self.sync_gpu_layout();
            }
            Err(err) => {
                log::error!("{err}");
                self.ui_state.error_message = Some(err.to_string());
            }
        }
    }

    fn sync_gpu_layout(&mut self) {
        if let Some(gpu) = &mut self.gpu {
            self.tabs
                .glyphs
                .atlas
                .upload(&gpu.device, &gpu.queue, &mut gpu.texture_cache);

            let Some(layout) = self.tabs.active_mut() else {
                return;
            };

            let layout_size = layout.tree.layout_size;

            self.ui_state.texture_editor.selected_texture = None;

            for bntx in layout.tree.all_bntxs() {
                gpu.texture_cache
                    .load_from_bntx(&gpu.device, &gpu.queue, bntx);
            }

            let render_quads = layout.tree.collect_render_quads();

            gpu.pane_renderer.upload_quads(
                &gpu.device,
                &render_quads,
                &gpu.texture_cache,
                layout_size,
                &self.ui_state.visiblity_flags,
            );

            if let Some(window) = &self.window {
                let size = window.inner_size();
                self.camera.fit(
                    layout.tree.layout_size.x,
                    layout.tree.layout_size.y,
                    size.width as f32,
                    size.height as f32,
                );

                window.set_title(&format!("nnbfl-preview - {}", layout.file_name));
            }

            log::info!(
                "Synced GPU state: loaded {} panes from {}",
                layout.tree.iter().count(),
                layout.file_name
            );
        }
    }

    fn switch_active_tab(&mut self) {
        for layout in &mut self.tabs.items {
            layout.commit_anim_session();
        }

        self.ui_state.anim_names = self.tabs.active_anim_names();
        self.ui_state.pane_tree_view.select(None);

        if let Some(gpu) = &mut self.gpu {
            self.tabs
                .glyphs
                .atlas
                .upload(&gpu.device, &gpu.queue, &mut gpu.texture_cache);

            let Some(layout) = self.tabs.active_mut() else {
                self.clear_active_view();
                return;
            };

            let layout_size = layout.tree.layout_size;

            for bntx in layout.tree.all_bntxs() {
                gpu.texture_cache
                    .load_from_bntx(&gpu.device, &gpu.queue, bntx);
            }

            let render_quads = layout.tree.collect_render_quads();
            gpu.pane_renderer.upload_quads(
                &gpu.device,
                &render_quads,
                &gpu.texture_cache,
                layout_size,
                &self.ui_state.visiblity_flags,
            );

            if let Some(window) = &self.window {
                let size = window.inner_size();
                self.camera.fit(
                    layout.tree.layout_size.x,
                    layout.tree.layout_size.y,
                    size.width as f32,
                    size.height as f32,
                );

                window.set_title(&format!("nnbfl-preview - {}", layout.file_name));
            }
        }
    }

    fn clear_active_view(&mut self) {
        self.ui_state.pane_tree_view.select(None);
        self.ui_state.texture_editor.selected_texture = None;
        self.ui_state.anim_names.clear();

        if let Some(gpu) = &mut self.gpu {
            gpu.pane_renderer.upload_quads(
                &gpu.device,
                &[],
                &gpu.texture_cache,
                Vector2f::new(1280.0, 720.0),
                &self.ui_state.visiblity_flags,
            );

            gpu.texture_cache.clear();
        }

        if let Some(window) = &self.window {
            window.set_title("nnbfl-preview - No file loaded");
        }
    }

    fn perform_pane_edit(&mut self, edit: edit_history::PaneEdit) {
        self.perform_pane_edits(|_| vec![edit]);
    }

    fn perform_pane_edits(&mut self, edits: impl FnOnce(&PaneTree) -> Vec<edit_history::PaneEdit>) {
        let Some(layout) = self.tabs.active_mut() else {
            return;
        };

        let is_parts_content = |idx: PaneId| {
            layout
                .tree
                .find_by_idx(idx)
                .is_some_and(|n| n.parts_source.is_some())
        };

        let edits: Vec<_> = edits(&layout.tree)
            .into_iter()
            .filter(|edit| {
                let target_idx = match edit {
                    edit_history::PaneEdit::Delete { target_idx } => Some(*target_idx),
                    edit_history::PaneEdit::Duplicate { source_idx } => Some(*source_idx),
                    edit_history::PaneEdit::Add { parent, .. } => *parent,
                    edit_history::PaneEdit::Move { source_idx, .. } => Some(*source_idx),
                    edit_history::PaneEdit::Rename { target_idx, .. } => Some(*target_idx),
                };

                !target_idx.is_some_and(is_parts_content)
            })
            .collect();

        if edits.is_empty() {
            return;
        }

        layout.commit_anim_session();
        let (history, mut edit) = layout.edit_ctx();
        let resulting = history.perform_all(&mut edit, edits);
        self.ui_state.pane_tree_view.select_many(resulting);

        self.after_structural_edit();
    }

    fn after_structural_edit(&mut self) {
        let tabs = &mut self.tabs;
        let Some(layout) = tabs.items.get_mut(tabs.active_index) else {
            return;
        };

        let exists = |id: &PaneId| layout.tree.path_map.contains_key(id);
        let tree_view = &mut self.ui_state.pane_tree_view;
        tree_view.hidden_panes.retain(exists);

        tree_view.retain(exists);

        layout.tree.recompute_dirty();
        layout.tree.recompute_dirty_materials(&mut tabs.glyphs);
        layout.tree.update_animation_visuals();
        layout.tree.take_rebatch();

        let Some(gpu) = &mut self.gpu else { return };

        let layout_size = layout.tree.layout_size;
        let render_quads = layout.tree.collect_render_quads();

        gpu.pane_renderer.upload_quads(
            &gpu.device,
            &render_quads,
            &gpu.texture_cache,
            layout_size,
            &self.ui_state.visiblity_flags,
        );

        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn save_active(&mut self, choose_path: bool) {
        let Some(source) = self.tabs.active().and_then(|layout| layout.source.as_ref()) else {
            return;
        };
        let doc_id = source.doc;
        let doc = &self.docs.docs[doc_id];

        let target = match (&doc.save_path, choose_path) {
            (Some(path), false) => path.clone(),
            _ => {
                let dialog = rfd::FileDialog::new()
                    .set_title("Save archive as")
                    .set_file_name(doc.file_name().unwrap_or_default());
                let Some(path) = dialog.save_file() else {
                    return;
                };
                path
            }
        };

        let mut edits = HashMap::new();
        let mut problems = Vec::new();
        let raw = |path: &EntryPath| doc.root.at(path).map(|node| node.raw().to_vec());

        for tab in &self.tabs.items {
            let Some(source) = tab.source.as_ref().filter(|s| s.doc == doc_id) else {
                continue;
            };
            problems.extend(tab.save_problems());

            let bflyt = tab.bake_bflyt().write_file().buffer;
            if raw(&source.layout).as_ref() != Some(&bflyt) {
                edits.insert(source.layout.clone(), bflyt);
            }

            for anim in &tab.timeline.anim_player.anims {
                let Some(path) = &anim.source else { continue };
                let bflan = anim.bflan.write_file().buffer;
                if raw(path).as_ref() != Some(&bflan) {
                    edits.insert(path.clone(), bflan);
                }
            }
        }

        if !problems.is_empty() {
            self.ui_state.error_message = Some(problems.join("\n"));
            return;
        }

        let changed = edits.len();
        match self.docs.docs[doc_id].save(&target, &edits) {
            Ok(()) => log::info!("Saved {target:?} with {changed} changed files"),
            Err(err) => {
                log::error!("{err}");
                self.ui_state.error_message = Some(err);
            }
        }
    }

    fn export_bflyt(&mut self) {
        let Some(layout) = self.tabs.active() else {
            return;
        };

        let dialog = rfd::FileDialog::new()
            .set_title("Export .bflyt")
            .add_filter("BFLYT Layout", &["bflyt"])
            .set_file_name(&layout.file_name);
        let Some(target_path) = dialog.save_file() else {
            return;
        };

        let writer = layout.bake_bflyt().write_file();
        match std::fs::write(&target_path, &writer.buffer) {
            Ok(_) => log::info!("File succesfully written to: {target_path:?}"),
            Err(e) => {
                log::error!("Failed writing to disk: {e:?}");
                self.ui_state.error_message = Some(format!("Write error: {e}"));
            }
        }
    }

    fn purge_unused_textures(&mut self) {
        let Some(gpu) = &mut self.gpu else { return };

        let active_names = self.tabs.collect_all_texture_names();

        gpu.texture_cache.retain_active(&active_names);
    }
}

pub fn extract_all_files_recursive(
    data: Vec<u8>,
    out_files: &mut Vec<MagicFiles>,
    current_file_name: Option<String>,
) {
    let current_file = SarcFile {
        name: current_file_name,
        hash: 0,
        alignment: None,
        data,
    };

    match current_file.match_by_magic() {
        MagicFiles::Zstd(file_name, compressed_data) => {
            let mut decompressed = Vec::new();

            if tomolib::formats::zs::decompress(&compressed_data[..], &mut decompressed).is_ok() {
                extract_all_files_recursive(decompressed, out_files, file_name);
            } else {
                log::error!("Failed to decompress Zstd data for {file_name:?}.");
                out_files.push(MagicFiles::Unknown(file_name, compressed_data));
            }
        }

        MagicFiles::Yaz0(file_name, compressed_data) => match szs::decode(&compressed_data) {
            Ok(decompressed) => {
                extract_all_files_recursive(decompressed, out_files, file_name);
            }
            Err(err) => {
                log::error!("Failed to decompress Yaz0 data for {file_name:?}: {err}");
                out_files.push(MagicFiles::Unknown(file_name, compressed_data));
            }
        },

        MagicFiles::Sarc(file_name, sarc_bytes) => {
            if let Ok(sarc) = Sarc::parse_file(&sarc_bytes) {
                for file in sarc.files {
                    extract_all_files_recursive(file.data, out_files, file.name);
                }
            } else {
                out_files.push(MagicFiles::Sarc(file_name, sarc_bytes));
            }
        }

        file => out_files.push(file),
    }
}

fn decompress_if_needed(data: Vec<u8>, filename: &str) -> Vec<u8> {
    if data.len() >= 4 && data[0..4] == [0x28, 0xB5, 0x2F, 0xFD] {
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

        let title_path = if let Some(layout) = self.tabs.active() {
            &layout.file_name
        } else {
            "No file loaded"
        };

        let icon_bytes = include_bytes!("../assets/icon.rgba");
        let (width, height) = (64, 64);

        let icon = winit::window::Icon::from_rgba(icon_bytes.to_vec(), width, height).ok();

        let mut window_attributes = winit::window::WindowAttributes::default()
            .with_title(format!("nnbfl-preview - {title_path}"))
            .with_inner_size(winit::dpi::LogicalSize::new(1280u32, 720u32))
            .with_window_icon(icon);

        #[cfg(any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        {
            use winit::platform::wayland::WindowAttributesExtWayland;
            window_attributes = window_attributes.with_name("nnbfl-preview", "nnbfl-preview");
        }

        let window = Arc::new(
            event_loop
                .create_window(window_attributes)
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
                UiAction::SetBlarcDir => {
                    let Some(dir) = rfd::FileDialog::new().pick_folder() else {
                        return;
                    };

                    self.ui_state.archive_browser.layout_dir = Some(dir);

                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }

                UiAction::SwitchActiveTab => {
                    self.switch_active_tab();

                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }

                UiAction::PurgeUnusedTexturesAndSwitch(switch_tab) => {
                    self.purge_unused_textures();

                    if switch_tab {
                        self.switch_active_tab();
                    }

                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }

                UiAction::LoadFile => {
                    let Some(path) = rfd::FileDialog::new()
                        .add_filter(
                            "Supported files",
                            &[SUPPORTED_SARC_EXTENSIONS, &["bflyt"]].concat(),
                        )
                        .pick_file()
                    else {
                        return;
                    };

                    self.ui_state.reset();
                    self.open_document(&path, Vec::new(), None);

                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }

                UiAction::SaveFile => self.save_active(false),
                UiAction::SaveFileAs => self.save_active(true),
                UiAction::ExportBflyt => self.export_bflyt(),

                UiAction::StartArchiveScan => {
                    if let Some(dir) = self.ui_state.archive_browser.layout_dir.clone() {
                        self.ui_state.archive_browser.archive_scan = Some(ArchiveScan::start(dir));
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                }

                UiAction::CancelArchiveScan => {
                    if let Some(scan) = &mut self.ui_state.archive_browser.archive_scan {
                        scan.request_cancel();
                    }
                }

                UiAction::LoadArchiveEntry(entry) => {
                    self.ui_state.reset();

                    let mut layout = entry.nested_path.clone();
                    layout.push(entry.file_idx);
                    self.open_document(&entry.path, entry.nested_path.clone(), Some(layout));

                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }

                UiAction::RenamePane { target_idx, name } => {
                    self.perform_pane_edit(edit_history::PaneEdit::Rename { target_idx, name })
                }

                UiAction::DeletePanes(panes) => self.perform_pane_edits(|tree| {
                    tree.selection_roots(&panes)
                        .into_iter()
                        .map(|target_idx| edit_history::PaneEdit::Delete { target_idx })
                        .collect()
                }),

                UiAction::DuplicatePanes(panes) => self.perform_pane_edits(|tree| {
                    tree.selection_roots(&panes)
                        .into_iter()
                        .map(|source_idx| edit_history::PaneEdit::Duplicate { source_idx })
                        .collect()
                }),

                UiAction::AddPane {
                    parent,
                    anchor,
                    kind,
                } => self.perform_pane_edit(edit_history::PaneEdit::Add {
                    parent,
                    anchor,
                    kind,
                }),

                UiAction::MovePanes {
                    sources,
                    new_parent,
                    anchor,
                } => self.perform_pane_edits(|tree| {
                    let mut anchor = anchor;

                    tree.selection_roots(&sources)
                        .into_iter()
                        .map(|source_idx| {
                            let edit = edit_history::PaneEdit::Move {
                                source_idx,
                                new_parent,
                                anchor,
                            };
                            anchor = Anchor::After(source_idx);
                            edit
                        })
                        .collect()
                }),

                UiAction::Undo => {
                    if let Some(layout) = self.tabs.active_mut() {
                        layout.commit_anim_session();
                        let (history, mut edit) = layout.edit_ctx();

                        let resulting = history.undo(&mut edit);
                        self.ui_state.pane_tree_view.select_many(resulting);
                        self.after_structural_edit();
                    }
                }

                UiAction::Redo => {
                    if let Some(layout) = self.tabs.active_mut() {
                        layout.commit_anim_session();
                        let (history, mut edit) = layout.edit_ctx();

                        let resulting = history.redo(&mut edit);
                        self.ui_state.pane_tree_view.select_many(resulting);
                        self.after_structural_edit();
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

                if self.drag_state.is_some() && !egui_wants_pointer {
                    self.update_drag(pos);
                }

                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => match state {
                winit::event::ElementState::Pressed => {
                    if !egui_wants_pointer {
                        let pos = self.camera.cursor_screen;

                        let (space, toggle) = self
                            .egui_ctx
                            .input(|i| (i.key_down(egui::Key::Space), i.modifiers.command));

                        if space {
                            self.camera.start_pan(pos);
                        }

                        if toggle && !space {
                            if let Some(pane) = self.pane_at(pos) {
                                self.ui_state.pane_tree_view.toggle(pane);
                            }
                        } else if !self.try_start_drag(pos) && !self.camera.is_panning {
                            self.try_select_at(pos);

                            if self.try_start_drag(pos)
                                && let Some(drag) = &mut self.drag_state
                            {
                                drag.cycles = false;
                            }
                        }
                    }
                }
                winit::event::ElementState::Released => {
                    self.end_drag();

                    if self.camera.is_panning {
                        self.camera.end_pan();
                    }
                }
            },

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

            WindowEvent::MouseInput {
                state,
                button: MouseButton::Right,
                ..
            } if state == winit::event::ElementState::Pressed
                && !egui_wants_pointer
                && let Some(pane_idx) = self.ui_state.pane_tree_view.selected_pane =>
            {
                self.ui_state
                    .context_menu
                    .open_context_menu(self.camera.cursor_screen, pane_idx);
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
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(size);
                }

                if let Some(layout) = self.tabs.active() {
                    self.camera.fit(
                        layout.tree.layout_size.x,
                        layout.tree.layout_size.y,
                        size.width as f32,
                        size.height as f32,
                    );
                }
            }

            WindowEvent::DroppedFile(path) => {
                if path.extension().and_then(|s| s.to_str()) == Some("bflyt") {
                    self.open_document(&path, Vec::new(), None);
                } else {
                    self.ui_state.error_message =
                        Some("Invalid file type. Please drop a .bflyt file".to_string());
                }
            }

            WindowEvent::RedrawRequested => {
                if let (Some(gpu), Some(window), Some(egui_state)) =
                    (&mut self.gpu, &self.window, &mut self.egui_state)
                {
                    if window.has_focus() {
                        let dt = self.last_tick.elapsed().as_secs_f32();
                        self.last_tick = Instant::now();

                        if let Some(layout) = self.tabs.active_mut() {
                            if let Some(next) = layout
                                .timeline
                                .anim_player
                                .tick(dt, layout.timeline.frame_rate)
                            {
                                let anim_idx = layout
                                    .timeline
                                    .anim_player
                                    .anims
                                    .iter()
                                    .position(|a| a.name == next);

                                layout.timeline.anim_player.play(
                                    anim_idx,
                                    &layout.tree,
                                    &mut self.ui_state.pane_tree_view.hidden_panes,
                                );
                            }

                            if let Some(anim_idx) = self.ui_state.pending_play_anim.take() {
                                layout.timeline.anim_player.play(
                                    Some(anim_idx),
                                    &layout.tree,
                                    &mut self.ui_state.pane_tree_view.hidden_panes,
                                );
                            }
                        }
                    }

                    gpu.render(
                        window,
                        &self.egui_ctx,
                        egui_state,
                        RenderContext {
                            layout_tabs: &mut self.tabs,
                            ui_state: &mut self.ui_state,
                            camera: &self.camera,
                        },
                    );

                    let scan_active = self
                        .ui_state
                        .archive_browser
                        .archive_scan
                        .as_mut()
                        .map(|s| s.poll())
                        .unwrap_or(false);

                    let scan_in_progress = self
                        .ui_state
                        .archive_browser
                        .archive_scan
                        .as_ref()
                        .is_some_and(|s| !s.done && !s.cancelled);

                    let is_animation_playing = if let Some(layout) = self.tabs.active() {
                        layout.timeline.anim_player.is_playing()
                    } else {
                        false
                    };

                    if (is_animation_playing || scan_active || scan_in_progress)
                        && window.has_focus()
                    {
                        window.request_redraw();
                    }
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.egui_ctx.has_requested_repaint()
            && let Some(window) = &self.window
        {
            window.request_redraw();
        }
    }
}

fn main() {
    env_logger::init();

    if let Some(code) = cli::run_if_command() {
        std::process::exit(code);
    }
    let event_loop = EventLoop::new().expect("create event loop");
    let mut app = App::new();

    event_loop.run_app(&mut app).expect("run event loop");
}
